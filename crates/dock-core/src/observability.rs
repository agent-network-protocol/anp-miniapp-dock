use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub const OBSERVABILITY_EVENT_SCHEMA_VERSION: &str = "dock.observability.event.v1";
pub const OBSERVABILITY_METRIC_SCHEMA_VERSION: &str = "dock.observability.metric.v1";
pub const OBSERVABILITY_TRACE_SCHEMA_VERSION: &str = "dock.observability.trace.v1";
pub const OBSERVABILITY_REDACTION_POLICY: &str = "dock.observability.redaction.v1";
pub const OBSERVABILITY_REDACTION_MARKER: &str = "[REDACTED]";
pub const DEFAULT_RUNTIME_VERSION: &str = "dock.runtime.v1";

static NEXT_EVENT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservabilityEventKind {
    SkillLoadStart,
    SkillLoadEnd,
    ApiCallStart,
    ApiCallEnd,
    WxApiCallStart,
    WxApiCallEnd,
    RequestStart,
    RequestEnd,
    ConsentPrompt,
    ConsentDecision,
    ComponentRenderStart,
    ComponentRenderEnd,
    ComponentEvent,
    FallbackUsed,
    AuditRecordWritten,
    SandboxLimitHit,
}

impl ObservabilityEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SkillLoadStart => "skill_load_start",
            Self::SkillLoadEnd => "skill_load_end",
            Self::ApiCallStart => "api_call_start",
            Self::ApiCallEnd => "api_call_end",
            Self::WxApiCallStart => "wx_api_call_start",
            Self::WxApiCallEnd => "wx_api_call_end",
            Self::RequestStart => "request_start",
            Self::RequestEnd => "request_end",
            Self::ConsentPrompt => "consent_prompt",
            Self::ConsentDecision => "consent_decision",
            Self::ComponentRenderStart => "component_render_start",
            Self::ComponentRenderEnd => "component_render_end",
            Self::ComponentEvent => "component_event",
            Self::FallbackUsed => "fallback_used",
            Self::AuditRecordWritten => "audit_record_written",
            Self::SandboxLimitHit => "sandbox_limit_hit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ObservabilitySeverity {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservabilityRedaction {
    pub marker: String,
    pub policy: String,
    pub applied_by_default: bool,
    pub raw_payload_visible: bool,
}

impl Default for ObservabilityRedaction {
    fn default() -> Self {
        Self {
            marker: OBSERVABILITY_REDACTION_MARKER.to_owned(),
            policy: OBSERVABILITY_REDACTION_POLICY.to_owned(),
            applied_by_default: true,
            raw_payload_visible: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservabilityEvent {
    pub schema_version: String,
    pub event_id: String,
    pub kind: ObservabilityEventKind,
    pub severity: ObservabilitySeverity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_did: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_did_hash: Option<String>,
    pub runtime_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_ir_version: Option<String>,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub fields: Map<String, Value>,
    pub redaction: ObservabilityRedaction,
}

impl ObservabilityEvent {
    pub fn new(kind: ObservabilityEventKind) -> Self {
        let severity = match kind {
            ObservabilityEventKind::FallbackUsed | ObservabilityEventKind::SandboxLimitHit => {
                ObservabilitySeverity::Warn
            }
            _ => ObservabilitySeverity::Info,
        };
        Self {
            schema_version: OBSERVABILITY_EVENT_SCHEMA_VERSION.to_owned(),
            event_id: next_observability_id("evt"),
            kind,
            severity,
            trace_id: None,
            session_id: None,
            skill_id: None,
            api_name: None,
            component_path: None,
            merchant_did: None,
            user_did_hash: None,
            runtime_version: DEFAULT_RUNTIME_VERSION.to_owned(),
            render_ir_version: None,
            outcome: "unknown".to_owned(),
            latency_ms: None,
            fields: Map::new(),
            redaction: ObservabilityRedaction::default(),
        }
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(redact_observability_text(&trace_id.into()));
        self
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(redact_observability_text(&session_id.into()));
        self
    }

    pub fn with_skill_id(mut self, skill_id: impl Into<String>) -> Self {
        self.skill_id = Some(redact_observability_text(&skill_id.into()));
        self
    }

    pub fn with_api_name(mut self, api_name: impl Into<String>) -> Self {
        self.api_name = Some(redact_observability_text(&api_name.into()));
        self
    }

    pub fn with_component_path(mut self, component_path: impl Into<String>) -> Self {
        self.component_path = Some(redact_observability_text(&component_path.into()));
        self
    }

    pub fn with_merchant_did(mut self, merchant_did: Option<String>) -> Self {
        self.merchant_did = merchant_did.map(|value| redact_observability_text(&value));
        self
    }

    pub fn with_user_did_hash(mut self, user_did: Option<&str>) -> Self {
        self.user_did_hash = user_did.map(hash_user_did);
        self
    }

    pub fn with_render_ir_version(mut self, version: impl Into<String>) -> Self {
        self.render_ir_version = Some(redact_observability_text(&version.into()));
        self
    }

    pub fn with_outcome(mut self, outcome: impl Into<String>) -> Self {
        self.outcome = redact_observability_text(&outcome.into());
        self.severity = severity_for_outcome(&self.outcome, self.severity);
        self
    }

    pub fn with_latency_ms(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    pub fn with_field(mut self, key: impl Into<String>, value: Value) -> Self {
        let key = key.into();
        let value = if is_sensitive_key(&key) {
            Value::String(OBSERVABILITY_REDACTION_MARKER.to_owned())
        } else {
            redact_observability_value(&value)
        };
        self.fields.insert(redact_observability_text(&key), value);
        self
    }
}

pub trait ObservabilitySink: Clone {
    fn emit(&self, event: ObservabilityEvent);
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
}

impl TraceContext {
    pub fn new(trace_id: impl Into<String>, span_id: impl Into<String>) -> Self {
        Self {
            trace_id: safe_trace_identifier(trace_id.into(), "trace"),
            span_id: safe_trace_identifier(span_id.into(), "span"),
            parent_span_id: None,
        }
    }

    pub fn root() -> Self {
        Self::new(
            next_observability_id("trace"),
            next_observability_id("span"),
        )
    }

    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: next_observability_id("span"),
            parent_span_id: Some(self.span_id.clone()),
        }
    }

    pub fn with_parent_span_id(mut self, parent_span_id: impl Into<String>) -> Self {
        self.parent_span_id = Some(safe_trace_identifier(parent_span_id.into(), "span"));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceSpanKind {
    RuntimeIpc,
    ApiCall,
    WxApiCall,
    Request,
    Consent,
    Render,
    ComponentAction,
    Audit,
    Sandbox,
    Token,
}

impl TraceSpanKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RuntimeIpc => "runtime_ipc",
            Self::ApiCall => "api_call",
            Self::WxApiCall => "wx_api_call",
            Self::Request => "request",
            Self::Consent => "consent",
            Self::Render => "render",
            Self::ComponentAction => "component_action",
            Self::Audit => "audit",
            Self::Sandbox => "sandbox",
            Self::Token => "token",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceSpan {
    pub schema_version: String,
    pub trace_id: String,
    pub span_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind: TraceSpanKind,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, String>,
    pub redaction: ObservabilityRedaction,
}

impl TraceSpan {
    pub fn new(context: TraceContext, kind: TraceSpanKind, name: impl Into<String>) -> Self {
        Self {
            schema_version: OBSERVABILITY_TRACE_SCHEMA_VERSION.to_owned(),
            trace_id: context.trace_id,
            span_id: context.span_id,
            parent_span_id: context.parent_span_id,
            name: redact_observability_text(&name.into()),
            kind,
            outcome: "unknown".to_owned(),
            latency_ms: None,
            attributes: BTreeMap::new(),
            redaction: ObservabilityRedaction::default(),
        }
    }

    pub fn with_outcome(mut self, outcome: impl Into<String>) -> Self {
        self.outcome = low_cardinality_label(&outcome.into());
        self
    }

    pub fn with_latency_ms(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(
            low_cardinality_key(&key.into()),
            low_cardinality_label(&value.into()),
        );
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservabilityMetric {
    pub schema_version: String,
    pub name: String,
    pub kind: MetricKind,
    pub value: f64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    pub redaction: ObservabilityRedaction,
}

impl ObservabilityMetric {
    pub fn counter(name: impl Into<String>, value: u64) -> Self {
        Self::new(name, MetricKind::Counter, value as f64)
    }

    pub fn histogram_ms(name: impl Into<String>, value_ms: u64) -> Self {
        Self::new(name, MetricKind::HistogramMs, value_ms as f64)
    }

    pub fn gauge(name: impl Into<String>, value: f64) -> Self {
        Self::new(name, MetricKind::Gauge, value)
    }

    fn new(name: impl Into<String>, kind: MetricKind, value: f64) -> Self {
        Self {
            schema_version: OBSERVABILITY_METRIC_SCHEMA_VERSION.to_owned(),
            name: low_cardinality_key(&name.into()),
            kind,
            value,
            labels: BTreeMap::new(),
            trace_id: None,
            redaction: ObservabilityRedaction::default(),
        }
    }

    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(
            low_cardinality_key(&key.into()),
            low_cardinality_label(&value.into()),
        );
        self
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(safe_trace_identifier(trace_id.into(), "trace"));
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    Counter,
    HistogramMs,
    Gauge,
}

pub trait MetricsSink: Clone {
    fn record_metric(&self, metric: ObservabilityMetric);
    fn record_span(&self, span: TraceSpan);
}

#[derive(Debug, Clone, Default)]
pub struct NoopMetricsSink;

impl MetricsSink for NoopMetricsSink {
    fn record_metric(&self, _metric: ObservabilityMetric) {}

    fn record_span(&self, _span: TraceSpan) {}
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryMetricsSink {
    metrics: Arc<Mutex<Vec<ObservabilityMetric>>>,
    spans: Arc<Mutex<Vec<TraceSpan>>>,
}

impl InMemoryMetricsSink {
    pub fn metrics(&self) -> Vec<ObservabilityMetric> {
        self.metrics
            .lock()
            .expect("metrics sink mutex poisoned")
            .clone()
    }

    pub fn spans(&self) -> Vec<TraceSpan> {
        self.spans
            .lock()
            .expect("trace sink mutex poisoned")
            .clone()
    }
}

impl MetricsSink for InMemoryMetricsSink {
    fn record_metric(&self, metric: ObservabilityMetric) {
        self.metrics
            .lock()
            .expect("metrics sink mutex poisoned")
            .push(metric);
    }

    fn record_span(&self, span: TraceSpan) {
        self.spans
            .lock()
            .expect("trace sink mutex poisoned")
            .push(span);
    }
}

#[derive(Debug, Clone, Default)]
pub struct NoopObservabilitySink;

impl ObservabilitySink for NoopObservabilitySink {
    fn emit(&self, _event: ObservabilityEvent) {}
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryObservabilitySink {
    events: Arc<Mutex<Vec<ObservabilityEvent>>>,
}

impl InMemoryObservabilitySink {
    pub fn events(&self) -> Vec<ObservabilityEvent> {
        self.events
            .lock()
            .expect("observability sink mutex poisoned")
            .clone()
    }
}

impl ObservabilitySink for InMemoryObservabilitySink {
    fn emit(&self, event: ObservabilityEvent) {
        self.events
            .lock()
            .expect("observability sink mutex poisoned")
            .push(event);
    }
}

pub fn next_observability_id(prefix: &str) -> String {
    let id = NEXT_EVENT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{id:016x}")
}

pub fn hash_user_did(user_did: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(user_did.as_bytes());
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(71);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded
}

pub fn redact_observability_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    let value = if is_sensitive_key(key) {
                        Value::String(OBSERVABILITY_REDACTION_MARKER.to_owned())
                    } else {
                        redact_observability_value(value)
                    };
                    (redact_observability_text(key), value)
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(redact_observability_value)
                .collect::<Vec<_>>(),
        ),
        Value::String(value) => Value::String(redact_observability_text(value)),
        _ => value.clone(),
    }
}

pub fn redact_observability_text(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if contains_sensitive_text(trimmed) {
        return OBSERVABILITY_REDACTION_MARKER.to_owned();
    }
    if trimmed.chars().count() > 160 {
        let truncated = trimmed.chars().take(160).collect::<String>();
        format!("{truncated}...")
    } else {
        trimmed.to_owned()
    }
}

fn severity_for_outcome(
    outcome: &str,
    default_severity: ObservabilitySeverity,
) -> ObservabilitySeverity {
    match outcome {
        "error" | "timeout" | "denied" | "blocked" => ObservabilitySeverity::Error,
        "warning" | "fallback" | "limit_hit" | "skipped" => ObservabilitySeverity::Warn,
        _ => default_severity,
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "authorization",
        "signature",
        "signature-input",
        "capabilitytoken",
        "capability_token",
        "token",
        "secret",
        "private",
        "password",
        "cookie",
        "credential",
        "phone",
        "address",
        "latitude",
        "longitude",
        "location",
        "filecontent",
        "file_content",
        "rawpayload",
        "raw_payload",
    ]
    .iter()
    .any(|marker| key.contains(marker))
}

fn contains_sensitive_text(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "authorization",
        "bearer ",
        "signature",
        "capabilitytoken",
        "capability-token",
        "secret",
        "private key",
        "private.pem",
        "password",
        "cookie",
        "credential",
        "-----begin",
        "/home/",
        "/users/",
        "file:",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn low_cardinality_key(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return OBSERVABILITY_REDACTION_MARKER.to_owned();
    }
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | '.' => ch.to_ascii_lowercase(),
            _ => '_',
        })
        .take(96)
        .collect()
}

pub fn low_cardinality_label(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return "none".to_owned();
    }
    let lower = value.to_ascii_lowercase();
    if contains_sensitive_text(value)
        || value.contains('?')
        || lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("did:")
        || value.chars().count() > 64
    {
        return OBSERVABILITY_REDACTION_MARKER.to_owned();
    }
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | '.' | ':' => ch,
            _ => '_',
        })
        .take(64)
        .collect()
}

fn safe_trace_identifier(value: String, prefix: &str) -> String {
    let value = value.trim();
    if value.is_empty() || contains_sensitive_text(value) {
        return next_observability_id(prefix);
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
        && value.chars().count() <= 96
    {
        value.to_owned()
    } else {
        next_observability_id(prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn observability_event_serializes_with_redaction_and_hashed_user_did() {
        let event = ObservabilityEvent::new(ObservabilityEventKind::ApiCallEnd)
            .with_trace_id("trace-001")
            .with_session_id("session-001")
            .with_skill_id("coffee")
            .with_api_name("payOrder")
            .with_merchant_did(Some("did:wba:merchant.example".to_owned()))
            .with_user_did_hash(Some("did:wba:user.example"))
            .with_outcome("ok")
            .with_latency_ms(12)
            .with_field(
                "payload",
                json!({
                    "capabilityToken": "capability-secret-token",
                    "deliveryAddress": "1 Private Road",
                    "safe": "metadata"
                }),
            );

        let rendered = serde_json::to_string(&event).expect("event serializes");
        assert!(rendered.contains("dock.observability.event.v1"));
        assert!(rendered.contains("api_call_end"));
        assert!(rendered.contains("sha256:"));
        assert!(!rendered.contains("did:wba:user.example"));
        assert!(!rendered.contains("capability-secret-token"));
        assert!(!rendered.contains("1 Private Road"));
        assert!(rendered.contains("metadata"));
    }

    #[test]
    fn metrics_and_trace_redact_high_cardinality_labels() {
        let metric = ObservabilityMetric::counter("dock.request.total", 1)
            .with_label("url", "https://merchant.example/orders?token=secret")
            .with_label("outcome", "ok")
            .with_trace_id("trace-001");
        let span = TraceSpan::new(
            TraceContext::new("trace-001", "span-001"),
            TraceSpanKind::Request,
            "wx.request",
        )
        .with_attribute("Authorization", "Bearer secret-token")
        .with_outcome("ok");

        assert_eq!(
            metric.labels.get("url").map(String::as_str),
            Some(OBSERVABILITY_REDACTION_MARKER)
        );
        assert_eq!(metric.labels.get("outcome").map(String::as_str), Some("ok"));
        assert_eq!(
            span.attributes.get("authorization").map(String::as_str),
            Some(OBSERVABILITY_REDACTION_MARKER)
        );
        assert_eq!(span.trace_id, "trace-001");
    }
}
