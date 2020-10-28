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
    /// Total Amount is 21000000000000000000000000 wei.
    const TOTAL_AMOUNT: &'static str = "115EEC47F6CF7E35000000";
    /// Initial unlock bucket is 525000000000000000000000 wei.
    const INITIAL_BUCKET: &'static str = "6F2C4E995EC98E200000";

    /// Primary unlock token method
    pub fn try_unlock(timestamp: u64, state: &mut State<NullBackend>) {
        let beneficiary = Address::from_str(FundManager::BENEFICIARY).unwrap();
        let total_amount = U256::from_str(FundManager::TOTAL_AMOUNT).unwrap();
        let initial_bucket = U256::from_str(FundManager::INITIAL_BUCKET).unwrap();

        // +---------------+-------------------+---------+---------+
        // | Storage field |       0:23        |  24:27  |  28:32  |
        // +---------------+-------------------+---------+---------+
        // |             0 |     reserved      |  init timestamp   |
        // +---------------+-------------------+-------------------+
        // |             1 |          reserved           |  round  |
        // +---------------+-----------------------------+---------+
        // |             2 |             unlock amount             |
        // +---------------+---------------------------------------+
        let value = state.storage_at(&beneficiary, &H256::from(0)).unwrap();
        let init_timestamp = value.get(24..).unwrap().read_i64::<BigEndian>().unwrap();
        let value = state.storage_at(&beneficiary, &H256::from(1)).unwrap();
        let mut unlock_round = value.get(28..).unwrap().read_u32::<BigEndian>().unwrap();
        let value = state.storage_at(&beneficiary, &H256::from(2)).unwrap();
        let mut unlock_amount: U256 = value.into();

        // At first we use step_ym point initial unlock (year, month),
        // than step_ym plus unlock round to point unhandled (year, month).
        let mut step_ym = timestamp_to_mt(init_timestamp as i64);
        step_ym.add_months(unlock_round);
        // The current (year, month) is converted from the timestamp that passed from ctx.header.
        let cur_ym = timestamp_to_mt(timestamp as i64);

        let mut funding = U256::from(0);
        let mut exponent = 0;
        let mut bucket = initial_bucket;
        // If unhandled (year, month) is still before current (year, month)
        while step_ym <= cur_ym {
            if exponent != unlock_round / FundManager::PERIOD {
                exponent = unlock_round / FundManager::PERIOD;
                bucket = initial_bucket / U256::from(FundManager::FACTOR).pow(U256::from(exponent));
            }
            funding = funding + bucket;
            unlock_amount = unlock_amount + bucket;
            step_ym.add_months(1);
            unlock_round += 1;
        }
        state
            .add_balance(&beneficiary, &funding, CleanupMode::NoEmpty)
            .unwrap();
        state
            .set_storage(&beneficiary, H256::from(1), H256::from(unlock_round as u64))
            .unwrap();
        state
            .set_storage(&beneficiary, H256::from(2), unlock_amount.into())
            .unwrap();
    }
}
