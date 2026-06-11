use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub const CAPABILITY_TOKEN_VERSION: &str = "dock.capability.v1";
pub const DEFAULT_CAPABILITY_TOKEN_TTL_MS: u64 = 300_000;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityTokenScope {
    pub merchant_did: String,
    pub user_did: String,
    pub skill_id: String,
    pub agent_did: Option<String>,
    pub session_id: Option<String>,
}

impl CapabilityTokenScope {
    pub fn new(
        merchant_did: impl Into<String>,
        user_did: impl Into<String>,
        skill_id: impl Into<String>,
    ) -> Self {
        Self::for_subject(merchant_did, user_did, None, skill_id, None)
    }

    pub fn for_subject(
        merchant_did: impl Into<String>,
        user_did: impl Into<String>,
        agent_did: Option<String>,
        skill_id: impl Into<String>,
        session_id: Option<String>,
    ) -> Self {
        Self {
            merchant_did: merchant_did.into(),
            user_did: user_did.into(),
            agent_did,
            skill_id: skill_id.into(),
            session_id,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
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

impl fmt::Debug for CapabilityToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityToken")
            .field("value", &"[REDACTED]")
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityTokenClaims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub merchant_did: String,
    pub user_did: String,
    pub agent_did: Option<String>,
    pub skill_id: String,
    pub session_id: String,
    pub scopes: Vec<String>,
    pub iat: u64,
    pub nbf: u64,
    pub exp: u64,
    pub jti: String,
    pub version: String,
}

impl CapabilityTokenClaims {
    pub fn expires_at_ms(&self) -> u64 {
        self.exp.saturating_mul(1_000)
    }

    pub fn scope(&self) -> CapabilityTokenScope {
        CapabilityTokenScope::for_subject(
            self.merchant_did.clone(),
            self.user_did.clone(),
            self.agent_did.clone(),
            self.skill_id.clone(),
            Some(self.session_id.clone()),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityTokenRequest {
    pub merchant_did: String,
    pub user_did: String,
    pub agent_did: Option<String>,
    pub skill_id: String,
    pub session_id: String,
    pub scopes: Vec<String>,
}

impl CapabilityTokenRequest {
    pub fn new(
        merchant_did: impl Into<String>,
        user_did: impl Into<String>,
        agent_did: Option<String>,
        skill_id: impl Into<String>,
        session_id: impl Into<String>,
        scopes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            merchant_did: merchant_did.into(),
            user_did: user_did.into(),
            agent_did,
            skill_id: skill_id.into(),
            session_id: session_id.into(),
            scopes: scopes.into_iter().map(Into::into).collect(),
        }
    }

    fn validate(&self) -> Result<(), CapabilityTokenError> {
        if self.merchant_did.trim().is_empty()
            || self.user_did.trim().is_empty()
            || self.skill_id.trim().is_empty()
            || self.session_id.trim().is_empty()
            || self
                .agent_did
                .as_deref()
                .is_some_and(|did| did.trim().is_empty())
            || self.scopes.iter().any(|scope| scope.trim().is_empty())
        {
            return Err(CapabilityTokenError::InvalidClaims);
        }
        if self.scopes.is_empty() {
            return Err(CapabilityTokenError::MissingScope);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedCapability {
    pub issuer: String,
    pub audience: String,
    pub merchant_did: String,
    pub user_did: Option<String>,
    pub agent_did: Option<String>,
    pub skill_id: String,
    pub session_id: Option<String>,
    pub required_scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedCapabilitySubject {
    pub user_did: String,
    pub agent_did: Option<String>,
    pub session_id: String,
}

impl ExpectedCapabilitySubject {
    pub fn new(
        user_did: impl Into<String>,
        agent_did: Option<String>,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            user_did: user_did.into(),
            agent_did,
            session_id: session_id.into(),
        }
    }
}

impl ExpectedCapability {
    pub fn new(
        issuer: impl Into<String>,
        audience: impl Into<String>,
        merchant_did: impl Into<String>,
        subject: ExpectedCapabilitySubject,
        skill_id: impl Into<String>,
        required_scope: impl Into<String>,
    ) -> Self {
        Self {
            issuer: issuer.into(),
            audience: audience.into(),
            merchant_did: merchant_did.into(),
            user_did: Some(subject.user_did),
            agent_did: subject.agent_did,
            skill_id: skill_id.into(),
            session_id: Some(subject.session_id),
            required_scope: required_scope.into(),
        }
    }

    pub fn for_route(
        issuer: impl Into<String>,
        audience: impl Into<String>,
        merchant_did: impl Into<String>,
        skill_id: impl Into<String>,
        required_scope: impl Into<String>,
    ) -> Self {
        Self {
            issuer: issuer.into(),
            audience: audience.into(),
            merchant_did: merchant_did.into(),
            user_did: None,
            agent_did: None,
            skill_id: skill_id.into(),
            session_id: None,
            required_scope: required_scope.into(),
        }
    }

    fn validate(&self) -> Result<(), CapabilityTokenError> {
        if self.issuer.trim().is_empty()
            || self.audience.trim().is_empty()
            || self.merchant_did.trim().is_empty()
            || self.skill_id.trim().is_empty()
            || self.required_scope.trim().is_empty()
            || self
                .user_did
                .as_deref()
                .is_some_and(|did| did.trim().is_empty())
            || self
                .agent_did
                .as_deref()
                .is_some_and(|did| did.trim().is_empty())
            || self
                .session_id
                .as_deref()
                .is_some_and(|session_id| session_id.trim().is_empty())
        {
            return Err(CapabilityTokenError::InvalidClaims);
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CapabilityTokenIssuerConfig {
    pub issuer: String,
    pub audience: String,
    secret: String,
    pub ttl_ms: u64,
}

impl CapabilityTokenIssuerConfig {
    pub fn new(
        issuer: impl Into<String>,
        audience: impl Into<String>,
        secret: impl Into<String>,
    ) -> Self {
        Self {
            issuer: issuer.into(),
            audience: audience.into(),
            secret: secret.into(),
            ttl_ms: DEFAULT_CAPABILITY_TOKEN_TTL_MS,
        }
    }

    pub fn with_ttl_ms(mut self, ttl_ms: u64) -> Self {
        self.ttl_ms = ttl_ms;
        self
    }

    fn validate(&self) -> Result<(), CapabilityTokenError> {
        if self.issuer.trim().is_empty() || self.audience.trim().is_empty() {
            return Err(CapabilityTokenError::InvalidClaims);
        }
        validate_secret(&self.secret)?;
        if self.ttl_ms == 0 {
            return Err(CapabilityTokenError::InvalidTimestamp);
        }
        Ok(())
    }
}

impl fmt::Debug for CapabilityTokenIssuerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityTokenIssuerConfig")
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("secret", &"[REDACTED]")
            .field("ttl_ms", &self.ttl_ms)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CapabilityTokenVerifierConfig {
    pub issuer: String,
    pub audience: String,
    secret: String,
}

impl CapabilityTokenVerifierConfig {
    pub fn new(
        issuer: impl Into<String>,
        audience: impl Into<String>,
        secret: impl Into<String>,
    ) -> Self {
        Self {
            issuer: issuer.into(),
            audience: audience.into(),
            secret: secret.into(),
        }
    }

    fn validate(&self) -> Result<(), CapabilityTokenError> {
        if self.issuer.trim().is_empty() || self.audience.trim().is_empty() {
            return Err(CapabilityTokenError::InvalidClaims);
        }
        validate_secret(&self.secret)
    }
}

impl fmt::Debug for CapabilityTokenVerifierConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityTokenVerifierConfig")
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityTokenIssueOutcome {
    pub token: CapabilityToken,
    pub claims: CapabilityTokenClaims,
}

#[derive(Debug, Clone)]
pub struct CapabilityTokenIssuer {
    config: CapabilityTokenIssuerConfig,
}

impl CapabilityTokenIssuer {
    pub fn new(config: CapabilityTokenIssuerConfig) -> Result<Self, CapabilityTokenError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn issue(
        &self,
        request: CapabilityTokenRequest,
    ) -> Result<CapabilityTokenIssueOutcome, CapabilityTokenError> {
        self.issue_at(request, now_ms())
    }

    pub fn issue_at(
        &self,
        request: CapabilityTokenRequest,
        now_ms: u64,
    ) -> Result<CapabilityTokenIssueOutcome, CapabilityTokenError> {
        request.validate()?;
        let iat = unix_seconds_floor(now_ms)?;
        let exp = unix_seconds_ceil(now_ms.saturating_add(self.config.ttl_ms))?;
        if iat >= exp {
            return Err(CapabilityTokenError::InvalidTimestamp);
        }
        let claims = CapabilityTokenClaims {
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            sub: request.user_did.clone(),
            merchant_did: request.merchant_did,
            user_did: request.user_did,
            agent_did: request.agent_did,
            skill_id: request.skill_id,
            session_id: request.session_id,
            scopes: request.scopes,
            iat,
            nbf: iat,
            exp,
            jti: generate_jti(),
            version: CAPABILITY_TOKEN_VERSION.to_owned(),
        };
        validate_claims_basic(&claims)?;

        let mut header = Header::new(Algorithm::HS256);
        header.typ = Some(CAPABILITY_TOKEN_VERSION.to_owned());
        let encoded = encode(
            &header,
            &claims,
            &EncodingKey::from_secret(self.config.secret.as_bytes()),
        )
        .map_err(|_| CapabilityTokenError::SigningFailed)?;

        Ok(CapabilityTokenIssueOutcome {
            token: CapabilityToken::new(encoded, Some(claims.expires_at_ms())),
            claims,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityTokenVerifier {
    config: CapabilityTokenVerifierConfig,
}

impl CapabilityTokenVerifier {
    pub fn new(config: CapabilityTokenVerifierConfig) -> Result<Self, CapabilityTokenError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn verify(
        &self,
        token: &str,
        expected: &ExpectedCapability,
    ) -> Result<CapabilityTokenClaims, CapabilityTokenError> {
        self.verify_at(token, expected, now_ms())
    }

    pub fn verify_at(
        &self,
        token: &str,
        expected: &ExpectedCapability,
        now_ms: u64,
    ) -> Result<CapabilityTokenClaims, CapabilityTokenError> {
        if token.trim().is_empty() || token.starts_with("demo-cap-") {
            return Err(CapabilityTokenError::Malformed);
        }
        expected.validate()?;
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = false;
        validation.validate_nbf = false;
        validation.validate_aud = false;
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);

        let data = decode::<CapabilityTokenClaims>(
            token,
            &DecodingKey::from_secret(self.config.secret.as_bytes()),
            &validation,
        )
        .map_err(|_| CapabilityTokenError::InvalidSignature)?;
        let claims = data.claims;
        validate_claims_basic(&claims)?;
        validate_claims_time(&claims, now_ms)?;
        validate_expected_claims(&claims, &self.config, expected)?;
        Ok(claims)
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CapabilityTokenError {
    #[error("capability token claims are invalid")]
    InvalidClaims,

    #[error("capability token secret is missing")]
    MissingSecret,

    #[error("capability token timestamp is invalid")]
    InvalidTimestamp,

    #[error("capability token signing failed")]
    SigningFailed,

    #[error("capability token is malformed")]
    Malformed,

    #[error("capability token signature is invalid")]
    InvalidSignature,

    #[error("capability token is expired")]
    Expired,

    #[error("capability token is not active")]
    NotYetValid,

    #[error("capability token scope is not allowed")]
    ScopeMismatch,

    #[error("capability token is missing required scope")]
    MissingScope,

    #[error("capability token version is unsupported")]
    UnsupportedVersion,
}

pub fn bearer_token_expiry_ms(token: &str) -> Option<u64> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    validation.validate_nbf = false;
    validation.validate_aud = false;
    validation.required_spec_claims.clear();
    decode::<CapabilityTokenClaims>(token, &DecodingKey::from_secret(&[]), &validation)
        .ok()
        .map(|data| data.claims.expires_at_ms())
}

fn now_ms() -> u64 {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn unix_seconds_floor(ms: u64) -> Result<u64, CapabilityTokenError> {
    Ok(ms / 1_000)
}

fn unix_seconds_ceil(ms: u64) -> Result<u64, CapabilityTokenError> {
    Ok(ms.div_ceil(1_000))
}

fn validate_secret(secret: &str) -> Result<(), CapabilityTokenError> {
    if secret.trim().is_empty() {
        return Err(CapabilityTokenError::MissingSecret);
    }
    Ok(())
}

fn validate_claims_basic(claims: &CapabilityTokenClaims) -> Result<(), CapabilityTokenError> {
    if claims.version != CAPABILITY_TOKEN_VERSION {
        return Err(CapabilityTokenError::UnsupportedVersion);
    }
    if claims.iss.trim().is_empty()
        || claims.aud.trim().is_empty()
        || claims.sub.trim().is_empty()
        || claims.merchant_did.trim().is_empty()
        || claims.user_did.trim().is_empty()
        || claims.skill_id.trim().is_empty()
        || claims.session_id.trim().is_empty()
        || claims.jti.trim().is_empty()
        || claims.scopes.iter().any(|scope| scope.trim().is_empty())
    {
        return Err(CapabilityTokenError::InvalidClaims);
    }
    if claims.sub != claims.user_did || claims.scopes.is_empty() {
        return Err(CapabilityTokenError::InvalidClaims);
    }
    if claims.iat > claims.nbf || claims.nbf >= claims.exp {
        return Err(CapabilityTokenError::InvalidTimestamp);
    }
    Ok(())
}

fn validate_claims_time(
    claims: &CapabilityTokenClaims,
    now_ms: u64,
) -> Result<(), CapabilityTokenError> {
    let now = unix_seconds_floor(now_ms)?;
    if claims.exp <= now {
        return Err(CapabilityTokenError::Expired);
    }
    if claims.nbf > now {
        return Err(CapabilityTokenError::NotYetValid);
    }
    Ok(())
}

fn validate_expected_claims(
    claims: &CapabilityTokenClaims,
    config: &CapabilityTokenVerifierConfig,
    expected: &ExpectedCapability,
) -> Result<(), CapabilityTokenError> {
    if claims.iss != config.issuer
        || claims.iss != expected.issuer
        || claims.aud != config.audience
        || claims.aud != expected.audience
        || claims.merchant_did != expected.merchant_did
        || claims.skill_id != expected.skill_id
    {
        return Err(CapabilityTokenError::ScopeMismatch);
    }
    if expected
        .user_did
        .as_ref()
        .is_some_and(|user_did| claims.user_did != *user_did)
        || expected
            .agent_did
            .as_ref()
            .is_some_and(|agent_did| claims.agent_did.as_ref() != Some(agent_did))
        || expected
            .session_id
            .as_ref()
            .is_some_and(|session_id| claims.session_id != *session_id)
    {
        return Err(CapabilityTokenError::ScopeMismatch);
    }
    if !claims
        .scopes
        .iter()
        .any(|scope| scope == &expected.required_scope)
    {
        return Err(CapabilityTokenError::MissingScope);
    }
    Ok(())
}

fn generate_jti() -> String {
    let mut bytes = [0_u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
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
    fn token_scope_can_include_agent_and_session() {
        let cache = InMemoryTokenCache::new();
        let scoped = CapabilityTokenScope::for_subject(
            "did:wba:merchant",
            "did:wba:user",
            Some("did:wba:agent".to_owned()),
            "coffee",
            Some("session-1".to_owned()),
        );
        let other_session = CapabilityTokenScope::for_subject(
            "did:wba:merchant",
            "did:wba:user",
            Some("did:wba:agent".to_owned()),
            "coffee",
            Some("session-2".to_owned()),
        );

        cache.put(scoped.clone(), CapabilityToken::new("coffee-token", None));

        assert_eq!(
            cache.get(&scoped).map(|token| token.value),
            Some("coffee-token".to_owned())
        );
        assert!(cache.get(&other_session).is_none());
    }

    #[test]
    fn expired_token_is_not_returned() {
        let cache = InMemoryTokenCache::new();
        let scope = CapabilityTokenScope::new("did:wba:merchant", "did:wba:user", "coffee");

        cache.put(scope.clone(), CapabilityToken::new("old", Some(1)));

        assert!(cache.get(&scope).is_none());
    }

    #[test]
    fn capability_token_issues_and_verifies_claims() {
        let issuer = issuer();
        let verifier = verifier();
        let expected = expected("coffee:drinks:read");

        let outcome = issuer
            .issue_at(request(), 1_780_000_000_000)
            .expect("token issues");
        let claims = verifier
            .verify_at(&outcome.token.value, &expected, 1_780_000_001_000)
            .expect("token verifies");

        assert_eq!(claims.version, CAPABILITY_TOKEN_VERSION);
        assert_eq!(claims.iss, "did:wba:merchant.example");
        assert_eq!(claims.aud, "did:wba:merchant.example");
        assert_eq!(claims.sub, "did:wba:user.example");
        assert_eq!(claims.merchant_did, "did:wba:merchant.example");
        assert_eq!(claims.user_did, "did:wba:user.example");
        assert_eq!(claims.agent_did.as_deref(), Some("did:wba:agent.example"));
        assert_eq!(claims.skill_id, "coffee");
        assert_eq!(claims.session_id, "session-1");
        assert!(claims.scopes.contains(&"coffee:drinks:read".to_owned()));
        assert_eq!(outcome.token.expires_at_ms, Some(claims.expires_at_ms()));
    }

    #[test]
    fn capability_token_rejects_expired_token() {
        let issuer = issuer();
        let verifier = verifier();
        let outcome = issuer
            .issue_at(request(), 1_780_000_000_000)
            .expect("token issues");

        let error = verifier
            .verify_at(
                &outcome.token.value,
                &expected("coffee:drinks:read"),
                1_780_000_300_000,
            )
            .expect_err("expired token fails");

        assert_eq!(error, CapabilityTokenError::Expired);
    }

    #[test]
    fn capability_token_rejects_wrong_scope_dimensions() {
        let issuer = issuer();
        let outcome = issuer
            .issue_at(request(), 1_780_000_000_000)
            .expect("token issues");

        for expected in [
            ExpectedCapability::new(
                "did:wba:merchant.example",
                "did:wba:merchant.example",
                "did:wba:merchant-2.example",
                ExpectedCapabilitySubject::new(
                    "did:wba:user.example",
                    Some("did:wba:agent.example".to_owned()),
                    "session-1",
                ),
                "coffee",
                "coffee:drinks:read",
            ),
            ExpectedCapability::new(
                "did:wba:merchant.example",
                "did:wba:merchant.example",
                "did:wba:merchant.example",
                ExpectedCapabilitySubject::new(
                    "did:wba:user-2.example",
                    Some("did:wba:agent.example".to_owned()),
                    "session-1",
                ),
                "coffee",
                "coffee:drinks:read",
            ),
            ExpectedCapability::new(
                "did:wba:merchant.example",
                "did:wba:merchant.example",
                "did:wba:merchant.example",
                ExpectedCapabilitySubject::new(
                    "did:wba:user.example",
                    Some("did:wba:agent-2.example".to_owned()),
                    "session-1",
                ),
                "coffee",
                "coffee:drinks:read",
            ),
            ExpectedCapability::new(
                "did:wba:merchant.example",
                "did:wba:merchant.example",
                "did:wba:merchant.example",
                ExpectedCapabilitySubject::new(
                    "did:wba:user.example",
                    Some("did:wba:agent.example".to_owned()),
                    "session-1",
                ),
                "tea",
                "coffee:drinks:read",
            ),
            ExpectedCapability::new(
                "did:wba:merchant.example",
                "did:wba:merchant.example",
                "did:wba:merchant.example",
                ExpectedCapabilitySubject::new(
                    "did:wba:user.example",
                    Some("did:wba:agent.example".to_owned()),
                    "session-2",
                ),
                "coffee",
                "coffee:drinks:read",
            ),
        ] {
            let error = verifier()
                .verify_at(&outcome.token.value, &expected, 1_780_000_001_000)
                .expect_err("scope mismatch fails");
            assert_eq!(error, CapabilityTokenError::ScopeMismatch);
        }
    }

    #[test]
    fn capability_token_rejects_wrong_issuer_or_audience() {
        let issuer = issuer();
        let outcome = issuer
            .issue_at(request(), 1_780_000_000_000)
            .expect("token issues");

        for expected in [
            ExpectedCapability::new(
                "did:wba:issuer-2.example",
                "did:wba:merchant.example",
                "did:wba:merchant.example",
                ExpectedCapabilitySubject::new(
                    "did:wba:user.example",
                    Some("did:wba:agent.example".to_owned()),
                    "session-1",
                ),
                "coffee",
                "coffee:drinks:read",
            ),
            ExpectedCapability::new(
                "did:wba:merchant.example",
                "did:wba:audience-2.example",
                "did:wba:merchant.example",
                ExpectedCapabilitySubject::new(
                    "did:wba:user.example",
                    Some("did:wba:agent.example".to_owned()),
                    "session-1",
                ),
                "coffee",
                "coffee:drinks:read",
            ),
        ] {
            let error = verifier()
                .verify_at(&outcome.token.value, &expected, 1_780_000_001_000)
                .expect_err("issuer or audience mismatch fails");
            assert_eq!(error, CapabilityTokenError::ScopeMismatch);
        }
    }

    #[test]
    fn capability_token_rejects_missing_required_scope() {
        let issuer = issuer();
        let verifier = verifier();
        let outcome = issuer
            .issue_at(request(), 1_780_000_000_000)
            .expect("token issues");

        let error = verifier
            .verify_at(
                &outcome.token.value,
                &expected("coffee:order:pay"),
                1_780_000_001_000,
            )
            .expect_err("missing scope fails");

        assert_eq!(error, CapabilityTokenError::MissingScope);
    }

    #[test]
    fn capability_token_route_expected_scope_allows_claim_bound_subject() {
        let issuer = issuer();
        let verifier = verifier();
        let outcome = issuer
            .issue_at(request(), 1_780_000_000_000)
            .expect("token issues");
        let expected = ExpectedCapability::for_route(
            "did:wba:merchant.example",
            "did:wba:merchant.example",
            "did:wba:merchant.example",
            "coffee",
            "coffee:drinks:read",
        );

        let claims = verifier
            .verify_at(&outcome.token.value, &expected, 1_780_000_001_000)
            .expect("route scope verifies");

        assert_eq!(claims.user_did, "did:wba:user.example");
        assert_eq!(claims.session_id, "session-1");
    }

    #[test]
    fn capability_token_rejects_malformed_and_demo_tokens() {
        let verifier = verifier();
        let expected = expected("coffee:drinks:read");

        let malformed = verifier
            .verify_at("not-a-jwt", &expected, 1_780_000_001_000)
            .expect_err("malformed token fails");
        let demo = verifier
            .verify_at("demo-cap-challenge-1", &expected, 1_780_000_001_000)
            .expect_err("demo token fails");

        assert_eq!(malformed, CapabilityTokenError::InvalidSignature);
        assert_eq!(demo, CapabilityTokenError::Malformed);
    }

    #[test]
    fn capability_token_rejects_wrong_secret() {
        let issuer = issuer();
        let outcome = issuer
            .issue_at(request(), 1_780_000_000_000)
            .expect("token issues");
        let verifier = CapabilityTokenVerifier::new(CapabilityTokenVerifierConfig::new(
            "did:wba:merchant.example",
            "did:wba:merchant.example",
            "wrong-test-secret",
        ))
        .expect("verifier config");

        let error = verifier
            .verify_at(
                &outcome.token.value,
                &expected("coffee:drinks:read"),
                1_780_000_001_000,
            )
            .expect_err("wrong secret fails");

        assert_eq!(error, CapabilityTokenError::InvalidSignature);
    }

    #[test]
    fn capability_token_errors_and_debug_are_redacted() {
        let config = CapabilityTokenIssuerConfig::new(
            "did:wba:merchant.example",
            "did:wba:merchant.example",
            test_secret(),
        );
        let debug = format!("{config:?}");
        assert!(!debug.contains(test_secret()));
        assert!(debug.contains("[REDACTED]"));

        let issuer = CapabilityTokenIssuer::new(config).expect("issuer config");
        let outcome = issuer
            .issue_at(request(), 1_780_000_000_000)
            .expect("token issues");
        let token_debug = format!("{:?}", outcome.token);
        assert!(!token_debug.contains(&outcome.token.value));
        assert!(token_debug.contains("[REDACTED]"));

        let error = CapabilityTokenError::InvalidSignature;
        let display = error.to_string();
        let debug = format!("{error:?}");
        assert!(!display.contains(&outcome.token.value));
        assert!(!display.contains(test_secret()));
        assert!(!debug.contains(&outcome.token.value));
        assert!(!debug.contains(test_secret()));
    }

    #[test]
    fn bearer_token_expiry_reads_jwt_exp_without_verifying_secret() {
        let issuer = issuer();
        let outcome = issuer
            .issue_at(request(), 1_780_000_000_000)
            .expect("token issues");

        assert_eq!(
            bearer_token_expiry_ms(&outcome.token.value),
            outcome.token.expires_at_ms
        );
    }

    fn issuer() -> CapabilityTokenIssuer {
        CapabilityTokenIssuer::new(
            CapabilityTokenIssuerConfig::new(
                "did:wba:merchant.example",
                "did:wba:merchant.example",
                test_secret(),
            )
            .with_ttl_ms(300_000),
        )
        .expect("issuer config")
    }

    fn verifier() -> CapabilityTokenVerifier {
        CapabilityTokenVerifier::new(CapabilityTokenVerifierConfig::new(
            "did:wba:merchant.example",
            "did:wba:merchant.example",
            test_secret(),
        ))
        .expect("verifier config")
    }

    fn request() -> CapabilityTokenRequest {
        CapabilityTokenRequest::new(
            "did:wba:merchant.example",
            "did:wba:user.example",
            Some("did:wba:agent.example".to_owned()),
            "coffee",
            "session-1",
            ["coffee:drinks:read", "coffee:order:confirm"],
        )
    }

    fn expected(required_scope: &str) -> ExpectedCapability {
        ExpectedCapability::new(
            "did:wba:merchant.example",
            "did:wba:merchant.example",
            "did:wba:merchant.example",
            ExpectedCapabilitySubject::new(
                "did:wba:user.example",
                Some("did:wba:agent.example".to_owned()),
                "session-1",
            ),
            "coffee",
            required_scope,
        )
    }

    fn test_secret() -> &'static str {
        "test-only-capability-token-secret-do-not-use-in-production"
    }
}
