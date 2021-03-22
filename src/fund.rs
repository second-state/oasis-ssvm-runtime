use byteorder::{BigEndian, ReadBytesExt};
use ethcore::{
    self,
    state::{CleanupMode, State},
};
use ethereum_types::{Address, H256, U256};
use oasis_ethwasi_runtime_common::parity::NullBackend;
use std::str::FromStr;

pub struct FundManager;

impl FundManager {
    // There is a total fixed supply of 21 million OETHs.
    // The blockchain unlocks 525,000 OETHs every month in the first 20 months and
    // the monthly release is cut to 1/2 every 20 months.
    // Here we use 1 round denote 30 days (represent 1 month).

    /// Unlock address.
    const BENEFICIARY: &'static str = "7110316b618d20d0c44728ac2a3d683536ea682b";
    /// Adjust unlock amount period.
    const PERIOD: i64 = 20;
    /// Ticks means seconds in 30 days.
    const TICKS_IN_ROUND: i64 = 30 * 24 * 3600;
    /// Factor
    const FACTOR: u32 = 2;
    /// Total token amount is 21000000000000000000000000 wei.
    const TOTAL_AMOUNT: &'static str = "115EEC47F6CF7E35000000";

    /// Primary unlock token method
    pub fn try_unlock(timestamp: i64, state: &mut State<NullBackend>) -> U256 {
        // +---------------+-------------------+--------------------+-------------------+
        // | Storage field |       [0:16)      |       [16:24)      |      [24:32)      |
        // +---------------+-------------------+--------------------+-------------------+
        // |             0 |               reserved                 |  init timestamp   |
        // +---------------+-------------------+--------------------+-------------------+
        // |             1 |               reserved                 |   pending round   |
        // +---------------+-------------------+----------------------------------------+
        // |             2 |               reserved                 |  unlocked ticks   |
        // +---------------+-------------------+----------------------------------------+
        // |  (speed up) 3 |     reserved      |       round        |       period      |
        // +---------------+-------------------+----------------------------------------+
        let beneficiary = Address::from_str(FundManager::BENEFICIARY).unwrap();

        // The start time to apply unlock token mechanism.
        let value = state.storage_at(&beneficiary, &H256::from(0)).unwrap();
        let init_timestamp = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();

        // Error handling for two cases.
        // 1. `cargo test` will generate unpredictable timestamp from other module test cases.
        // 2. runtime with some empty fileds in genesis file.
        if timestamp <= init_timestamp || init_timestamp == 0 {
            return U256::from(0);
        }

        // Pending round is point to which not fully unlocked round after last time try_unlock.
        let value = state.storage_at(&beneficiary, &H256::from(1)).unwrap();
        let mut pending_round = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();

        // It record that already unlocked ticks in last unlocked round.
        let value = state.storage_at(&beneficiary, &H256::from(2)).unwrap();
        let mut unlocked_ticks = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();

        // Speed up parameters.
        // 1. shorten time of the round (default round is 30 days)
        // 2. shorten cut down period (default period is 20 rounds)
        let value = state.storage_at(&beneficiary, &H256::from(3)).unwrap();
        let fraction_r = value.get(16..).unwrap().read_i64::<BigEndian>().unwrap();
        let ticks_in_round = if fraction_r > 1 {
            FundManager::TICKS_IN_ROUND / fraction_r
        } else {
            FundManager::TICKS_IN_ROUND
        };
        let fraction_p = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
        let period = if fraction_p > 1 {
            FundManager::PERIOD / fraction_p
        } else {
            FundManager::PERIOD
        };

        // The funding used to accumulate unlock amount at this time.
        let mut funding = U256::from(0);

        // Expect to unlock to which rounds at this time.
        let expected_round = (timestamp - init_timestamp) / ticks_in_round;

        // The number of times we should decrease unlocks amount cut to 1/2.
        let mut exponent = 0;

        let initial_bucket = U256::from_str(FundManager::TOTAL_AMOUNT).unwrap()
            / (U256::from(FundManager::FACTOR) * U256::from(period));
        let mut bucket = initial_bucket;
        let mut tick_bucket = bucket / U256::from(ticks_in_round);
        while expected_round >= pending_round {
            // Reduce duplicate calculate action if need. Only re-calculate each 20 rounds.
            if exponent != pending_round / period {
                exponent = pending_round / period;
                bucket = initial_bucket / U256::from(FundManager::FACTOR).pow(U256::from(exponent));
                tick_bucket = bucket / U256::from(ticks_in_round);
            }
            if expected_round - pending_round >= 1 {
                // Condition 1
                // Increase funding with this round remain amount.
                funding = funding + bucket - (U256::from(unlocked_ticks) * tick_bucket);
                unlocked_ticks = 0;
                pending_round += 1;
            } else {
                // Condition 2
                // Increase funding by this round tick's count * tick_bucket - already unlocked.
                let ticks =
                    timestamp - init_timestamp - (ticks_in_round * pending_round) - unlocked_ticks;
                funding = funding + U256::from(ticks) * tick_bucket;
                unlocked_ticks = (unlocked_ticks + ticks) % ticks_in_round;
                break;
            }
        }

        state
            .add_balance(&beneficiary, &funding, CleanupMode::NoEmpty)
            .unwrap();
        state
            .set_storage(
                &beneficiary,
                H256::from(1),
                H256::from(pending_round as u64),
            )
            .unwrap();
        state
            .set_storage(
                &beneficiary,
                H256::from(2),
                H256::from(unlocked_ticks as u64),
            )
            .unwrap();
        return funding;
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use io_context::Context as IoContext;
    use oasis_core_runtime::storage::{
        mkvs::{sync::NoopReadSyncer, Tree},
        StorageContext,
    };
    use oasis_ethwasi_runtime_common::{
        parity::NullBackend,
        storage::{MemoryKeyValue, ThreadLocalMKVS},
    };
    use rand::Rng;
    use std::sync::Arc;

    const INIT_TIMESTAMP: i64 = 1604188800;
    const SECONDS_OF_30DAYS: i64 = 30 * 24 * 3600;
    // make sure random interval not tool small
    const MIN_TX_INTERVAL: i64 = SECONDS_OF_30DAYS / 100;

    fn get_init_state() -> State<NullBackend> {
        let mut state = State::from_existing(
            Box::new(ThreadLocalMKVS::new(IoContext::background())),
            NullBackend,
            U256::zero(),
            Default::default(),
            None,
        )
        .unwrap();

        let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
        state.new_contract(&monitor_address, U256::from(0), U256::from(0), 0);
        // init timestamp is point to 11/01/2020 @ 12:00am (UTC)
        state
            .set_storage(
                &monitor_address,
                H256::from(0),
                H256::from(INIT_TIMESTAMP as u64),
            )
            .unwrap();
        return state;
    }

    #[test]
    fn test_try_unlock_zero() {
        let untrusted_local = Arc::new(MemoryKeyValue::new());
        let mut mkvs = Tree::make()
            .with_capacity(0, 0)
            .new(Box::new(NoopReadSyncer {}));

        StorageContext::enter(&mut mkvs, untrusted_local, || {
            // Shift timestamp base on init_timestamp with -1 second
            let timestamp = INIT_TIMESTAMP - 1;
            let mut state = get_init_state();
            let funding = FundManager::try_unlock(timestamp, &mut state);
            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let pending_round = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(funding, U256::from(0));
            assert_eq!(pending_round, 0);
            assert_eq!(balance, U256::from(0));
        })
    }

    #[test]
    fn test_try_unlock_1_round() {
        let untrusted_local = Arc::new(MemoryKeyValue::new());
        let mut mkvs = Tree::make()
            .with_capacity(0, 0)
            .new(Box::new(NoopReadSyncer {}));

        StorageContext::enter(&mut mkvs, untrusted_local, || {
            // Shift timestamp base on init_timestamp with 1 second
            let timestamp = INIT_TIMESTAMP + SECONDS_OF_30DAYS;
            let mut state = get_init_state();
            let funding = FundManager::try_unlock(timestamp, &mut state);
            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let pending_round = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(funding, U256::from_str("6f2c4e995ec98e200000").unwrap());
            assert_eq!(pending_round, 1);
            assert_eq!(balance, U256::from_str("6f2c4e995ec98e200000").unwrap());
        })
    }

    #[test]
    fn test_try_unlock_10_rounds() {
        let untrusted_local = Arc::new(MemoryKeyValue::new());
        let mut mkvs = Tree::make()
            .with_capacity(0, 0)
            .new(Box::new(NoopReadSyncer {}));

        StorageContext::enter(&mut mkvs, untrusted_local, || {
            // Shift timestamp base on init_timestamp with 300 days (in seconds).
            let timestamp = INIT_TIMESTAMP + SECONDS_OF_30DAYS * 10;
            let mut state = get_init_state();
            let funding = FundManager::try_unlock(timestamp, &mut state);
            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let pending_round = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(funding, U256::from_str("457bb11fdb3df8d400000").unwrap());
            assert_eq!(pending_round, 10);
            assert_eq!(balance, U256::from_str("457bb11fdb3df8d400000").unwrap());
        })
    }

    #[test]
    fn test_try_unlock_100_rounds() {
        let untrusted_local = Arc::new(MemoryKeyValue::new());
        let mut mkvs = Tree::make()
            .with_capacity(0, 0)
            .new(Box::new(NoopReadSyncer {}));

        StorageContext::enter(&mut mkvs, untrusted_local, || {
            // Shift timestamp base on init_timestamp with 3000 days (in seconds).
            let timestamp = INIT_TIMESTAMP + SECONDS_OF_30DAYS * 100;
            let mut state = get_init_state();
            let funding = FundManager::try_unlock(timestamp, &mut state);
            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let pending_round = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(funding, U256::from_str("10d3f4e5b7190243580000").unwrap());
            assert_eq!(pending_round, 100);
            assert_eq!(balance, U256::from_str("10d3f4e5b7190243580000").unwrap());
        })
    }

    #[test]
    fn test_try_unlock_1000_rounds() {
        let untrusted_local = Arc::new(MemoryKeyValue::new());
        let mut mkvs = Tree::make()
            .with_capacity(0, 0)
            .new(Box::new(NoopReadSyncer {}));

        StorageContext::enter(&mut mkvs, untrusted_local, || {
            // Shift timestamp base on init_timestamp with 30000 days (in seconds).
            let timestamp = INIT_TIMESTAMP + SECONDS_OF_30DAYS * 1000;
            let mut state = get_init_state();
            let funding = FundManager::try_unlock(timestamp, &mut state);
            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let pending_round = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(funding, U256::from_str("115eec47f6cf79dd44ece4").unwrap());
            assert_eq!(pending_round, 1000);
            assert_eq!(balance, U256::from_str("115eec47f6cf79dd44ece4").unwrap());
        })
    }

    #[test]
    fn test_try_unlock_sequential_3_ticks() {
        let untrusted_local = Arc::new(MemoryKeyValue::new());
        let mut mkvs = Tree::make()
            .with_capacity(0, 0)
            .new(Box::new(NoopReadSyncer {}));

        StorageContext::enter(&mut mkvs, untrusted_local, || {
            let mut state = get_init_state();
            // Shift timestamp base on init_timestamp with 1..3 seconds.
            let timestamp = INIT_TIMESTAMP + 1;
            FundManager::try_unlock(timestamp, &mut state);
            let timestamp = INIT_TIMESTAMP + 2;
            FundManager::try_unlock(timestamp, &mut state);
            let timestamp = INIT_TIMESTAMP + 3;
            let funding = FundManager::try_unlock(timestamp, &mut state);

            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let pending_round = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(2)).unwrap();
            let unlocked_ticks = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(funding, U256::from_str("2cf96c8894fcf68").unwrap());
            assert_eq!(pending_round, 0);
            assert_eq!(unlocked_ticks, 3);
            assert_eq!(balance, U256::from_str("86ec4599bef6e38").unwrap());
        })
    }

    #[test]
    fn test_try_unlock_random_txs_20rounds() {
        let untrusted_local = Arc::new(MemoryKeyValue::new());
        let mut mkvs = Tree::make()
            .with_capacity(0, 0)
            .new(Box::new(NoopReadSyncer {}));

        StorageContext::enter(&mut mkvs, untrusted_local, || {
            let mut state = get_init_state();
            let mut timestamp = INIT_TIMESTAMP;
            let target_timestamp = INIT_TIMESTAMP + SECONDS_OF_30DAYS * 20;
            let mut rng = rand::thread_rng();
            while timestamp != target_timestamp {
                if timestamp + MIN_TX_INTERVAL >= target_timestamp {
                    timestamp = target_timestamp;
                } else {
                    timestamp = rng.gen_range(timestamp + MIN_TX_INTERVAL, target_timestamp + 1);
                }
                FundManager::try_unlock(timestamp, &mut state);
            }

            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let pending_round = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(2)).unwrap();
            let unlocked_ticks = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(pending_round, 20);
            assert_eq!(unlocked_ticks, 0);
            assert_eq!(balance, U256::from_str("8af7623fb67bf1a800000").unwrap());
        })
    }

    #[test]
    fn test_try_unlock_random_txs_cross_20rounds() {
        let untrusted_local = Arc::new(MemoryKeyValue::new());
        let mut mkvs = Tree::make()
            .with_capacity(0, 0)
            .new(Box::new(NoopReadSyncer {}));

        StorageContext::enter(&mut mkvs, untrusted_local, || {
            let mut state = get_init_state();
            let mut timestamp = INIT_TIMESTAMP;
            let target_timestamp = INIT_TIMESTAMP + SECONDS_OF_30DAYS * 20 + 1;
            let mut rng = rand::thread_rng();
            while timestamp != target_timestamp {
                if timestamp + MIN_TX_INTERVAL >= target_timestamp {
                    timestamp = target_timestamp;
                } else {
                    timestamp = rng.gen_range(timestamp + MIN_TX_INTERVAL, target_timestamp + 1);
                }
                FundManager::try_unlock(timestamp, &mut state);
            }

            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let pending_round = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(2)).unwrap();
            let unlocked_ticks = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(pending_round, 20);
            assert_eq!(unlocked_ticks, 1);
            assert_eq!(balance, U256::from_str("8af76256333235f27e7b4").unwrap());
        })
    }

    #[test]
    fn test_try_unlock_random_txs_cross_600days_with_speedup_factors() {
        let untrusted_local = Arc::new(MemoryKeyValue::new());
        let mut mkvs = Tree::make()
            .with_capacity(0, 0)
            .new(Box::new(NoopReadSyncer {}));

        StorageContext::enter(&mut mkvs, untrusted_local, || {
            let mut state = get_init_state();
            // Speed up factors
            // shorten time of the round to original 1/4
            // shorten cut down period to original 1/2
            state
                .set_storage(
                    &Address::from_str(FundManager::BENEFICIARY).unwrap(),
                    H256::from(3),
                    H256::from_str(
                        "0000000000000000000000000000000000000000000000040000000000000002",
                    )
                    .unwrap(),
                )
                .unwrap();

            let mut timestamp = INIT_TIMESTAMP;
            let target_timestamp = INIT_TIMESTAMP + SECONDS_OF_30DAYS * 20 + 1;
            let mut rng = rand::thread_rng();
            while timestamp != target_timestamp {
                if timestamp + MIN_TX_INTERVAL >= target_timestamp {
                    timestamp = target_timestamp;
                } else {
                    timestamp = rng.gen_range(timestamp + MIN_TX_INTERVAL, target_timestamp + 1);
                }
                FundManager::try_unlock(timestamp, &mut state);
            }

            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let pending_round = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(2)).unwrap();
            let unlocked_ticks = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(pending_round, 80);
            assert_eq!(unlocked_ticks, 1);
            assert_eq!(balance, U256::from_str("114d8d5bc55564fb157e7b").unwrap());
        })
    }
}
