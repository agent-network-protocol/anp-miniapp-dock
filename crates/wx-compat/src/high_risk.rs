use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::Cell;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HighRiskApiKind {
    PhoneNumber,
    Address,
    Location,
    Media,
    File,
    Payment,
    Scan,
    PhoneCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HighRiskLevel {
    L3,
    L4,
}

impl HighRiskLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::L3 => "L3",
            Self::L4 => "L4",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighRiskApiSpec {
    pub name: &'static str,
    pub kind: HighRiskApiKind,
    pub risk_level: HighRiskLevel,
    pub requires_consent: bool,
    pub dev_only_allowed: bool,
}

pub const HIGH_RISK_API_SPECS: &[HighRiskApiSpec] = &[
    HighRiskApiSpec {
        name: "getPhoneNumber",
        kind: HighRiskApiKind::PhoneNumber,
        risk_level: HighRiskLevel::L4,
        requires_consent: true,
        dev_only_allowed: true,
    },
    HighRiskApiSpec {
        name: "chooseAddress",
        kind: HighRiskApiKind::Address,
        risk_level: HighRiskLevel::L4,
        requires_consent: true,
        dev_only_allowed: true,
    },
    HighRiskApiSpec {
        name: "getLocation",
        kind: HighRiskApiKind::Location,
        risk_level: HighRiskLevel::L4,
        requires_consent: true,
        dev_only_allowed: true,
    },
    HighRiskApiSpec {
        name: "getFuzzyLocation",
        kind: HighRiskApiKind::Location,
        risk_level: HighRiskLevel::L4,
        requires_consent: true,
        dev_only_allowed: true,
    },
    HighRiskApiSpec {
        name: "chooseLocation",
        kind: HighRiskApiKind::Location,
        risk_level: HighRiskLevel::L4,
        requires_consent: true,
        dev_only_allowed: true,
    },
    HighRiskApiSpec {
        name: "chooseMedia",
        kind: HighRiskApiKind::Media,
        risk_level: HighRiskLevel::L4,
        requires_consent: true,
        dev_only_allowed: true,
    },
    HighRiskApiSpec {
        name: "chooseMessageFile",
        kind: HighRiskApiKind::File,
        risk_level: HighRiskLevel::L4,
        requires_consent: true,
        dev_only_allowed: true,
    },
    HighRiskApiSpec {
        name: "requestPayment",
        kind: HighRiskApiKind::Payment,
        risk_level: HighRiskLevel::L3,
        requires_consent: true,
        dev_only_allowed: true,
    },
    HighRiskApiSpec {
        name: "requestVirtualPayment",
        kind: HighRiskApiKind::Payment,
        risk_level: HighRiskLevel::L3,
        requires_consent: true,
        dev_only_allowed: true,
    },
    HighRiskApiSpec {
        name: "requestJointPayment",
        kind: HighRiskApiKind::Payment,
        risk_level: HighRiskLevel::L3,
        requires_consent: true,
        dev_only_allowed: true,
    },
    HighRiskApiSpec {
        name: "scanCode",
        kind: HighRiskApiKind::Scan,
        risk_level: HighRiskLevel::L4,
        requires_consent: true,
        dev_only_allowed: true,
    },
    HighRiskApiSpec {
        name: "makePhoneCall",
        kind: HighRiskApiKind::PhoneCall,
        risk_level: HighRiskLevel::L4,
        requires_consent: true,
        dev_only_allowed: true,
    },
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighRiskApiRequest {
    pub api_name: String,
    pub options: Value,
}

impl HighRiskApiRequest {
    pub fn new(api_name: impl Into<String>, options: Value) -> Self {
        Self {
            api_name: api_name.into(),
            options,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighRiskApiSuccess {
    pub data: Value,
    pub dev_only: bool,
    pub audit_summary: Value,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum HighRiskApiError {
    #[error("high-risk api requires consent")]
    ConsentRequired,

    #[error("high-risk api provider is unavailable")]
    ProviderUnavailable,

    #[error("high-risk api provider denied the request")]
    PermissionDenied,

    #[error("high-risk api options are invalid")]
    InvalidOptions,
}

pub trait HighRiskHostProvider {
    fn call_high_risk_api(
        &self,
        spec: &HighRiskApiSpec,
        request: &HighRiskApiRequest,
    ) -> Result<HighRiskApiSuccess, HighRiskApiError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UnavailableHighRiskHostProvider;

impl HighRiskHostProvider for UnavailableHighRiskHostProvider {
    fn call_high_risk_api(
        &self,
        _spec: &HighRiskApiSpec,
        _request: &HighRiskApiRequest,
    ) -> Result<HighRiskApiSuccess, HighRiskApiError> {
        Err(HighRiskApiError::ProviderUnavailable)
    }
}

#[derive(Debug, Default)]
pub struct DevOnlyHighRiskHostProvider {
    calls: Cell<usize>,
}

impl DevOnlyHighRiskHostProvider {
    pub fn calls(&self) -> usize {
        self.calls.get()
    }
}

impl HighRiskHostProvider for DevOnlyHighRiskHostProvider {
    fn call_high_risk_api(
        &self,
        spec: &HighRiskApiSpec,
        request: &HighRiskApiRequest,
    ) -> Result<HighRiskApiSuccess, HighRiskApiError> {
        self.calls.set(self.calls.get() + 1);
        let data = match spec.kind {
            HighRiskApiKind::PhoneNumber => json!({
                "phoneNumberToken": "mock-phone-token",
                "devOnly": true,
            }),
            HighRiskApiKind::Address => json!({
                "addressToken": "mock-address-token",
                "devOnly": true,
            }),
            HighRiskApiKind::Location => json!({
                "locationToken": "mock-location-token",
                "accuracy": "fuzzy",
                "devOnly": true,
            }),
            HighRiskApiKind::Media => json!({
                "tempFiles": [{
                    "fileHandle": "mock-media-handle",
                    "mediaType": "image"
                }],
                "devOnly": true,
            }),
            HighRiskApiKind::File => json!({
                "tempFiles": [{
                    "fileHandle": "mock-file-handle",
                    "name": "mock.txt"
                }],
                "devOnly": true,
            }),
            HighRiskApiKind::Payment => json!({
                "paymentIntentId": "mock-payment-intent",
                "status": "requires_host_confirmation",
                "devOnly": true,
            }),
            HighRiskApiKind::Scan => json!({
                "scanToken": "mock-scan-token",
                "devOnly": true,
            }),
            HighRiskApiKind::PhoneCall => json!({
                "phoneCallIntentId": "mock-phone-call-intent",
                "devOnly": true,
            }),
        };
        Ok(HighRiskApiSuccess {
            data,
            dev_only: true,
            audit_summary: high_risk_audit_summary(spec, request),
        })
    }
}

pub fn high_risk_api_spec(api_name: &str) -> Option<&'static HighRiskApiSpec> {
    HIGH_RISK_API_SPECS
        .iter()
        .find(|spec| spec.name == api_name)
}

pub fn high_risk_consent_required_json(api_name: &str) -> Value {
    high_risk_failure_json(
        api_name,
        "consent_required",
        "This high-risk wx API requires explicit user consent before a Host provider can run.",
        "Route this request through a Host ConsentGate and retry only after approval.",
    )
}

pub fn high_risk_error_json(api_name: &str, error: HighRiskApiError) -> Value {
    match error {
        HighRiskApiError::ConsentRequired => high_risk_consent_required_json(api_name),
        HighRiskApiError::ProviderUnavailable => high_risk_failure_json(
            api_name,
            "provider_unavailable",
            "No Host provider is configured for this high-risk wx API.",
            "Configure a least-privilege Host provider with ConsentGate and audit, or keep this API fail-closed.",
        ),
        HighRiskApiError::PermissionDenied => high_risk_failure_json(
            api_name,
            "permission_denied",
            "The Host provider denied this high-risk wx API request.",
            "Ask the Host to grant an explicit provider permission before retrying.",
        ),
        HighRiskApiError::InvalidOptions => high_risk_failure_json(
            api_name,
            "invalid_options",
            "High-risk wx API options are invalid or not JSON-safe.",
            "Pass a plain JSON object without secrets, local file paths, or raw private data.",
        ),
    }
}

pub fn high_risk_success_json(api_name: &str, success: HighRiskApiSuccess) -> Value {
    let mut data = success.data;
    if let Some(object) = data.as_object_mut() {
        object.insert("errMsg".to_owned(), Value::String(format!("{api_name}:ok")));
        object.insert("devOnly".to_owned(), Value::Bool(success.dev_only));
        object.insert("mock".to_owned(), Value::Bool(success.dev_only));
        object.insert("auditSummary".to_owned(), success.audit_summary);
    } else {
        data = json!({
            "errMsg": format!("{api_name}:ok"),
            "data": data,
            "devOnly": success.dev_only,
            "mock": success.dev_only,
            "auditSummary": success.audit_summary,
        });
    }
    data
}

pub fn call_high_risk_api_with_boundary(
    provider: &impl HighRiskHostProvider,
    spec: &HighRiskApiSpec,
    request: &HighRiskApiRequest,
    consent_granted: bool,
) -> Result<HighRiskApiSuccess, HighRiskApiError> {
    if spec.requires_consent && !consent_granted {
        return Err(HighRiskApiError::ConsentRequired);
    }
    provider.call_high_risk_api(spec, request)
}

pub fn high_risk_audit_summary(spec: &HighRiskApiSpec, request: &HighRiskApiRequest) -> Value {
    json!({
        "apiName": spec.name,
        "riskLevel": spec.risk_level.as_str(),
        "kind": format!("{:?}", spec.kind),
        "requiresConsent": spec.requires_consent,
        "devOnlyProviderAllowed": spec.dev_only_allowed,
        "parameterSummary": redact_high_risk_value(&request.options),
    })
}

pub fn redact_high_risk_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let redacted = map
                .iter()
                .map(|(key, value)| {
                    if is_high_risk_sensitive_key(key) {
                        (key.clone(), Value::String("[REDACTED]".to_owned()))
                    } else {
                        (key.clone(), redact_high_risk_value(value))
                    }
                })
                .collect();
            Value::Object(redacted)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_high_risk_value).collect()),
        Value::String(text) if looks_like_local_path(text) => {
            Value::String("[REDACTED]".to_owned())
        }
        Value::String(text) if text.chars().count() > 160 => {
            let mut truncated: String = text.chars().take(160).collect();
            truncated.push_str("...[TRUNCATED]");
            Value::String(truncated)
        }
        _ => value.clone(),
    }
}

fn high_risk_failure_json(
    api_name: &str,
    code: &'static str,
    reason: &'static str,
    suggestion: &'static str,
) -> Value {
    json!({
        "errMsg": format!("{api_name}:fail {code}"),
        "code": code,
        "reason": reason,
        "suggestion": suggestion,
    })
}

fn is_high_risk_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    [
        "token",
        "authorization",
        "signature",
        "secret",
        "private",
        "credential",
        "phone",
        "mobile",
        "address",
        "identity",
        "idcard",
        "passport",
        "latitude",
        "longitude",
        "location",
        "filepath",
        "filecontent",
        "path",
        "password",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn looks_like_local_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with("\\\\")
        || value.contains(":\\")
        || value.starts_with("file:")
}
