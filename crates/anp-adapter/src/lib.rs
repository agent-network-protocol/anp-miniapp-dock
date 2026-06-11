#![doc = "ANP DID, signed HTTP, challenge, and capability token adapter crate."]

pub mod challenge;
pub mod did;
pub mod signed_request;
pub mod token;

pub use challenge::{
    sign_challenge_proof, verify_challenge_proof, verify_challenge_proof_at,
    verify_challenge_proof_at_with_resolver, verify_challenge_proof_with_resolver,
    ChallengeLoginRequest, ChallengeLoginResponse, ChallengeProofError, ChallengeProofKind,
    ChallengeProofPayload, DidChallenge, DidDocumentResolver, DockDidChallengeProof,
    StaticDidDocumentResolver, VerifiedChallengeProof, CHALLENGE_PROOF_METHOD,
    CHALLENGE_PROOF_TYPE,
};
pub use did::{
    DidCredential, DidCredentialConfig, DidCredentialError, DidCredentialProvider,
    FileDidCredentialProvider, IdentitySession,
};
pub use signed_request::{
    redact_for_log, AnpHttpClient, AnpRequestBroker, AnpRequestError, AuthMaterial, HttpTransport,
    ReqwestHttpTransport, SignedRequestPolicy, TransportRequest, TransportResponse,
};
pub use token::{
    bearer_token_expiry_ms, CapabilityToken, CapabilityTokenCache, CapabilityTokenClaims,
    CapabilityTokenError, CapabilityTokenIssueOutcome, CapabilityTokenIssuer,
    CapabilityTokenIssuerConfig, CapabilityTokenRequest, CapabilityTokenScope,
    CapabilityTokenVerifier, CapabilityTokenVerifierConfig, ExpectedCapability,
    ExpectedCapabilitySubject, InMemoryTokenCache, CAPABILITY_TOKEN_VERSION,
    DEFAULT_CAPABILITY_TOKEN_TTL_MS,
};
