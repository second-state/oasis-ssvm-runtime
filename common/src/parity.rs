//! Common parity helpers.
use std::sync::Arc;

use account_state::{backend, Account};
use ethereum_types::{Address, H256};
use hashdb::HashDB;
use keccak_hasher::KeccakHasher;
use kvdb::DBValue;

/// Null backend for parity state.
///
/// This backend is never actually used as a HashDB because Parity
/// has been updated to use our MKVS for storage.
pub struct NullBackend;

impl backend::Backend for NullBackend {
    fn as_hash_db(&self) -> &dyn HashDB<KeccakHasher, DBValue> {
        unimplemented!("HashDB should never be used");
    }

    fn as_hash_db_mut(&mut self) -> &mut dyn HashDB<KeccakHasher, DBValue> {
        unimplemented!("HashDB should never be used");
    }

    fn add_to_account_cache(&mut self, _: Address, _: Option<Account>, _: bool) {}

    fn cache_code(&self, _: H256, _: Arc<Vec<u8>>) {}

    fn get_cached_account(&self, _: &Address) -> Option<Option<Account>> {
        None
    }

    fn get_cached<F, U>(&self, _: &Address, _: F) -> Option<U>
    where
        F: FnOnce(Option<&mut Account>) -> U,
    {
        None
    }

    fn get_cached_code(&self, _: &H256) -> Option<Arc<Vec<u8>>> {
        None
    }
}
