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
    /// Unlock address.
    const BENEFICIARY: &'static str = "7110316b618d20d0c44728ac2a3d683536ea682b";
    /// Adjust period.
    const PERIOD: u32 = 20;
    // Factor
    const FACTOR: f32 = 0.5;
    /// Total Amount.
    const TOTAL_AMOUNT: u32 = 21000000;
    /// Initial unlock bucket.
    const INITIAL_BUCKET: f32 = 525000.0;

    // There is a total fixed supply of 21 million OETHs.
    // The blockchain unlocks 525,000 OETHs every month in the first 20 months and
    // the monthly release is cut to 1/2 every 20 months.

    pub fn try_unlock(timestamp: u64, state: &mut State<NullBackend>) {
        let beneficiary = Address::from_str(FundManager::BENEFICIARY).unwrap();

        // +---------------+----------------+-----------+-----------+
        // | Storage field |       0:23     |   24:27   |   28:32   |
        // +---------------+----------------+-----------+-----------+
        // |             0 |     reserved   |    init timestamp     |
        // +---------------+----------+-----------------+-----------+
        // |             1 |            reserved        |   round   |
        // +---------------+----------+-----------------+-----------+
        // |             2 |            reserved        |   amount  |
        // +---------------+----------+-----------------+-----------+
        let value = state.storage_at(&beneficiary, &H256::from(0)).unwrap();
        let init_timestamp = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
        let value = state.storage_at(&beneficiary, &H256::from(1)).unwrap();
        let mut unlock_round = value.get(28..).unwrap().read_u32::<BigEndian>().unwrap();
        let value = state.storage_at(&beneficiary, &H256::from(2)).unwrap();
        let mut unlock_amount = value.get(28..).unwrap().read_u32::<BigEndian>().unwrap();

        if unlock_amount < FundManager::TOTAL_AMOUNT {
            let mut last_ym = timestamp_to_mt(init_timestamp as i64);
            last_ym.add_months(unlock_round);
            let cur_ym = timestamp_to_mt(timestamp as i64);
            let mut funding = 0;
            while last_ym < cur_ym && unlock_amount < FundManager::TOTAL_AMOUNT {
                last_ym.add_months(1);
                unlock_round += 1;
                let exponent = unlock_round / FundManager::PERIOD;
                let mut amount = (FundManager::INITIAL_BUCKET
                    * (FundManager::FACTOR.powf(exponent as f32)))
                    as u32;
                if unlock_amount + amount > FundManager::TOTAL_AMOUNT {
                    amount = FundManager::TOTAL_AMOUNT - unlock_amount;
                }
                println!("{:?} {:?} {:?}", last_ym, unlock_round, amount);
                funding += amount;
                unlock_amount += amount;
            }
            state
                .add_balance(&beneficiary, &U256::from(funding), CleanupMode::NoEmpty)
                .unwrap();
            state
                .set_storage(&beneficiary, H256::from(1), H256::from(unlock_round as u64))
                .unwrap();
            state
                .set_storage(
                    &beneficiary,
                    H256::from(2),
                    H256::from(unlock_amount as u64),
                )
                .unwrap();
        }
    }
}
