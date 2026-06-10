use crate::audit::redact_value;
use mcp_schema::ApiDeclaration;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    L0,
    L1,
    L2,
    L3,
    L4,
}

impl RiskLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::L0 => "L0",
            Self::L1 => "L1",
            Self::L2 => "L2",
            Self::L3 => "L3",
            Self::L4 => "L4",
        }
    }

    pub fn requires_consent(self) -> bool {
        self >= Self::L3
    }

    pub fn from_label(label: &str) -> Option<Self> {
        let normalized = label
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_', ' '], "");
        match normalized.as_str() {
            "l0" | "none" | "public" | "query" | "search" | "read" | "low" => Some(Self::L0),
            "l1" | "personalizedquery" | "personalizedread" | "login" | "account" => Some(Self::L1),
            "l2" | "write" | "update" | "cart" | "preference" | "mutation" => Some(Self::L2),
            "l3" | "transaction" | "trade" | "order" | "payment" | "pay" | "refund" => {
                Some(Self::L3)
            }
            "l4" | "privacy" | "private" | "phone" | "address" | "identity" | "credential"
            | "file" | "externalurl" => Some(Self::L4),
            _ => None,
        }
    }
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsentStatus {
    Approved,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsentRequest {
    pub user_did: Option<String>,
    pub agent_did: Option<String>,
    pub merchant_did: Option<String>,
    pub skill_id: String,
    pub session_id: String,
    pub api_name: String,
    pub risk_level: RiskLevel,
    pub requested_at_ms: u64,
    pub parameter_summary: Value,
}

#[derive(Debug, Clone)]
pub struct ConsentRequestInput<'a> {
    pub user_did: Option<String>,
    pub agent_did: Option<String>,
    pub merchant_did: Option<String>,
    pub skill_id: String,
    pub session_id: String,
    pub api_name: String,
    pub risk_level: RiskLevel,
    pub arguments: &'a Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsentProof {
    pub proof_id: String,
    pub user_did: Option<String>,
    pub agent_did: Option<String>,
    pub merchant_did: Option<String>,
    pub skill_id: String,
    pub session_id: String,
    pub api_name: String,
    pub risk_level: RiskLevel,
    pub granted_at_ms: u64,
    pub parameter_summary: Value,
    pub parameter_digest: String,
    pub provider: String,
}

pub trait ConsentProvider {
    fn request_consent(&self, request: &ConsentRequest) -> Result<ConsentStatus, ConsentError>;
}

#[derive(Debug, Clone)]
pub struct DecisionConsentProvider {
    status: ConsentStatus,
}

impl DecisionConsentProvider {
    pub fn approved() -> Self {
        Self {
            status: ConsentStatus::Approved,
        }
    }

    pub fn denied() -> Self {
        Self {
            status: ConsentStatus::Denied,
        }
    }
}

impl ConsentProvider for DecisionConsentProvider {
    fn request_consent(&self, _request: &ConsentRequest) -> Result<ConsentStatus, ConsentError> {
        Ok(self.status.clone())
    }
}

#[derive(Debug, Error)]
pub enum ConsentError {
    #[error("consent denied for `{api_name}`")]
    Denied { api_name: String },

    #[error("consent provider failed: {0}")]
    Provider(String),
}

#[derive(Debug, Clone, Default)]
pub struct RiskPolicy {
    overrides: BTreeMap<String, RiskLevel>,
}

impl RiskPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_override(mut self, api_name: impl Into<String>, level: RiskLevel) -> Self {
        self.overrides.insert(api_name.into(), level);
        self
    }

    pub fn infer_api_risk(&self, declaration: &ApiDeclaration) -> RiskLevel {
        if let Some(level) = self.overrides.get(&declaration.name) {
            return *level;
        }

        risk_from_meta(declaration)
            .or_else(|| risk_from_api_shape(declaration))
            .unwrap_or(RiskLevel::L2)
    }
}

pub fn build_consent_request(input: ConsentRequestInput<'_>) -> ConsentRequest {
    ConsentRequest {
        user_did: input.user_did,
        agent_did: input.agent_did,
        merchant_did: input.merchant_did,
        skill_id: input.skill_id,
        session_id: input.session_id,
        api_name: input.api_name,
        risk_level: input.risk_level,
        requested_at_ms: now_ms(),
        parameter_summary: redact_value(input.arguments),
    }
}

pub fn consent_proof(
    request: &ConsentRequest,
    provider: impl Into<String>,
    parameter_digest: impl Into<String>,
) -> ConsentProof {
    let granted_at_ms = now_ms();
    let parameter_digest = parameter_digest.into();
    let proof_id = format!(
        "consent:{}:{}:{}:{}",
        request.skill_id, request.api_name, granted_at_ms, parameter_digest
    );

    ConsentProof {
        proof_id,
        user_did: request.user_did.clone(),
        agent_did: request.agent_did.clone(),
        merchant_did: request.merchant_did.clone(),
        skill_id: request.skill_id.clone(),
        session_id: request.session_id.clone(),
        api_name: request.api_name.clone(),
        risk_level: request.risk_level,
        granted_at_ms,
        parameter_summary: request.parameter_summary.clone(),
        parameter_digest,
        provider: provider.into(),
    }
}

pub fn parameter_digest(value: &Value) -> String {
    let redacted = redact_value(value);
    let encoded = serde_json::to_vec(&redacted).unwrap_or_default();
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in encoded {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn risk_from_meta(declaration: &ApiDeclaration) -> Option<RiskLevel> {
    declaration
        .meta
        .as_ref()
        .and_then(|meta| {
            meta.anp
                .as_ref()
                .and_then(risk_from_value)
                .or_else(|| risk_from_extra_map(&meta.extra))
                .or_else(|| {
                    meta.extra
                        .get("x_anp")
                        .and_then(risk_from_value)
                        .or_else(|| meta.extra.get("xAnp").and_then(risk_from_value))
                })
        })
        .or_else(|| risk_from_extra_map(&declaration.extra))
        .or_else(|| {
            declaration
                .extra
                .get("x_anp")
                .and_then(risk_from_value)
                .or_else(|| declaration.extra.get("xAnp").and_then(risk_from_value))
        })
}

fn risk_from_extra_map(extra: &BTreeMap<String, Value>) -> Option<RiskLevel> {
    extra
        .get("risk")
        .and_then(risk_from_value)
        .or_else(|| extra.get("riskLevel").and_then(risk_from_value))
}

fn risk_from_value(value: &Value) -> Option<RiskLevel> {
    match value {
        Value::String(label) => RiskLevel::from_label(label),
        Value::Object(map) => risk_from_object(map),
        _ => None,
    }
}

fn risk_from_object(map: &Map<String, Value>) -> Option<RiskLevel> {
    map.get("risk")
        .and_then(risk_from_value)
        .or_else(|| map.get("riskLevel").and_then(risk_from_value))
        .or_else(|| map.get("level").and_then(risk_from_value))
}

fn risk_from_api_shape(declaration: &ApiDeclaration) -> Option<RiskLevel> {
    let name = declaration.name.to_ascii_lowercase();
    let description = declaration.description.to_ascii_lowercase();
    let combined = format!("{name} {description}");

    if contains_any(
        &combined,
        &[
            "phone",
            "address",
            "identity",
            "credential",
            "privacy",
            "private",
            "file",
            "external",
            "手机号",
            "地址",
            "证件",
            "隐私",
        ],
    ) {
        return Some(RiskLevel::L4);
    }

    if contains_any(
        &combined,
        &[
            "pay", "payment", "refund", "order", "checkout", "支付", "退款", "下单", "订单",
        ],
    ) {
        return Some(RiskLevel::L3);
    }

    if contains_any(
        &combined,
        &[
            "update", "set", "create", "confirm", "submit", "write", "修改", "创建", "确认", "提交",
        ],
    ) {
        return Some(RiskLevel::L2);
    }

    if contains_any(
        &combined,
        &["get", "list", "search", "query", "搜索", "查询"],
    ) {
        return Some(RiskLevel::L0);
    }

    None
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
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
    use mcp_schema::ManifestMeta;
    use serde_json::json;

    fn api(name: &str, description: &str, anp: Option<Value>) -> ApiDeclaration {
        ApiDeclaration {
            name: name.to_owned(),
            description: description.to_owned(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            meta: Some(ManifestMeta {
                anp,
                ..ManifestMeta::default()
            }),
            extra: Default::default(),
        }
    }

    #[test]
    fn recognizes_existing_coffee_risk_labels() {
        let policy = RiskPolicy::new();

        assert_eq!(
            policy.infer_api_risk(&api("searchDrinks", "search", Some(json!({"risk": "low"})))),
            RiskLevel::L0
        );
        assert_eq!(
            policy.infer_api_risk(&api(
                "confirmOrder",
                "confirm",
                Some(json!({"risk": "order"}))
            )),
            RiskLevel::L3
        );
        assert_eq!(
            policy.infer_api_risk(&api("payOrder", "pay", Some(json!({"risk": "payment"})))),
            RiskLevel::L3
        );
    }

    #[test]
    fn defaults_unknown_api_to_write_risk() {
        let policy = RiskPolicy::new();

        assert_eq!(
            policy.infer_api_risk(&api("custom", "custom action", None)),
            RiskLevel::L2
        );
    }

    #[test]
    fn proof_uses_redacted_parameter_digest() {
        let arguments = json!({"orderId": "order-1", "token": "secret-token"});
        let request = build_consent_request(ConsentRequestInput {
            user_did: Some("did:wba:user.example".to_owned()),
            agent_did: None,
            merchant_did: None,
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

        assert_eq!(proof.user_did.as_deref(), Some("did:wba:user.example"));
        assert_eq!(proof.api_name, "payOrder");
        assert_eq!(proof.risk_level, RiskLevel::L3);
        assert_eq!(proof.parameter_summary["token"], "[REDACTED]");
        assert!(!proof.parameter_digest.contains("secret-token"));
    }
}
