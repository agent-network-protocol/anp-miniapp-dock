use crate::consent::{ConsentProof, RiskLevel};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
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

    pub fn from_label(label: &str) -> Self {
        match label {
            "ok" => Self::Ok,
            "blocked_consent_required" => Self::BlockedConsentRequired,
            "blocked_permission_denied" => Self::BlockedPermissionDenied,
            "validation_failed" => Self::ValidationFailed,
            _ => Self::Error,
        }
    }
}

impl std::fmt::Display for AuditOutcome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

pub trait AuditSink {
    fn profile(&self) -> AuditPersistenceProfile {
        AuditPersistenceProfile::InMemoryDev
    }

    fn ensure_available(&self) -> Result<(), AuditError> {
        Ok(())
    }

    fn record(&self, record: AuditRecord) -> Result<(), AuditError>;
}

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("audit sink failed: {0}")]
    Sink(String),

    #[error("audit record serialization failed: {0}")]
    Serialize(String),

    #[error("audit record deserialization failed at line {line}: {message}")]
    Deserialize { line: usize, message: String },

    #[error("audit retention policy failed: {0}")]
    Retention(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditPersistenceProfile {
    InMemoryDev,
    LocalFileJsonl,
    HostPersistentSink,
    EncryptedSqlite,
}

impl AuditPersistenceProfile {
    pub fn production_ready(self) -> bool {
        matches!(
            self,
            AuditPersistenceProfile::HostPersistentSink | AuditPersistenceProfile::EncryptedSqlite
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditUnavailablePolicy {
    FailClosedHighRisk,
    DegradedReleaseBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRedaction {
    pub marker: String,
    pub policy: String,
    pub raw_parameter_visible: bool,
    pub raw_consent_proof_visible: bool,
}

impl Default for AuditRedaction {
    fn default() -> Self {
        Self {
            marker: REDACTED.to_owned(),
            policy: "dock.audit.redaction.v1".to_owned(),
            raw_parameter_visible: false,
            raw_consent_proof_visible: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditExportReport {
    pub backend_profile: AuditPersistenceProfile,
    pub production_ready: bool,
    pub exported_count: usize,
    pub records: Vec<AuditRecord>,
    pub redaction: AuditRedaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRetentionReport {
    pub backend_profile: AuditPersistenceProfile,
    pub production_ready: bool,
    pub before_count: usize,
    pub retained_count: usize,
    pub removed_count: usize,
    pub min_occurred_at_ms: u64,
    pub redaction: AuditRedaction,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_decision: Option<Value>,
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
            permission_decision: None,
            consent_proof: input.consent_proof.map(redact_consent_proof),
            outcome: input.outcome,
            occurred_at_ms: now_ms(),
        }
    }

    pub fn redacted(mut self) -> Self {
        self.parameter_summary = redact_value(&self.parameter_summary);
        self.permission_decision = self
            .permission_decision
            .map(|permission_decision| redact_value(&permission_decision));
        self.consent_proof = self.consent_proof.map(redact_consent_proof);
        self
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
    fn profile(&self) -> AuditPersistenceProfile {
        AuditPersistenceProfile::InMemoryDev
    }

    fn record(&self, record: AuditRecord) -> Result<(), AuditError> {
        let mut records = self.records.lock().expect("audit sink mutex poisoned");
        records.push(record.redacted());
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FileAuditSink {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl FileAuditSink {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn records(&self) -> Result<Vec<AuditRecord>, AuditError> {
        let _guard = self.lock.lock().expect("audit sink mutex poisoned");
        self.read_records_unlocked()
    }

    pub fn query(&self, query: AuditQuery<'_>) -> Result<Vec<AuditRecord>, AuditError> {
        let records = self.records()?;
        Ok(records
            .into_iter()
            .filter(|record| query.matches(record))
            .collect())
    }

    pub fn export_redacted_json(&self, query: AuditQuery<'_>) -> Result<Value, AuditError> {
        serde_json::to_value(self.export_redacted_report(query)?.records)
            .map_err(|error| AuditError::Serialize(error.to_string()))
    }

    pub fn export_redacted_report(
        &self,
        query: AuditQuery<'_>,
    ) -> Result<AuditExportReport, AuditError> {
        let records: Vec<_> = self
            .query(query)?
            .into_iter()
            .map(AuditRecord::redacted)
            .collect();
        let profile = self.profile();
        Ok(AuditExportReport {
            backend_profile: profile,
            production_ready: profile.production_ready(),
            exported_count: records.len(),
            records,
            redaction: AuditRedaction::default(),
        })
    }

    pub fn retain_since(&self, min_occurred_at_ms: u64) -> Result<usize, AuditError> {
        Ok(self.retain_since_report(min_occurred_at_ms)?.retained_count)
    }

    pub fn retain_since_report(
        &self,
        min_occurred_at_ms: u64,
    ) -> Result<AuditRetentionReport, AuditError> {
        let _guard = self.lock.lock().expect("audit sink mutex poisoned");
        let records = self.read_records_unlocked()?;
        let before_count = records.len();
        let retained: Vec<_> = records
            .into_iter()
            .filter(|record| record.occurred_at_ms >= min_occurred_at_ms)
            .collect();
        let retained_count = retained.len();
        self.write_records_unlocked(&retained)?;
        let profile = self.profile();
        Ok(AuditRetentionReport {
            backend_profile: profile,
            production_ready: profile.production_ready(),
            before_count,
            retained_count,
            removed_count: before_count.saturating_sub(retained_count),
            min_occurred_at_ms,
            redaction: AuditRedaction::default(),
        })
    }

    fn read_records_unlocked(&self) -> Result<Vec<AuditRecord>, AuditError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path).map_err(|error| AuditError::Sink(error.to_string()))?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();
        for (index, line) in reader.lines().enumerate() {
            let line = line.map_err(|error| AuditError::Sink(error.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let record = serde_json::from_str::<AuditRecord>(&line).map_err(|error| {
                AuditError::Deserialize {
                    line: index + 1,
                    message: error.to_string(),
                }
            })?;
            records.push(record);
        }
        Ok(records)
    }

    fn write_records_unlocked(&self, records: &[AuditRecord]) -> Result<(), AuditError> {
        if let Some(parent) = non_empty_parent(&self.path) {
            fs::create_dir_all(parent).map_err(|error| AuditError::Sink(error.to_string()))?;
        }
        let tmp_path = self.path.with_extension("jsonl.tmp");
        {
            let mut file = File::create(&tmp_path)
                .map_err(|error| AuditError::Retention(error.to_string()))?;
            for record in records {
                write_record_line(&mut file, &record.clone().redacted())?;
            }
            file.sync_all()
                .map_err(|error| AuditError::Retention(error.to_string()))?;
        }
        fs::rename(&tmp_path, &self.path).map_err(|error| AuditError::Retention(error.to_string()))
    }
}

impl AuditSink for FileAuditSink {
    fn profile(&self) -> AuditPersistenceProfile {
        AuditPersistenceProfile::LocalFileJsonl
    }

    fn ensure_available(&self) -> Result<(), AuditError> {
        if let Some(parent) = non_empty_parent(&self.path) {
            fs::create_dir_all(parent).map_err(|error| AuditError::Sink(error.to_string()))?;
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| AuditError::Sink(error.to_string()))?;
        Ok(())
    }

    fn record(&self, record: AuditRecord) -> Result<(), AuditError> {
        let _guard = self.lock.lock().expect("audit sink mutex poisoned");
        if let Some(parent) = non_empty_parent(&self.path) {
            fs::create_dir_all(parent).map_err(|error| AuditError::Sink(error.to_string()))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| AuditError::Sink(error.to_string()))?;
        write_record_line(&mut file, &record.redacted())?;
        file.flush()
            .map_err(|error| AuditError::Sink(error.to_string()))
    }
}

fn non_empty_parent(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

fn write_record_line(file: &mut File, record: &AuditRecord) -> Result<(), AuditError> {
    let line =
        serde_json::to_string(record).map_err(|error| AuditError::Serialize(error.to_string()))?;
    file.write_all(line.as_bytes())
        .map_err(|error| AuditError::Sink(error.to_string()))?;
    file.write_all(b"\n")
        .map_err(|error| AuditError::Sink(error.to_string()))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AuditQuery<'a> {
    pub session_id: Option<&'a str>,
    pub skill_id: Option<&'a str>,
    pub api_name: Option<&'a str>,
    pub min_occurred_at_ms: Option<u64>,
    pub max_occurred_at_ms: Option<u64>,
}

impl<'a> AuditQuery<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn session_id(mut self, session_id: &'a str) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn skill_id(mut self, skill_id: &'a str) -> Self {
        self.skill_id = Some(skill_id);
        self
    }

    pub fn api_name(mut self, api_name: &'a str) -> Self {
        self.api_name = Some(api_name);
        self
    }

    pub fn min_occurred_at_ms(mut self, min_occurred_at_ms: u64) -> Self {
        self.min_occurred_at_ms = Some(min_occurred_at_ms);
        self
    }

    pub fn max_occurred_at_ms(mut self, max_occurred_at_ms: u64) -> Self {
        self.max_occurred_at_ms = Some(max_occurred_at_ms);
        self
    }

    fn matches(&self, record: &AuditRecord) -> bool {
        self.session_id
            .is_none_or(|session_id| record.session_id == session_id)
            && self
                .skill_id
                .is_none_or(|skill_id| record.skill_id == skill_id)
            && self
                .api_name
                .is_none_or(|api_name| record.api_name == api_name)
            && self
                .min_occurred_at_ms
                .is_none_or(|min| record.occurred_at_ms >= min)
            && self
                .max_occurred_at_ms
                .is_none_or(|max| record.occurred_at_ms <= max)
    }
}

pub fn redact_value(value: &Value) -> Value {
    redact_value_at_key(None, value)
}

fn redact_consent_proof(mut proof: ConsentProof) -> ConsentProof {
    proof.parameter_summary = redact_value(&proof.parameter_summary);
    proof
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
