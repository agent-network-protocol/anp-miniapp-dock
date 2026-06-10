use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StorageScope {
    pub user_did: String,
    pub merchant_did: String,
    pub skill_id: String,
}

impl StorageScope {
    pub fn new(
        user_did: impl Into<String>,
        merchant_did: impl Into<String>,
        skill_id: impl Into<String>,
    ) -> Self {
        Self {
            user_did: user_did.into(),
            merchant_did: merchant_did.into(),
            skill_id: skill_id.into(),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StorageError {
    #[error("storage key is empty")]
    EmptyKey,

    #[error("storage lock is poisoned")]
    LockPoisoned,
}

pub trait ScopedStorage {
    fn get_storage(&self, scope: &StorageScope, key: &str) -> Result<Option<Value>, StorageError>;
    fn set_storage(
        &self,
        scope: &StorageScope,
        key: impl Into<String>,
        value: Value,
    ) -> Result<(), StorageError>;
    fn remove_storage(
        &self,
        scope: &StorageScope,
        key: &str,
    ) -> Result<Option<Value>, StorageError>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryScopedStorage {
    inner: Arc<Mutex<BTreeMap<StorageScope, BTreeMap<String, Value>>>>,
}

impl InMemoryScopedStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ScopedStorage for InMemoryScopedStorage {
    fn get_storage(&self, scope: &StorageScope, key: &str) -> Result<Option<Value>, StorageError> {
        validate_key(key)?;
        let guard = self.inner.lock().map_err(|_| StorageError::LockPoisoned)?;
        Ok(guard.get(scope).and_then(|values| values.get(key).cloned()))
    }

    fn set_storage(
        &self,
        scope: &StorageScope,
        key: impl Into<String>,
        value: Value,
    ) -> Result<(), StorageError> {
        let key = key.into();
        validate_key(&key)?;
        let mut guard = self.inner.lock().map_err(|_| StorageError::LockPoisoned)?;
        guard.entry(scope.clone()).or_default().insert(key, value);
        Ok(())
    }

    fn remove_storage(
        &self,
        scope: &StorageScope,
        key: &str,
    ) -> Result<Option<Value>, StorageError> {
        validate_key(key)?;
        let mut guard = self.inner.lock().map_err(|_| StorageError::LockPoisoned)?;
        Ok(guard.get_mut(scope).and_then(|values| values.remove(key)))
    }
}

fn validate_key(key: &str) -> Result<(), StorageError> {
    if key.trim().is_empty() {
        Err(StorageError::EmptyKey)
    } else {
        Ok(())
    }
}
