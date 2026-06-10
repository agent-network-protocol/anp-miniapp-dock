use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityTokenScope {
    pub merchant_did: String,
    pub user_did: String,
    pub skill_id: String,
}

impl CapabilityTokenScope {
    pub fn new(
        merchant_did: impl Into<String>,
        user_did: impl Into<String>,
        skill_id: impl Into<String>,
    ) -> Self {
        Self {
            merchant_did: merchant_did.into(),
            user_did: user_did.into(),
            skill_id: skill_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityToken {
    pub value: String,
    pub expires_at_ms: Option<u64>,
}

impl CapabilityToken {
    pub fn new(value: impl Into<String>, expires_at_ms: Option<u64>) -> Self {
        Self {
            value: value.into(),
            expires_at_ms,
        }
    }

    pub fn is_expired_at(&self, now_ms: u64) -> bool {
        self.expires_at_ms
            .map(|expires_at_ms| expires_at_ms <= now_ms)
            .unwrap_or(false)
    }
}

pub trait CapabilityTokenCache: Clone {
    fn get(&self, scope: &CapabilityTokenScope) -> Option<CapabilityToken>;
    fn put(&self, scope: CapabilityTokenScope, token: CapabilityToken);
    fn clear(&self, scope: &CapabilityTokenScope);
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryTokenCache {
    tokens: Arc<Mutex<BTreeMap<CapabilityTokenScope, CapabilityToken>>>,
}

impl InMemoryTokenCache {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CapabilityTokenCache for InMemoryTokenCache {
    fn get(&self, scope: &CapabilityTokenScope) -> Option<CapabilityToken> {
        let mut tokens = self.tokens.lock().expect("token cache mutex poisoned");
        let token = tokens.get(scope).cloned()?;
        if token.is_expired_at(now_ms()) {
            tokens.remove(scope);
            return None;
        }
        Some(token)
    }

    fn put(&self, scope: CapabilityTokenScope, token: CapabilityToken) {
        let mut tokens = self.tokens.lock().expect("token cache mutex poisoned");
        tokens.insert(scope, token);
    }

    fn clear(&self, scope: &CapabilityTokenScope) {
        let mut tokens = self.tokens.lock().expect("token cache mutex poisoned");
        tokens.remove(scope);
    }
}

fn now_ms() -> u64 {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_scope_is_merchant_user_skill_specific() {
        let cache = InMemoryTokenCache::new();
        let coffee = CapabilityTokenScope::new("did:wba:merchant", "did:wba:user", "coffee");
        let tea = CapabilityTokenScope::new("did:wba:merchant", "did:wba:user", "tea");

        cache.put(coffee.clone(), CapabilityToken::new("coffee-token", None));

        assert_eq!(
            cache.get(&coffee).map(|token| token.value),
            Some("coffee-token".to_owned())
        );
        assert!(cache.get(&tea).is_none());
    }

    #[test]
    fn expired_token_is_not_returned() {
        let cache = InMemoryTokenCache::new();
        let scope = CapabilityTokenScope::new("did:wba:merchant", "did:wba:user", "coffee");

        cache.put(scope.clone(), CapabilityToken::new("old", Some(1)));

        assert!(cache.get(&scope).is_none());
    }
}
