#![doc = "ANP DID, signed HTTP, challenge, and capability token adapter crate."]

pub mod challenge;
pub mod did;
pub mod session;
pub mod signed_request;
pub mod token;

pub use challenge::{
    sign_challenge_proof, verify_challenge_proof, verify_challenge_proof_at,
    verify_challenge_proof_at_with_resolver,
    verify_challenge_proof_at_with_resolver_and_nonce_store, verify_challenge_proof_with_resolver,
    ChallengeLoginRequest, ChallengeLoginResponse, ChallengeNonceStore, ChallengeProofError,
    ChallengeProofKind, ChallengeProofPayload, DidChallenge, DidDocumentResolver,
    DockDidChallengeProof, InMemoryChallengeNonceStore, StaticDidDocumentResolver,
    TrustedDidDocumentResolver, VerifiedChallengeProof, CHALLENGE_PROOF_METHOD,
    CHALLENGE_PROOF_TYPE,
};
pub use did::{
    DidCredential, DidCredentialConfig, DidCredentialError, DidCredentialProvider,
    FileDidCredentialProvider, IdentitySession,
};
pub use session::{
    decode_capability_token_scopes, DidAuthReceipt, DidAuthSession, DidAuthSessionError,
    DidAuthSessionKey, DidAuthSessionManager,
};
pub use signed_request::{
    redact_for_log, AnpHttpClient, AnpRequestBroker, AnpRequestError, AuthMaterial, HttpTransport,
    NetworkAllowlistDenial, NetworkAllowlistRule, ReqwestHttpTransport, SignedRequestPolicy,
    TransportRequest, TransportResponse,
};
pub use token::{
    bearer_token_expiry_ms, CapabilityToken, CapabilityTokenCache, CapabilityTokenClaims,
    CapabilityTokenError, CapabilityTokenIssueOutcome, CapabilityTokenIssuer,
    CapabilityTokenIssuerConfig, CapabilityTokenLifecycleMode, CapabilityTokenLifecycleStore,
    CapabilityTokenRequest, CapabilityTokenScope, CapabilityTokenScopeSummary,
    CapabilityTokenVerifier, CapabilityTokenVerifierConfig, ExpectedCapability,
    ExpectedCapabilitySubject, InMemoryTokenCache, InMemoryTokenCachePersistenceBackend,
    InMemoryTokenLifecycleStore, PersistentCapabilityTokenCache, PersistentCapabilityTokenEntry,
    TokenCachePersistenceBackend, TokenCachePersistenceProfile, TokenCacheRedaction,
    TokenCacheRestoreRejection, TokenCacheRestoreRejectionReason, TokenCacheRestoreReport,
    CAPABILITY_TOKEN_SCOPE_DERIVATION_SOURCE, CAPABILITY_TOKEN_VERSION,
    DEFAULT_CAPABILITY_TOKEN_TTL_MS,
};
