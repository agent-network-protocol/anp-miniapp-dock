use crate::token::{bearer_token_expiry_ms, CapabilityToken};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const SESSION_EXPIRY_SKEW_MS: u64 = 5_000;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidAuthSessionKey {
    pub base_url: String,
    pub merchant_did: String,
    pub user_did: String,
    pub agent_did: Option<String>,
    pub skill_id: String,
    pub session_id: String,
}

impl DidAuthSessionKey {
    pub fn new(
        base_url: impl Into<String>,
        merchant_did: impl Into<String>,
        user_did: impl Into<String>,
        agent_did: Option<String>,
        skill_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            merchant_did: merchant_did.into(),
            user_did: user_did.into(),
            agent_did,
            skill_id: skill_id.into(),
            session_id: session_id.into(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DidAuthSession {
    token: CapabilityToken,
    scopes: Vec<String>,
}

impl DidAuthSession {
    pub fn new(
        token: impl Into<String>,
        expires_at_ms: Option<u64>,
        scopes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            token: CapabilityToken::new(token, expires_at_ms),
            scopes: scopes.into_iter().map(Into::into).collect(),
        }
    }

    pub fn bearer_token(&self) -> &str {
        &self.token.value
    }

    pub fn expires_at_ms(&self) -> Option<u64> {
        self.token.expires_at_ms
    }

    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }

    pub fn is_expired(&self) -> bool {
        self.token
            .is_expired_at(now_ms().saturating_add(SESSION_EXPIRY_SKEW_MS))
    }
}

impl fmt::Debug for DidAuthSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DidAuthSession")
            .field("token", &"[REDACTED]")
            .field("expires_at_ms", &self.expires_at_ms())
            .field("scopes", &self.scopes)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidAuthReceipt {
    pub code: String,
    pub token_received: bool,
    pub token_visible_to_skill: bool,
    pub user_did: String,
    pub agent_did: Option<String>,
    pub merchant_did: String,
    pub scopes: Vec<String>,
    pub expires_at_ms: Option<u64>,
}

impl DidAuthReceipt {
    pub fn from_session(key: &DidAuthSessionKey, session: &DidAuthSession) -> Self {
        Self {
            code: format!("dock-login-receipt-{}", key.session_id),
            token_received: true,
            token_visible_to_skill: false,
            user_did: key.user_did.clone(),
            agent_did: key.agent_did.clone(),
            merchant_did: key.merchant_did.clone(),
            scopes: session.scopes.clone(),
            expires_at_ms: session.expires_at_ms(),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DidAuthSessionError {
    #[error("DID auth session cache is unavailable")]
    Unavailable,

    #[error("DID auth session is missing")]
    Missing,

    #[error("DID auth session is expired")]
    Expired,

    #[error("DID auth login failed: {0}")]
    LoginFailed(String),
}

#[derive(Debug, Clone, Default)]
pub struct DidAuthSessionManager {
    sessions: Arc<Mutex<BTreeMap<DidAuthSessionKey, DidAuthSession>>>,
}

impl DidAuthSessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ensure_session(
        &self,
        key: DidAuthSessionKey,
        login: impl FnOnce(&DidAuthSessionKey) -> Result<DidAuthSession, DidAuthSessionError>,
    ) -> Result<DidAuthSession, DidAuthSessionError> {
        match self.active_session(&key) {
            Ok(Some(session)) => return Ok(session),
            Ok(None) | Err(DidAuthSessionError::Expired) => {}
            Err(error) => return Err(error),
        }
        let session = login(&key)?;
        self.put_session(key, session.clone())?;
        Ok(session)
    }

    pub fn check_session(
        &self,
        key: &DidAuthSessionKey,
    ) -> Result<DidAuthSession, DidAuthSessionError> {
        self.active_session(key)?
            .ok_or(DidAuthSessionError::Missing)
    }

    pub fn put_session(
        &self,
        key: DidAuthSessionKey,
        session: DidAuthSession,
    ) -> Result<(), DidAuthSessionError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| DidAuthSessionError::Unavailable)?;
        sessions.insert(key, session);
        Ok(())
    }

    pub fn clear_session(&self, key: &DidAuthSessionKey) -> Result<(), DidAuthSessionError> {
        self.revoke_session(key)
    }

    pub fn revoke_session(&self, key: &DidAuthSessionKey) -> Result<(), DidAuthSessionError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| DidAuthSessionError::Unavailable)?;
        sessions.remove(key);
        Ok(())
    }

    pub fn logout(&self, key: &DidAuthSessionKey) -> Result<(), DidAuthSessionError> {
        self.revoke_session(key)
    }

    pub fn evict_expired_sessions(&self) -> Result<usize, DidAuthSessionError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| DidAuthSessionError::Unavailable)?;
        let before = sessions.len();
        sessions.retain(|_, session| !session.is_expired());
        Ok(before.saturating_sub(sessions.len()))
    }

    fn active_session(
        &self,
        key: &DidAuthSessionKey,
    ) -> Result<Option<DidAuthSession>, DidAuthSessionError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| DidAuthSessionError::Unavailable)?;
        let Some(session) = sessions.get(key).cloned() else {
            return Ok(None);
        };
        if session.is_expired() {
            sessions.remove(key);
            return Err(DidAuthSessionError::Expired);
        }
        Ok(Some(session))
    }
}

pub fn decode_capability_token_scopes(token: &str) -> Option<Vec<String>> {
    let _ = bearer_token_expiry_ms(token)?;
    let payload = token.split('.').nth(1)?;
    let decoded = base64_url_decode(payload).ok()?;
    let value = serde_json::from_slice::<Value>(&decoded).ok()?;
    value
        .get("scopes")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
}

fn base64_url_decode(input: &str) -> Result<Vec<u8>, String> {
    const TABLE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut bits = 0_u32;
    let mut bit_count = 0_u8;
    let mut output = Vec::new();
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        let value = TABLE
            .bytes()
            .position(|candidate| candidate == byte)
            .ok_or_else(|| "invalid base64url character".to_owned())? as u32;
        bits = (bits << 6) | value;
        bit_count += 6;
        while bit_count >= 8 {
            bit_count -= 8;
            output.push(((bits >> bit_count) & 0xff) as u8);
        }
    }
    Ok(output)
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
    fn manager_keeps_sessions_isolated_by_full_key() {
        let manager = DidAuthSessionManager::new();
        let session_key = key("coffee", "session-1");
        let other_skill = key("tea", "session-1");
        let other_session = key("coffee", "session-2");

        manager
            .put_session(
                session_key.clone(),
                DidAuthSession::new("coffee-token", None, ["coffee:read"]),
            )
            .expect("session caches");
        manager
            .put_session(
                other_skill.clone(),
                DidAuthSession::new("tea-token", None, ["tea:read"]),
            )
            .expect("other skill caches");
        manager
            .put_session(
                other_session.clone(),
                DidAuthSession::new("other-session-token", None, ["coffee:read"]),
            )
            .expect("other session caches");

        assert_eq!(
            manager
                .check_session(&session_key)
                .expect("session exists")
                .bearer_token(),
            "coffee-token"
        );
        assert_eq!(
            manager
                .check_session(&other_skill)
                .expect("other skill session exists")
                .bearer_token(),
            "tea-token"
        );
        assert_eq!(
            manager
                .check_session(&other_session)
                .expect("other session exists")
                .bearer_token(),
            "other-session-token"
        );
    }

    #[test]
    fn check_session_removes_expired_session() {
        let manager = DidAuthSessionManager::new();
        let key = key("coffee", "session-1");
        manager
            .put_session(
                key.clone(),
                DidAuthSession::new("expired-token", Some(1), ["coffee:read"]),
            )
            .expect("session caches");

        assert_eq!(
            manager.check_session(&key),
            Err(DidAuthSessionError::Expired)
        );
        assert_eq!(
            manager.check_session(&key),
            Err(DidAuthSessionError::Missing)
        );
    }

    #[test]
    fn clear_session_revokes_cached_session() {
        let manager = DidAuthSessionManager::new();
        let key = key("coffee", "session-1");
        manager
            .put_session(
                key.clone(),
                DidAuthSession::new("active-token", None, ["coffee:read"]),
            )
            .expect("session caches");

        manager.clear_session(&key).expect("session clears");

        assert_eq!(
            manager.check_session(&key),
            Err(DidAuthSessionError::Missing)
        );
    }

    #[test]
    fn logout_alias_revokes_cached_session() {
        let manager = DidAuthSessionManager::new();
        let key = key("coffee", "session-1");
        manager
            .put_session(
                key.clone(),
                DidAuthSession::new("active-token", None, ["coffee:read"]),
            )
            .expect("session caches");

        manager.logout(&key).expect("logout clears");

        assert_eq!(
            manager.check_session(&key),
            Err(DidAuthSessionError::Missing)
        );
    }

    #[test]
    fn evict_expired_sessions_reports_removed_count() {
        let manager = DidAuthSessionManager::new();
        let active = key("coffee", "session-1");
        let expired = key("coffee", "session-2");
        manager
            .put_session(
                active.clone(),
                DidAuthSession::new("active-token", None, ["coffee:read"]),
            )
            .expect("active session caches");
        manager
            .put_session(
                expired.clone(),
                DidAuthSession::new("expired-token", Some(1), ["coffee:read"]),
            )
            .expect("expired session caches");

        assert_eq!(
            manager
                .evict_expired_sessions()
                .expect("expired sessions evict"),
            1
        );
        assert!(manager.check_session(&active).is_ok());
        assert_eq!(
            manager.check_session(&expired),
            Err(DidAuthSessionError::Missing)
        );
    }

    #[test]
    fn ensure_session_uses_cache_before_login_callback() {
        let manager = DidAuthSessionManager::new();
        let key = key("coffee", "session-1");
        manager
            .put_session(
                key.clone(),
                DidAuthSession::new("cached-token", None, ["coffee:read"]),
            )
            .expect("session caches");

        let session = manager
            .ensure_session(key, |_| {
                panic!("login callback should not run for cached session")
            })
            .expect("cached session returns");

        assert_eq!(session.bearer_token(), "cached-token");
    }

    #[test]
    fn ensure_session_refreshes_expired_session() {
        let manager = DidAuthSessionManager::new();
        let key = key("coffee", "session-1");
        manager
            .put_session(
                key.clone(),
                DidAuthSession::new("expired-token", Some(1), ["coffee:read"]),
            )
            .expect("session caches");

        let session = manager
            .ensure_session(key.clone(), |login_key| {
                assert_eq!(login_key, &key);
                Ok(DidAuthSession::new(
                    "refreshed-token",
                    None,
                    ["coffee:read"],
                ))
            })
            .expect("expired session refreshes");

        assert_eq!(session.bearer_token(), "refreshed-token");
        assert_eq!(
            manager
                .check_session(&key)
                .expect("refreshed session is cached")
                .bearer_token(),
            "refreshed-token"
        );
    }

    #[test]
    fn receipt_debug_does_not_expose_token() {
        let session = DidAuthSession::new("secret-token", None, ["coffee:read"]);
        let rendered = format!("{session:?}");

        assert!(!rendered.contains("secret-token"));
        assert!(rendered.contains("[REDACTED]"));
    }

    fn key(skill_id: &str, session_id: &str) -> DidAuthSessionKey {
        DidAuthSessionKey::new(
            "http://127.0.0.1:3000/",
            "did:wba:merchant.example",
            "did:wba:user.example",
            Some("did:wba:agent.example".to_owned()),
            skill_id,
            session_id,
        )
    }
}
