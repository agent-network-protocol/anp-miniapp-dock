use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub const DEFAULT_MAX_STORAGE_KEY_BYTES: usize = 128;
pub const DEFAULT_MAX_STORAGE_VALUE_BYTES: usize = 16 * 1024;
pub const DEFAULT_MAX_STORAGE_SCOPE_BYTES: usize = 128 * 1024;
pub const DEFAULT_STORAGE_NAMESPACE: &str = "default";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageScope {
    pub user_did: String,
    pub merchant_did: String,
    pub skill_id: String,
    pub namespace: String,
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
            namespace: DEFAULT_STORAGE_NAMESPACE.to_owned(),
        }
    }

    pub fn with_namespace(
        user_did: impl Into<String>,
        merchant_did: impl Into<String>,
        skill_id: impl Into<String>,
        namespace: impl Into<String>,
    ) -> Self {
        Self {
            user_did: user_did.into(),
            merchant_did: merchant_did.into(),
            skill_id: skill_id.into(),
            namespace: namespace.into(),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StorageError {
    #[error("storage key is empty")]
    EmptyKey,

    #[error("storage key contains a NUL byte")]
    KeyContainsNul,

    #[error("storage key is too large")]
    KeyTooLarge,

    #[error("storage key is sensitive")]
    SensitiveKey,

    #[error("storage value is too large")]
    ValueTooLarge,

    #[error("storage value is not JSON-safe")]
    ValueNotJsonSafe,

    #[error("storage scope is invalid")]
    InvalidScope,

    #[error("storage quota exceeded")]
    QuotaExceeded,

    #[error("storage lock is poisoned")]
    LockPoisoned,

    #[error("storage persistence backend is unavailable")]
    BackendUnavailable,

    #[error("storage persistence data is corrupt")]
    BackendCorrupt,
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
    fn clear_storage(&self, scope: &StorageScope) -> Result<(), StorageError>;
    fn delete_scope(&self, scope: &StorageScope) -> Result<(), StorageError> {
        self.clear_storage(scope)
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryScopedStorage {
    inner: Arc<Mutex<BTreeMap<StorageScope, BTreeMap<String, Value>>>>,
}

impl InMemoryScopedStorage {
    pub fn new() -> Self {
        Self::default()
    }

    fn snapshot_entries(&self) -> Result<Vec<PersistentStorageEntry>, StorageError> {
        let guard = self.inner.lock().map_err(|_| StorageError::LockPoisoned)?;
        guard
            .iter()
            .flat_map(|(scope, values)| {
                values
                    .iter()
                    .map(move |(key, value)| (scope.clone(), key.clone(), value.clone()))
            })
            .map(|(scope, key, value)| PersistentStorageEntry::new(scope, key, value))
            .collect()
    }
}

impl ScopedStorage for InMemoryScopedStorage {
    fn get_storage(&self, scope: &StorageScope, key: &str) -> Result<Option<Value>, StorageError> {
        validate_scope(scope)?;
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
        validate_scope(scope)?;
        validate_key(&key)?;
        validate_value(&value)?;
        let mut guard = self.inner.lock().map_err(|_| StorageError::LockPoisoned)?;
        let scope_values = guard.entry(scope.clone()).or_default();
        let previous = scope_values.insert(key.clone(), value);
        if scope_size_bytes(scope_values)? > DEFAULT_MAX_STORAGE_SCOPE_BYTES {
            if let Some(previous) = previous {
                scope_values.insert(key, previous);
            } else {
                scope_values.remove(&key);
            }
            return Err(StorageError::QuotaExceeded);
        }
        Ok(())
    }

    fn remove_storage(
        &self,
        scope: &StorageScope,
        key: &str,
    ) -> Result<Option<Value>, StorageError> {
        validate_scope(scope)?;
        validate_key(key)?;
        let mut guard = self.inner.lock().map_err(|_| StorageError::LockPoisoned)?;
        Ok(guard.get_mut(scope).and_then(|values| values.remove(key)))
    }

    fn clear_storage(&self, scope: &StorageScope) -> Result<(), StorageError> {
        validate_scope(scope)?;
        let mut guard = self.inner.lock().map_err(|_| StorageError::LockPoisoned)?;
        guard.remove(scope);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoragePersistenceProfile {
    InMemoryDev,
    LocalFileUnencrypted,
    HostEncryptedStore,
    EncryptedSqlite,
}

impl StoragePersistenceProfile {
    pub fn production_ready(self) -> bool {
        matches!(
            self,
            StoragePersistenceProfile::HostEncryptedStore
                | StoragePersistenceProfile::EncryptedSqlite
        )
    }
}

#[derive(Clone, PartialEq)]
pub struct PersistentStorageEntry {
    pub scope: StorageScope,
    pub key: String,
    value: Value,
}

impl PersistentStorageEntry {
    pub fn new(
        scope: StorageScope,
        key: impl Into<String>,
        value: Value,
    ) -> Result<Self, StorageError> {
        let key = key.into();
        validate_scope(&scope)?;
        validate_key(&key)?;
        validate_value(&value)?;
        Ok(Self { scope, key, value })
    }

    pub fn value(&self) -> Value {
        self.value.clone()
    }

    fn value_size_bytes(&self) -> Result<usize, StorageError> {
        serde_json::to_vec(&self.value)
            .map(|bytes| bytes.len())
            .map_err(|_| StorageError::ValueNotJsonSafe)
    }
}

impl fmt::Debug for PersistentStorageEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PersistentStorageEntry")
            .field("scope", &StorageScopeSummary::from(&self.scope))
            .field("key_bytes", &self.key.len())
            .field("value", &"[REDACTED]")
            .field("value_bytes", &self.value_size_bytes().unwrap_or(0))
            .finish()
    }
}

pub trait ScopedStoragePersistenceBackend: Clone {
    fn profile(&self) -> StoragePersistenceProfile;
    fn load_entries(&self) -> Result<Vec<PersistentStorageEntry>, StorageError>;
    fn load_restore_snapshot(&self) -> Result<StoragePersistenceSnapshot, StorageError> {
        self.load_entries()
            .map(StoragePersistenceSnapshot::from_entries)
    }
    fn replace_entries(&self, entries: Vec<PersistentStorageEntry>) -> Result<(), StorageError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoragePersistenceSnapshot {
    pub loaded_count: usize,
    pub entries: Vec<PersistentStorageEntry>,
    pub rejected: Vec<StorageRestoreRejection>,
}

impl StoragePersistenceSnapshot {
    pub fn from_entries(entries: Vec<PersistentStorageEntry>) -> Self {
        Self {
            loaded_count: entries.len(),
            entries,
            rejected: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalFileScopedStorageBackend {
    path: PathBuf,
}

impl LocalFileScopedStorageBackend {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn load_records(&self) -> Result<Vec<StorageFileRecord>, StorageError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(StorageError::BackendUnavailable),
        };
        serde_json::from_slice(&bytes).map_err(|_| StorageError::BackendCorrupt)
    }
}

impl ScopedStoragePersistenceBackend for LocalFileScopedStorageBackend {
    fn profile(&self) -> StoragePersistenceProfile {
        StoragePersistenceProfile::LocalFileUnencrypted
    }

    fn load_entries(&self) -> Result<Vec<PersistentStorageEntry>, StorageError> {
        Ok(self.load_restore_snapshot()?.entries)
    }

    fn load_restore_snapshot(&self) -> Result<StoragePersistenceSnapshot, StorageError> {
        let records = self.load_records()?;
        let loaded_count = records.len();
        let mut entries = Vec::new();
        let mut rejected = Vec::new();

        for record in records {
            match PersistentStorageEntry::new(
                record.scope.clone(),
                record.key.clone(),
                record.value.clone(),
            ) {
                Ok(entry) => entries.push(entry),
                Err(_) => rejected.push(storage_record_rejection(
                    &record,
                    StorageRestoreRejectionReason::InvalidEntry,
                )),
            }
        }

        Ok(StoragePersistenceSnapshot {
            loaded_count,
            entries,
            rejected,
        })
    }

    fn replace_entries(&self, entries: Vec<PersistentStorageEntry>) -> Result<(), StorageError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| StorageError::BackendUnavailable)?;
        }
        let records: Vec<StorageFileRecord> = entries
            .into_iter()
            .map(|entry| StorageFileRecord {
                scope: entry.scope,
                key: entry.key,
                value: entry.value,
            })
            .collect();
        let bytes =
            serde_json::to_vec_pretty(&records).map_err(|_| StorageError::BackendCorrupt)?;
        let mut tmp_path = self.path.clone();
        tmp_path.set_extension("tmp");
        fs::write(&tmp_path, bytes).map_err(|_| StorageError::BackendUnavailable)?;
        fs::rename(tmp_path, &self.path).map_err(|_| StorageError::BackendUnavailable)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageFileRecord {
    scope: StorageScope,
    key: String,
    value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageRestoreReport {
    pub backend_profile: StoragePersistenceProfile,
    pub production_ready: bool,
    pub loaded_count: usize,
    pub restored_count: usize,
    pub rejected: Vec<StorageRestoreRejection>,
    pub redaction: StorageRedaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageRestoreRejection {
    pub scope: StorageScopeSummary,
    pub key_bytes: usize,
    pub value_bytes: usize,
    pub reason: StorageRestoreRejectionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StorageRestoreRejectionReason {
    InvalidEntry,
    QuotaExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageScopeSummary {
    pub skill_id: String,
    pub namespace: String,
    pub has_user_did: bool,
    pub has_merchant_did: bool,
}

impl From<&StorageScope> for StorageScopeSummary {
    fn from(scope: &StorageScope) -> Self {
        Self {
            skill_id: scope.skill_id.clone(),
            namespace: scope.namespace.clone(),
            has_user_did: !scope.user_did.trim().is_empty(),
            has_merchant_did: !scope.merchant_did.trim().is_empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageRedaction {
    pub marker: String,
    pub policy: String,
    pub raw_key_visible: bool,
    pub raw_value_visible: bool,
}

impl Default for StorageRedaction {
    fn default() -> Self {
        Self {
            marker: "[REDACTED]".to_owned(),
            policy: "dock.scoped-storage.redaction.v1".to_owned(),
            raw_key_visible: false,
            raw_value_visible: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PersistentScopedStorage<B> {
    backend: B,
    cache: InMemoryScopedStorage,
}

impl<B> PersistentScopedStorage<B>
where
    B: ScopedStoragePersistenceBackend,
{
    pub fn restore(backend: B) -> Result<(Self, StorageRestoreReport), StorageError> {
        let snapshot = backend.load_restore_snapshot()?;
        let loaded_count = snapshot.loaded_count;
        let cache = InMemoryScopedStorage::new();
        let mut restored_entries = Vec::new();
        let mut rejected = snapshot.rejected;

        for entry in snapshot.entries {
            match cache.set_storage(&entry.scope, entry.key.clone(), entry.value()) {
                Ok(()) => restored_entries.push(entry),
                Err(StorageError::QuotaExceeded) => rejected.push(storage_rejection(
                    &entry,
                    StorageRestoreRejectionReason::QuotaExceeded,
                )),
                Err(_) => rejected.push(storage_rejection(
                    &entry,
                    StorageRestoreRejectionReason::InvalidEntry,
                )),
            }
        }

        backend.replace_entries(restored_entries.clone())?;
        let profile = backend.profile();
        let report = StorageRestoreReport {
            backend_profile: profile,
            production_ready: profile.production_ready(),
            loaded_count,
            restored_count: restored_entries.len(),
            rejected,
            redaction: StorageRedaction::default(),
        };

        Ok((Self { backend, cache }, report))
    }

    pub fn try_set_storage(
        &self,
        scope: &StorageScope,
        key: impl Into<String>,
        value: Value,
    ) -> Result<(), StorageError> {
        let key = key.into();
        let entry = PersistentStorageEntry::new(scope.clone(), key.clone(), value.clone())?;
        let mut entries = self.cache.snapshot_entries()?;
        entries.retain(|existing| !(existing.scope == entry.scope && existing.key == entry.key));
        entries.push(entry);
        validate_entries_quota(&entries)?;
        self.backend.replace_entries(entries)?;
        self.cache.set_storage(scope, key, value)
    }

    pub fn try_remove_storage(
        &self,
        scope: &StorageScope,
        key: &str,
    ) -> Result<Option<Value>, StorageError> {
        validate_scope(scope)?;
        validate_key(key)?;
        let mut entries = self.cache.snapshot_entries()?;
        entries.retain(|existing| !(existing.scope == *scope && existing.key == key));
        self.backend.replace_entries(entries)?;
        self.cache.remove_storage(scope, key)
    }

    pub fn try_clear_storage(&self, scope: &StorageScope) -> Result<(), StorageError> {
        validate_scope(scope)?;
        let mut entries = self.cache.snapshot_entries()?;
        entries.retain(|existing| existing.scope != *scope);
        self.backend.replace_entries(entries)?;
        self.cache.clear_storage(scope)
    }

    pub fn try_delete_scope(&self, scope: &StorageScope) -> Result<(), StorageError> {
        self.try_clear_storage(scope)
    }

    pub fn restore_report(&self) -> Result<(StoragePersistenceProfile, usize), StorageError> {
        Ok((self.backend.profile(), self.backend.load_entries()?.len()))
    }
}

impl<B> ScopedStorage for PersistentScopedStorage<B>
where
    B: ScopedStoragePersistenceBackend,
{
    fn get_storage(&self, scope: &StorageScope, key: &str) -> Result<Option<Value>, StorageError> {
        self.cache.get_storage(scope, key)
    }

    fn set_storage(
        &self,
        scope: &StorageScope,
        key: impl Into<String>,
        value: Value,
    ) -> Result<(), StorageError> {
        self.try_set_storage(scope, key, value)
    }

    fn remove_storage(
        &self,
        scope: &StorageScope,
        key: &str,
    ) -> Result<Option<Value>, StorageError> {
        self.try_remove_storage(scope, key)
    }

    fn clear_storage(&self, scope: &StorageScope) -> Result<(), StorageError> {
        self.try_clear_storage(scope)
    }

    fn delete_scope(&self, scope: &StorageScope) -> Result<(), StorageError> {
        self.try_delete_scope(scope)
    }
}

fn validate_key(key: &str) -> Result<(), StorageError> {
    if key.trim().is_empty() {
        return Err(StorageError::EmptyKey);
    }
    if key.contains('\0') {
        return Err(StorageError::KeyContainsNul);
    }
    if key.len() > DEFAULT_MAX_STORAGE_KEY_BYTES {
        return Err(StorageError::KeyTooLarge);
    }
    let normalized = key.to_ascii_lowercase();
    if [
        "token",
        "authorization",
        "signature",
        "secret",
        "private",
        "credential",
        "phone",
        "address",
        "filecontent",
    ]
    .iter()
    .any(|sensitive| normalized.contains(sensitive))
    {
        return Err(StorageError::SensitiveKey);
    }
    Ok(())
}

fn validate_scope(scope: &StorageScope) -> Result<(), StorageError> {
    if scope.user_did.trim().is_empty()
        || scope.merchant_did.trim().is_empty()
        || scope.skill_id.trim().is_empty()
        || scope.namespace.trim().is_empty()
        || scope.user_did.contains('\0')
        || scope.merchant_did.contains('\0')
        || scope.skill_id.contains('\0')
        || scope.namespace.contains('\0')
    {
        return Err(StorageError::InvalidScope);
    }
    Ok(())
}

fn validate_value(value: &Value) -> Result<(), StorageError> {
    let serialized = serde_json::to_vec(value).map_err(|_| StorageError::ValueNotJsonSafe)?;
    if serialized.len() > DEFAULT_MAX_STORAGE_VALUE_BYTES {
        return Err(StorageError::ValueTooLarge);
    }
    Ok(())
}

fn validate_entries_quota(entries: &[PersistentStorageEntry]) -> Result<(), StorageError> {
    let mut by_scope: BTreeMap<StorageScope, BTreeMap<String, Value>> = BTreeMap::new();
    for entry in entries {
        by_scope
            .entry(entry.scope.clone())
            .or_default()
            .insert(entry.key.clone(), entry.value());
    }
    for values in by_scope.values() {
        if scope_size_bytes(values)? > DEFAULT_MAX_STORAGE_SCOPE_BYTES {
            return Err(StorageError::QuotaExceeded);
        }
    }
    Ok(())
}

fn storage_rejection(
    entry: &PersistentStorageEntry,
    reason: StorageRestoreRejectionReason,
) -> StorageRestoreRejection {
    StorageRestoreRejection {
        scope: StorageScopeSummary::from(&entry.scope),
        key_bytes: entry.key.len(),
        value_bytes: entry.value_size_bytes().unwrap_or(0),
        reason,
    }
}

fn storage_record_rejection(
    record: &StorageFileRecord,
    reason: StorageRestoreRejectionReason,
) -> StorageRestoreRejection {
    let value_bytes = serde_json::to_vec(&record.value).map_or(0, |bytes| bytes.len());
    StorageRestoreRejection {
        scope: StorageScopeSummary::from(&record.scope),
        key_bytes: record.key.len(),
        value_bytes,
        reason,
    }
}

fn scope_size_bytes(values: &BTreeMap<String, Value>) -> Result<usize, StorageError> {
    values.iter().try_fold(0usize, |total, (key, value)| {
        let value_bytes = serde_json::to_vec(value).map_err(|_| StorageError::ValueNotJsonSafe)?;
        Ok(total + key.len() + value_bytes.len())
    })
}
