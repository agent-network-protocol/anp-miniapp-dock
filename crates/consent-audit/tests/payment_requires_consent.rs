use consent_audit::{
    build_consent_request, consent_proof, parameter_digest, redact_value, AuditOutcome,
    AuditPersistenceProfile, AuditQuery, AuditRecord, AuditRecordInput, ConsentError,
    ConsentRequestInput, ConsentStatus, DecisionConsentProvider, FileAuditSink, RiskLevel,
    RiskPolicy, UnavailableConsentProvider, CONSENT_POLICY_VERSION, DEV_HEADLESS_CONSENT_PROVIDER,
    DEV_HEADLESS_DECISION_ACTOR,
};
use consent_audit::{AuditSink, ConsentProvider, HostConsentAdapter};
use mcp_schema::{ApiDeclaration, ManifestMeta};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

#[test]
fn payment_policy_requires_human_consent() {
    let declaration = ApiDeclaration {
        name: "payOrder".to_owned(),
        description: "对待支付订单执行 mock 支付".to_owned(),
        input_schema: json!({"type": "object"}),
        output_schema: None,
        meta: Some(ManifestMeta {
            anp: Some(json!({"risk": "payment"})),
            ..ManifestMeta::default()
        }),
        extra: Default::default(),
    };

    let risk = RiskPolicy::new().infer_api_risk(&declaration);

    assert_eq!(risk, RiskLevel::L3);
    assert!(risk.requires_consent());
}

#[test]
fn mock_provider_can_deny_or_approve_payment() {
    let arguments = json!({"orderId": "order-1", "capabilityToken": "real-token"});
    let request = build_consent_request(ConsentRequestInput {
        user_did: Some("did:wba:user.example".to_owned()),
        agent_did: Some("did:wba:agent.example".to_owned()),
        merchant_did: Some("did:wba:merchant.example".to_owned()),
        skill_id: "coffee".to_owned(),
        session_id: "session-1".to_owned(),
        api_name: "payOrder".to_owned(),
        risk_level: RiskLevel::L3,
        arguments: &arguments,
    });

    assert_eq!(
        DecisionConsentProvider::denied()
            .request_consent(&request)
            .expect("provider responds"),
        ConsentStatus::Denied
    );
    assert_eq!(
        DecisionConsentProvider::approved()
            .request_consent(&request)
            .expect("provider responds"),
        ConsentStatus::Approved
    );
}

#[test]
fn host_consent_adapter_reports_provider_actor_and_unavailable() {
    let arguments = json!({"orderId": "order-1"});
    let request = build_consent_request(ConsentRequestInput {
        user_did: Some("did:wba:user.example".to_owned()),
        agent_did: Some("did:wba:agent.example".to_owned()),
        merchant_did: Some("did:wba:merchant.example".to_owned()),
        skill_id: "coffee".to_owned(),
        session_id: "session-1".to_owned(),
        api_name: "payOrder".to_owned(),
        risk_level: RiskLevel::L3,
        arguments: &arguments,
    });
    let provider = DecisionConsentProvider::approved();

    let decision = provider
        .request_host_consent(&request)
        .expect("dev adapter responds");

    assert_eq!(decision.status, ConsentStatus::Approved);
    assert_eq!(decision.provider, DEV_HEADLESS_CONSENT_PROVIDER);
    assert_eq!(decision.decision_actor, DEV_HEADLESS_DECISION_ACTOR);
    assert_eq!(
        provider
            .request_consent(&request)
            .expect("legacy trait works"),
        ConsentStatus::Approved
    );
    assert!(matches!(
        UnavailableConsentProvider::new("host-ui")
            .request_host_consent(&request)
            .expect_err("unavailable provider fails closed"),
        ConsentError::ProviderUnavailable { .. }
    ));
}

#[test]
fn consent_proof_and_audit_record_are_redacted() {
    let arguments = json!({
        "orderId": "order-1",
        "token": "real-token",
        "privateNote": "do not store",
        "deliveryAddress": "1 Private Road"
    });
    let request = build_consent_request(ConsentRequestInput {
        user_did: Some("did:wba:user.example".to_owned()),
        agent_did: Some("did:wba:agent.example".to_owned()),
        merchant_did: Some("did:wba:merchant.example".to_owned()),
        skill_id: "coffee".to_owned(),
        session_id: "session-1".to_owned(),
        api_name: "payOrder".to_owned(),
        risk_level: RiskLevel::L3,
        arguments: &arguments,
    });
    let proof = consent_proof(
        &request,
        "mock",
        parameter_digest(&request.parameter_summary),
    );
    let record = AuditRecord::new(AuditRecordInput {
        user_did: request.user_did.clone(),
        agent_did: request.agent_did.clone(),
        merchant_did: request.merchant_did.clone(),
        session_id: request.session_id.clone(),
        skill_id: request.skill_id.clone(),
        api_name: request.api_name.clone(),
        risk_level: request.risk_level,
        arguments: &arguments,
        consent_proof: Some(proof.clone()),
        outcome: AuditOutcome::Ok,
    });
    let encoded = serde_json::to_string(&record).expect("audit record serializes");

    assert_eq!(proof.policy_version, CONSENT_POLICY_VERSION);
    assert!(!proof.prompt_digest.is_empty());
    assert_eq!(proof.provider, "mock");
    assert_eq!(proof.decision_actor, DEV_HEADLESS_DECISION_ACTOR);
    assert!(proof.granted_at_ms > 0);
    assert!(!proof.parameter_digest.is_empty());
    assert_eq!(proof.parameter_summary["token"], "[REDACTED]");
    assert_eq!(record.parameter_summary["privateNote"], "[REDACTED]");
    assert_eq!(record.parameter_summary["deliveryAddress"], "[REDACTED]");
    assert!(!encoded.contains("real-token"));
    assert!(!encoded.contains("do not store"));
    assert!(!encoded.contains("1 Private Road"));
}

#[test]
fn file_audit_sink_persists_queries_retains_and_exports_redacted_records() {
    let fixture = TempDir::new("consent-audit-jsonl");
    let path = fixture.path().join("audit").join("records.jsonl");
    let sink = FileAuditSink::new(&path);
    let first_arguments = json!({
        "orderId": "order-1",
        "capabilityToken": "real-token",
        "Authorization": "Bearer real-token",
        "httpSignature": "sig-real",
        "privateKeyPath": "/tmp/secret.pem",
        "phoneNumber": "1234567890",
        "deliveryAddress": "1 Private Road",
        "fileContent": "private file"
    });
    let second_arguments = json!({"orderId": "order-2", "token": "another-token"});
    sink.record(AuditRecord::new(AuditRecordInput {
        user_did: Some("did:wba:user.example".to_owned()),
        agent_did: None,
        merchant_did: Some("did:wba:merchant.example".to_owned()),
        session_id: "session-1".to_owned(),
        skill_id: "coffee".to_owned(),
        api_name: "payOrder".to_owned(),
        risk_level: RiskLevel::L3,
        arguments: &first_arguments,
        consent_proof: None,
        outcome: AuditOutcome::Ok,
    }))
    .expect("first audit record stores");
    sink.record(AuditRecord::new(AuditRecordInput {
        user_did: Some("did:wba:user.example".to_owned()),
        agent_did: None,
        merchant_did: Some("did:wba:merchant.example".to_owned()),
        session_id: "session-2".to_owned(),
        skill_id: "coffee".to_owned(),
        api_name: "confirmOrder".to_owned(),
        risk_level: RiskLevel::L3,
        arguments: &second_arguments,
        consent_proof: None,
        outcome: AuditOutcome::BlockedConsentRequired,
    }))
    .expect("second audit record stores");

    let restarted = FileAuditSink::new(&path);
    let pay_records = restarted
        .query(AuditQuery::new().api_name("payOrder"))
        .expect("query reads persisted records");
    assert_eq!(pay_records.len(), 1);
    assert_eq!(pay_records[0].parameter_summary["orderId"], "order-1");
    assert_eq!(
        pay_records[0].parameter_summary["capabilityToken"],
        "[REDACTED]"
    );

    let export = restarted
        .export_redacted_json(AuditQuery::new().skill_id("coffee"))
        .expect("export serializes");
    let report = restarted
        .export_redacted_report(AuditQuery::new().skill_id("coffee"))
        .expect("export report serializes");
    assert_eq!(
        report.backend_profile,
        AuditPersistenceProfile::LocalFileJsonl
    );
    assert!(!report.production_ready);
    assert_eq!(report.exported_count, 2);
    assert!(!report.redaction.raw_parameter_visible);
    assert!(!report.redaction.raw_consent_proof_visible);
    let exported = serde_json::to_string(&export).expect("export stringifies");
    let exported_report = serde_json::to_string(&report).expect("report stringifies");
    for raw in [
        "real-token",
        "Bearer real-token",
        "sig-real",
        "/tmp/secret.pem",
        "1234567890",
        "1 Private Road",
        "private file",
        "another-token",
    ] {
        assert!(
            !exported.contains(raw),
            "export should not contain raw sensitive value {raw}"
        );
        assert!(
            !exported_report.contains(raw),
            "export report should not contain raw sensitive value {raw}"
        );
    }
    for redacted_key in [
        "capabilityToken",
        "Authorization",
        "httpSignature",
        "privateKeyPath",
        "phoneNumber",
        "deliveryAddress",
        "fileContent",
    ] {
        assert!(
            exported.contains(redacted_key),
            "export should keep redacted key {redacted_key}"
        );
    }

    let max_seen = restarted
        .records()
        .expect("records read")
        .into_iter()
        .map(|record| record.occurred_at_ms)
        .max()
        .expect("records exist");
    let retention = restarted
        .retain_since_report(max_seen + 1)
        .expect("retention rewrites audit file");
    assert_eq!(
        retention.backend_profile,
        AuditPersistenceProfile::LocalFileJsonl
    );
    assert!(!retention.production_ready);
    assert_eq!(retention.before_count, 2);
    assert_eq!(retention.retained_count, 0);
    assert_eq!(retention.removed_count, 2);
    let retained_records = restarted.records().expect("retained records read");
    assert!(retained_records.is_empty());
}

#[test]
fn audit_persistence_profiles_mark_only_host_or_encrypted_backends_production_ready() {
    assert!(!AuditPersistenceProfile::InMemoryDev.production_ready());
    assert!(!AuditPersistenceProfile::LocalFileJsonl.production_ready());
    assert!(AuditPersistenceProfile::HostPersistentSink.production_ready());
    assert!(AuditPersistenceProfile::EncryptedSqlite.production_ready());
}

#[test]
fn file_audit_export_redacts_legacy_raw_records() {
    let fixture = TempDir::new("consent-audit-legacy-jsonl");
    let path = fixture.path().join("records.jsonl");
    fs::write(
        &path,
        r#"{"userDid":null,"agentDid":null,"merchantDid":null,"sessionId":"session-1","skillId":"coffee","apiName":"payOrder","riskLevel":"L3","parameterSummary":{"token":"legacy-token","phoneNumber":"1234567890","deliveryAddress":"1 Private Road"},"consentProof":null,"outcome":"ok","occurredAtMs":1}
"#,
    )
    .expect("write legacy audit record");

    let export = FileAuditSink::new(&path)
        .export_redacted_json(AuditQuery::new())
        .expect("legacy export succeeds");
    let exported = serde_json::to_string(&export).expect("export stringifies");

    assert!(!exported.contains("legacy-token"));
    assert!(!exported.contains("1234567890"));
    assert!(!exported.contains("1 Private Road"));
    assert!(exported.contains("[REDACTED]"));
}

#[test]
fn in_memory_audit_sink_keeps_redacted_records() {
    let sink = consent_audit::InMemoryAuditSink::default();
    let arguments = json!({"orderId": "order-1", "token": "real-token"});
    sink.record(AuditRecord::new(AuditRecordInput {
        user_did: None,
        agent_did: None,
        merchant_did: None,
        session_id: "session-1".to_owned(),
        skill_id: "coffee".to_owned(),
        api_name: "payOrder".to_owned(),
        risk_level: RiskLevel::L3,
        arguments: &arguments,
        consent_proof: None,
        outcome: AuditOutcome::BlockedConsentRequired,
    }))
    .expect("audit record stores");

    let records = sink.records();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].parameter_summary["orderId"], "order-1");
    assert_eq!(records[0].parameter_summary["token"], "[REDACTED]");
    assert_eq!(
        redact_value(&json!({"phoneNumber": "123"}))["phoneNumber"],
        "[REDACTED]"
    );
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{prefix}-{}", unique_suffix()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn unique_suffix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    format!("{}-{nanos}", std::process::id())
}
