#![doc = "High-risk action consent and audit trail crate."]

pub mod audit;
pub mod consent;

pub use audit::{
    redact_value, AuditError, AuditOutcome, AuditRecord, AuditRecordInput, AuditSink,
    InMemoryAuditSink,
};
pub use consent::{
    build_consent_request, consent_proof, parameter_digest, ConsentError, ConsentProof,
    ConsentProvider, ConsentRequest, ConsentRequestInput, ConsentStatus, DecisionConsentProvider,
    RiskLevel, RiskPolicy,
};
