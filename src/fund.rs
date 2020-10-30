use ethcore::{
    self,
    state::{CleanupMode, State},
};
use ethereum_types::{Address, H256, U256};
use oasis_ethwasi_runtime_common::parity::NullBackend;
extern crate chrono;
use chrono::{DateTime, Datelike, NaiveDateTime, Utc};
extern crate date_time;
use byteorder::{BigEndian, ReadBytesExt};
use date_time::month_tuple::MonthTuple;
use std::{convert::TryInto, str::FromStr};

/// Help function converts timestamp to month tuple (year, month)
/// In our use case we don't need day and time information from the timestamp.
fn timestamp_to_mt(timestamp: i64) -> MonthTuple {
    let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(timestamp, 0), Utc);
    return MonthTuple::new(
        dt.year().try_into().unwrap(),
        dt.month().try_into().unwrap(),
    )
    .unwrap();
}

pub struct FundManager;

impl FundManager {
    // There is a total fixed supply of 21 million OETHs.
    // The blockchain unlocks 525,000 OETHs every month in the first 20 months and
    // the monthly release is cut to 1/2 every 20 months.

    /// Unlock address.
    const BENEFICIARY: &'static str = "7110316b618d20d0c44728ac2a3d683536ea682b";
    /// Adjust period.
    const PERIOD: u32 = 20;
    /// Factor
    const FACTOR: u32 = 2;
    /// Initial unlock bucket is 525000000000000000000000 wei.
    const INITIAL_BUCKET: &'static str = "6F2C4E995EC98E200000";

    /// Primary unlock token method
    pub fn try_unlock(timestamp: u64, state: &mut State<NullBackend>) -> U256 {
        let beneficiary = Address::from_str(FundManager::BENEFICIARY).unwrap();
        let initial_bucket = U256::from_str(FundManager::INITIAL_BUCKET).unwrap();

        // +---------------+-------------------+---------+---------+
        // | Storage field |       0:23        |  24:27  |  28:32  |
        // +---------------+-------------------+---------+---------+
        // |             0 |     reserved      |  init timestamp   |
        // +---------------+-------------------+-------------------+
        // |             1 |          reserved           |  round  |
        // +---------------+---------------------------------------+
        let value = state.storage_at(&beneficiary, &H256::from(0)).unwrap();
        let init_timestamp = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
        let value = state.storage_at(&beneficiary, &H256::from(1)).unwrap();
        let mut unlock_round = value.get(28..).unwrap().read_u32::<BigEndian>().unwrap();

        let mut funding = U256::from(0);
        if timestamp as i64 >= init_timestamp {
            // At first we use step_ym point initial unlock (year, month),
            // than use step_ym plus unlock round to point unhandled (year, month).
            let mut step_ym = timestamp_to_mt(init_timestamp);
            step_ym.add_months(unlock_round);
            // The current (year, month) is converted from the timestamp that passed from ctx.header.
            let cur_ym = timestamp_to_mt(timestamp as i64);
            let mut exponent = 0;
            let mut bucket = initial_bucket;
            // If unhandled (year, month) is still before current (year, month)
            while step_ym <= cur_ym {
                if exponent != unlock_round / FundManager::PERIOD {
                    exponent = unlock_round / FundManager::PERIOD;
                    bucket =
                        initial_bucket / U256::from(FundManager::FACTOR).pow(U256::from(exponent));
                }
                funding = funding + bucket;
                step_ym.add_months(1);
                unlock_round += 1;
            }
            state
                .add_balance(&beneficiary, &funding, CleanupMode::NoEmpty)
                .unwrap();
            state
                .set_storage(&beneficiary, H256::from(1), H256::from(unlock_round as u64))
                .unwrap();
        }
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
    use std::sync::Arc;

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
            .set_storage(&monitor_address, H256::from(0), H256::from(1604188800))
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
            let current_timestamp: u64 = 1604102400; // 10/31/2020 @ 12:00am (UTC)
            let mut state = get_init_state();
            let funding = FundManager::try_unlock(current_timestamp, &mut state);
            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let unlock_round = value.get(28..).unwrap().read_u32::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(funding, U256::from(0));
            assert_eq!(unlock_round, 0);
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
            let current_timestamp: u64 = 1604188860; // 11/01/2020 @ 12:01am (UTC)
            let mut state = get_init_state();
            let funding = FundManager::try_unlock(current_timestamp, &mut state);
            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let unlock_round = value.get(28..).unwrap().read_u32::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(funding, U256::from_str("6f2c4e995ec98e200000").unwrap());
            assert_eq!(unlock_round, 1);
            assert_eq!(balance, U256::from_str("6f2c4e995ec98e200000").unwrap());
        })
    }

    #[test]
    fn test_try_unlock_10_round() {
        let untrusted_local = Arc::new(MemoryKeyValue::new());
        let mut mkvs = Tree::make()
            .with_capacity(0, 0)
            .new(Box::new(NoopReadSyncer {}));

        StorageContext::enter(&mut mkvs, untrusted_local, || {
            let current_timestamp: u64 = 1627776000; // 08/01/2021 @ 12:00am (UTC)
            let mut state = get_init_state();
            let funding = FundManager::try_unlock(current_timestamp, &mut state);
            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let unlock_round = value.get(28..).unwrap().read_u32::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(funding, U256::from_str("457bb11fdb3df8d400000").unwrap());
            assert_eq!(unlock_round, 10);
            assert_eq!(balance, U256::from_str("457bb11fdb3df8d400000").unwrap());
        })
    }

    #[test]
    fn test_try_unlock_100_round() {
        let untrusted_local = Arc::new(MemoryKeyValue::new());
        let mut mkvs = Tree::make()
            .with_capacity(0, 0)
            .new(Box::new(NoopReadSyncer {}));

        StorageContext::enter(&mut mkvs, untrusted_local, || {
            let current_timestamp: u64 = 1864598400; // 02/01/2029 @ 12:00am (UTC)
            let mut state = get_init_state();
            let funding = FundManager::try_unlock(current_timestamp, &mut state);
            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let unlock_round = value.get(28..).unwrap().read_u32::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(funding, U256::from_str("10d3f4e5b7190243580000").unwrap());
            assert_eq!(unlock_round, 100);
            assert_eq!(balance, U256::from_str("10d3f4e5b7190243580000").unwrap());
        })
    }

    #[test]
    fn test_try_unlock_1000_round() {
        let untrusted_local = Arc::new(MemoryKeyValue::new());
        let mut mkvs = Tree::make()
            .with_capacity(0, 0)
            .new(Box::new(NoopReadSyncer {}));

        StorageContext::enter(&mut mkvs, untrusted_local, || {
            let current_timestamp: u64 = 4231267200; // 02/01/2104 @ 12:00am (UTC)
            let mut state = get_init_state();
            let funding = FundManager::try_unlock(current_timestamp, &mut state);
            let monitor_address = Address::from_str(FundManager::BENEFICIARY).unwrap();
            let value = state.storage_at(&monitor_address, &H256::from(1)).unwrap();
            let unlock_round = value.get(28..).unwrap().read_u32::<BigEndian>().unwrap();
            let balance = state.balance(&monitor_address).unwrap();

            assert_eq!(funding, U256::from_str("115eec47f6cf79dd44ece4").unwrap());
            assert_eq!(unlock_round, 1000);
            assert_eq!(balance, U256::from_str("115eec47f6cf79dd44ece4").unwrap());
        })
    }
}
