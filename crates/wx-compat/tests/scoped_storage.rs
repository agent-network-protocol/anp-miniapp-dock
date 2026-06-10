use serde_json::json;
use wx_compat::{InMemoryScopedStorage, ModelContext, ScopedStorage, StorageError, StorageScope};

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
