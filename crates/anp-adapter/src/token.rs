use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub const CAPABILITY_TOKEN_VERSION: &str = "dock.capability.v1";
pub const CAPABILITY_TOKEN_SCOPE_DERIVATION_SOURCE: &str =
    "CapabilityTokenRequest merchant_did/user_did/agent_did/skill_id/session_id/scopes";
pub const DEFAULT_CAPABILITY_TOKEN_TTL_MS: u64 = 300_000;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

pub trait CapabilityTokenLifecycleStore {
    fn revoke_jti(&self, jti: &str, expires_at_ms: u64) -> Result<(), CapabilityTokenError>;
    fn is_revoked(&self, jti: &str, now_ms: u64) -> Result<bool, CapabilityTokenError>;
    fn is_consumed_once(&self, jti: &str, now_ms: u64) -> Result<bool, CapabilityTokenError>;
    fn consume_jti_once(
        &self,
        jti: &str,
        expires_at_ms: u64,
        now_ms: u64,
    ) -> Result<(), CapabilityTokenError>;
    fn prune_expired(&self, now_ms: u64) -> Result<usize, CapabilityTokenError>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryTokenLifecycleStore {
    state: Arc<Mutex<TokenLifecycleState>>,
}

#[derive(Debug, Clone, Default)]
struct TokenLifecycleState {
    revoked: BTreeMap<String, u64>,
    seen_once: BTreeMap<String, u64>,
}

impl InMemoryTokenLifecycleStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CapabilityTokenLifecycleStore for InMemoryTokenLifecycleStore {
    fn revoke_jti(&self, jti: &str, expires_at_ms: u64) -> Result<(), CapabilityTokenError> {
        if jti.trim().is_empty() {
            return Err(CapabilityTokenError::InvalidClaims);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| CapabilityTokenError::LifecycleUnavailable)?;
        state.revoked.insert(jti.to_owned(), expires_at_ms);
        Ok(())
    }

    fn is_revoked(&self, jti: &str, now_ms: u64) -> Result<bool, CapabilityTokenError> {
        if jti.trim().is_empty() {
            return Err(CapabilityTokenError::InvalidClaims);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| CapabilityTokenError::LifecycleUnavailable)?;
        prune_state(&mut state, now_ms);
        Ok(state.revoked.contains_key(jti))
    }

    fn is_consumed_once(&self, jti: &str, now_ms: u64) -> Result<bool, CapabilityTokenError> {
        if jti.trim().is_empty() {
            return Err(CapabilityTokenError::InvalidClaims);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| CapabilityTokenError::LifecycleUnavailable)?;
        prune_state(&mut state, now_ms);
        Ok(state.seen_once.contains_key(jti))
    }

    fn consume_jti_once(
        &self,
        jti: &str,
        expires_at_ms: u64,
        now_ms: u64,
    ) -> Result<(), CapabilityTokenError> {
        if jti.trim().is_empty() {
            return Err(CapabilityTokenError::InvalidClaims);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| CapabilityTokenError::LifecycleUnavailable)?;
        prune_state(&mut state, now_ms);
        if state.seen_once.contains_key(jti) {
            return Err(CapabilityTokenError::Replayed);
        }
        state.seen_once.insert(jti.to_owned(), expires_at_ms);
        Ok(())
    }

    fn prune_expired(&self, now_ms: u64) -> Result<usize, CapabilityTokenError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| CapabilityTokenError::LifecycleUnavailable)?;
        Ok(prune_state(&mut state, now_ms))
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryTokenCache {
    tokens: Arc<Mutex<BTreeMap<CapabilityTokenScope, CapabilityToken>>>,
}

impl InMemoryTokenCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn snapshot_persistent_entries(&self, now_ms: u64) -> Vec<PersistentCapabilityTokenEntry> {
        let mut tokens = self.tokens.lock().expect("token cache mutex poisoned");
        tokens.retain(|_, token| !token.is_expired_at(now_ms));
        tokens
            .iter()
            .map(|(scope, token)| PersistentCapabilityTokenEntry::new(scope.clone(), token.clone()))
            .collect()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TokenCachePersistenceProfile {
    InMemoryDev,
    HostSecureStore,
    EncryptedBackend,
}

impl TokenCachePersistenceProfile {
    pub fn production_ready(self) -> bool {
        matches!(
            self,
            TokenCachePersistenceProfile::HostSecureStore
                | TokenCachePersistenceProfile::EncryptedBackend
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PersistentCapabilityTokenEntry {
    pub scope: CapabilityTokenScope,
    pub issuer: String,
    pub audience: String,
    pub jti: String,
    token_value: String,
    pub expires_at_ms: Option<u64>,
}

impl PersistentCapabilityTokenEntry {
    pub fn new(scope: CapabilityTokenScope, token: CapabilityToken) -> Self {
        let metadata = decode_token_for_restore(&token.value).ok();
        Self {
            scope,
            issuer: metadata
                .as_ref()
                .map(|claims| claims.iss.clone())
                .unwrap_or_default(),
            audience: metadata
                .as_ref()
                .map(|claims| claims.aud.clone())
                .unwrap_or_default(),
            jti: metadata
                .as_ref()
                .map(|claims| claims.jti.clone())
                .unwrap_or_default(),
            token_value: token.value,
            expires_at_ms: token.expires_at_ms,
        }
    }

    pub fn token(&self) -> CapabilityToken {
        CapabilityToken::new(self.token_value.clone(), self.expires_at_ms)
    }
}

impl fmt::Debug for PersistentCapabilityTokenEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PersistentCapabilityTokenEntry")
            .field("scope", &self.scope)
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("jti", &"[REDACTED]")
            .field("token_value", &"[REDACTED]")
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

pub trait TokenCachePersistenceBackend: Clone {
    fn profile(&self) -> TokenCachePersistenceProfile;
    fn load_entries(&self) -> Result<Vec<PersistentCapabilityTokenEntry>, CapabilityTokenError>;
    fn replace_entries(
        &self,
        entries: Vec<PersistentCapabilityTokenEntry>,
    ) -> Result<(), CapabilityTokenError>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryTokenCachePersistenceBackend {
    entries: Arc<Mutex<Vec<PersistentCapabilityTokenEntry>>>,
}

impl InMemoryTokenCachePersistenceBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_entries(entries: Vec<PersistentCapabilityTokenEntry>) -> Self {
        Self {
            entries: Arc::new(Mutex::new(entries)),
        }
    }

    pub fn entries(&self) -> Result<Vec<PersistentCapabilityTokenEntry>, CapabilityTokenError> {
        self.load_entries()
    }
}

impl TokenCachePersistenceBackend for InMemoryTokenCachePersistenceBackend {
    fn profile(&self) -> TokenCachePersistenceProfile {
        TokenCachePersistenceProfile::InMemoryDev
    }

    fn load_entries(&self) -> Result<Vec<PersistentCapabilityTokenEntry>, CapabilityTokenError> {
        self.entries
            .lock()
            .map(|entries| entries.clone())
            .map_err(|_| CapabilityTokenError::TokenCachePersistenceUnavailable)
    }

    fn replace_entries(
        &self,
        entries: Vec<PersistentCapabilityTokenEntry>,
    ) -> Result<(), CapabilityTokenError> {
        *self
            .entries
            .lock()
            .map_err(|_| CapabilityTokenError::TokenCachePersistenceUnavailable)? = entries;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCacheRestoreReport {
    pub backend_profile: TokenCachePersistenceProfile,
    pub production_ready: bool,
    pub loaded_count: usize,
    pub restored_count: usize,
    pub rejected: Vec<TokenCacheRestoreRejection>,
    pub redaction: TokenCacheRedaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCacheRestoreRejection {
    pub scope: CapabilityTokenScopeSummary,
    pub reason: TokenCacheRestoreRejectionReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityTokenScopeSummary {
    pub skill_id: String,
    pub has_agent_did: bool,
    pub has_session_id: bool,
}

impl From<&CapabilityTokenScope> for CapabilityTokenScopeSummary {
    fn from(scope: &CapabilityTokenScope) -> Self {
        Self {
            skill_id: scope.skill_id.clone(),
            has_agent_did: scope.agent_did.is_some(),
            has_session_id: scope.session_id.is_some(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TokenCacheRestoreRejectionReason {
    MissingExpiry,
    Expired,
    MissingScope,
    InvalidSignatureOrTrust,
    ScopeMismatch,
    Revoked,
    Replayed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCacheRedaction {
    pub marker: String,
    pub policy: String,
    pub raw_token_visible: bool,
}

impl Default for TokenCacheRedaction {
    fn default() -> Self {
        Self {
            marker: "[REDACTED]".to_owned(),
            policy: "dock.token-cache.redaction.v1".to_owned(),
            raw_token_visible: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PersistentCapabilityTokenCache<B> {
    backend: B,
    cache: InMemoryTokenCache,
}

impl<B> PersistentCapabilityTokenCache<B>
where
    B: TokenCachePersistenceBackend,
{
    pub fn restore(
        backend: B,
        verifier: &CapabilityTokenVerifier,
        lifecycle: &impl CapabilityTokenLifecycleStore,
        now_ms: u64,
    ) -> Result<(Self, TokenCacheRestoreReport), CapabilityTokenError> {
        let loaded = backend.load_entries()?;
        let loaded_count = loaded.len();
        let cache = InMemoryTokenCache::new();
        let mut restored_entries = Vec::new();
        let mut rejected = Vec::new();

        for entry in loaded {
            match restore_entry(&entry, verifier, lifecycle, now_ms) {
                Ok(()) => {
                    cache.put(entry.scope.clone(), entry.token());
                    restored_entries.push(entry);
                }
                Err(reason) => rejected.push(TokenCacheRestoreRejection {
                    scope: CapabilityTokenScopeSummary::from(&entry.scope),
                    reason,
                }),
            }
        }

        backend.replace_entries(restored_entries.clone())?;
        let profile = backend.profile();
        let report = TokenCacheRestoreReport {
            backend_profile: profile,
            production_ready: profile.production_ready(),
            loaded_count,
            restored_count: restored_entries.len(),
            rejected,
            redaction: TokenCacheRedaction::default(),
        };

        Ok((Self { backend, cache }, report))
    }

    pub fn try_put(
        &self,
        scope: CapabilityTokenScope,
        token: CapabilityToken,
    ) -> Result<(), CapabilityTokenError> {
        let mut entries = self.cache.snapshot_persistent_entries(now_ms());
        entries.retain(|entry| entry.scope != scope);
        entries.push(PersistentCapabilityTokenEntry::new(
            scope.clone(),
            token.clone(),
        ));
        self.backend.replace_entries(entries)?;
        self.cache.put(scope, token);
        Ok(())
    }

    pub fn try_clear(&self, scope: &CapabilityTokenScope) -> Result<(), CapabilityTokenError> {
        let mut entries = self.cache.snapshot_persistent_entries(now_ms());
        entries.retain(|entry| &entry.scope != scope);
        self.backend.replace_entries(entries)?;
        self.cache.clear(scope);
        Ok(())
    }

    pub fn restore_report(
        &self,
    ) -> Result<(TokenCachePersistenceProfile, usize), CapabilityTokenError> {
        Ok((self.backend.profile(), self.backend.load_entries()?.len()))
    }
}

impl<B> CapabilityTokenCache for PersistentCapabilityTokenCache<B>
where
    B: TokenCachePersistenceBackend,
{
    fn get(&self, scope: &CapabilityTokenScope) -> Option<CapabilityToken> {
        self.cache.get(scope)
    }

    fn put(&self, scope: CapabilityTokenScope, token: CapabilityToken) {
        let _ = self.try_put(scope, token);
    }

    fn clear(&self, scope: &CapabilityTokenScope) {
        let _ = self.try_clear(scope);
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

    pub fn verify_with_lifecycle_at<S>(
        &self,
        token: &str,
        expected: &ExpectedCapability,
        lifecycle: &S,
        mode: CapabilityTokenLifecycleMode,
        now_ms: u64,
    ) -> Result<CapabilityTokenClaims, CapabilityTokenError>
    where
        S: CapabilityTokenLifecycleStore,
    {
        let claims = self.verify_at(token, expected, now_ms)?;
        if lifecycle.is_revoked(&claims.jti, now_ms)? {
            return Err(CapabilityTokenError::Revoked);
        }
        if mode == CapabilityTokenLifecycleMode::ConsumeOnce {
            lifecycle.consume_jti_once(&claims.jti, claims.expires_at_ms(), now_ms)?;
        }
        Ok(claims)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityTokenLifecycleMode {
    CheckOnly,
    ConsumeOnce,
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

    #[error("capability token has been revoked")]
    Revoked,

    #[error("capability token replay was detected")]
    Replayed,

    #[error("capability token lifecycle store is unavailable")]
    LifecycleUnavailable,

    #[error("capability token persistence backend is unavailable")]
    TokenCachePersistenceUnavailable,
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

fn restore_entry(
    entry: &PersistentCapabilityTokenEntry,
    verifier: &CapabilityTokenVerifier,
    lifecycle: &impl CapabilityTokenLifecycleStore,
    now_ms: u64,
) -> Result<(), TokenCacheRestoreRejectionReason> {
    let Some(expires_at_ms) = entry.expires_at_ms else {
        return Err(TokenCacheRestoreRejectionReason::MissingExpiry);
    };
    if expires_at_ms <= now_ms {
        return Err(TokenCacheRestoreRejectionReason::Expired);
    }

    let claims = decode_token_for_restore(&entry.token().value)
        .map_err(|_| TokenCacheRestoreRejectionReason::InvalidSignatureOrTrust)?;
    if entry.issuer != claims.iss || entry.audience != claims.aud || entry.jti != claims.jti {
        return Err(TokenCacheRestoreRejectionReason::InvalidSignatureOrTrust);
    }
    if claims.scope() != entry.scope {
        return Err(TokenCacheRestoreRejectionReason::ScopeMismatch);
    }
    let Some(required_scope) = claims
        .scopes
        .first()
        .filter(|scope| !scope.trim().is_empty())
    else {
        return Err(TokenCacheRestoreRejectionReason::MissingScope);
    };
    let expected = ExpectedCapability::new(
        claims.iss.clone(),
        claims.aud.clone(),
        claims.merchant_did.clone(),
        ExpectedCapabilitySubject::new(
            claims.user_did.clone(),
            claims.agent_did.clone(),
            claims.session_id.clone(),
        ),
        claims.skill_id.clone(),
        required_scope.clone(),
    );
    let verified = verifier
        .verify_at(&entry.token().value, &expected, now_ms)
        .map_err(map_restore_verify_error)?;
    if verified.scope() != entry.scope {
        return Err(TokenCacheRestoreRejectionReason::ScopeMismatch);
    }
    if lifecycle
        .is_revoked(&verified.jti, now_ms)
        .map_err(|_| TokenCacheRestoreRejectionReason::InvalidSignatureOrTrust)?
    {
        return Err(TokenCacheRestoreRejectionReason::Revoked);
    }
    if lifecycle
        .is_consumed_once(&verified.jti, now_ms)
        .map_err(|_| TokenCacheRestoreRejectionReason::InvalidSignatureOrTrust)?
    {
        return Err(TokenCacheRestoreRejectionReason::Replayed);
    }
    Ok(())
}

fn decode_token_for_restore(token: &str) -> Result<CapabilityTokenClaims, CapabilityTokenError> {
    if token.trim().is_empty() || token.starts_with("demo-cap-") {
        return Err(CapabilityTokenError::Malformed);
    }
    let mut validation = Validation::new(Algorithm::HS256);
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    validation.validate_nbf = false;
    validation.validate_aud = false;
    validation.required_spec_claims.clear();
    decode::<CapabilityTokenClaims>(token, &DecodingKey::from_secret(&[]), &validation)
        .map(|data| data.claims)
        .map_err(|_| CapabilityTokenError::Malformed)
}

fn map_restore_verify_error(error: CapabilityTokenError) -> TokenCacheRestoreRejectionReason {
    match error {
        CapabilityTokenError::Expired => TokenCacheRestoreRejectionReason::Expired,
        CapabilityTokenError::MissingScope => TokenCacheRestoreRejectionReason::MissingScope,
        CapabilityTokenError::ScopeMismatch => TokenCacheRestoreRejectionReason::ScopeMismatch,
        CapabilityTokenError::Revoked => TokenCacheRestoreRejectionReason::Revoked,
        CapabilityTokenError::Replayed => TokenCacheRestoreRejectionReason::Replayed,
        _ => TokenCacheRestoreRejectionReason::InvalidSignatureOrTrust,
    }
}

fn generate_jti() -> String {
    let mut bytes = [0_u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn prune_state(state: &mut TokenLifecycleState, now_ms: u64) -> usize {
    let before = state.revoked.len() + state.seen_once.len();
    state
        .revoked
        .retain(|_, expires_at_ms| *expires_at_ms > now_ms);
    state
        .seen_once
        .retain(|_, expires_at_ms| *expires_at_ms > now_ms);
    before.saturating_sub(state.revoked.len() + state.seen_once.len())
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

    #[test]
    fn token_claims_record_version_and_scope_derivation_source() {
        let issuer = issuer();
        let outcome = issuer
            .issue_at(request(), 1_780_000_000_000)
            .expect("token issues");

        assert_eq!(outcome.claims.version, CAPABILITY_TOKEN_VERSION);
        assert_eq!(
            CAPABILITY_TOKEN_SCOPE_DERIVATION_SOURCE,
            "CapabilityTokenRequest merchant_did/user_did/agent_did/skill_id/session_id/scopes"
        );
        assert_eq!(outcome.claims.scopes, request().scopes);
    }

    #[test]
    fn lifecycle_store_rejects_revoked_token() {
        let issuer = issuer();
        let verifier = verifier();
        let lifecycle = InMemoryTokenLifecycleStore::new();
        let outcome = issuer
            .issue_at(request(), 1_780_000_000_000)
            .expect("token issues");

        lifecycle
            .revoke_jti(&outcome.claims.jti, outcome.claims.expires_at_ms())
            .expect("jti revokes");

        let error = verifier
            .verify_with_lifecycle_at(
                &outcome.token.value,
                &expected("coffee:drinks:read"),
                &lifecycle,
                CapabilityTokenLifecycleMode::CheckOnly,
                1_780_000_001_000,
            )
            .expect_err("revoked token fails");

        assert_eq!(error, CapabilityTokenError::Revoked);
    }

    #[test]
    fn lifecycle_store_can_consume_jti_once_for_replay_sensitive_routes() {
        let issuer = issuer();
        let verifier = verifier();
        let lifecycle = InMemoryTokenLifecycleStore::new();
        let outcome = issuer
            .issue_at(request(), 1_780_000_000_000)
            .expect("token issues");

        verifier
            .verify_with_lifecycle_at(
                &outcome.token.value,
                &expected("coffee:order:confirm"),
                &lifecycle,
                CapabilityTokenLifecycleMode::ConsumeOnce,
                1_780_000_001_000,
            )
            .expect("first use verifies");
        let error = verifier
            .verify_with_lifecycle_at(
                &outcome.token.value,
                &expected("coffee:order:confirm"),
                &lifecycle,
                CapabilityTokenLifecycleMode::ConsumeOnce,
                1_780_000_002_000,
            )
            .expect_err("replay fails");

        assert_eq!(error, CapabilityTokenError::Replayed);
    }

    #[test]
    fn lifecycle_store_prunes_expired_revocation_and_replay_entries() {
        let lifecycle = InMemoryTokenLifecycleStore::new();
        lifecycle.revoke_jti("revoked-jti", 1_000).expect("revokes");
        lifecycle
            .consume_jti_once("seen-jti", 2_000, 500)
            .expect("consumes");

        assert_eq!(lifecycle.prune_expired(2_000).expect("prunes"), 2);
        assert!(!lifecycle
            .is_revoked("revoked-jti", 2_000)
            .expect("checks revoked"));
        assert_eq!(lifecycle.consume_jti_once("seen-jti", 3_000, 2_000), Ok(()));
    }

    #[test]
    fn token_cache_persistence_restores_valid_entries_and_persists_snapshot() {
        let issuer = issuer();
        let verifier = verifier();
        let lifecycle = InMemoryTokenLifecycleStore::new();
        let base_ms = token_cache_test_base_ms();
        let outcome = issuer.issue_at(request(), base_ms).expect("token issues");
        let scope = outcome.claims.scope();
        let backend = InMemoryTokenCachePersistenceBackend::with_entries(vec![
            PersistentCapabilityTokenEntry::new(scope.clone(), outcome.token.clone()),
        ]);

        let (cache, report) = PersistentCapabilityTokenCache::restore(
            backend.clone(),
            &verifier,
            &lifecycle,
            base_ms + 1_000,
        )
        .expect("cache restores");

        assert_eq!(
            report.backend_profile,
            TokenCachePersistenceProfile::InMemoryDev
        );
        assert!(!report.production_ready);
        assert_eq!(report.loaded_count, 1);
        assert_eq!(report.restored_count, 1);
        assert!(report.rejected.is_empty());
        assert_eq!(cache.get(&scope), Some(outcome.token.clone()));
        assert_eq!(backend.entries().expect("entries").len(), 1);

        let new_outcome = issuer
            .issue_at(request(), base_ms + 10_000)
            .expect("new token issues");
        cache
            .try_put(scope.clone(), new_outcome.token.clone())
            .expect("snapshot persists");
        assert_eq!(
            backend
                .entries()
                .expect("entries")
                .first()
                .map(PersistentCapabilityTokenEntry::token),
            Some(new_outcome.token)
        );
    }

    #[test]
    fn token_cache_persistence_rejects_expired_entries() {
        let issuer = issuer();
        let verifier = verifier();
        let lifecycle = InMemoryTokenLifecycleStore::new();
        let base_ms = token_cache_test_base_ms();
        let outcome = issuer.issue_at(request(), base_ms).expect("token issues");
        let backend = InMemoryTokenCachePersistenceBackend::with_entries(vec![
            PersistentCapabilityTokenEntry::new(outcome.claims.scope(), outcome.token),
        ]);

        let (_cache, report) = PersistentCapabilityTokenCache::restore(
            backend.clone(),
            &verifier,
            &lifecycle,
            base_ms + 301_000,
        )
        .expect("cache restore rejects expired");

        assert_eq!(report.loaded_count, 1);
        assert_eq!(report.restored_count, 0);
        assert_eq!(
            report.rejected[0].reason,
            TokenCacheRestoreRejectionReason::Expired
        );
        assert!(backend.entries().expect("entries").is_empty());
    }

    #[test]
    fn token_cache_persistence_rejects_revoked_entries() {
        let issuer = issuer();
        let verifier = verifier();
        let lifecycle = InMemoryTokenLifecycleStore::new();
        let base_ms = token_cache_test_base_ms();
        let outcome = issuer.issue_at(request(), base_ms).expect("token issues");
        lifecycle
            .revoke_jti(&outcome.claims.jti, outcome.claims.expires_at_ms())
            .expect("jti revokes");
        let backend = InMemoryTokenCachePersistenceBackend::with_entries(vec![
            PersistentCapabilityTokenEntry::new(outcome.claims.scope(), outcome.token),
        ]);

        let (_cache, report) = PersistentCapabilityTokenCache::restore(
            backend,
            &verifier,
            &lifecycle,
            base_ms + 1_000,
        )
        .expect("cache restore rejects revoked");

        assert_eq!(report.restored_count, 0);
        assert_eq!(
            report.rejected[0].reason,
            TokenCacheRestoreRejectionReason::Revoked
        );
    }

    #[test]
    fn token_cache_persistence_rejects_replayed_entries() {
        let issuer = issuer();
        let verifier = verifier();
        let lifecycle = InMemoryTokenLifecycleStore::new();
        let base_ms = token_cache_test_base_ms();
        let outcome = issuer.issue_at(request(), base_ms).expect("token issues");
        lifecycle
            .consume_jti_once(
                &outcome.claims.jti,
                outcome.claims.expires_at_ms(),
                base_ms + 1_000,
            )
            .expect("jti consumes");
        let backend = InMemoryTokenCachePersistenceBackend::with_entries(vec![
            PersistentCapabilityTokenEntry::new(outcome.claims.scope(), outcome.token),
        ]);

        let (_cache, report) = PersistentCapabilityTokenCache::restore(
            backend,
            &verifier,
            &lifecycle,
            base_ms + 2_000,
        )
        .expect("cache restore rejects replayed");

        assert_eq!(report.restored_count, 0);
        assert_eq!(
            report.rejected[0].reason,
            TokenCacheRestoreRejectionReason::Replayed
        );
    }

    #[test]
    fn token_cache_persistence_rejects_scope_mismatch_entries() {
        let issuer = issuer();
        let verifier = verifier();
        let lifecycle = InMemoryTokenLifecycleStore::new();
        let base_ms = token_cache_test_base_ms();
        let outcome = issuer.issue_at(request(), base_ms).expect("token issues");
        let wrong_scope = CapabilityTokenScope::for_subject(
            "did:wba:merchant.example",
            "did:wba:user.example",
            Some("did:wba:agent.example".to_owned()),
            "tea",
            Some("session-1".to_owned()),
        );
        let backend = InMemoryTokenCachePersistenceBackend::with_entries(vec![
            PersistentCapabilityTokenEntry::new(wrong_scope, outcome.token),
        ]);

        let (_cache, report) = PersistentCapabilityTokenCache::restore(
            backend,
            &verifier,
            &lifecycle,
            base_ms + 1_000,
        )
        .expect("cache restore rejects mismatch");

        assert_eq!(report.restored_count, 0);
        assert_eq!(
            report.rejected[0].reason,
            TokenCacheRestoreRejectionReason::ScopeMismatch
        );
        assert_eq!(report.rejected[0].scope.skill_id, "tea");
    }

    #[test]
    fn token_cache_persistence_rejects_metadata_trust_mismatch_entries() {
        let issuer = issuer();
        let verifier = verifier();
        let lifecycle = InMemoryTokenLifecycleStore::new();
        let base_ms = token_cache_test_base_ms();
        let outcome = issuer.issue_at(request(), base_ms).expect("token issues");
        let mut entry = PersistentCapabilityTokenEntry::new(outcome.claims.scope(), outcome.token);
        entry.issuer = "did:wba:attacker.example".to_owned();
        let backend = InMemoryTokenCachePersistenceBackend::with_entries(vec![entry]);

        let (_cache, report) = PersistentCapabilityTokenCache::restore(
            backend,
            &verifier,
            &lifecycle,
            base_ms + 1_000,
        )
        .expect("cache restore rejects trust mismatch");

        assert_eq!(report.restored_count, 0);
        assert_eq!(
            report.rejected[0].reason,
            TokenCacheRestoreRejectionReason::InvalidSignatureOrTrust
        );
    }

    #[test]
    fn token_cache_persistence_report_and_debug_redact_raw_token() {
        let issuer = issuer();
        let verifier = verifier();
        let lifecycle = InMemoryTokenLifecycleStore::new();
        let base_ms = token_cache_test_base_ms();
        let outcome = issuer.issue_at(request(), base_ms).expect("token issues");
        let entry =
            PersistentCapabilityTokenEntry::new(outcome.claims.scope(), outcome.token.clone());
        let entry_debug = format!("{entry:?}");
        assert!(!entry_debug.contains(&outcome.token.value));
        assert!(!entry_debug.contains(&outcome.claims.jti));
        assert!(entry_debug.contains("[REDACTED]"));

        let backend = InMemoryTokenCachePersistenceBackend::with_entries(vec![entry]);
        let (_cache, report) = PersistentCapabilityTokenCache::restore(
            backend,
            &verifier,
            &lifecycle,
            base_ms + 1_000,
        )
        .expect("cache restores");
        let report_json = serde_json::to_string(&report).expect("report serializes");
        assert!(!report_json.contains(&outcome.token.value));
        assert!(!report_json.contains("Authorization"));
        assert!(!report_json.contains(test_secret()));
        assert!(!report_json.contains("private"));
        assert!(!report_json.contains("signature"));
        assert!(!report.redaction.raw_token_visible);
    }

    #[test]
    fn token_cache_persistence_marks_in_memory_backend_dev_only() {
        assert!(!TokenCachePersistenceProfile::InMemoryDev.production_ready());
        assert!(TokenCachePersistenceProfile::HostSecureStore.production_ready());
        assert!(TokenCachePersistenceProfile::EncryptedBackend.production_ready());

        let backend = InMemoryTokenCachePersistenceBackend::new();
        assert_eq!(backend.profile(), TokenCachePersistenceProfile::InMemoryDev);
    }

    #[test]
    fn token_cache_persistence_try_put_fails_before_memory_update() {
        let issuer = issuer();
        let verifier = verifier();
        let lifecycle = InMemoryTokenLifecycleStore::new();
        let base_ms = token_cache_test_base_ms();
        let outcome = issuer.issue_at(request(), base_ms).expect("token issues");
        let scope = outcome.claims.scope();
        let (cache, _report) = PersistentCapabilityTokenCache::restore(
            FailingTokenCachePersistenceBackend,
            &verifier,
            &lifecycle,
            base_ms + 1_000,
        )
        .expect("empty restore succeeds");

        let error = cache
            .try_put(scope.clone(), outcome.token)
            .expect_err("backend failure blocks put");

        assert_eq!(
            error,
            CapabilityTokenError::TokenCachePersistenceUnavailable
        );
        assert!(cache.get(&scope).is_none());
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

    fn token_cache_test_base_ms() -> u64 {
        now_ms().saturating_add(60_000)
    }

    #[derive(Clone)]
    struct FailingTokenCachePersistenceBackend;

    impl TokenCachePersistenceBackend for FailingTokenCachePersistenceBackend {
        fn profile(&self) -> TokenCachePersistenceProfile {
            TokenCachePersistenceProfile::EncryptedBackend
        }

        fn load_entries(
            &self,
        ) -> Result<Vec<PersistentCapabilityTokenEntry>, CapabilityTokenError> {
            Ok(Vec::new())
        }

        fn replace_entries(
            &self,
            entries: Vec<PersistentCapabilityTokenEntry>,
        ) -> Result<(), CapabilityTokenError> {
            if entries.is_empty() {
                Ok(())
            } else {
                Err(CapabilityTokenError::TokenCachePersistenceUnavailable)
            }
        }
    }
}
