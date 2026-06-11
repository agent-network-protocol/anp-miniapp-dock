use crate::audit::now_ms;
use anp_adapter::{
    verify_challenge_proof_at, CapabilityTokenClaims, CapabilityTokenError, CapabilityTokenIssuer,
    CapabilityTokenIssuerConfig, CapabilityTokenRequest, CapabilityTokenVerifier,
    CapabilityTokenVerifierConfig, ChallengeProofError, ChallengeProofPayload,
    DidChallenge as AdapterDidChallenge, DockDidChallengeProof, ExpectedCapability,
    IdentitySession,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const TOKEN_TTL_MS: u64 = 15 * 60 * 1000;
const CHALLENGE_TTL_MS: u64 = 5 * 60 * 1000;
const LOGIN_AUDIENCE_PATH: &str = "/agents/coffee/auth/login";
const COFFEE_DEMO_SCOPES: [&str; 4] = [
    "coffee:drinks:read",
    "coffee:order:confirm",
    "coffee:order:pay",
    "coffee:order:read",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerAuthConfig {
    pub merchant_did: String,
    pub challenge_ttl_ms: u64,
    pub token_ttl_ms: u64,
    pub token_issuer: Option<TokenIssuerConfig>,
    pub trusted_did_documents: BTreeMap<String, PathBuf>,
}

impl ServerAuthConfig {
    pub fn new(merchant_did: impl Into<String>) -> Self {
        Self {
            merchant_did: merchant_did.into(),
            challenge_ttl_ms: CHALLENGE_TTL_MS,
            token_ttl_ms: TOKEN_TTL_MS,
            token_issuer: None,
            trusted_did_documents: BTreeMap::new(),
        }
    }

    pub fn with_token_issuer(mut self, token_issuer: TokenIssuerConfig) -> Self {
        self.token_issuer = Some(token_issuer);
        self
    }

    pub fn with_trusted_did_document(
        mut self,
        did: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Self {
        self.trusted_did_documents.insert(did.into(), path.into());
        self
    }

    pub fn for_tests() -> Self {
        Self::new("did:wba:coffee-merchant.example").with_token_issuer(
            TokenIssuerConfig::test_only("test-only-token-issuer-secret"),
        )
    }

    fn token_issuer(&self) -> Result<CapabilityTokenIssuer, AuthError> {
        let token_issuer = self
            .token_issuer
            .as_ref()
            .ok_or(AuthError::TokenIssuerUnavailable)?;
        CapabilityTokenIssuer::new(
            CapabilityTokenIssuerConfig::new(
                self.merchant_did.clone(),
                self.merchant_did.clone(),
                token_issuer.secret.clone(),
            )
            .with_ttl_ms(self.token_ttl_ms),
        )
        .map_err(map_token_error)
    }

    fn token_verifier(&self) -> Result<CapabilityTokenVerifier, AuthError> {
        let token_issuer = self
            .token_issuer
            .as_ref()
            .ok_or(AuthError::TokenIssuerUnavailable)?;
        CapabilityTokenVerifier::new(CapabilityTokenVerifierConfig::new(
            self.merchant_did.clone(),
            self.merchant_did.clone(),
            token_issuer.secret.clone(),
        ))
        .map_err(map_token_error)
    }

    pub fn issue_localhost_login_token(
        &self,
        session_id: impl Into<String>,
        skill_id: impl Into<String>,
        user_did: impl Into<String>,
        agent_did: Option<String>,
    ) -> Result<ChallengeLoginResponse, AuthError> {
        let outcome = self
            .token_issuer()?
            .issue(CapabilityTokenRequest::new(
                self.merchant_did.clone(),
                user_did.into(),
                agent_did,
                skill_id.into(),
                session_id.into(),
                COFFEE_DEMO_SCOPES,
            ))
            .map_err(map_token_error)?;
        Ok(ChallengeLoginResponse {
            capability_token: outcome.token.value,
            expires_at_ms: outcome.token.expires_at_ms,
        })
    }

    fn resolve_trusted_did_document(&self, did: &str) -> Result<Value, AuthError> {
        let path = self
            .trusted_did_documents
            .get(did)
            .ok_or(AuthError::UnknownDid)?;
        let document = std::fs::read_to_string(path).map_err(|_| AuthError::UnknownDid)?;
        let value = serde_json::from_str::<Value>(&document).map_err(|_| AuthError::UnknownDid)?;
        if value.get("id").and_then(Value::as_str) != Some(did) {
            return Err(AuthError::UnknownDid);
        }
        Ok(value)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TokenIssuerConfig {
    pub algorithm: String,
    secret: String,
}

impl TokenIssuerConfig {
    pub fn new_hs256(secret: impl Into<String>) -> Result<Self, AuthConfigError> {
        let secret = secret.into();
        if secret.trim().is_empty() {
            return Err(AuthConfigError::MissingTokenIssuer);
        }
        Ok(Self {
            algorithm: "HS256".to_owned(),
            secret,
        })
    }

    pub fn redacted_summary(&self) -> BTreeMap<&'static str, &'static str> {
        BTreeMap::from([("algorithm", "HS256"), ("secret", "[REDACTED]")])
    }

    fn test_only(secret: impl Into<String>) -> Self {
        Self {
            algorithm: "HS256".to_owned(),
            secret: secret.into(),
        }
    }
}

impl fmt::Debug for TokenIssuerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TokenIssuerConfig")
            .field("algorithm", &self.algorithm)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthConfigError {
    MissingTokenIssuer,
    InvalidTrustedDidDocument,
}

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
    pub issued_at_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub audience: String,
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

#[derive(Debug, Clone)]
struct ChallengeRecord {
    request: ChallengeRequest,
    challenge: DidChallenge,
}

#[derive(Debug, Clone, Default)]
pub struct AuthStore {
    challenges: Arc<Mutex<BTreeMap<String, ChallengeRecord>>>,
}

impl AuthStore {
    pub fn challenge(
        &self,
        merchant_did: &str,
        login_audience: &str,
        challenge_ttl_ms: u64,
        request: ChallengeRequest,
    ) -> DidChallenge {
        let now = now_ms();
        let challenge = DidChallenge {
            challenge_id: random_id("challenge"),
            merchant_did: merchant_did.to_owned(),
            nonce: random_id("nonce"),
            issued_at_ms: now,
            expires_at_ms: Some(now.saturating_add(challenge_ttl_ms)),
            audience: login_audience.to_owned(),
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
        auth_config: &ServerAuthConfig,
        request: ChallengeLoginRequest,
    ) -> Result<ChallengeLoginResponse, AuthError> {
        if request.merchant_did != auth_config.merchant_did {
            return Err(AuthError::ScopeMismatch);
        }

        let record = self.challenge_record(&request.challenge_id)?;
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
            || record.request.agent_did != request.agent_did
            || record.challenge.merchant_did != request.merchant_did
        {
            return Err(AuthError::ScopeMismatch);
        }

        let proof = serde_json::from_value::<DockDidChallengeProof>(request.signed_challenge)
            .map_err(|_| AuthError::InvalidSignature)?;
        let expected_payload = ChallengeProofPayload::from_challenge(
            &AdapterDidChallenge {
                challenge_id: record.challenge.challenge_id.clone(),
                merchant_did: record.challenge.merchant_did.clone(),
                nonce: record.challenge.nonce.clone(),
                expires_at_ms: record.challenge.expires_at_ms,
            },
            &IdentitySession::new(
                request.user_did.clone(),
                request.agent_did.clone(),
                request.merchant_did.clone(),
                request.skill_id.clone(),
                request.session_id.clone(),
            ),
            record.challenge.audience.clone(),
            record.challenge.issued_at_ms,
        );
        let did_document = auth_config.resolve_trusted_did_document(&request.user_did)?;
        verify_challenge_proof_at(&proof, &expected_payload, &did_document, now_ms())
            .map_err(map_challenge_error)?;

        self.remove_challenge(&request.challenge_id)?;

        let outcome = auth_config
            .token_issuer()?
            .issue(CapabilityTokenRequest::new(
                auth_config.merchant_did.clone(),
                request.user_did,
                request.agent_did,
                request.skill_id,
                request.session_id,
                COFFEE_DEMO_SCOPES,
            ))
            .map_err(map_token_error)?;
        Ok(ChallengeLoginResponse {
            capability_token: outcome.token.value,
            expires_at_ms: outcome.token.expires_at_ms,
        })
    }

    pub fn verify_bearer(
        &self,
        auth_config: &ServerAuthConfig,
        header: Option<&str>,
        expected: ExpectedCapability,
    ) -> Result<CapabilityTokenClaims, AuthError> {
        let header = header.ok_or(AuthError::MissingToken)?;
        let token = header
            .strip_prefix("Bearer ")
            .ok_or(AuthError::MissingToken)?;
        auth_config
            .token_verifier()?
            .verify(token, &expected)
            .map_err(map_token_error)
    }

    fn challenge_record(&self, challenge_id: &str) -> Result<ChallengeRecord, AuthError> {
        let challenges = self.challenges.lock().map_err(|_| AuthError::Unavailable)?;
        challenges
            .get(challenge_id)
            .cloned()
            .ok_or(AuthError::UnknownChallenge)
    }

    fn remove_challenge(&self, challenge_id: &str) -> Result<(), AuthError> {
        let mut challenges = self.challenges.lock().map_err(|_| AuthError::Unavailable)?;
        if challenges.remove(challenge_id).is_none() {
            return Err(AuthError::UnknownChallenge);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    MissingToken,
    InvalidToken,
    ExpiredToken,
    InsufficientScope,
    UnknownChallenge,
    ExpiredChallenge,
    InvalidSignature,
    UnknownDid,
    ScopeMismatch,
    TokenIssuerUnavailable,
    Unavailable,
}

pub fn login_audience(base_url: &str) -> String {
    format!("{}{}", base_url.trim_end_matches('/'), LOGIN_AUDIENCE_PATH)
}

fn random_id(prefix: &str) -> String {
    let mut bytes = [0_u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    let suffix = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{prefix}-{suffix}")
}

fn map_challenge_error(error: ChallengeProofError) -> AuthError {
    match error {
        ChallengeProofError::Expired => AuthError::ExpiredChallenge,
        ChallengeProofError::PayloadMismatch
        | ChallengeProofError::MethodMismatch
        | ChallengeProofError::AudienceMismatch
        | ChallengeProofError::SignerDidMismatch => AuthError::ScopeMismatch,
        ChallengeProofError::DidDocumentResolution
        | ChallengeProofError::DidDocumentMismatch
        | ChallengeProofError::InvalidDidDocument => AuthError::UnknownDid,
        ChallengeProofError::MissingSignatureHeaders
        | ChallengeProofError::InvalidSignatureMetadata
        | ChallengeProofError::SignatureVerificationFailed
        | ChallengeProofError::InvalidSignerDid => AuthError::InvalidSignature,
        _ => AuthError::InvalidSignature,
    }
}

fn map_token_error(error: CapabilityTokenError) -> AuthError {
    match error {
        CapabilityTokenError::Expired => AuthError::ExpiredToken,
        CapabilityTokenError::MissingScope => AuthError::InsufficientScope,
        CapabilityTokenError::ScopeMismatch => AuthError::ScopeMismatch,
        CapabilityTokenError::MissingSecret => AuthError::TokenIssuerUnavailable,
        _ => AuthError::InvalidToken,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anp::authentication::{create_did_wba_document, AuthMode, DidDocumentOptions};
    use anp_adapter::{sign_challenge_proof, DidCredentialConfig, FileDidCredentialProvider};
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn login_issues_scoped_jwt_once_for_valid_challenge() {
        let fixture = DidFixture::new();
        let config = ServerAuthConfig::for_tests()
            .with_trusted_did_document(fixture.did(), fixture.did_path.clone());
        let store = AuthStore::default();
        let challenge = store.challenge(
            &config.merchant_did,
            "https://merchant.example/agents/coffee/auth/login",
            config.challenge_ttl_ms,
            ChallengeRequest {
                session_id: "session-1".to_owned(),
                skill_id: "coffee".to_owned(),
                user_did: fixture.did(),
                agent_did: Some("did:wba:agent.example".to_owned()),
            },
        );
        let session = IdentitySession::new(
            fixture.did(),
            Some("did:wba:agent.example".to_owned()),
            config.merchant_did.clone(),
            "coffee",
            "session-1",
        );
        let payload = ChallengeProofPayload::from_challenge(
            &AdapterDidChallenge {
                challenge_id: challenge.challenge_id.clone(),
                merchant_did: challenge.merchant_did.clone(),
                nonce: challenge.nonce.clone(),
                expires_at_ms: challenge.expires_at_ms,
            },
            &session,
            challenge.audience.clone(),
            challenge.issued_at_ms,
        );
        let provider =
            FileDidCredentialProvider::from_config(fixture.credential()).expect("credential");
        let proof = sign_challenge_proof(&payload, &provider, &session, AuthMode::HttpSignatures)
            .expect("proof signs");

        let response = store
            .login(
                &config,
                ChallengeLoginRequest {
                    session_id: "session-1".to_owned(),
                    skill_id: "coffee".to_owned(),
                    user_did: fixture.did(),
                    agent_did: Some("did:wba:agent.example".to_owned()),
                    merchant_did: config.merchant_did.clone(),
                    challenge_id: challenge.challenge_id.clone(),
                    signed_challenge: serde_json::to_value(proof).expect("proof serializes"),
                },
            )
            .expect("login succeeds");

        assert!(!response.capability_token.starts_with("demo-cap-"));
        assert_eq!(
            store
                .login(
                    &config,
                    ChallengeLoginRequest {
                        session_id: "session-1".to_owned(),
                        skill_id: "coffee".to_owned(),
                        user_did: fixture.did(),
                        agent_did: Some("did:wba:agent.example".to_owned()),
                        merchant_did: config.merchant_did.clone(),
                        challenge_id: challenge.challenge_id,
                        signed_challenge: Value::Null,
                    },
                )
                .expect_err("challenge is one time"),
            AuthError::UnknownChallenge
        );
    }

    #[test]
    fn server_auth_config_has_no_silent_token_issuer() {
        let config = ServerAuthConfig::new("did:wba:merchant.example");

        assert_eq!(config.merchant_did, "did:wba:merchant.example");
        assert!(config.token_issuer.is_none());
        assert!(config.trusted_did_documents.is_empty());
    }

    #[test]
    fn token_issuer_summary_redacts_secret() {
        let issuer = TokenIssuerConfig::new_hs256("real-secret").expect("issuer config");

        assert_eq!(issuer.redacted_summary().get("secret"), Some(&"[REDACTED]"));
        assert!(!format!("{:?}", issuer.redacted_summary()).contains("real-secret"));
    }

    struct DidFixture {
        _dir: TempDir,
        did_document: Value,
        did_path: PathBuf,
        key_path: PathBuf,
    }

    impl DidFixture {
        fn new() -> Self {
            let bundle = create_did_wba_document("user.example", DidDocumentOptions::default())
                .expect("DID fixture creates");
            let dir = TempDir::new("anp-miniapp-dock-demo-auth").expect("temp dir creates");
            let did_path = dir.path().join("did.json");
            let key_path = dir.path().join("key.pem");
            fs::write(&did_path, serde_json::to_vec(&bundle.did_document).unwrap()).unwrap();
            fs::write(&key_path, &bundle.keys["key-1"].private_key_pem).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();
            }
            Self {
                _dir: dir,
                did_document: bundle.did_document,
                did_path,
                key_path,
            }
        }

        fn did(&self) -> String {
            self.did_document["id"]
                .as_str()
                .expect("fixture has DID")
                .to_owned()
        }

        fn credential(&self) -> DidCredentialConfig {
            DidCredentialConfig::new(&self.did_path, &self.key_path)
        }
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> std::io::Result<Self> {
            let path = std::env::temp_dir().join(format!(
                "{}-{}-{}",
                prefix,
                std::process::id(),
                unique_suffix()
            ));
            fs::create_dir(&path)?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn unique_suffix() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        format!("{nanos}-{counter}")
    }
}
