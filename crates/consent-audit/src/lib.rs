#![doc = "High-risk action consent and audit trail crate."]

pub mod audit;
pub mod consent;

pub use audit::{
    redact_value, AuditError, AuditExportReport, AuditOutcome, AuditPersistenceProfile, AuditQuery,
    AuditRecord, AuditRecordInput, AuditRedaction, AuditRetentionReport, AuditSink,
    AuditUnavailablePolicy, FileAuditSink, InMemoryAuditSink,
};
pub use consent::{
    build_consent_request, consent_prompt_digest, consent_proof, consent_proof_with_decision,
    parameter_digest, ConsentError, ConsentProof, ConsentProvider, ConsentRequest,
    ConsentRequestInput, ConsentStatus, DecisionConsentProvider, HostConsentAdapter,
    HostConsentDecision, RiskLevel, RiskPolicy, UnavailableConsentProvider, CONSENT_POLICY_VERSION,
    DEV_HEADLESS_CONSENT_PROVIDER, DEV_HEADLESS_DECISION_ACTOR,
};
