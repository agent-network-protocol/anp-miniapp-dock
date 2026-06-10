use crate::audit::now_ms;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

const TOKEN_TTL_MS: u64 = 15 * 60 * 1000;
const CHALLENGE_TTL_MS: u64 = 5 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeRequest {
    pub session_id: String,
    pub skill_id: String,
    pub user_did: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_did: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChallenge {
    pub challenge_id: String,
    pub merchant_did: String,
    pub nonce: String,
    pub expires_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeLoginRequest {
    pub session_id: String,
    pub skill_id: String,
    pub user_did: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_did: Option<String>,
    pub merchant_did: String,
    pub challenge_id: String,
    pub signed_challenge: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeLoginResponse {
    pub capability_token: String,
    pub expires_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenScope {
    pub merchant_did: String,
    pub user_did: String,
    pub skill_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRecord {
    pub token: String,
    pub scope: TokenScope,
    pub expires_at_ms: u64,
}

impl TokenRecord {
    pub fn is_expired(&self, now_ms: u64) -> bool {
        self.expires_at_ms <= now_ms
    }
}

#[derive(Debug, Clone)]
struct ChallengeRecord {
    request: ChallengeRequest,
    challenge: DidChallenge,
}

#[derive(Debug, Clone, Default)]
pub struct AuthStore {
    challenges: Arc<Mutex<BTreeMap<String, ChallengeRecord>>>,
    tokens: Arc<Mutex<BTreeMap<String, TokenRecord>>>,
}

impl AuthStore {
    pub fn challenge(&self, merchant_did: &str, request: ChallengeRequest) -> DidChallenge {
        let now = now_ms();
        let challenge = DidChallenge {
            challenge_id: format!("challenge-{now}-{}", request.session_id),
            merchant_did: merchant_did.to_owned(),
            nonce: format!("nonce-{now}-{}", request.skill_id),
            expires_at_ms: Some(now.saturating_add(CHALLENGE_TTL_MS)),
        };
        if let Ok(mut challenges) = self.challenges.lock() {
            challenges.insert(
                challenge.challenge_id.clone(),
                ChallengeRecord {
                    request,
                    challenge: challenge.clone(),
                },
            );
        }
        challenge
    }

    pub fn login(
        &self,
        merchant_did: &str,
        request: ChallengeLoginRequest,
    ) -> Result<ChallengeLoginResponse, AuthError> {
        if request.merchant_did != merchant_did {
            return Err(AuthError::ScopeMismatch);
        }
        if !has_demo_signature(&request.signed_challenge) {
            return Err(AuthError::InvalidSignature);
        }

        let record = self
            .challenges
            .lock()
            .map_err(|_| AuthError::Unavailable)?
            .remove(&request.challenge_id)
            .ok_or(AuthError::UnknownChallenge)?;
        if record
            .challenge
            .expires_at_ms
            .map(|expires_at_ms| expires_at_ms <= now_ms())
            .unwrap_or(false)
        {
            return Err(AuthError::ExpiredChallenge);
        }
        if record.request.session_id != request.session_id
            || record.request.skill_id != request.skill_id
            || record.request.user_did != request.user_did
        {
            return Err(AuthError::ScopeMismatch);
        }

        let expires_at_ms = now_ms().saturating_add(TOKEN_TTL_MS);
        let scope = TokenScope {
            merchant_did: merchant_did.to_owned(),
            user_did: request.user_did,
            skill_id: request.skill_id,
            session_id: request.session_id,
        };
        let token = format!("demo-cap-{}-{expires_at_ms}", request.challenge_id);
        self.insert_token(TokenRecord {
            token: token.clone(),
            scope,
            expires_at_ms,
        });
        Ok(ChallengeLoginResponse {
            capability_token: token,
            expires_at_ms: Some(expires_at_ms),
        })
    }

    pub fn verify_bearer(&self, header: Option<&str>) -> Result<TokenRecord, AuthError> {
        let header = header.ok_or(AuthError::MissingToken)?;
        let token = header
            .strip_prefix("Bearer ")
            .ok_or(AuthError::MissingToken)?;
        let mut tokens = self.tokens.lock().map_err(|_| AuthError::Unavailable)?;
        let record = tokens.get(token).cloned().ok_or(AuthError::InvalidToken)?;
        if record.is_expired(now_ms()) {
            tokens.remove(token);
            return Err(AuthError::ExpiredToken);
        }
        Ok(record)
    }

    pub fn insert_token(&self, record: TokenRecord) {
        if let Ok(mut tokens) = self.tokens.lock() {
            tokens.insert(record.token.clone(), record);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    MissingToken,
    InvalidToken,
    ExpiredToken,
    UnknownChallenge,
    ExpiredChallenge,
    InvalidSignature,
    ScopeMismatch,
    Unavailable,
}

fn has_demo_signature(value: &Value) -> bool {
    value
        .get("proof")
        .or_else(|| value.get("signature"))
        .and_then(Value::as_str)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn login_issues_scoped_token_once_for_valid_challenge() {
        let store = AuthStore::default();
        let challenge = store.challenge(
            "did:wba:merchant",
            ChallengeRequest {
                session_id: "session-1".to_owned(),
                skill_id: "coffee".to_owned(),
                user_did: "did:wba:user".to_owned(),
                agent_did: None,
            },
        );

        let response = store
            .login(
                "did:wba:merchant",
                ChallengeLoginRequest {
                    session_id: "session-1".to_owned(),
                    skill_id: "coffee".to_owned(),
                    user_did: "did:wba:user".to_owned(),
                    agent_did: None,
                    merchant_did: "did:wba:merchant".to_owned(),
                    challenge_id: challenge.challenge_id,
                    signed_challenge: json!({"proof": "demo"}),
                },
            )
            .expect("login succeeds");

        assert!(response.capability_token.starts_with("demo-cap-"));
    }
}
