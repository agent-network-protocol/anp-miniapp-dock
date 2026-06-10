use consent_audit::{
    build_consent_request, consent_proof, parameter_digest, redact_value, AuditOutcome,
    AuditRecord, AuditRecordInput, ConsentRequestInput, ConsentStatus, DecisionConsentProvider,
    RiskLevel, RiskPolicy,
};
use consent_audit::{AuditSink, ConsentProvider};
use mcp_schema::{ApiDeclaration, ManifestMeta};
use serde_json::json;

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

    assert_eq!(proof.parameter_summary["token"], "[REDACTED]");
    assert_eq!(record.parameter_summary["privateNote"], "[REDACTED]");
    assert_eq!(record.parameter_summary["deliveryAddress"], "[REDACTED]");
    assert!(!encoded.contains("real-token"));
    assert!(!encoded.contains("do not store"));
    assert!(!encoded.contains("1 Private Road"));
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
