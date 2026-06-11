use crate::did::{DidCredentialError, DidCredentialProvider, IdentitySession};
use anp::authentication::{
    extract_signature_metadata, generate_http_signature_headers, verify_http_message_signature,
    AuthMode, HttpSignatureError, HttpSignatureOptions, SignatureMetadata,
};
use anp::PrivateKeyMaterial;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub const CHALLENGE_PROOF_TYPE: &str = "anp-http-signature/v1";
pub const CHALLENGE_PROOF_METHOD: &str = "POST";
const SIGNATURE_SKEW_MS: u64 = 300_000;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeProofPayload {
    pub challenge_id: String,
    pub nonce: String,
    pub merchant_did: String,
    pub user_did: String,
    pub agent_did: Option<String>,
    pub skill_id: String,
    pub session_id: String,
    pub audience: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
}

impl ChallengeProofPayload {
    pub fn from_challenge(
        challenge: &DidChallenge,
        session: &IdentitySession,
        audience: impl Into<String>,
        issued_at_ms: u64,
    ) -> Self {
        Self {
            challenge_id: challenge.challenge_id.clone(),
            nonce: challenge.nonce.clone(),
            merchant_did: challenge.merchant_did.clone(),
            user_did: session.user_did.clone(),
            agent_did: session.agent_did.clone(),
            skill_id: session.skill_id.clone(),
            session_id: session.session_id.clone(),
            audience: audience.into(),
            issued_at_ms,
            expires_at_ms: challenge
                .expires_at_ms
                .unwrap_or_else(|| issued_at_ms.saturating_add(SIGNATURE_SKEW_MS)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChallengeProofKind {
    #[serde(rename = "anp-http-signature/v1")]
    AnpHttpSignatureV1,
}

impl ChallengeProofKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AnpHttpSignatureV1 => CHALLENGE_PROOF_TYPE,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockDidChallengeProof {
    #[serde(rename = "type")]
    pub proof_type: ChallengeProofKind,
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub payload: ChallengeProofPayload,
}

impl fmt::Debug for DockDidChallengeProof {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DockDidChallengeProof")
            .field("proof_type", &self.proof_type.as_str())
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &redacted_headers(&self.headers))
            .field("payload", &self.payload)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedChallengeProof {
    pub signer_did: String,
    pub key_id: String,
    pub auth_scheme: String,
}

pub trait DidDocumentResolver {
    fn resolve_did_document(&self, did: &str) -> Result<Value, ChallengeProofError>;
}

impl<F> DidDocumentResolver for F
where
    F: Fn(&str) -> Result<Value, ChallengeProofError>,
{
    fn resolve_did_document(&self, did: &str) -> Result<Value, ChallengeProofError> {
        self(did)
    }
}

#[derive(Clone)]
pub struct StaticDidDocumentResolver {
    did_document: Value,
}

impl StaticDidDocumentResolver {
    pub fn new(did_document: Value) -> Self {
        Self { did_document }
    }
}

impl fmt::Debug for StaticDidDocumentResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StaticDidDocumentResolver")
            .field("did_document", &"[CONFIGURED]")
            .finish()
    }
}

impl DidDocumentResolver for StaticDidDocumentResolver {
    fn resolve_did_document(&self, did: &str) -> Result<Value, ChallengeProofError> {
        if did_from_document(&self.did_document)? != did {
            return Err(ChallengeProofError::DidDocumentMismatch);
        }
        Ok(self.did_document.clone())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ChallengeProofError {
    #[error("DID credential error: {0}")]
    Credential(#[from] DidCredentialError),

    #[error("challenge proof uses an unsupported auth mode")]
    UnsupportedAuthMode,

    #[error("challenge proof uses an unsupported type")]
    UnsupportedProofType,

    #[error("challenge proof payload is invalid")]
    InvalidPayload,

    #[error("challenge proof payload does not match expected challenge")]
    PayloadMismatch,

    #[error("challenge proof method does not match expected method")]
    MethodMismatch,

    #[error("challenge proof audience does not match expected audience")]
    AudienceMismatch,

    #[error("challenge proof timestamp is invalid")]
    InvalidTimestamp,

    #[error("challenge proof is expired")]
    Expired,

    #[error("DID document is not readable")]
    DidDocumentRead,

    #[error("DID document is invalid")]
    InvalidDidDocument,

    #[error("DID private key is not readable")]
    PrivateKeyRead,

    #[error("DID private key is invalid")]
    InvalidPrivateKey,

    #[error("challenge proof serialization failed")]
    Serialization,

    #[error("challenge proof signing failed")]
    SigningFailed,

    #[error("challenge proof is missing signature headers")]
    MissingSignatureHeaders,

    #[error("challenge proof signature metadata is invalid")]
    InvalidSignatureMetadata,

    #[error("challenge proof signature verification failed")]
    SignatureVerificationFailed,

    #[error("challenge proof signer DID is invalid")]
    InvalidSignerDid,

    #[error("challenge proof signer DID does not match user DID")]
    SignerDidMismatch,

    #[error("DID document could not be resolved")]
    DidDocumentResolution,

    #[error("resolved DID document does not match signer DID")]
    DidDocumentMismatch,
}

pub fn sign_challenge_proof<P>(
    payload: &ChallengeProofPayload,
    credential_provider: &P,
    session: &IdentitySession,
    auth_mode: AuthMode,
) -> Result<DockDidChallengeProof, ChallengeProofError>
where
    P: DidCredentialProvider,
{
    if matches!(auth_mode, AuthMode::LegacyDidWba) {
        return Err(ChallengeProofError::UnsupportedAuthMode);
    }
    validate_payload(payload)?;
    validate_payload_matches_session(payload, session)?;

    let credential = credential_provider.credential_for(session)?;
    let did_document = read_did_document(&credential.did_document_path)?;
    if did_from_document(&did_document)? != payload.user_did {
        return Err(ChallengeProofError::SignerDidMismatch);
    }
    let private_key = read_private_key(&credential.private_key_path)?;
    let body = canonical_payload_bytes(payload)?;
    let headers = generate_http_signature_headers(
        &did_document,
        &payload.audience,
        CHALLENGE_PROOF_METHOD,
        &private_key,
        None,
        Some(&body),
        http_signature_options(payload)?,
    )
    .map_err(map_signing_error)?;

    Ok(DockDidChallengeProof {
        proof_type: ChallengeProofKind::AnpHttpSignatureV1,
        method: CHALLENGE_PROOF_METHOD.to_owned(),
        url: payload.audience.clone(),
        headers,
        payload: payload.clone(),
    })
}

pub fn verify_challenge_proof(
    proof: &DockDidChallengeProof,
    expected_payload: &ChallengeProofPayload,
    did_document: &Value,
) -> Result<VerifiedChallengeProof, ChallengeProofError> {
    verify_challenge_proof_at(proof, expected_payload, did_document, current_time_ms()?)
}

pub fn verify_challenge_proof_at(
    proof: &DockDidChallengeProof,
    expected_payload: &ChallengeProofPayload,
    did_document: &Value,
    now_ms: u64,
) -> Result<VerifiedChallengeProof, ChallengeProofError> {
    let resolver = StaticDidDocumentResolver::new(did_document.clone());
    verify_challenge_proof_at_with_resolver(proof, expected_payload, &resolver, now_ms)
}

pub fn verify_challenge_proof_with_resolver<R>(
    proof: &DockDidChallengeProof,
    expected_payload: &ChallengeProofPayload,
    resolver: &R,
) -> Result<VerifiedChallengeProof, ChallengeProofError>
where
    R: DidDocumentResolver,
{
    verify_challenge_proof_at_with_resolver(proof, expected_payload, resolver, current_time_ms()?)
}

pub fn verify_challenge_proof_at_with_resolver<R>(
    proof: &DockDidChallengeProof,
    expected_payload: &ChallengeProofPayload,
    resolver: &R,
    now_ms: u64,
) -> Result<VerifiedChallengeProof, ChallengeProofError>
where
    R: DidDocumentResolver,
{
    if proof.proof_type != ChallengeProofKind::AnpHttpSignatureV1 {
        return Err(ChallengeProofError::UnsupportedProofType);
    }
    validate_payload(expected_payload)?;
    validate_payload(&proof.payload)?;
    if proof.payload != *expected_payload {
        return Err(ChallengeProofError::PayloadMismatch);
    }
    if proof.method != CHALLENGE_PROOF_METHOD {
        return Err(ChallengeProofError::MethodMismatch);
    }
    if proof.url != expected_payload.audience || proof.payload.audience != expected_payload.audience
    {
        return Err(ChallengeProofError::AudienceMismatch);
    }
    validate_payload_time(expected_payload, now_ms)?;

    let metadata =
        extract_signature_metadata(&proof.headers).map_err(map_signature_metadata_error)?;
    validate_signature_metadata(&metadata, expected_payload)?;
    let signer_did = signer_did_from_key_id(&metadata.keyid)?;
    if signer_did != expected_payload.user_did {
        return Err(ChallengeProofError::SignerDidMismatch);
    }

    let did_document = resolver.resolve_did_document(&signer_did)?;
    if did_from_document(&did_document)? != signer_did {
        return Err(ChallengeProofError::DidDocumentMismatch);
    }

    let body = canonical_payload_bytes(&proof.payload)?;
    let verified_metadata = verify_http_message_signature(
        &did_document,
        &proof.method,
        &proof.url,
        &proof.headers,
        Some(&body),
    )
    .map_err(map_signature_verification_error)?;
    validate_signature_metadata(&verified_metadata, expected_payload)?;

    Ok(VerifiedChallengeProof {
        signer_did,
        key_id: verified_metadata.keyid,
        auth_scheme: proof.proof_type.as_str().to_owned(),
    })
}

fn validate_payload(payload: &ChallengeProofPayload) -> Result<(), ChallengeProofError> {
    if payload.challenge_id.trim().is_empty()
        || payload.nonce.trim().is_empty()
        || payload.merchant_did.trim().is_empty()
        || payload.user_did.trim().is_empty()
        || payload.skill_id.trim().is_empty()
        || payload.session_id.trim().is_empty()
        || payload.audience.trim().is_empty()
        || payload
            .agent_did
            .as_deref()
            .is_some_and(|did| did.trim().is_empty())
    {
        return Err(ChallengeProofError::InvalidPayload);
    }
    if payload.issued_at_ms >= payload.expires_at_ms {
        return Err(ChallengeProofError::InvalidTimestamp);
    }
    Ok(())
}

fn validate_payload_matches_session(
    payload: &ChallengeProofPayload,
    session: &IdentitySession,
) -> Result<(), ChallengeProofError> {
    if payload.user_did != session.user_did
        || payload.agent_did != session.agent_did
        || payload.merchant_did != session.merchant_did
        || payload.skill_id != session.skill_id
        || payload.session_id != session.session_id
    {
        return Err(ChallengeProofError::PayloadMismatch);
    }
    Ok(())
}

fn validate_payload_time(
    payload: &ChallengeProofPayload,
    now_ms: u64,
) -> Result<(), ChallengeProofError> {
    if payload.expires_at_ms <= now_ms {
        return Err(ChallengeProofError::Expired);
    }
    if payload.issued_at_ms > now_ms.saturating_add(SIGNATURE_SKEW_MS) {
        return Err(ChallengeProofError::InvalidTimestamp);
    }
    Ok(())
}

fn validate_signature_metadata(
    metadata: &SignatureMetadata,
    payload: &ChallengeProofPayload,
) -> Result<(), ChallengeProofError> {
    if metadata.nonce.as_deref() != Some(payload.nonce.as_str()) {
        return Err(ChallengeProofError::InvalidSignatureMetadata);
    }
    if metadata.created != unix_seconds_floor(payload.issued_at_ms)?
        || metadata.expires != Some(unix_seconds_ceil(payload.expires_at_ms)?)
    {
        return Err(ChallengeProofError::InvalidTimestamp);
    }
    for required in ["@method", "@target-uri", "@authority", "content-digest"] {
        if !metadata
            .components
            .iter()
            .any(|component| component.eq_ignore_ascii_case(required))
        {
            return Err(ChallengeProofError::InvalidSignatureMetadata);
        }
    }
    Ok(())
}

fn canonical_payload_bytes(
    payload: &ChallengeProofPayload,
) -> Result<Vec<u8>, ChallengeProofError> {
    serde_json::to_vec(payload).map_err(|_| ChallengeProofError::Serialization)
}

fn http_signature_options(
    payload: &ChallengeProofPayload,
) -> Result<HttpSignatureOptions, ChallengeProofError> {
    Ok(HttpSignatureOptions {
        keyid: None,
        nonce: Some(payload.nonce.clone()),
        created: Some(unix_seconds_floor(payload.issued_at_ms)?),
        expires: Some(unix_seconds_ceil(payload.expires_at_ms)?),
        covered_components: Some(vec![
            "@method".to_owned(),
            "@target-uri".to_owned(),
            "@authority".to_owned(),
            "content-digest".to_owned(),
        ]),
    })
}

fn unix_seconds_floor(ms: u64) -> Result<i64, ChallengeProofError> {
    i64::try_from(ms / 1_000).map_err(|_| ChallengeProofError::InvalidTimestamp)
}

fn unix_seconds_ceil(ms: u64) -> Result<i64, ChallengeProofError> {
    i64::try_from(ms.div_ceil(1_000)).map_err(|_| ChallengeProofError::InvalidTimestamp)
}

fn read_did_document(path: &std::path::Path) -> Result<Value, ChallengeProofError> {
    let document =
        std::fs::read_to_string(path).map_err(|_| ChallengeProofError::DidDocumentRead)?;
    serde_json::from_str(&document).map_err(|_| ChallengeProofError::InvalidDidDocument)
}

fn read_private_key(path: &std::path::Path) -> Result<PrivateKeyMaterial, ChallengeProofError> {
    let private_key =
        std::fs::read_to_string(path).map_err(|_| ChallengeProofError::PrivateKeyRead)?;
    PrivateKeyMaterial::from_pem(&private_key).map_err(|_| ChallengeProofError::InvalidPrivateKey)
}

fn did_from_document(document: &Value) -> Result<&str, ChallengeProofError> {
    document
        .get("id")
        .and_then(Value::as_str)
        .filter(|did| !did.trim().is_empty())
        .ok_or(ChallengeProofError::InvalidDidDocument)
}

fn signer_did_from_key_id(key_id: &str) -> Result<String, ChallengeProofError> {
    key_id
        .split_once('#')
        .map(|(did, _)| did)
        .filter(|did| !did.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or(ChallengeProofError::InvalidSignerDid)
}

fn current_time_ms() -> Result<u64, ChallengeProofError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ChallengeProofError::InvalidTimestamp)?
        .as_millis();
    u64::try_from(millis).map_err(|_| ChallengeProofError::InvalidTimestamp)
}

fn map_signing_error(error: HttpSignatureError) -> ChallengeProofError {
    match error {
        HttpSignatureError::VerificationMethodNotFound => ChallengeProofError::InvalidDidDocument,
        HttpSignatureError::InvalidSignatureInput => ChallengeProofError::InvalidPayload,
        HttpSignatureError::SigningFailed => ChallengeProofError::SigningFailed,
        _ => ChallengeProofError::SigningFailed,
    }
}

fn map_signature_metadata_error(error: HttpSignatureError) -> ChallengeProofError {
    match error {
        HttpSignatureError::MissingSignatureHeaders => ChallengeProofError::MissingSignatureHeaders,
        _ => ChallengeProofError::InvalidSignatureMetadata,
    }
}

fn map_signature_verification_error(error: HttpSignatureError) -> ChallengeProofError {
    match error {
        HttpSignatureError::MissingSignatureHeaders => ChallengeProofError::MissingSignatureHeaders,
        _ => ChallengeProofError::SignatureVerificationFailed,
    }
}

fn redacted_headers(headers: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    headers
        .iter()
        .map(|(name, value)| {
            let redacted = if name.eq_ignore_ascii_case("Signature")
                || name.eq_ignore_ascii_case("Signature-Input")
                || name.eq_ignore_ascii_case("Authorization")
            {
                "[REDACTED]".to_owned()
            } else {
                value.clone()
            };
            (name.clone(), redacted)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anp::authentication::{create_did_wba_document, DidDocumentOptions};
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn challenge_login_contract_is_camel_case() {
        let request = ChallengeLoginRequest {
            session_id: "session-1".to_owned(),
            skill_id: "coffee".to_owned(),
            user_did: "did:wba:user.example".to_owned(),
            agent_did: Some("did:wba:agent.example".to_owned()),
            merchant_did: "did:wba:merchant.example".to_owned(),
            challenge_id: "challenge-1".to_owned(),
            signed_challenge: json!({"proof": "mock"}),
        };

        let encoded = serde_json::to_value(request).expect("request serializes");

        assert_eq!(encoded["sessionId"], "session-1");
        assert_eq!(encoded["capabilityToken"], Value::Null);
        assert_eq!(encoded["signedChallenge"]["proof"], "mock");
    }

    #[test]
    fn challenge_proof_signs_and_verifies() {
        let fixture = DidFixture::new("user.example");
        let session = fixture.session();
        let payload = fixture.payload();
        let provider = fixture.provider();

        let proof = sign_challenge_proof(&payload, &provider, &session, AuthMode::HttpSignatures)
            .expect("proof signs");
        let encoded = serde_json::to_value(&proof).expect("proof serializes");

        assert_eq!(encoded["type"], CHALLENGE_PROOF_TYPE);
        assert_eq!(encoded["method"], CHALLENGE_PROOF_METHOD);
        assert!(proof.headers.contains_key("Signature"));
        assert!(proof.headers.contains_key("Signature-Input"));
        assert!(proof.headers.contains_key("Content-Digest"));

        let verified = verify_challenge_proof_at(
            &proof,
            &payload,
            &fixture.did_document,
            payload.issued_at_ms + 1_000,
        )
        .expect("proof verifies");

        assert_eq!(verified.signer_did, payload.user_did);
        assert!(verified.key_id.ends_with("#key-1"));
        assert_eq!(verified.auth_scheme, CHALLENGE_PROOF_TYPE);
    }

    #[test]
    fn challenge_proof_rejects_expected_payload_mismatch() {
        let fixture = DidFixture::new("user.example");
        let session = fixture.session();
        let payload = fixture.payload();
        let provider = fixture.provider();
        let proof = sign_challenge_proof(&payload, &provider, &session, AuthMode::HttpSignatures)
            .expect("proof signs");

        for mut expected in [
            payload_with_nonce(&payload, "tampered-nonce"),
            payload_with_skill(&payload, "tea"),
            payload_with_session(&payload, "session-2"),
            payload_with_merchant(&payload, "did:wba:merchant-2.example"),
            payload_with_user(&payload, "did:wba:other-user.example"),
            payload_with_audience(&payload, "https://merchant.example/auth/other"),
        ] {
            expected.issued_at_ms = payload.issued_at_ms;
            expected.expires_at_ms = payload.expires_at_ms;
            let error = verify_challenge_proof_at(
                &proof,
                &expected,
                &fixture.did_document,
                payload.issued_at_ms + 1_000,
            )
            .expect_err("payload mismatch fails");
            assert_eq!(error, ChallengeProofError::PayloadMismatch);
        }
    }

    #[test]
    fn challenge_proof_rejects_tampered_signed_payload() {
        let fixture = DidFixture::new("user.example");
        let session = fixture.session();
        let mut payload = fixture.payload();
        let provider = fixture.provider();
        let mut proof =
            sign_challenge_proof(&payload, &provider, &session, AuthMode::HttpSignatures)
                .expect("proof signs");

        payload.skill_id = "tea".to_owned();
        proof.payload.skill_id = payload.skill_id.clone();

        let error = verify_challenge_proof_at(
            &proof,
            &payload,
            &fixture.did_document,
            payload.issued_at_ms + 1_000,
        )
        .expect_err("tampered body fails signature verification");

        assert_eq!(error, ChallengeProofError::SignatureVerificationFailed);
    }

    #[test]
    fn challenge_proof_rejects_wrong_did_document() {
        let fixture = DidFixture::new("user.example");
        let wrong_fixture = DidFixture::new("wrong-user.example");
        let session = fixture.session();
        let payload = fixture.payload();
        let provider = fixture.provider();
        let proof = sign_challenge_proof(&payload, &provider, &session, AuthMode::HttpSignatures)
            .expect("proof signs");

        let error = verify_challenge_proof_at(
            &proof,
            &payload,
            &wrong_fixture.did_document,
            payload.issued_at_ms + 1_000,
        )
        .expect_err("wrong DID document fails");

        assert_eq!(error, ChallengeProofError::DidDocumentMismatch);
    }

    #[test]
    fn challenge_proof_rejects_missing_signature_headers() {
        let fixture = DidFixture::new("user.example");
        let session = fixture.session();
        let payload = fixture.payload();
        let provider = fixture.provider();
        let mut proof =
            sign_challenge_proof(&payload, &provider, &session, AuthMode::HttpSignatures)
                .expect("proof signs");
        proof.headers.remove("Signature");

        let error = verify_challenge_proof_at(
            &proof,
            &payload,
            &fixture.did_document,
            payload.issued_at_ms + 1_000,
        )
        .expect_err("missing Signature fails");

        assert_eq!(error, ChallengeProofError::MissingSignatureHeaders);
    }

    #[test]
    fn challenge_proof_signing_requires_payload_to_match_session() {
        let fixture = DidFixture::new("user.example");
        let mut session = fixture.session();
        session.skill_id = "tea".to_owned();
        let payload = fixture.payload();
        let provider = fixture.provider();

        let error = sign_challenge_proof(&payload, &provider, &session, AuthMode::HttpSignatures)
            .expect_err("scope mismatch fails before signing");

        assert_eq!(error, ChallengeProofError::PayloadMismatch);
    }

    #[test]
    fn challenge_proof_rejects_signer_did_mismatch() {
        let fixture = DidFixture::new("user.example");
        let mut payload = fixture.payload();
        payload.user_did = "did:wba:other-user.example".to_owned();
        let proof = raw_proof_signed_by_fixture(&payload, &fixture);

        let error = verify_challenge_proof_at(
            &proof,
            &payload,
            &fixture.did_document,
            payload.issued_at_ms + 1_000,
        )
        .expect_err("signer DID mismatch fails");

        assert_eq!(error, ChallengeProofError::SignerDidMismatch);
    }

    #[test]
    fn challenge_proof_rejects_expired_payload() {
        let fixture = DidFixture::new("user.example");
        let session = fixture.session();
        let mut payload = fixture.payload();
        payload.issued_at_ms = 1_000;
        payload.expires_at_ms = 2_000;
        let provider = fixture.provider();
        let proof = sign_challenge_proof(&payload, &provider, &session, AuthMode::HttpSignatures)
            .expect("proof signs");

        let error = verify_challenge_proof_at(&proof, &payload, &fixture.did_document, 2_001)
            .expect_err("expired proof fails");

        assert_eq!(error, ChallengeProofError::Expired);
    }

    #[test]
    fn challenge_proof_debug_and_errors_are_redacted() {
        let fixture = DidFixture::new("user.example");
        let session = fixture.session();
        let payload = fixture.payload();
        let provider = fixture.provider();
        let proof = sign_challenge_proof(&payload, &provider, &session, AuthMode::HttpSignatures)
            .expect("proof signs");
        let raw_signature = proof
            .headers
            .get("Signature")
            .expect("signature header exists");
        let debug = format!("{proof:?}");

        assert!(!debug.contains(raw_signature));
        assert!(!debug.contains(fixture.key_path.to_string_lossy().as_ref()));

        let secret_path = PathBuf::from("/tmp/private-key-secret-token.pem");
        let error = ChallengeProofError::PrivateKeyRead;
        let display = error.to_string();
        let debug = format!("{error:?}");
        assert!(!display.contains(secret_path.to_string_lossy().as_ref()));
        assert!(!debug.contains(secret_path.to_string_lossy().as_ref()));
    }

    fn payload_with_nonce(payload: &ChallengeProofPayload, nonce: &str) -> ChallengeProofPayload {
        let mut payload = payload.clone();
        payload.nonce = nonce.to_owned();
        payload
    }

    fn payload_with_skill(payload: &ChallengeProofPayload, skill: &str) -> ChallengeProofPayload {
        let mut payload = payload.clone();
        payload.skill_id = skill.to_owned();
        payload
    }

    fn payload_with_session(
        payload: &ChallengeProofPayload,
        session: &str,
    ) -> ChallengeProofPayload {
        let mut payload = payload.clone();
        payload.session_id = session.to_owned();
        payload
    }

    fn payload_with_merchant(
        payload: &ChallengeProofPayload,
        merchant: &str,
    ) -> ChallengeProofPayload {
        let mut payload = payload.clone();
        payload.merchant_did = merchant.to_owned();
        payload
    }

    fn payload_with_user(payload: &ChallengeProofPayload, user: &str) -> ChallengeProofPayload {
        let mut payload = payload.clone();
        payload.user_did = user.to_owned();
        payload
    }

    fn payload_with_audience(
        payload: &ChallengeProofPayload,
        audience: &str,
    ) -> ChallengeProofPayload {
        let mut payload = payload.clone();
        payload.audience = audience.to_owned();
        payload
    }

    fn raw_proof_signed_by_fixture(
        payload: &ChallengeProofPayload,
        fixture: &DidFixture,
    ) -> DockDidChallengeProof {
        let body = canonical_payload_bytes(payload).expect("payload serializes");
        let private_key = read_private_key(&fixture.key_path).expect("private key reads");
        let headers = generate_http_signature_headers(
            &fixture.did_document,
            &payload.audience,
            CHALLENGE_PROOF_METHOD,
            &private_key,
            None,
            Some(&body),
            http_signature_options(payload).expect("signature options build"),
        )
        .expect("raw proof signs");

        DockDidChallengeProof {
            proof_type: ChallengeProofKind::AnpHttpSignatureV1,
            method: CHALLENGE_PROOF_METHOD.to_owned(),
            url: payload.audience.clone(),
            headers,
            payload: payload.clone(),
        }
    }

    struct DidFixture {
        _dir: TempDir,
        did_document: Value,
        did_path: PathBuf,
        key_path: PathBuf,
    }

    impl DidFixture {
        fn new(hostname: &str) -> Self {
            let bundle = create_did_wba_document(hostname, DidDocumentOptions::default())
                .expect("DID fixture creates");
            let dir = TempDir::new("anp-miniapp-dock-challenge").expect("temp dir creates");
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

        fn session(&self) -> IdentitySession {
            IdentitySession::new(
                self.did(),
                Some("did:wba:agent.example".to_owned()),
                "did:wba:merchant.example",
                "coffee",
                "session-1",
            )
        }

        fn payload(&self) -> ChallengeProofPayload {
            ChallengeProofPayload {
                challenge_id: "challenge-1".to_owned(),
                nonce: "nonce-1".to_owned(),
                merchant_did: "did:wba:merchant.example".to_owned(),
                user_did: self.did(),
                agent_did: Some("did:wba:agent.example".to_owned()),
                skill_id: "coffee".to_owned(),
                session_id: "session-1".to_owned(),
                audience: "https://merchant.example/agents/coffee/auth/login".to_owned(),
                issued_at_ms: 1_780_000_000_000,
                expires_at_ms: 1_780_000_300_000,
            }
        }

        fn provider(&self) -> crate::did::FileDidCredentialProvider {
            crate::did::FileDidCredentialProvider::new(&self.did_path, &self.key_path)
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
