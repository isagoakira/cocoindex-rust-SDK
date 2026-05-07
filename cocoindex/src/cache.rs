//! Cache module for CocoIndex

use crate::Result;
use lmdb::{
    Database, DatabaseFlags, Environment, RoTransaction, RwTransaction, Transaction, WriteFlags,
};
use std::sync::Arc;

/// LMDB-backed cache storage
#[derive(Clone)]
pub struct Cache {
    db: Database,
    env: Arc<Environment>,
}

impl Cache {
    /// Open a cache with the given LMDB environment
    pub fn open(env: &Arc<Environment>) -> Result<Self> {
        let db = env
            .create_db(Some("cache"), DatabaseFlags::empty())
            .map_err(|e| crate::CocoError::Lmdb(e.to_string()))?;

        Ok(Cache {
            db,
            env: env.clone(),
        })
    }

    /// Get a cached value by key
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let txn: RoTransaction = self
            .env
            .begin_ro_txn()
            .map_err(|e| crate::CocoError::Lmdb(e.to_string()))?;

        let result = match txn.get(self.db, &key) {
            Ok(bytes) => Ok(Some(bytes.to_vec())),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(crate::CocoError::Lmdb(e.to_string())),
        };

        // txn is dropped here
        result
    }

    /// Set a cached value
    pub fn set(&self, key: &str, value: &[u8]) -> Result<()> {
        let mut txn: RwTransaction = self
            .env
            .begin_rw_txn()
            .map_err(|e| crate::CocoError::Lmdb(e.to_string()))?;

        txn.put(self.db, &key, &value, WriteFlags::empty())
            .map_err(|e| crate::CocoError::Lmdb(e.to_string()))?;

        txn.commit()
            .map_err(|e| crate::CocoError::Lmdb(e.to_string()))
    }

    /// Delete a cached value
    pub fn delete(&self, key: &str) -> Result<()> {
        let mut txn: RwTransaction = self
            .env
            .begin_rw_txn()
            .map_err(|e| crate::CocoError::Lmdb(e.to_string()))?;

        txn.del(self.db, &key, None)
            .map_err(|e| crate::CocoError::Lmdb(e.to_string()))?;

        txn.commit()
            .map_err(|e| crate::CocoError::Lmdb(e.to_string()))
    }

    /// Clear all cached values
    pub fn clear(&self) -> Result<()> {
        let mut txn: RwTransaction = self
            .env
            .begin_rw_txn()
            .map_err(|e| crate::CocoError::Lmdb(e.to_string()))?;

        txn.clear_db(self.db)
            .map_err(|e| crate::CocoError::Lmdb(e.to_string()))?;

        txn.commit()
            .map_err(|e| crate::CocoError::Lmdb(e.to_string()))
    }
}
