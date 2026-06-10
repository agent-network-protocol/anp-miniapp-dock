use crate::consent::{ConsentProof, RiskLevel};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const REDACTED: &str = "[REDACTED]";
const MAX_STRING_LEN: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Ok,
    BlockedConsentRequired,
    BlockedPermissionDenied,
    ValidationFailed,
    Error,
}

impl AuditOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::BlockedConsentRequired => "blocked_consent_required",
            Self::BlockedPermissionDenied => "blocked_permission_denied",
            Self::ValidationFailed => "validation_failed",
            Self::Error => "error",
        }
    }
}

impl std::fmt::Display for AuditOutcome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

pub trait AuditSink {
    fn record(&self, record: AuditRecord) -> Result<(), AuditError>;
}

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("audit sink failed: {0}")]
    Sink(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRecord {
    pub user_did: Option<String>,
    pub agent_did: Option<String>,
    pub merchant_did: Option<String>,
    pub session_id: String,
    pub skill_id: String,
    pub api_name: String,
    pub risk_level: RiskLevel,
    pub parameter_summary: Value,
    pub consent_proof: Option<ConsentProof>,
    pub outcome: AuditOutcome,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone)]
pub struct AuditRecordInput<'a> {
    pub user_did: Option<String>,
    pub agent_did: Option<String>,
    pub merchant_did: Option<String>,
    pub session_id: String,
    pub skill_id: String,
    pub api_name: String,
    pub risk_level: RiskLevel,
    pub arguments: &'a Value,
    pub consent_proof: Option<ConsentProof>,
    pub outcome: AuditOutcome,
}

impl AuditRecord {
    pub fn new(input: AuditRecordInput<'_>) -> Self {
        Self {
            user_did: input.user_did,
            agent_did: input.agent_did,
            merchant_did: input.merchant_did,
            session_id: input.session_id,
            skill_id: input.skill_id,
            api_name: input.api_name,
            risk_level: input.risk_level,
            parameter_summary: redact_value(input.arguments),
            consent_proof: input.consent_proof,
            outcome: input.outcome,
            occurred_at_ms: now_ms(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAuditSink {
    records: Arc<Mutex<Vec<AuditRecord>>>,
}

impl InMemoryAuditSink {
    pub fn records(&self) -> Vec<AuditRecord> {
        self.records
            .lock()
            .expect("audit sink mutex poisoned")
            .clone()
    }
}

impl AuditSink for InMemoryAuditSink {
    fn record(&self, record: AuditRecord) -> Result<(), AuditError> {
        let mut records = self.records.lock().expect("audit sink mutex poisoned");
        records.push(record);
        Ok(())
    }
}

pub fn redact_value(value: &Value) -> Value {
    redact_value_at_key(None, value)
}

fn redact_value_at_key(key: Option<&str>, value: &Value) -> Value {
    if key.is_some_and(is_sensitive_key) {
        return Value::String(REDACTED.to_owned());
    }

    match value {
        Value::Object(map) => Value::Object(redact_object(map)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| redact_value_at_key(None, item))
                .collect(),
        ),
        Value::String(text) => Value::String(redact_string(text)),
        _ => value.clone(),
    }
}

fn redact_object(map: &Map<String, Value>) -> Map<String, Value> {
    map.iter()
        .map(|(key, value)| (key.clone(), redact_value_at_key(Some(key), value)))
        .collect()
}

fn redact_string(text: &str) -> String {
    if text.chars().count() <= MAX_STRING_LEN {
        return text.to_owned();
    }

    let mut truncated: String = text.chars().take(MAX_STRING_LEN).collect();
    truncated.push_str("...[TRUNCATED]");
    truncated
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    [
        "token",
        "secret",
        "private",
        "privacy",
        "password",
        "authorization",
        "authheader",
        "cookie",
        "sessionkey",
        "signature",
        "signingkey",
        "privatekey",
        "credential",
        "phone",
        "mobile",
        "address",
        "idcard",
        "identity",
        "passport",
        "filecontent",
        "document",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn now_ms() -> u64 {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_tokens_private_fields_and_privacy_data() {
        let redacted = redact_value(&json!({
            "orderId": "order-1",
            "capabilityToken": "real-token",
            "privateComponentState": {"secret": "value"},
            "deliveryAddress": "1 Private Road",
            "phoneNumber": "1234567890",
            "items": [{"name": "latte", "sessionKey": "abc"}]
        }));

        assert_eq!(redacted["orderId"], "order-1");
        assert_eq!(redacted["capabilityToken"], REDACTED);
        assert_eq!(redacted["privateComponentState"], REDACTED);
        assert_eq!(redacted["deliveryAddress"], REDACTED);
        assert_eq!(redacted["phoneNumber"], REDACTED);
        assert_eq!(redacted["items"][0]["name"], "latte");
        assert_eq!(redacted["items"][0]["sessionKey"], REDACTED);
    }

    #[test]
    fn audit_record_stores_only_redacted_parameters() {
        let arguments = json!({"orderId": "order-1", "token": "real-token"});
        let record = AuditRecord::new(AuditRecordInput {
            user_did: Some("did:wba:user.example".to_owned()),
            agent_did: None,
            merchant_did: Some("did:wba:merchant.example".to_owned()),
            session_id: "session-1".to_owned(),
            skill_id: "coffee".to_owned(),
            api_name: "payOrder".to_owned(),
            risk_level: RiskLevel::L3,
            arguments: &arguments,
            consent_proof: None,
            outcome: AuditOutcome::Ok,
        });

        assert_eq!(record.parameter_summary["orderId"], "order-1");
        assert_eq!(record.parameter_summary["token"], REDACTED);
    }
}
