use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const REDACTED: &str = "[REDACTED]";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRecord {
    pub at_ms: u64,
    pub event: String,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_did: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_summary: Option<Value>,
}

impl AuditRecord {
    pub fn new(event: impl Into<String>, outcome: impl Into<String>) -> Self {
        Self {
            at_ms: now_ms(),
            event: event.into(),
            outcome: outcome.into(),
            session_id: None,
            skill_id: None,
            user_did: None,
            order_id: None,
            parameter_summary: None,
        }
    }

    pub fn with_scope(
        mut self,
        session_id: Option<String>,
        skill_id: Option<String>,
        user_did: Option<String>,
    ) -> Self {
        self.session_id = session_id;
        self.skill_id = skill_id;
        self.user_did = user_did;
        self
    }

    pub fn with_order(mut self, order_id: impl Into<String>) -> Self {
        self.order_id = Some(order_id.into());
        self
    }

    pub fn with_parameters(mut self, value: &Value) -> Self {
        self.parameter_summary = Some(redact_value(value));
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct AuditLog {
    records: Arc<Mutex<Vec<AuditRecord>>>,
}

impl AuditLog {
    pub fn record(&self, record: AuditRecord) {
        if let Ok(mut records) = self.records.lock() {
            records.push(record);
        }
    }

    pub fn records(&self) -> Vec<AuditRecord> {
        self.records
            .lock()
            .map(|records| records.clone())
            .unwrap_or_default()
    }
}

pub fn redact_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let redacted = map
                .iter()
                .map(|(key, value)| {
                    if is_sensitive_key(key) {
                        (key.clone(), Value::String(REDACTED.to_owned()))
                    } else {
                        (key.clone(), redact_value(value))
                    }
                })
                .collect::<Map<_, _>>();
            Value::Object(redacted)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_value).collect()),
        value => value.clone(),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "authorization",
        "token",
        "private",
        "secret",
        "phone",
        "address",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

pub fn now_ms() -> u64 {
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
    fn audit_log_redacts_token_like_values() {
        let record = AuditRecord::new("pay", "ok")
            .with_parameters(&json!({"capabilityToken": "real", "orderId": "order-1"}));

        assert_eq!(
            record.parameter_summary.unwrap()["capabilityToken"],
            Value::String(REDACTED.to_owned())
        );
    }
}
