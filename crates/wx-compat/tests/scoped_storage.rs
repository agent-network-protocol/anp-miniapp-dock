use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use wx_compat::{
    InMemoryScopedStorage, LocalFileScopedStorageBackend, ModelContext, PersistentScopedStorage,
    PersistentStorageEntry, ScopedStorage, ScopedStoragePersistenceBackend, StorageError,
    StoragePersistenceProfile, StorageScope, DEFAULT_MAX_STORAGE_KEY_BYTES,
    DEFAULT_MAX_STORAGE_SCOPE_BYTES, DEFAULT_MAX_STORAGE_VALUE_BYTES,
};

#[test]
fn storage_is_scoped_by_user_merchant_and_skill() {
    let storage = InMemoryScopedStorage::new();
    let alice = StorageScope::new("did:example:alice", "did:example:merchant-a", "coffee");
    let bob = StorageScope::new("did:example:bob", "did:example:merchant-a", "coffee");
    let merchant_b = StorageScope::new("did:example:alice", "did:example:merchant-b", "coffee");
    let other_skill = StorageScope::new("did:example:alice", "did:example:merchant-a", "tea");

    storage
        .set_storage(&alice, "cart", json!({ "drinkId": "latte" }))
        .expect("set storage");

    assert_eq!(
        storage.get_storage(&alice, "cart").expect("get alice"),
        Some(json!({ "drinkId": "latte" }))
    );
    assert_eq!(storage.get_storage(&bob, "cart").expect("get bob"), None);
    assert_eq!(
        storage
            .get_storage(&merchant_b, "cart")
            .expect("get merchant-b"),
        None
    );
    assert_eq!(
        storage
            .get_storage(&other_skill, "cart")
            .expect("get other skill"),
        None
    );
}

#[test]
fn model_context_builds_storage_scope() {
    let context = ModelContext::new(
        "session-1",
        "coffee",
        "did:example:alice",
        "did:example:merchant",
    );

    assert_eq!(
        context.storage_scope(),
        StorageScope::new("did:example:alice", "did:example:merchant", "coffee")
    );
    assert_eq!(context.get_session_id(), "session-1");
}

#[test]
fn empty_storage_key_is_rejected() {
    let storage = InMemoryScopedStorage::new();
    let scope = StorageScope::new("did:example:alice", "did:example:merchant", "coffee");

    assert_eq!(
        storage.set_storage(&scope, " ", json!(true)),
        Err(StorageError::EmptyKey)
    );
}

#[test]
fn storage_rejects_nul_and_oversized_key() {
    let storage = InMemoryScopedStorage::new();
    let scope = StorageScope::new("did:example:alice", "did:example:merchant", "coffee");

    assert_eq!(
        storage.set_storage(&scope, "cart\0token", json!(true)),
        Err(StorageError::KeyContainsNul)
    );
    assert_eq!(
        storage.set_storage(
            &scope,
            "k".repeat(DEFAULT_MAX_STORAGE_KEY_BYTES + 1),
            json!(true)
        ),
        Err(StorageError::KeyTooLarge)
    );
}

#[test]
fn storage_rejects_sensitive_key_names() {
    let storage = InMemoryScopedStorage::new();
    let scope = StorageScope::new("did:example:alice", "did:example:merchant", "coffee");

    assert_eq!(
        storage.set_storage(&scope, "Authorization", json!("Bearer test-token")),
        Err(StorageError::SensitiveKey)
    );
}

#[test]
fn storage_rejects_oversized_value() {
    let storage = InMemoryScopedStorage::new();
    let scope = StorageScope::new("did:example:alice", "did:example:merchant", "coffee");

    assert_eq!(
        storage.set_storage(
            &scope,
            "cart",
            json!("x".repeat(DEFAULT_MAX_STORAGE_VALUE_BYTES + 1))
        ),
        Err(StorageError::ValueTooLarge)
    );
}

#[test]
fn storage_clear_removes_only_current_scope() {
    let storage = InMemoryScopedStorage::new();
    let alice = StorageScope::new("did:example:alice", "did:example:merchant", "coffee");
    let bob = StorageScope::new("did:example:bob", "did:example:merchant", "coffee");

    storage
        .set_storage(&alice, "cart", json!({ "drinkId": "latte" }))
        .expect("set alice");
    storage
        .set_storage(&bob, "cart", json!({ "drinkId": "tea" }))
        .expect("set bob");
    storage.clear_storage(&alice).expect("clear alice");

    assert_eq!(
        storage.get_storage(&alice, "cart").expect("get alice"),
        None
    );
    assert_eq!(
        storage.get_storage(&bob, "cart").expect("get bob"),
        Some(json!({ "drinkId": "tea" }))
    );
}

#[test]
fn storage_scope_quota_is_enforced_without_partial_write() {
    let storage = InMemoryScopedStorage::new();
    let scope = StorageScope::new("did:example:alice", "did:example:merchant", "coffee");

    storage
        .set_storage(&scope, "cart", json!("ok"))
        .expect("set baseline");
    let payload = json!("x".repeat(1024));
    let mut rejected_key = None;
    for index in 0..DEFAULT_MAX_STORAGE_SCOPE_BYTES {
        let key = format!("quota-{index}");
        match storage.set_storage(&scope, key.clone(), payload.clone()) {
            Ok(()) => {}
            Err(StorageError::QuotaExceeded) => {
                rejected_key = Some(key);
                break;
            }
            Err(error) => panic!("unexpected storage error: {error:?}"),
        }
    }
    let rejected_key = rejected_key.expect("scope quota should be reached");
    assert_eq!(
        storage.get_storage(&scope, "cart").expect("get baseline"),
        Some(json!("ok"))
    );
    assert_eq!(
        storage
            .get_storage(&scope, &rejected_key)
            .expect("get rejected key"),
        None
    );
}

#[test]
fn storage_persistence_restores_same_scope_and_keeps_cross_scope_isolated() {
    let path = temp_storage_path("restore");
    let backend = LocalFileScopedStorageBackend::new(&path);
    let (storage, report) = PersistentScopedStorage::restore(backend.clone()).expect("restore");
    let alice = StorageScope::new("did:example:alice", "did:example:merchant", "coffee");
    let bob = StorageScope::new("did:example:bob", "did:example:merchant", "coffee");
    let alternate_namespace = StorageScope::with_namespace(
        "did:example:alice",
        "did:example:merchant",
        "coffee",
        "checkout",
    );

    storage
        .set_storage(&alice, "cart", json!({ "drinkId": "latte" }))
        .expect("set alice");
    assert_eq!(
        report.backend_profile,
        StoragePersistenceProfile::LocalFileUnencrypted
    );
    assert!(!report.production_ready);

    let (restored, restore_report) = PersistentScopedStorage::restore(backend).expect("restore");
    assert_eq!(restore_report.loaded_count, 1);
    assert_eq!(restore_report.restored_count, 1);
    assert_eq!(
        restored.get_storage(&alice, "cart").expect("get alice"),
        Some(json!({ "drinkId": "latte" }))
    );
    assert_eq!(restored.get_storage(&bob, "cart").expect("get bob"), None);
    assert_eq!(
        restored
            .get_storage(&alternate_namespace, "cart")
            .expect("get alternate namespace"),
        None
    );

    let _ = fs::remove_file(path);
}

#[test]
fn storage_persistence_remove_clear_and_delete_scope_are_persistent() {
    let path = temp_storage_path("cleanup");
    let backend = LocalFileScopedStorageBackend::new(&path);
    let (storage, _report) = PersistentScopedStorage::restore(backend.clone()).expect("restore");
    let alice = StorageScope::new("did:example:alice", "did:example:merchant", "coffee");
    let bob = StorageScope::new("did:example:bob", "did:example:merchant", "coffee");

    storage
        .set_storage(&alice, "cart", json!("latte"))
        .expect("set alice cart");
    storage
        .set_storage(&alice, "draft", json!("keep"))
        .expect("set alice draft");
    storage
        .set_storage(&bob, "cart", json!("tea"))
        .expect("set bob cart");

    assert_eq!(
        storage
            .remove_storage(&alice, "cart")
            .expect("remove alice cart"),
        Some(json!("latte"))
    );
    storage.delete_scope(&bob).expect("delete bob scope");
    let (restored, _report) = PersistentScopedStorage::restore(backend.clone()).expect("restore");
    assert_eq!(restored.get_storage(&alice, "cart").expect("cart"), None);
    assert_eq!(
        restored.get_storage(&alice, "draft").expect("draft"),
        Some(json!("keep"))
    );
    assert_eq!(restored.get_storage(&bob, "cart").expect("bob"), None);

    restored.clear_storage(&alice).expect("clear alice");
    let (cleared, _report) = PersistentScopedStorage::restore(backend).expect("restore");
    assert_eq!(cleared.get_storage(&alice, "draft").expect("draft"), None);

    let _ = fs::remove_file(path);
}

#[test]
fn storage_persistence_restore_rejects_entries_over_quota() {
    let scope = StorageScope::new("did:example:alice", "did:example:merchant", "coffee");
    let mut entries = Vec::new();
    let payload = json!("x".repeat(1024));
    for index in 0..DEFAULT_MAX_STORAGE_SCOPE_BYTES {
        let entry =
            PersistentStorageEntry::new(scope.clone(), format!("quota-{index}"), payload.clone())
                .expect("entry");
        entries.push(entry);
        if entries
            .iter()
            .map(|entry| entry.key.len() + serde_json::to_vec(&entry.value()).unwrap().len())
            .sum::<usize>()
            > DEFAULT_MAX_STORAGE_SCOPE_BYTES
        {
            break;
        }
    }
    let path = temp_storage_path("quota");
    let backend = LocalFileScopedStorageBackend::new(&path);
    backend.replace_entries(entries).expect("seed backend");

    let (storage, report) = PersistentScopedStorage::restore(backend.clone()).expect("restore");

    assert!(report.loaded_count > report.restored_count);
    assert!(report.rejected.iter().any(
        |rejection| rejection.reason == wx_compat::StorageRestoreRejectionReason::QuotaExceeded
    ));
    assert!(backend.load_entries().expect("entries").len() < report.loaded_count);
    assert!(storage
        .get_storage(&scope, "quota-0")
        .expect("get")
        .is_some());

    let _ = fs::remove_file(path);
}

#[test]
fn storage_persistence_restore_rejects_invalid_entries_and_cleans_snapshot() {
    let valid_scope = StorageScope::new("did:example:alice", "did:example:merchant", "coffee");
    let path = temp_storage_path("invalid");
    fs::write(
        &path,
        serde_json::to_vec_pretty(&json!([
            {
                "scope": {
                    "userDid": "did:example:alice",
                    "merchantDid": "did:example:merchant",
                    "skillId": "coffee",
                    "namespace": "default"
                },
                "key": "cart",
                "value": "latte"
            },
            {
                "scope": {
                    "userDid": "",
                    "merchantDid": "did:example:merchant",
                    "skillId": "coffee",
                    "namespace": "default"
                },
                "key": "leaky",
                "value": "must not restore"
            }
        ]))
        .expect("json"),
    )
    .expect("seed invalid snapshot");
    let backend = LocalFileScopedStorageBackend::new(&path);

    let (storage, report) = PersistentScopedStorage::restore(backend.clone()).expect("restore");

    assert_eq!(report.loaded_count, 2);
    assert_eq!(report.restored_count, 1);
    assert_eq!(report.rejected.len(), 1);
    assert_eq!(
        report.rejected[0].reason,
        wx_compat::StorageRestoreRejectionReason::InvalidEntry
    );
    assert_eq!(
        storage.get_storage(&valid_scope, "cart").expect("get cart"),
        Some(json!("latte"))
    );
    assert_eq!(backend.load_entries().expect("entries").len(), 1);

    let report_json = serde_json::to_string(&report).expect("report");
    assert!(!report_json.contains("leaky"));
    assert!(!report_json.contains("must not restore"));

    let _ = fs::remove_file(path);
}

#[test]
fn storage_persistence_report_and_debug_redact_keys_and_values() {
    let entry = PersistentStorageEntry::new(
        StorageScope::new("did:example:alice", "did:example:merchant", "coffee"),
        "cart",
        json!({ "drinkId": "latte", "note": "private preference" }),
    )
    .expect("entry");
    let debug = format!("{entry:?}");
    assert!(!debug.contains("cart"));
    assert!(!debug.contains("latte"));
    assert!(debug.contains("[REDACTED]"));

    let path = temp_storage_path("redaction");
    let backend = LocalFileScopedStorageBackend::new(&path);
    backend.replace_entries(vec![entry]).expect("seed backend");
    let (_storage, report) = PersistentScopedStorage::restore(backend).expect("restore");
    let report_json = serde_json::to_string(&report).expect("report json");
    assert!(!report_json.contains("cart"));
    assert!(!report_json.contains("latte"));
    assert!(!report_json.contains("private preference"));
    assert!(!report.redaction.raw_key_visible);
    assert!(!report.redaction.raw_value_visible);

    let _ = fs::remove_file(path);
}

#[test]
fn storage_persistence_profiles_mark_only_encrypted_backends_production_ready() {
    assert!(!StoragePersistenceProfile::InMemoryDev.production_ready());
    assert!(!StoragePersistenceProfile::LocalFileUnencrypted.production_ready());
    assert!(StoragePersistenceProfile::HostEncryptedStore.production_ready());
    assert!(StoragePersistenceProfile::EncryptedSqlite.production_ready());
}

fn temp_storage_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("dock-storage-{name}-{unique}.json"))
}
