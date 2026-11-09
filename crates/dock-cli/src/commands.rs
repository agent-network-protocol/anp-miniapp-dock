use anp::authentication::AuthMode;
use anp_adapter::{
    sign_challenge_proof, CapabilityToken, CapabilityTokenCache, CapabilityTokenScope,
    ChallengeLoginRequest, ChallengeLoginResponse, ChallengeProofError, ChallengeProofPayload,
    DidChallenge as AdapterDidChallenge, DidCredentialConfig, DidCredentialError,
    FileDidCredentialProvider, IdentitySession, InMemoryTokenCache,
};
use card_spec::{fallback_from_result, FallbackReason};
use clap::{Parser, Subcommand};
use component_runtime::{
    ComponentEvent, ComponentInput, ComponentInstance, ComponentMetadata, ComponentPackage,
    ComponentRenderOutput, ComponentVmAction, DynamicComponentConfig, RenderEventKind, RenderNode,
};
use consent_audit::{ConsentRequest, DEV_HEADLESS_CONSENT_PROVIDER, DEV_HEADLESS_DECISION_ACTOR};
use dock_core::{
    ApiCallContext, AuditEvent, AuditSink, ComponentRenderInput, ConsentDecision, ConsentGate,
    DockCoreError, ErrorCode, PermissionDecision, RenderOutcome, RenderRouter, RuntimeAuditReader,
    RuntimeCallRequest, RuntimeConfig, RuntimeErrorResponse, RuntimeHost, RuntimeIpcRequest,
    RuntimeIpcResponse, RuntimeService, RuntimeSessionContext,
};
use js_runtime_quickjs::{ApiCall, ApiVm, ApiVmConfig, ApiVmError, HostDidAuthConfig};
use mcp_schema::{
    ApiDeclaration, AtomicApiResult, ComponentDeclaration, ValidationIssue,
    ValidationIssueCategory, ValidationReport,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use skill_loader::{load_skill, resolve_component_path, LoadedSkill};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use wx_compat::{
    CapabilityProfile, InMemoryScopedStorage, RequestBroker, ScopedStorage, StorageError,
    StorageScope, WxRequest, WxRequestError, WxResponse,
};

const DEFAULT_SESSION_ID: &str = "session-cli";
const DEFAULT_SKILL_ID: &str = "coffee";
const DEFAULT_USER_DID: &str = "did:wba:user.example";
const DEFAULT_AGENT_DID: &str = "did:wba:agent.example";
const DEFAULT_MERCHANT_DID: &str = "did:wba:coffee-merchant.example";
const DEFAULT_IDENTITY_DIR: &str = "examples/identity";
const DEFAULT_DID_DOCUMENT_FILE: &str = "did_document.json";
const DEFAULT_PRIVATE_KEY_FILE: &str = "key-1-private.pem";
const VALIDATE_REPORT_SCHEMA_VERSION: &str = "dock.validate-report.v1";
const IMPORT_REPORT_SCHEMA_VERSION: &str = "dock.import-wechat-mcp-report.v1";
const DOCTOR_REPORT_SCHEMA_VERSION: &str = "dock.doctor-report.v1";
const PERF_REPORT_SCHEMA_VERSION: &str = "dock.perf-baseline-report.v1";

#[derive(Debug, Parser)]
#[command(name = "dock-cli", about = "MiniApp MCP Skill runtime developer CLI")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Validate {
        skill: PathBuf,
    },
    Inspect {
        skill: PathBuf,
    },
    TestSkill {
        skill: PathBuf,
    },
    ImportWechatMcp {
        source: PathBuf,
        #[arg(long)]
        dest: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        #[arg(long, default_value_t = false)]
        write: bool,
        #[arg(long, default_value_t = false)]
        overwrite: bool,
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        generate_patch: bool,
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        include_fixtures: bool,
    },
    Doctor {
        #[arg(long)]
        skill: Option<PathBuf>,
        #[arg(long)]
        server: Option<String>,
        #[arg(long)]
        runtime_config: Option<PathBuf>,
        #[arg(long)]
        did_document: Option<PathBuf>,
        #[arg(long)]
        private_key: Option<PathBuf>,
        #[arg(long)]
        user_did: Option<String>,
        #[arg(long)]
        agent_did: Option<String>,
        #[arg(long)]
        identity_handle: Option<String>,
        #[arg(long)]
        identity_root: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        ci: bool,
    },
    Perf {
        skill: PathBuf,
        #[arg(long, default_value_t = false)]
        full: bool,
        #[arg(long)]
        iterations: Option<usize>,
    },
    CallApi {
        skill: PathBuf,
        api_name: String,
        json_args: String,
    },
    PreviewComponent {
        skill: PathBuf,
        component_path: String,
        json_input: String,
    },
    PreviewCard {
        result_json: String,
    },
    RuntimeJson {
        skill: PathBuf,
        request_json: String,
    },
    RunDemo {
        #[arg(long)]
        skill: PathBuf,
        #[arg(long)]
        server: String,
        #[arg(long)]
        did_document: Option<PathBuf>,
        #[arg(long)]
        private_key: Option<PathBuf>,
        #[arg(long)]
        user_did: Option<String>,
        #[arg(long)]
        agent_did: Option<String>,
        #[arg(long)]
        identity_handle: Option<String>,
        #[arg(long)]
        identity_root: Option<PathBuf>,
    },
}

pub fn run() -> Result<(), CliError> {
    let cli = Cli::parse();
    run_with_writer(cli, &mut std::io::stdout())
}

pub fn run_with_writer(mut cli: Cli, writer: &mut impl Write) -> Result<(), CliError> {
    let output = cli.execute()?;
    write_json(writer, &output)?;
    if output
        .get("commandStatus")
        .and_then(Value::as_str)
        .is_some_and(|status| status == "failed")
    {
        return Err(CliError::Demo(
            "command completed with failing checks".to_owned(),
        ));
    }
    Ok(())
}

impl Cli {
    pub fn try_parse_from_args<I, T>(args: I) -> Result<Self, CliError>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::try_parse_from(args).map_err(CliError::from)
    }

    fn execute(&mut self) -> Result<Value, CliError> {
        match &self.command {
            Command::Validate { skill } => validate(skill),
            Command::Inspect { skill } => inspect(skill),
            Command::TestSkill { skill } => test_skill(skill),
            Command::ImportWechatMcp {
                source,
                dest,
                dry_run,
                write,
                overwrite,
                generate_patch,
                include_fixtures,
            } => import_wechat_mcp(ImportOptions {
                source,
                dest: dest.as_deref(),
                dry_run: !*write || *dry_run,
                overwrite: *overwrite,
                generate_patch: *generate_patch,
                include_fixtures: *include_fixtures,
            }),
            Command::Doctor {
                skill,
                server,
                runtime_config,
                did_document,
                private_key,
                user_did,
                agent_did,
                identity_handle,
                identity_root,
                ci,
            } => doctor(DoctorOptions {
                skill: skill.as_deref(),
                server: server.as_deref(),
                runtime_config: runtime_config.as_deref(),
                did_document: did_document.as_deref(),
                private_key: private_key.as_deref(),
                user_did: user_did.as_deref(),
                agent_did: agent_did.as_deref(),
                identity_handle: identity_handle.as_deref(),
                identity_root: identity_root.as_deref(),
                ci: *ci,
            }),
            Command::Perf {
                skill,
                full,
                iterations,
            } => perf(skill, *full, *iterations),
            Command::CallApi {
                skill,
                api_name,
                json_args,
            } => call_api(skill, api_name, json_args),
            Command::PreviewComponent {
                skill,
                component_path,
                json_input,
            } => preview_component(skill, component_path, json_input),
            Command::PreviewCard { result_json } => preview_card(result_json),
            Command::RuntimeJson {
                skill,
                request_json,
            } => runtime_json(skill, request_json),
            Command::RunDemo {
                skill,
                server,
                did_document,
                private_key,
                user_did,
                agent_did,
                identity_handle,
                identity_root,
            } => {
                let auth_config = DemoAuthConfig::from_inputs_optional(
                    did_document.clone(),
                    private_key.clone(),
                    user_did.clone(),
                    agent_did.clone(),
                    identity_handle.clone(),
                    identity_root.clone(),
                    EnvConfigSource,
                )?;
                run_demo(skill, server, auth_config.as_ref())
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Args(#[from] clap::Error),

    #[error("failed to load skill: {0}")]
    Skill(#[from] skill_loader::SkillPackageError),

    #[error("failed to load component: {0}")]
    ComponentLoad(#[from] component_runtime::ComponentLoadError),

    #[error("component VM failed: {0}")]
    ComponentVm(#[from] component_runtime::ComponentVmError),

    #[error("API VM failed: {0}")]
    ApiVm(#[from] js_runtime_quickjs::ApiVmError),

    #[error("runtime call failed: {0}")]
    Core(#[from] DockCoreError),

    #[error("runtime call failed: {0}")]
    Runtime(#[from] Box<RuntimeErrorResponse>),

    #[error("invalid JSON for {label}: {source}")]
    Json {
        label: String,
        source: serde_json::Error,
    },

    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("demo flow failed: {0}")]
    Demo(String),

    #[error("DID credential configuration failed: {0}")]
    Credential(#[from] DidCredentialError),

    #[error("DID challenge proof failed: {0}")]
    ChallengeProof(#[from] ChallengeProofError),
}

#[derive(Clone, PartialEq, Eq)]
struct DemoAuthConfig {
    user_did: String,
    agent_did: Option<String>,
    credential: DidCredentialConfig,
}

impl fmt::Debug for DemoAuthConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DemoAuthConfig")
            .field("user_did", &self.user_did)
            .field("agent_did", &self.agent_did)
            .field("credential", &"[REDACTED]")
            .finish()
    }
}

impl DemoAuthConfig {
    fn from_inputs_optional(
        did_document: Option<PathBuf>,
        private_key: Option<PathBuf>,
        user_did: Option<String>,
        agent_did: Option<String>,
        identity_handle: Option<String>,
        identity_root: Option<PathBuf>,
        env: impl ConfigSource,
    ) -> Result<Option<Self>, DidCredentialError> {
        Self::from_inputs(
            did_document,
            private_key,
            user_did,
            agent_did,
            identity_handle,
            identity_root,
            env,
        )
        .map(Some)
    }

    fn from_inputs(
        did_document: Option<PathBuf>,
        private_key: Option<PathBuf>,
        user_did: Option<String>,
        agent_did: Option<String>,
        identity_handle: Option<String>,
        identity_root: Option<PathBuf>,
        env: impl ConfigSource,
    ) -> Result<Self, DidCredentialError> {
        let did_document = did_document.or_else(|| env.path("ANP_DOCK_DID_DOCUMENT"));
        let private_key = private_key.or_else(|| env.path("ANP_DOCK_PRIVATE_KEY"));
        let user_did = user_did.or_else(|| env.string("ANP_DOCK_USER_DID"));
        let agent_did = agent_did.or_else(|| env.string("ANP_DOCK_AGENT_DID"));
        let identity_handle = identity_handle.or_else(|| env.string("ANP_DOCK_IDENTITY_HANDLE"));
        let identity_root = identity_root.or_else(|| env.path("ANP_DOCK_IDENTITY_ROOT"));

        if did_document.is_some() || private_key.is_some() || user_did.is_some() {
            if did_document.is_none() || private_key.is_none() {
                return Err(DidCredentialError::InvalidIdentity);
            }
            if identity_handle.is_some() || identity_root.is_some() {
                return Err(DidCredentialError::InvalidIdentity);
            }
            return Self::from_credential_paths(
                did_document.expect("checked did document"),
                private_key.expect("checked private key"),
                user_did,
                agent_did,
            );
        }

        if identity_handle.is_some() || identity_root.is_some() {
            let handle = identity_handle.ok_or(DidCredentialError::Unavailable)?;
            let root = identity_root.ok_or(DidCredentialError::Unavailable)?;
            let resolved = resolve_identity_from_store(&root, &handle)?;
            resolved.credential.validate()?;
            return Ok(Self {
                agent_did,
                ..resolved
            });
        }

        let project_root = default_project_root()?;
        Self::from_default_project_identity(&project_root, agent_did)
    }

    fn from_default_project_identity(
        project_root: &Path,
        agent_did: Option<String>,
    ) -> Result<Self, DidCredentialError> {
        let (did_document, private_key) = default_identity_paths(project_root);
        Self::from_credential_paths(did_document, private_key, None, agent_did)
    }

    fn from_credential_paths(
        did_document: PathBuf,
        private_key: PathBuf,
        user_did: Option<String>,
        agent_did: Option<String>,
    ) -> Result<Self, DidCredentialError> {
        let user_did = match user_did {
            Some(user_did) => user_did,
            None => did_from_document_path(&did_document)?,
        };
        let credential = DidCredentialConfig::new(did_document, private_key)
            .without_private_key_permission_check();
        credential.validate()?;
        Ok(Self {
            user_did,
            agent_did,
            credential,
        })
    }

    fn redacted_summary(&self) -> Value {
        json!({
            "userDid": self.user_did,
            "agentDid": self.agent_did,
            "credential": {
                "didDocument": "[CONFIGURED]",
                "privateKey": "[REDACTED]"
            }
        })
    }
}

trait ConfigSource: Copy {
    fn string(self, name: &str) -> Option<String>;

    fn path(self, name: &str) -> Option<PathBuf> {
        self.string(name)
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
    }
}

#[derive(Debug, Clone, Copy)]
struct EnvConfigSource;

impl ConfigSource for EnvConfigSource {
    fn string(self, name: &str) -> Option<String> {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }
}

fn resolve_identity_from_store(
    root: &Path,
    handle: &str,
) -> Result<DemoAuthConfig, DidCredentialError> {
    let root = identity_store_dir(root);
    let identity_dir = identity_dir_for_handle(&root, handle)?;
    let identity = read_json(identity_dir.join("identity.json"))?;
    let user_did = identity
        .get("did")
        .or_else(|| identity.get("userDid"))
        .and_then(Value::as_str)
        .ok_or(DidCredentialError::InvalidIdentity)?
        .to_owned();
    Ok(DemoAuthConfig {
        user_did,
        agent_did: None,
        credential: DidCredentialConfig::new(
            identity_dir.join("did_document.json"),
            identity_dir.join("key-1-private.pem"),
        ),
    })
}

fn default_project_root() -> Result<PathBuf, DidCredentialError> {
    let current_dir = std::env::current_dir().map_err(|_| DidCredentialError::Unavailable)?;
    find_project_root_from(&current_dir).ok_or(DidCredentialError::Unavailable)
}

fn find_project_root_from(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        if current.join("Cargo.toml").is_file() && current.join("examples/coffee-skill").is_dir() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn default_identity_paths(project_root: &Path) -> (PathBuf, PathBuf) {
    let identity_dir = project_root.join(DEFAULT_IDENTITY_DIR);
    (
        identity_dir.join(DEFAULT_DID_DOCUMENT_FILE),
        identity_dir.join(DEFAULT_PRIVATE_KEY_FILE),
    )
}

fn identity_store_dir(root: &Path) -> PathBuf {
    if root.join("index.json").exists() || root.file_name().is_some_and(|name| name == "identities")
    {
        return root.to_path_buf();
    }
    root.join("identities")
}

fn identity_dir_for_handle(root: &Path, handle: &str) -> Result<PathBuf, DidCredentialError> {
    if let Some(path) = identity_dir_from_index(root, handle)? {
        return Ok(path);
    }
    let entries = std::fs::read_dir(root).map_err(|_| DidCredentialError::Unavailable)?;
    let mut matches = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| DidCredentialError::Unavailable)?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(identity) = read_json(path.join("identity.json")) else {
            continue;
        };
        let matches_handle = identity
            .get("handle")
            .or_else(|| identity.get("name"))
            .and_then(Value::as_str)
            == Some(handle);
        if matches_handle {
            matches.push(path);
        }
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        _ => Err(DidCredentialError::InvalidIdentity),
    }
}

fn identity_dir_from_index(
    root: &Path,
    handle: &str,
) -> Result<Option<PathBuf>, DidCredentialError> {
    let index_path = root.join("index.json");
    if !index_path.exists() {
        return Ok(None);
    }
    let index = read_json(index_path)?;
    let entry = index
        .get(handle)
        .or_else(|| index.get("handles").and_then(|handles| handles.get(handle)));
    let Some(entry) = entry else {
        return Ok(None);
    };
    let relative = entry
        .as_str()
        .or_else(|| entry.get("dir").and_then(Value::as_str))
        .or_else(|| entry.get("path").and_then(Value::as_str))
        .ok_or(DidCredentialError::InvalidIdentity)?;
    Ok(Some(root.join(relative)))
}

fn read_json(path: PathBuf) -> Result<Value, DidCredentialError> {
    let content = std::fs::read_to_string(&path).map_err(|_| DidCredentialError::Unavailable)?;
    serde_json::from_str(&content).map_err(|_| DidCredentialError::InvalidIdentity)
}

fn did_from_document_path(path: &Path) -> Result<String, DidCredentialError> {
    let document = read_json(path.to_path_buf())?;
    document
        .get("id")
        .and_then(Value::as_str)
        .filter(|did| !did.trim().is_empty())
        .map(str::to_owned)
        .ok_or(DidCredentialError::InvalidIdentity)
}

fn validate(skill_path: &Path) -> Result<Value, CliError> {
    let skill = load_skill(skill_path)?;
    let skill_id = skill_id_for_path(&skill, skill_path);
    let registration = validate_api_registration(&skill);
    let api_reports = validate_api_reports(&skill, registration.as_ref());
    let component_reports = validate_component_reports(&skill);
    let permissions = validate_permissions(&component_reports);
    let risks = validate_risks(&skill);
    let fallbacks = validate_fallbacks(&skill, &component_reports);
    let release_blockers = validate_release_blockers(&skill, registration.as_ref());
    let repair_suggestions = validate_repair_suggestions(
        &skill.validation,
        &api_reports,
        &component_reports,
        &fallbacks,
        &release_blockers,
    );
    let supply_chain = supply_chain_report(&skill);
    let release_readiness = validate_release_readiness(&skill, &release_blockers);
    let compatibility_level = compatibility_level(&skill.validation, &release_blockers);
    let report_status = validate_report_status(&skill.validation, &release_blockers);
    let skill_ref = validate_skill_ref(skill_path);
    let skill_path_display = skill_ref
        .get("path")
        .cloned()
        .unwrap_or_else(|| json!("[REDACTED]"));

    Ok(json!({
        "schemaVersion": VALIDATE_REPORT_SCHEMA_VERSION,
        "status": report_status,
        "commandStatus": "ok",
        "reportStatus": report_status,
        "compatibilityLevel": compatibility_level,
        "skillRoot": skill_path_display,
        "skillRef": skill_ref,
        "skillId": skill_id.clone(),
        "apis": api_reports.clone(),
        "apiNames": skill.manifest.apis.iter().map(|api| api.name.as_str()).collect::<Vec<_>>(),
        "components": component_reports.clone(),
        "componentPaths": skill.components.keys().collect::<Vec<_>>(),
        "permissions": permissions.clone(),
        "risks": risks.clone(),
        "fallbacks": fallbacks.clone(),
        "releaseBlockers": release_blockers.clone(),
        "repairSuggestions": repair_suggestions.clone(),
        "releaseReadiness": release_readiness.clone(),
        "compatibilityReport": {
            "schemaVersion": VALIDATE_REPORT_SCHEMA_VERSION,
            "status": report_status,
            "compatibilityLevel": compatibility_level,
            "skillId": skill_id,
            "apis": api_reports,
            "components": component_reports,
            "permissions": permissions,
            "risks": risks,
            "supplyChain": supply_chain,
            "fallbacks": fallbacks,
            "releaseBlockers": release_blockers,
            "repairSuggestions": repair_suggestions,
            "releaseReadiness": release_readiness,
        },
        "validation": validation_summary(&skill.validation)
    }))
}

struct ImportOptions<'a> {
    source: &'a Path,
    dest: Option<&'a Path>,
    dry_run: bool,
    overwrite: bool,
    generate_patch: bool,
    include_fixtures: bool,
}

struct DoctorOptions<'a> {
    skill: Option<&'a Path>,
    server: Option<&'a str>,
    runtime_config: Option<&'a Path>,
    did_document: Option<&'a Path>,
    private_key: Option<&'a Path>,
    user_did: Option<&'a str>,
    agent_did: Option<&'a str>,
    identity_handle: Option<&'a str>,
    identity_root: Option<&'a Path>,
    ci: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DoctorCheckStatus {
    Pass,
    Warn,
    Fail,
    Skip,
}

impl DoctorCheckStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Fail => "fail",
            Self::Skip => "skip",
        }
    }
}

struct DoctorCheck {
    id: &'static str,
    title: &'static str,
    status: DoctorCheckStatus,
    severity: &'static str,
    evidence: Value,
    suggestion: &'static str,
}

fn doctor(options: DoctorOptions<'_>) -> Result<Value, CliError> {
    let mut checks = Vec::new();
    let (runtime_config, config_source) =
        doctor_runtime_config(options.runtime_config, &mut checks);
    checks.push(doctor_toolchain_check());
    checks.push(doctor_workspace_check());
    checks.push(doctor_runtime_config_check(
        &runtime_config,
        config_source
            .get("loaded")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    ));
    checks.push(doctor_skill_check(options.skill)?);
    checks.extend(doctor_identity_checks(&options));
    checks.push(doctor_resolver_check(&runtime_config));
    checks.push(doctor_allowlist_check(&runtime_config));
    checks.push(doctor_backend_check(
        "storage_backend",
        "Scoped storage backend",
        &runtime_config.storage,
    ));
    checks.push(doctor_backend_check(
        "audit_backend",
        "Audit backend",
        &runtime_config.audit,
    ));
    checks.push(doctor_host_provider_check(&runtime_config));
    checks.push(doctor_sandbox_gate_check());
    checks.push(doctor_server_health_check(options.server));

    let summary = doctor_summary(&checks);
    let status = if summary["fail"].as_u64().unwrap_or_default() > 0 {
        "error"
    } else if summary["warn"].as_u64().unwrap_or_default() > 0
        || summary["skip"].as_u64().unwrap_or_default() > 0
    {
        "warning"
    } else {
        "ok"
    };
    let command_status = if options.ci && status == "error" {
        "failed"
    } else {
        "ok"
    };
    let check_values = checks.iter().map(doctor_check_json).collect::<Vec<_>>();

    Ok(json!({
        "schemaVersion": DOCTOR_REPORT_SCHEMA_VERSION,
        "status": status,
        "commandStatus": command_status,
        "reportStatus": status,
        "ci": options.ci,
        "runtimeConfig": config_source,
        "summary": summary,
        "humanSummary": doctor_human_summary(&checks),
        "checks": check_values,
        "redaction": {
            "appliedByDefault": true,
            "policy": "dock.doctor-redaction.v1",
            "localPaths": "redacted",
            "credentialMaterial": "omitted"
        },
    }))
}

fn doctor_runtime_config(
    path: Option<&Path>,
    checks: &mut Vec<DoctorCheck>,
) -> (RuntimeConfig, Value) {
    let Some(path) = path else {
        return (
            RuntimeConfig::default(),
            json!({
                "source": "built-in-default",
                "path": Value::Null,
                "loaded": true,
            }),
        );
    };
    let (display_path, redacted) = report_path(path);
    match std::fs::read_to_string(path) {
        Ok(content) => match RuntimeConfig::from_json_str(&content) {
            Ok(config) => (
                config,
                json!({
                    "source": "file",
                    "path": display_path,
                    "redacted": redacted,
                    "loaded": true,
                }),
            ),
            Err(error) => {
                checks.push(DoctorCheck {
                    id: "runtime_config",
                    title: "Runtime config",
                    status: DoctorCheckStatus::Fail,
                    severity: "high",
                    evidence: json!({
                        "source": "file",
                        "path": display_path,
                        "redacted": redacted,
                        "loaded": false,
                        "message": redact_text(&error.to_string()),
                    }),
                    suggestion:
                        "Fix the runtime config JSON before using it for release diagnostics.",
                });
                (
                    RuntimeConfig::default(),
                    json!({
                        "source": "file",
                        "path": display_path,
                        "redacted": redacted,
                        "loaded": false,
                    }),
                )
            }
        },
        Err(error) => {
            checks.push(DoctorCheck {
                id: "runtime_config",
                title: "Runtime config",
                status: DoctorCheckStatus::Fail,
                severity: "high",
                evidence: json!({
                    "source": "file",
                    "path": display_path,
                    "redacted": redacted,
                    "loaded": false,
                    "message": redact_text(&error.to_string()),
                }),
                suggestion: "Provide a readable runtime config file or omit --runtime-config to inspect defaults.",
            });
            (
                RuntimeConfig::default(),
                json!({
                    "source": "file",
                    "path": display_path,
                    "redacted": redacted,
                    "loaded": false,
                }),
            )
        }
    }
}

fn doctor_toolchain_check() -> DoctorCheck {
    let toolchain_path = default_project_root()
        .map(|root| root.join("rust-toolchain.toml"))
        .unwrap_or_else(|_| PathBuf::from("rust-toolchain.toml"));
    let expected = std::fs::read_to_string(toolchain_path)
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find_map(|line| line.trim().strip_prefix("channel = "))
                .map(|value| value.trim_matches('"').to_owned())
        });
    let rustc = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| {
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        });
    let matches_expected = expected
        .as_ref()
        .zip(rustc.as_ref())
        .is_some_and(|(expected, rustc)| rustc.contains(expected));
    let status = match (&expected, &rustc) {
        (Some(_), Some(_)) if matches_expected => DoctorCheckStatus::Pass,
        (Some(_), Some(_)) => DoctorCheckStatus::Warn,
        _ => DoctorCheckStatus::Fail,
    };
    DoctorCheck {
        id: "rust_toolchain",
        title: "Rust toolchain",
        status,
        severity: if status == DoctorCheckStatus::Fail {
            "high"
        } else {
            "medium"
        },
        evidence: json!({
            "expectedChannel": expected,
            "rustc": rustc.map(|value| redact_text(&value)),
            "matchesPinnedToolchain": matches_expected,
        }),
        suggestion: "Install the pinned Rust toolchain and run the repository gates from the workspace root.",
    }
}

fn doctor_runtime_config_check(config: &RuntimeConfig, loaded: bool) -> DoctorCheck {
    if !loaded {
        return DoctorCheck {
            id: "runtime_config_contract",
            title: "Runtime config contract",
            status: DoctorCheckStatus::Skip,
            severity: "high",
            evidence: json!({
                "reason": "Runtime config file was not loaded.",
            }),
            suggestion: "Fix runtime config loading first, then rerun doctor.",
        };
    }
    let validation = config.validate();
    let issue_codes = validation
        .issues
        .iter()
        .map(|issue| redact_text(&issue.code))
        .collect::<Vec<_>>();
    let release_blocker_codes = validation
        .release_blockers
        .iter()
        .map(|blocker| redact_text(&blocker.code))
        .collect::<Vec<_>>();
    DoctorCheck {
        id: "runtime_config_contract",
        title: "Runtime config contract",
        status: if validation.valid && !validation.release_blocked {
            DoctorCheckStatus::Pass
        } else {
            DoctorCheckStatus::Fail
        },
        severity: "high",
        evidence: json!({
            "profile": validation.profile,
            "valid": validation.valid,
            "releaseBlocked": validation.release_blocked,
            "issueCount": validation.issues.len(),
            "releaseBlockerCount": validation.release_blockers.len(),
            "issueCodes": issue_codes,
            "releaseBlockerCodes": release_blocker_codes,
        }),
        suggestion:
            "Resolve runtime config errors and production release blockers before CI certification.",
    }
}

fn doctor_workspace_check() -> DoctorCheck {
    match default_project_root() {
        Ok(root) => {
            let coffee = root.join("examples/coffee-skill").is_dir();
            let cargo = root.join("Cargo.toml").is_file();
            DoctorCheck {
                id: "workspace",
                title: "Workspace layout",
                status: if coffee && cargo {
                    DoctorCheckStatus::Pass
                } else {
                    DoctorCheckStatus::Fail
                },
                severity: "high",
                evidence: json!({
                    "cargoToml": cargo,
                    "coffeeSkillFixture": coffee,
                    "root": "[REDACTED]",
                }),
                suggestion:
                    "Run doctor from the anp-miniapp-dock workspace or provide explicit paths.",
            }
        }
        Err(error) => DoctorCheck {
            id: "workspace",
            title: "Workspace layout",
            status: DoctorCheckStatus::Fail,
            severity: "high",
            evidence: json!({
                "message": redact_text(&error.to_string()),
            }),
            suggestion: "Run doctor from the anp-miniapp-dock workspace or provide explicit paths.",
        },
    }
}

fn doctor_skill_check(skill_path: Option<&Path>) -> Result<DoctorCheck, CliError> {
    let path = match skill_path {
        Some(path) => path.to_path_buf(),
        None => match default_project_root() {
            Ok(root) => root.join("examples/coffee-skill"),
            Err(_) => {
                return Ok(DoctorCheck {
                    id: "skill_package",
                    title: "Skill package",
                    status: DoctorCheckStatus::Skip,
                    severity: "low",
                    evidence: json!({
                        "reason": "No --skill was provided and the workspace default could not be located.",
                    }),
                    suggestion: "Pass --skill <path> when diagnosing a specific Skill package.",
                });
            }
        },
    };
    let skill_ref = validate_skill_ref(&path);
    match load_skill(&path) {
        Ok(skill) => {
            let release_blockers =
                validate_release_blockers(&skill, validate_api_registration(&skill).as_ref());
            Ok(DoctorCheck {
                id: "skill_package",
                title: "Skill package",
                status: if release_blockers.is_empty() {
                    DoctorCheckStatus::Pass
                } else {
                    DoctorCheckStatus::Warn
                },
                severity: "medium",
                evidence: json!({
                    "skillRef": skill_ref,
                    "skillId": skill_id(&skill),
                    "releaseBlockerCount": release_blockers.len(),
                    "supplyChainStatus": skill.integrity.status.as_str(),
                    "productionReady": skill.integrity.production_ready,
                }),
                suggestion:
                    "Run validate and test-skill for a full compatibility and fixture report.",
            })
        }
        Err(error) => Ok(DoctorCheck {
            id: "skill_package",
            title: "Skill package",
            status: DoctorCheckStatus::Fail,
            severity: "high",
            evidence: json!({
                "skillRef": skill_ref,
                "message": redact_text(&error.to_string()),
            }),
            suggestion: "Fix the Skill package before running runtime or release diagnostics.",
        }),
    }
}

fn doctor_identity_checks(options: &DoctorOptions<'_>) -> Vec<DoctorCheck> {
    let has_explicit_identity = options.did_document.is_some()
        || options.private_key.is_some()
        || options.user_did.is_some()
        || options.identity_handle.is_some()
        || options.identity_root.is_some();
    let did_document = options.did_document.map(Path::to_path_buf);
    let private_key = options.private_key.map(Path::to_path_buf);
    let user_did = options.user_did.map(ToOwned::to_owned);
    let agent_did = options.agent_did.map(ToOwned::to_owned);
    let identity_handle = options.identity_handle.map(ToOwned::to_owned);
    let identity_root = options.identity_root.map(Path::to_path_buf);
    let auth = DemoAuthConfig::from_inputs(
        did_document,
        private_key,
        user_did,
        agent_did,
        identity_handle,
        identity_root,
        EnvConfigSource,
    );

    match auth {
        Ok(config) => {
            let did_document_valid = did_from_document_path(&config.credential.did_document_path)
                .map(|did| did == config.user_did)
                .unwrap_or(false);
            let readable = config
                .credential
                .clone()
                .without_private_key_permission_check()
                .validate()
                .is_ok();
            let mut strict = config.credential.clone();
            strict.check_private_key_permissions = true;
            let permission_result = strict.validate();
            vec![
                DoctorCheck {
                    id: "did_identity",
                    title: "DID identity",
                    status: if readable && did_document_valid {
                        DoctorCheckStatus::Pass
                    } else {
                        DoctorCheckStatus::Fail
                    },
                    severity: "high",
                    evidence: json!({
                        "didDocument": "configured",
                        "credential": "configured-redacted",
                        "userDid": config.user_did,
                        "agentDid": config.agent_did,
                        "documentMatchesUserDid": did_document_valid,
                    }),
                    suggestion: "Provide a readable DID document and matching signing credential for local ANP auth.",
                },
                DoctorCheck {
                    id: "credential_permissions",
                    title: "Credential file permissions",
                    status: if permission_result.is_ok() {
                        DoctorCheckStatus::Pass
                    } else {
                        DoctorCheckStatus::Fail
                    },
                    severity: "high",
                    evidence: json!({
                        "checked": true,
                        "mode": "owner-only-required",
                        "message": permission_result.err().map(|error| redact_text(&error.to_string())),
                    }),
                    suggestion: "Restrict the signing credential file to owner-only permissions before use.",
                },
            ]
        }
        Err(error) if !has_explicit_identity && error == DidCredentialError::Unavailable => vec![
            DoctorCheck {
                id: "did_identity",
                title: "DID identity",
                status: DoctorCheckStatus::Skip,
                severity: "medium",
                evidence: json!({
                    "reason": "No DID identity was configured and no default project identity was found.",
                }),
                suggestion: "Pass DID identity flags or configure ANP_DOCK_DID_DOCUMENT and ANP_DOCK_PRIVATE_KEY for auth diagnostics.",
            },
            DoctorCheck {
                id: "credential_permissions",
                title: "Credential file permissions",
                status: DoctorCheckStatus::Skip,
                severity: "medium",
                evidence: json!({
                    "reason": "No signing credential was configured.",
                }),
                suggestion: "Configure a local signing credential to enable permission diagnostics.",
            },
        ],
        Err(error) => vec![
            DoctorCheck {
                id: "did_identity",
                title: "DID identity",
                status: DoctorCheckStatus::Fail,
                severity: "high",
                evidence: json!({
                    "message": redact_text(&error.to_string()),
                }),
                suggestion: "Provide a complete DID identity configuration or identity store handle.",
            },
            DoctorCheck {
                id: "credential_permissions",
                title: "Credential file permissions",
                status: DoctorCheckStatus::Skip,
                severity: "medium",
                evidence: json!({
                    "reason": "DID identity did not resolve, so signing credential permissions were not checked.",
                }),
                suggestion: "Fix DID identity configuration first, then rerun doctor.",
            },
        ],
    }
}

fn doctor_resolver_check(config: &RuntimeConfig) -> DoctorCheck {
    let configured = config.resolver.provider.is_some() || config.resolver.trust_anchor.is_some();
    DoctorCheck {
        id: "trusted_resolver",
        title: "Trusted DID resolver",
        status: if configured {
            DoctorCheckStatus::Pass
        } else {
            DoctorCheckStatus::Warn
        },
        severity: "high",
        evidence: json!({
            "provider": config.resolver.provider.is_some(),
            "trustAnchor": config.resolver.trust_anchor.is_some(),
            "cacheTtlSeconds": config.resolver.cache_ttl_seconds,
            "productionReady": configured,
        }),
        suggestion:
            "Configure a trusted resolver provider or trust anchor before production release.",
    }
}

fn doctor_allowlist_check(config: &RuntimeConfig) -> DoctorCheck {
    let count = config.allowlist.network_rules.len();
    DoctorCheck {
        id: "network_allowlist",
        title: "Network allowlist",
        status: if count > 0 {
            DoctorCheckStatus::Pass
        } else {
            DoctorCheckStatus::Warn
        },
        severity: "high",
        evidence: json!({
            "networkRuleCount": count,
            "productionReady": count > 0,
        }),
        suggestion: "Configure explicit network allowlist rules for Host-managed request brokers.",
    }
}

fn doctor_backend_check(
    id: &'static str,
    title: &'static str,
    backend: &dock_core::RuntimeDataBackendConfig,
) -> DoctorCheck {
    let backend_name = match backend.backend {
        dock_core::RuntimeDataBackendKind::InMemory => "inMemory",
        dock_core::RuntimeDataBackendKind::File => "file",
        dock_core::RuntimeDataBackendKind::Sqlite => "sqlite",
        dock_core::RuntimeDataBackendKind::EncryptedSqlite => "encryptedSqlite",
        dock_core::RuntimeDataBackendKind::HostProvider => "hostProvider",
    };
    let production_ready = matches!(
        backend.backend,
        dock_core::RuntimeDataBackendKind::EncryptedSqlite
            | dock_core::RuntimeDataBackendKind::HostProvider
    );
    DoctorCheck {
        id,
        title,
        status: if production_ready {
            DoctorCheckStatus::Pass
        } else {
            DoctorCheckStatus::Warn
        },
        severity: "high",
        evidence: json!({
            "backend": backend_name,
            "pathRef": backend.path_ref.is_some(),
            "provider": backend.provider.is_some(),
            "quotaBytes": backend.quota_bytes,
            "retentionDays": backend.retention_days,
            "productionReady": production_ready,
        }),
        suggestion: "Use a Host provider or encrypted backend for production persistence.",
    }
}

fn doctor_host_provider_check(config: &RuntimeConfig) -> DoctorCheck {
    let provider_count = config.host_providers.len();
    let has_mock_or_dev = config
        .host_providers
        .iter()
        .any(|provider| provider.mock || provider.dev_only);
    let capabilities = config
        .host_providers
        .iter()
        .flat_map(|provider| provider.capabilities.iter().map(|item| redact_text(item)))
        .collect::<Vec<_>>();
    DoctorCheck {
        id: "host_providers",
        title: "Host providers",
        status: if provider_count == 0 || has_mock_or_dev {
            DoctorCheckStatus::Warn
        } else {
            DoctorCheckStatus::Pass
        },
        severity: "high",
        evidence: json!({
            "count": provider_count,
            "capabilities": capabilities,
            "hasMockOrDevOnly": has_mock_or_dev,
            "productionReady": provider_count > 0 && !has_mock_or_dev,
        }),
        suggestion:
            "Configure production Host providers for render, consent, high-risk APIs, and actions.",
    }
}

fn doctor_sandbox_gate_check() -> DoctorCheck {
    let files = [
        "crates/js-runtime-quickjs/tests/middleware_chain.rs",
        "crates/component-runtime/tests/component_lifecycle.rs",
        "crates/wx-compat/tests/component_permissions.rs",
    ];
    let root = default_project_root().ok();
    let present = files
        .iter()
        .filter(|path| {
            root.as_ref()
                .map(|root| root.join(path).is_file())
                .unwrap_or_else(|| Path::new(path).is_file())
        })
        .count();
    DoctorCheck {
        id: "sandbox_gates",
        title: "Sandbox gates",
        status: if present == files.len() {
            DoctorCheckStatus::Warn
        } else {
            DoctorCheckStatus::Fail
        },
        severity: "high",
        evidence: json!({
            "gateFilesPresent": present,
            "gateFilesExpected": files.len(),
            "executedByDoctor": false,
            "commands": [
                "cargo test -p js-runtime-quickjs sandbox",
                "cargo test -p component-runtime sandbox",
                "cargo test -p wx-compat permission"
            ],
        }),
        suggestion: "Run the sandbox and permission gates before release; doctor records the gate surface but does not execute heavy tests.",
    }
}

fn doctor_server_health_check(server: Option<&str>) -> DoctorCheck {
    let Some(server) = server.filter(|server| !server.trim().is_empty()) else {
        return DoctorCheck {
            id: "server_health",
            title: "Remote server health",
            status: DoctorCheckStatus::Skip,
            severity: "medium",
            evidence: json!({
                "reason": "No --server was provided.",
            }),
            suggestion: "Pass --server http://host:port to check a local or remote merchant health endpoint.",
        };
    };
    match DemoHttpClient::new(server).get_json("/health", None) {
        Ok(value) => DoctorCheck {
            id: "server_health",
            title: "Remote server health",
            status: DoctorCheckStatus::Pass,
            severity: "medium",
            evidence: json!({
                "server": "[REDACTED]",
                "reachable": true,
                "health": redact_metadata_value(&value),
            }),
            suggestion: "No action required for the health endpoint.",
        },
        Err(error) => DoctorCheck {
            id: "server_health",
            title: "Remote server health",
            status: DoctorCheckStatus::Fail,
            severity: "medium",
            evidence: json!({
                "server": "[REDACTED]",
                "reachable": false,
                "message": redact_text(&error.to_string()),
            }),
            suggestion: "Start the merchant server or check the server URL before running demo or release checks.",
        },
    }
}

fn doctor_summary(checks: &[DoctorCheck]) -> Value {
    let mut pass = 0_u64;
    let mut warn = 0_u64;
    let mut fail = 0_u64;
    let mut skip = 0_u64;
    for check in checks {
        match check.status {
            DoctorCheckStatus::Pass => pass += 1,
            DoctorCheckStatus::Warn => warn += 1,
            DoctorCheckStatus::Fail => fail += 1,
            DoctorCheckStatus::Skip => skip += 1,
        }
    }
    json!({
        "total": checks.len(),
        "pass": pass,
        "warn": warn,
        "fail": fail,
        "skip": skip,
        "skipCountsAsPass": false,
    })
}

fn doctor_human_summary(checks: &[DoctorCheck]) -> Vec<String> {
    checks
        .iter()
        .map(|check| {
            format!(
                "{}: {} - {}",
                check.id,
                check.status.as_str(),
                check.suggestion
            )
        })
        .map(|line| redact_text(&line))
        .collect()
}

fn doctor_check_json(check: &DoctorCheck) -> Value {
    json!({
        "id": check.id,
        "title": check.title,
        "status": check.status.as_str(),
        "severity": check.severity,
        "evidence": check.evidence,
        "suggestion": check.suggestion,
    })
}

fn import_wechat_mcp(options: ImportOptions<'_>) -> Result<Value, CliError> {
    let source_root = canonical_dir(options.source)?;
    let dest_root = options.dest.map(resolve_import_destination);
    let dest_root = match dest_root {
        Some(result) => Some(result?),
        None => None,
    };

    let structure = import_structure_report(&source_root)?;
    let mut blockers = import_blockers(&structure);
    let app_agent_skills = read_app_agent_skills(&source_root)?;
    let app_agent_skill_count = app_agent_skills
        .get("items")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let loaded_skill = load_skill(&source_root);
    let (validation_report, compatibility_report, skill_id, migration_patch) = match loaded_skill {
        Ok(skill) => {
            let registration = validate_api_registration(&skill);
            let api_reports = validate_api_reports(&skill, registration.as_ref());
            let component_reports = validate_component_reports(&skill);
            let fallbacks = validate_fallbacks(&skill, &component_reports);
            let release_blockers = validate_release_blockers(&skill, registration.as_ref());
            let repair_suggestions = validate_repair_suggestions(
                &skill.validation,
                &api_reports,
                &component_reports,
                &fallbacks,
                &release_blockers,
            );
            (
                validation_summary(&skill.validation),
                json!({
                    "status": validate_report_status(&skill.validation, &release_blockers),
                    "compatibilityLevel": compatibility_level(&skill.validation, &release_blockers),
                    "apis": api_reports,
                    "components": component_reports,
                    "permissions": validate_permissions(&validate_component_reports(&skill)),
                    "risks": validate_risks(&skill),
                    "fallbacks": fallbacks,
                    "releaseBlockers": release_blockers,
                    "repairSuggestions": repair_suggestions,
                    "supplyChain": supply_chain_report(&skill),
                    "releaseReadiness": validate_release_readiness(&skill, &validate_release_blockers(&skill, registration.as_ref())),
                }),
                skill_id(&skill),
                import_patch_suggestions(&skill),
            )
        }
        Err(error) => {
            blockers.push(json!({
                "code": "load_skill_failed",
                "severity": "blocker",
                "message": redact_text(&error.to_string()),
                "suggestion": "Fix required MiniApp MCP package files before importing or validating.",
            }));
            (
                Value::Null,
                Value::Null,
                source_root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(DEFAULT_SKILL_ID)
                    .to_owned(),
                json!({
                    "status": "not-generated",
                    "reason": "Skill package could not be loaded safely.",
                    "changes": []
                }),
            )
        }
    };

    if dest_root.is_none() && !options.dry_run {
        blockers.push(json!({
            "code": "destination_required",
            "severity": "blocker",
            "message": "Safe copy requires --dest and --write.",
            "suggestion": "Rerun with --dest <dir> --write after reviewing the dry-run report.",
        }));
    }

    if let Some(dest) = &dest_root {
        if dest == &source_root || dest.starts_with(&source_root) || source_root.starts_with(dest) {
            blockers.push(json!({
                "code": "unsafe_destination",
                "severity": "blocker",
                "message": "Import destination must be outside the source tree and must not contain the source tree.",
                "suggestion": "Choose a separate controlled test directory for imported Skill packages.",
            }));
        }
    }

    let copy_plan = match &dest_root {
        Some(dest) => import_copy_plan(&source_root, dest, options.overwrite)?,
        None => Vec::new(),
    };
    for entry in &copy_plan {
        if entry
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| status == "blocked")
        {
            blockers.push(json!({
                "code": entry.get("code").cloned().unwrap_or_else(|| json!("copy_blocked")),
                "severity": "blocker",
                "path": entry.get("path").cloned(),
                "message": entry.get("message").cloned().unwrap_or_else(|| json!("Copy plan is blocked.")),
                "suggestion": entry.get("suggestion").cloned().unwrap_or_else(|| json!("Review import copy plan before writing.")),
            }));
        }
    }

    let copied = if options.dry_run || !blockers.is_empty() {
        Vec::new()
    } else if let Some(dest) = &dest_root {
        execute_import_copy_plan(&source_root, dest, &copy_plan)?;
        copy_plan
            .iter()
            .filter(|entry| {
                entry
                    .get("kind")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kind == "file")
            })
            .filter_map(|entry| entry.get("path").cloned())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let status = if blockers.is_empty() {
        if options.dry_run {
            "dry-run"
        } else {
            "copied"
        }
    } else {
        "blocked"
    };

    Ok(json!({
        "schemaVersion": IMPORT_REPORT_SCHEMA_VERSION,
        "status": status,
        "commandStatus": "ok",
        "skillId": skill_id,
        "source": validate_skill_ref(options.source),
        "destination": dest_root.as_ref().map(|dest| validate_skill_ref(dest)),
        "mode": {
            "dryRun": options.dry_run,
            "write": !options.dry_run,
            "overwrite": options.overwrite,
            "generatePatch": options.generate_patch,
            "includeFixtures": options.include_fixtures,
            "note": "import-wechat-mcp is a migration helper; copied packages still require validate/test-skill/doctor and production Host review."
        },
        "structure": structure,
        "appJson": app_agent_skills,
        "compatibilityReport": compatibility_report,
        "validation": validation_report,
        "migrationPatch": if options.generate_patch { migration_patch } else { json!({
            "status": "disabled",
            "changes": []
        }) },
        "copyPlan": copy_plan,
        "copied": copied,
        "blockers": blockers,
        "nextCommands": import_next_commands(dest_root.as_ref(), options.dry_run, blockers.is_empty(), app_agent_skill_count),
    }))
}

fn canonical_dir(path: &Path) -> Result<PathBuf, CliError> {
    let canonical = std::fs::canonicalize(path)?;
    let metadata = std::fs::symlink_metadata(&canonical)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CliError::Demo(
            "import path must be a real directory, not a symlink or file".to_owned(),
        ));
    }
    Ok(canonical)
}

fn resolve_import_destination(dest: &Path) -> Result<PathBuf, CliError> {
    if dest.as_os_str().is_empty()
        || dest
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(CliError::Demo(
            "import destination must be a concrete directory without parent traversal".to_owned(),
        ));
    }
    if dest.exists() {
        return canonical_dir(dest);
    }

    let absolute = if dest.is_absolute() {
        dest.to_path_buf()
    } else {
        std::env::current_dir()?.join(dest)
    };
    let mut missing = Vec::new();
    let mut ancestor = absolute.as_path();
    while !ancestor.exists() {
        let Some(name) = ancestor.file_name() else {
            return Err(CliError::Demo(
                "import destination must have an existing parent directory".to_owned(),
            ));
        };
        missing.push(name.to_owned());
        ancestor = ancestor.parent().ok_or_else(|| {
            CliError::Demo("import destination must have an existing parent directory".to_owned())
        })?;
    }

    let mut resolved = canonical_dir(ancestor)?;
    for component in missing.iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn import_structure_report(root: &Path) -> Result<Value, CliError> {
    let required = ["SKILL.md", "mcp.json", "index.js"]
        .into_iter()
        .map(|path| {
            let target = root.join(path);
            json!({
                "path": path,
                "present": target.is_file(),
            })
        })
        .collect::<Vec<_>>();
    let api_modules = import_directory_files(root, Path::new("apis"), "js")?;
    let components = import_component_dirs(root)?;
    let files = import_source_files(root)?;
    let symlinks = files
        .iter()
        .filter(|file| file.get("kind").and_then(Value::as_str) == Some("symlink"))
        .cloned()
        .collect::<Vec<_>>();

    Ok(json!({
        "requiredFiles": required,
        "apiModules": api_modules,
        "components": components,
        "files": files,
        "symlinks": symlinks,
    }))
}

fn import_blockers(structure: &Value) -> Vec<Value> {
    let mut blockers = Vec::new();
    if let Some(required) = structure.get("requiredFiles").and_then(Value::as_array) {
        for file in required {
            if file.get("present").and_then(Value::as_bool) != Some(true) {
                blockers.push(json!({
                    "code": "missing_required_file",
                    "severity": "blocker",
                    "path": file.get("path").cloned(),
                    "message": "Required MiniApp MCP Skill file is missing.",
                    "suggestion": "Provide SKILL.md, mcp.json, and index.js before import.",
                }));
            }
        }
    }
    if let Some(symlinks) = structure.get("symlinks").and_then(Value::as_array) {
        for link in symlinks {
            blockers.push(json!({
                "code": "symlink_denied",
                "severity": "blocker",
                "path": link.get("path").cloned(),
                "message": "Symlinks are not copied by import-wechat-mcp.",
                "suggestion": "Replace symlinks with real files inside the Skill package.",
            }));
        }
    }
    blockers
}

fn import_directory_files(
    root: &Path,
    relative: &Path,
    extension: &str,
) -> Result<Vec<Value>, CliError> {
    let dir = root.join(relative);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.is_file() && path.extension().is_some_and(|found| found == extension) {
            files.push(json!({
                "path": safe_relative_path(root, &path)?,
                "kind": "file",
                "sizeBytes": metadata.len(),
            }));
        }
    }
    files.sort_by(|left, right| {
        left.get("path")
            .and_then(Value::as_str)
            .cmp(&right.get("path").and_then(Value::as_str))
    });
    Ok(files)
}

fn import_component_dirs(root: &Path) -> Result<Vec<Value>, CliError> {
    let components_dir = root.join("components");
    if !components_dir.exists() {
        return Ok(Vec::new());
    }
    let mut components = Vec::new();
    for entry in std::fs::read_dir(&components_dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            let relative = safe_relative_path(root, &path)?;
            let mut files = Vec::new();
            for name in ["index.js", "index.wxml", "index.wxss", "index.json"] {
                files.push(json!({
                    "path": format!("{relative}/{name}"),
                    "present": path.join(name).is_file(),
                }));
            }
            components.push(json!({
                "path": format!("{relative}/index"),
                "directory": relative,
                "files": files,
            }));
        }
    }
    components.sort_by(|left, right| {
        left.get("path")
            .and_then(Value::as_str)
            .cmp(&right.get("path").and_then(Value::as_str))
    });
    Ok(components)
}

fn import_source_files(root: &Path) -> Result<Vec<Value>, CliError> {
    let mut files = Vec::new();
    import_source_files_inner(root, root, &mut files)?;
    files.sort_by(|left, right| {
        left.get("path")
            .and_then(Value::as_str)
            .cmp(&right.get("path").and_then(Value::as_str))
    });
    Ok(files)
}

fn import_source_files_inner(
    root: &Path,
    dir: &Path,
    files: &mut Vec<Value>,
) -> Result<(), CliError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        let relative = safe_relative_path(root, &path)?;
        let kind = if metadata.file_type().is_symlink() {
            "symlink"
        } else if metadata.is_dir() {
            "directory"
        } else {
            "file"
        };
        files.push(json!({
            "path": relative,
            "kind": kind,
            "sizeBytes": if metadata.is_file() { Some(metadata.len()) } else { None },
        }));
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            import_source_files_inner(root, &path, files)?;
        }
    }
    Ok(())
}

fn read_app_agent_skills(root: &Path) -> Result<Value, CliError> {
    let path = root.join("app.json");
    if !path.exists() {
        return Ok(json!({
            "status": "not-found",
            "items": [],
            "note": "app.json is optional for standalone MiniApp MCP Skill packages."
        }));
    }
    let source = std::fs::read_to_string(&path)?;
    let value: Value = serde_json::from_str(&source).map_err(|source| CliError::Json {
        label: "app.json".to_owned(),
        source,
    })?;
    let items = value
        .get("agent")
        .and_then(|agent| agent.get("skills"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|item| redact_metadata_value(&item))
        .collect::<Vec<_>>();
    Ok(json!({
        "status": if items.is_empty() { "missing-agent-skills" } else { "found" },
        "path": "app.json",
        "items": items,
    }))
}

fn import_patch_suggestions(skill: &LoadedSkill) -> Value {
    let api_changes = skill
        .manifest
        .apis
        .iter()
        .map(|api| {
            let input_formats = api
                .input_formats()
                .into_iter()
                .map(|field| json!({ "path": field.path, "format": field.format }))
                .collect::<Vec<_>>();
            json!({
                "path": format!("mcp.json/apis/{}", api.name),
                "type": "suggestion",
                "operation": "merge-meta",
                "suggested": {
                    "_meta": {
                        "anp": {
                            "risk": api.meta.as_ref().and_then(|meta| meta.anp.as_ref()).and_then(|anp| anp.get("risk")).cloned().unwrap_or_else(|| json!("review-required")),
                            "hostProviderRequired": !input_formats.is_empty() || api_risk_requires_consent(api),
                            "didSession": "use ANP DID runtime session; do not copy WeChat login credentials into the Skill package",
                        }
                    }
                },
                "reason": if input_formats.is_empty() && !api_risk_requires_consent(api) {
                    "Record ANP runtime ownership for this Atomic API."
                } else {
                    "Formatted input or high-risk API requires Host provider, ConsentGate, and audit review."
                },
            })
        })
        .collect::<Vec<_>>();
    let component_changes = skill
        .manifest
        .components
        .iter()
        .filter(|component| component.dynamic_permission().is_some())
        .map(|component| {
            json!({
                "path": format!("mcp.json/components/{}", component.path),
                "type": "suggestion",
                "operation": "review-permission",
                "suggested": {
                    "permissions": {
                        "scope.dynamic": redact_metadata_value(component.dynamic_permission().unwrap_or(&Value::Null))
                    },
                    "_meta": {
                        "anp": {
                            "hostBoundary": "dynamic request/timer requires production Host policy and audit sink"
                        }
                    }
                },
                "reason": "Dynamic component capabilities must stay behind the Step 02-05 sandbox/resource gate and Host production policy.",
            })
        })
        .collect::<Vec<_>>();
    let mut changes = Vec::new();
    changes.extend(api_changes);
    changes.extend(component_changes);
    json!({
        "status": "suggested",
        "appliesAutomatically": false,
        "productionReady": false,
        "note": "Patch suggestions are advisory and must be reviewed manually before editing mcp.json.",
        "changes": changes,
    })
}

fn import_copy_plan(root: &Path, dest: &Path, overwrite: bool) -> Result<Vec<Value>, CliError> {
    let mut plan = Vec::new();
    for file in import_source_files(root)? {
        let Some(relative) = file.get("path").and_then(Value::as_str) else {
            continue;
        };
        let Some(kind) = file.get("kind").and_then(Value::as_str) else {
            continue;
        };
        let dest_path = dest.join(relative);
        let target_exists = dest_path.exists();
        let mut entry = json!({
            "path": relative,
            "kind": kind,
            "action": if kind == "directory" { "create-dir" } else { "copy" },
            "status": "planned",
            "overwrite": overwrite,
        });
        if kind == "symlink" {
            entry["status"] = json!("blocked");
            entry["code"] = json!("symlink_denied");
            entry["message"] = json!("Symlink copy is denied.");
            entry["suggestion"] =
                json!("Replace this symlink with a real file under the Skill root.");
        } else if target_exists && !overwrite {
            entry["status"] = json!("blocked");
            entry["code"] = json!("overwrite_required");
            entry["message"] =
                json!("Destination path already exists and --overwrite was not set.");
            entry["suggestion"] = json!("Review the existing destination and rerun with --overwrite only when replacement is intended.");
        }
        plan.push(entry);
    }
    Ok(plan)
}

fn execute_import_copy_plan(root: &Path, dest: &Path, plan: &[Value]) -> Result<(), CliError> {
    std::fs::create_dir_all(dest)?;
    for entry in plan {
        if entry.get("status").and_then(Value::as_str) == Some("blocked") {
            continue;
        }
        let Some(relative) = entry.get("path").and_then(Value::as_str) else {
            continue;
        };
        let source = root.join(relative);
        let target = dest.join(relative);
        match entry.get("kind").and_then(Value::as_str) {
            Some("directory") => std::fs::create_dir_all(&target)?,
            Some("file") => {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&source, &target)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn import_next_commands(
    dest: Option<&PathBuf>,
    dry_run: bool,
    can_write: bool,
    app_agent_skill_count: usize,
) -> Vec<Value> {
    let mut commands = Vec::new();
    if dry_run {
        if let Some(dest) = dest {
            commands.push(json!({
                "label": "safe-copy",
                "command": format!("dock-cli import-wechat-mcp <source> --dest {} --write", report_path(dest).0),
                "note": "Run only after reviewing the dry-run report and blockers.",
            }));
        } else {
            commands.push(json!({
                "label": "safe-copy",
                "command": "dock-cli import-wechat-mcp <source> --dest <controlled-test-dir> --write",
                "note": "Choose a controlled destination outside the source tree.",
            }));
        }
    }
    if can_write {
        let target = dest
            .map(|dest| report_path(dest).0)
            .unwrap_or_else(|| "<imported-skill>".to_owned());
        commands.push(json!({
            "label": "validate",
            "command": format!("dock-cli validate {target}"),
        }));
        commands.push(json!({
            "label": "test-skill",
            "command": format!("dock-cli test-skill {target}"),
            "note": "Generated empty-argument cases still require explicit developer fixtures for third-party Skills.",
        }));
    }
    if app_agent_skill_count > 1 {
        commands.push(json!({
            "label": "split-agent-skills",
            "command": "Review app.json agent.skills[] and import each Skill package independently.",
        }));
    }
    commands
}

fn validate_api_registration(skill: &LoadedSkill) -> Result<Vec<String>, String> {
    ApiVm::load_skill(skill.clone())
        .map(|vm| {
            vm.registered_apis()
                .iter()
                .map(|api| api.name.clone())
                .collect()
        })
        .map_err(|error| redact_text(&error.to_string()))
}

fn validate_api_reports(
    skill: &LoadedSkill,
    registration: Result<&Vec<String>, &String>,
) -> Vec<Value> {
    skill
        .manifest
        .apis
        .iter()
        .map(|api| {
            let input_formats = api
                .input_formats()
                .into_iter()
                .map(|field| json!({ "path": field.path, "format": field.format }))
                .collect::<Vec<_>>();
            let registered = registration
                .as_ref()
                .is_ok_and(|registered| registered.iter().any(|name| name == &api.name));
            let compatibility_status = api_compatibility_status(api, registered, &input_formats);
            let suggestion = api_report_suggestion(api, registered, &input_formats);
            json!({
                "name": api.name,
                "registered": registered,
                "componentPath": api.component_path(),
                "inputFormats": input_formats,
                "hasOutputSchema": api.output_schema.is_some(),
                "risk": api.meta.as_ref().and_then(|meta| meta.anp.as_ref()).and_then(|anp| anp.get("risk")).cloned(),
                "consentRequired": api_risk_requires_consent(api),
                "compatibilityStatus": compatibility_status,
                "severity": if compatibility_status == "supported" { "info" } else { "warning" },
                "status": if registered { "declared-and-registered" } else { "registration-unverified" },
                "suggestion": suggestion,
            })
        })
        .collect()
}

fn validate_component_reports(skill: &LoadedSkill) -> Vec<Value> {
    skill
        .manifest
        .components
        .iter()
        .map(|component| component_report(component, skill))
        .collect()
}

fn component_report(component: &ComponentDeclaration, skill: &LoadedSkill) -> Value {
    let loaded = skill.components.contains_key(&component.path);
    let metadata = manifest_component_metadata(&skill.manifest, &component.path).ok();
    let dynamic = metadata.as_ref().is_some_and(|metadata| metadata.dynamic);
    let compatibility_status = if !loaded {
        "fallback"
    } else if dynamic {
        "host-boundary"
    } else {
        "supported"
    };
    json!({
        "path": component.path,
        "loaded": loaded,
        "status": compatibility_status,
        "compatibilityStatus": compatibility_status,
        "severity": if loaded { "info" } else { "warning" },
        "relatedPage": metadata.as_ref().and_then(|metadata| metadata.related_page.clone()),
        "permissions": {
            "dynamic": dynamic,
            "scopeDynamic": metadata.as_ref().and_then(|metadata| metadata.scope_dynamic.clone())
        },
        "expirable": metadata.as_ref().is_some_and(|metadata| metadata.expirable),
        "expiredText": metadata.as_ref().and_then(|metadata| metadata.expired_text.clone()),
        "runtimeMetadata": metadata,
        "fallback": if loaded { Value::Null } else { json!({
            "reason": "component_load_failed",
            "fallback": "card-spec",
            "suggestion": "Keep components[].path aligned with the component package directory and _meta.ui.componentPath."
        }) },
        "suggestion": component_report_suggestion(loaded, dynamic),
    })
}

fn validate_permissions(component_reports: &[Value]) -> Value {
    let dynamic_components = component_reports
        .iter()
        .filter_map(|component| {
            component
                .get("permissions")
                .and_then(|permissions| permissions.get("dynamic"))
                .and_then(Value::as_bool)
                .filter(|dynamic| *dynamic)
                .and_then(|_| component.get("path"))
                .and_then(Value::as_str)
        })
        .collect::<Vec<_>>();
    let required_host_capabilities = dynamic_components
        .iter()
        .map(|component_path| {
            json!({
                "capability": "component.dynamic",
                "componentPath": component_path,
                "status": "host-boundary",
                "suggestion": "Review dynamic request/timer policy, background lifecycle, and audit sink before production release.",
            })
        })
        .collect::<Vec<_>>();

    json!({
        "status": if dynamic_components.is_empty() { "ok" } else { "host-boundary" },
        "dynamicComponents": dynamic_components,
        "requiredHostCapabilities": required_host_capabilities,
        "policy": "dynamic request/timer declarations require Step 02-05 runtime gate plus explicit Host production policy",
    })
}

fn validate_risks(skill: &LoadedSkill) -> Vec<Value> {
    skill
        .manifest
        .apis
        .iter()
        .filter_map(|api| {
            api.meta
                .as_ref()
                .and_then(|meta| meta.anp.as_ref())
                .and_then(|anp| anp.get("risk"))
                .map(|risk| {
                    let consent_required = matches!(risk.as_str(), Some("order" | "payment" | "high" | "l3" | "l4"));
                    json!({
                        "api": api.name,
                        "risk": risk,
                        "severity": risk_severity(risk.as_str()),
                        "status": if consent_required { "host-boundary" } else { "supported" },
                        "consentRequired": consent_required,
                        "suggestion": if consent_required {
                            "Keep this API behind ConsentGate, permission decision audit, and Host provider review."
                        } else {
                            "No additional high-risk consent boundary is required by the declared risk metadata."
                        },
                    })
                })
        })
        .collect()
}

fn validate_fallbacks(skill: &LoadedSkill, component_reports: &[Value]) -> Vec<Value> {
    let mut fallbacks = Vec::new();
    for api in &skill.manifest.apis {
        if api.component_path().is_none() {
            fallbacks.push(json!({
                "api": api.name,
                "fallback": "card-spec",
                "reason": "no_component_path",
                "severity": "warning",
                "suggestion": "Add _meta.ui.componentPath and a matching components[] package to enable Component Runtime rendering.",
            }));
        }
    }
    for component in component_reports {
        if component.get("loaded").and_then(Value::as_bool) == Some(false) {
            fallbacks.push(json!({
                "componentPath": component.get("path"),
                "fallback": "card-spec",
                "reason": "component_load_failed",
                "severity": "warning",
                "suggestion": "Fix the component package directory and rerun dock-cli validate before release.",
            }));
        }
    }
    fallbacks
}

fn validate_release_blockers(
    skill: &LoadedSkill,
    registration: Result<&Vec<String>, &String>,
) -> Vec<Value> {
    let mut blockers = Vec::new();
    if let Err(error) = registration {
        blockers.push(json!({
            "code": "api_registration_mismatch",
            "category": "compatibility",
            "severity": "blocker",
            "message": error,
            "suggestion": "Keep apis[].name aligned with index.js registerAPI calls before production validation.",
        }));
    }

    for warning in &skill.validation.warnings {
        if warning.category == ValidationIssueCategory::Production {
            blockers.push(json!({
                "code": "production_warning",
                "category": "production-readiness",
                "severity": "blocker",
                "path": warning.path.clone(),
                "message": warning.message.clone(),
                "suggestion": warning.suggestion.clone(),
            }));
        }
    }

    if !skill.integrity.production_ready {
        blockers.push(json!({
            "code": "supply_chain",
            "category": "supply-chain",
            "severity": "blocker",
            "status": skill.integrity.status.as_str(),
            "issueCodes": skill.integrity.issue_codes,
            "message": "Skill package is not production-ready under the Step 03-06 supply-chain gate.",
            "suggestion": "Attach publisher DID, sha256 digest, package signature, and trusted publisher policy before production release.",
        }));
    }

    blockers
}

fn supply_chain_report(skill: &LoadedSkill) -> Value {
    json!({
        "digest": {
            "algorithm": skill.integrity.digest.algorithm,
            "value": skill.integrity.digest.value,
        },
        "status": skill.integrity.status.as_str(),
        "publisherDid": skill.integrity.publisher_did,
        "signature": {
            "algorithm": skill.integrity.signature_algorithm,
            "keyId": skill.integrity.signature_key_id,
        },
        "trustedPublisher": skill.integrity.trusted_publisher,
        "quarantine": skill.integrity.quarantine,
        "productionReady": skill.integrity.production_ready,
        "issueCodes": skill.integrity.issue_codes,
        "warnings": skill.integrity.warnings,
    })
}

fn compatibility_level(report: &ValidationReport, release_blockers: &[Value]) -> &'static str {
    if !report.is_valid() {
        "invalid"
    } else if !release_blockers.is_empty() {
        "demo-only"
    } else if report.warnings.is_empty() {
        "supported"
    } else {
        "compatible-with-warnings"
    }
}

fn validate_report_status(report: &ValidationReport, release_blockers: &[Value]) -> &'static str {
    if !report.is_valid() {
        "error"
    } else if !release_blockers.is_empty() || !report.warnings.is_empty() {
        "warning"
    } else {
        "ok"
    }
}

fn validate_skill_ref(input_path: &Path) -> Value {
    let (path, redacted) = report_path(input_path);
    json!({
        "kind": "local-directory",
        "path": path,
        "redacted": redacted,
        "note": if redacted {
            "Absolute or sensitive local paths are redacted from validate reports."
        } else {
            "Path is reported as provided because it is relative to the current working directory."
        },
    })
}

fn report_path(path: &Path) -> (String, bool) {
    let display = path.to_string_lossy().replace('\\', "/");
    let lower = display.to_ascii_lowercase();
    let sensitive = path.is_absolute()
        || lower.contains("/home/")
        || lower.contains("/users/")
        || lower.contains("private")
        || lower.contains("secret")
        || lower.contains("token");

    if sensitive {
        ("[REDACTED]".to_owned(), true)
    } else if display.trim().is_empty() {
        (".".to_owned(), false)
    } else {
        (display, false)
    }
}

fn api_compatibility_status(
    api: &ApiDeclaration,
    registered: bool,
    input_formats: &[Value],
) -> &'static str {
    if !registered {
        "unsupported"
    } else if api_uses_demo_only_metadata(api) {
        "demo-only"
    } else if !input_formats.is_empty() {
        "host-boundary"
    } else {
        "supported"
    }
}

fn api_report_suggestion(
    api: &ApiDeclaration,
    registered: bool,
    input_formats: &[Value],
) -> &'static str {
    if !registered {
        "Register this API in index.js with the same name declared in mcp.json."
    } else if api_uses_demo_only_metadata(api) {
        "Remove demo-only localhost compatibility metadata before production release."
    } else if !input_formats.is_empty() {
        "Provide Host file/media handles and provider tests for formatted input fields."
    } else if api_risk_requires_consent(api) {
        "Keep high-risk execution behind ConsentGate and audit review."
    } else {
        "No repair required for the current validate gate."
    }
}

fn component_report_suggestion(loaded: bool, dynamic: bool) -> &'static str {
    if !loaded {
        "Fix the component package path and ensure index.wxml/index.js are inside the Skill root."
    } else if dynamic {
        "Review dynamic component request/timer policy and Host production boundary before release."
    } else {
        "No repair required for the current validate gate."
    }
}

fn api_uses_demo_only_metadata(api: &ApiDeclaration) -> bool {
    api.meta.as_ref().is_some_and(|meta| {
        meta.extra.contains_key("remoteLogin")
            || meta.extra.contains_key("compatLoginEndpoint")
            || meta
                .extra
                .get("requestAuthMode")
                .and_then(Value::as_str)
                .is_some_and(|mode| mode == "host-managed-bearer")
    })
}

fn api_risk_requires_consent(api: &ApiDeclaration) -> bool {
    api.meta
        .as_ref()
        .and_then(|meta| meta.anp.as_ref())
        .and_then(|anp| anp.get("risk"))
        .and_then(Value::as_str)
        .is_some_and(|risk| matches!(risk, "order" | "payment" | "high" | "l3" | "l4"))
}

fn risk_severity(risk: Option<&str>) -> &'static str {
    match risk {
        Some("payment" | "l4") => "critical",
        Some("order" | "high" | "l3") => "high",
        Some("medium" | "location" | "media") => "medium",
        _ => "low",
    }
}

fn validate_repair_suggestions(
    validation: &ValidationReport,
    api_reports: &[Value],
    component_reports: &[Value],
    fallbacks: &[Value],
    release_blockers: &[Value],
) -> Vec<Value> {
    let mut suggestions = Vec::new();

    for blocker in release_blockers {
        suggestions.push(json!({
            "source": "releaseBlockers",
            "code": blocker.get("code").cloned().unwrap_or_else(|| json!("release_blocker")),
            "path": blocker.get("path").cloned(),
            "suggestion": blocker.get("suggestion").cloned().unwrap_or_else(|| json!("Resolve this blocker before production release.")),
            "severity": blocker.get("severity").cloned().unwrap_or_else(|| json!("blocker")),
        }));
    }

    for warning in &validation.warnings {
        if warning.category != ValidationIssueCategory::Production {
            suggestions.push(json!({
                "source": "validation",
                "path": warning.path,
                "suggestion": warning.suggestion.clone().unwrap_or_else(|| warning.message.clone()),
                "severity": "warning",
            }));
        }
    }

    for api in api_reports {
        if api.get("compatibilityStatus").and_then(Value::as_str) != Some("supported") {
            suggestions.push(json!({
                "source": "apis",
                "api": api.get("name").cloned(),
                "suggestion": api.get("suggestion").cloned().unwrap_or_else(|| json!("Review this API compatibility status.")),
                "severity": api.get("severity").cloned().unwrap_or_else(|| json!("warning")),
            }));
        }
    }

    for component in component_reports {
        if component.get("compatibilityStatus").and_then(Value::as_str) != Some("supported") {
            suggestions.push(json!({
                "source": "components",
                "componentPath": component.get("path").cloned(),
                "suggestion": component.get("suggestion").cloned().unwrap_or_else(|| json!("Review this component compatibility status.")),
                "severity": component.get("severity").cloned().unwrap_or_else(|| json!("warning")),
            }));
        }
    }

    for fallback in fallbacks {
        suggestions.push(json!({
            "source": "fallbacks",
            "api": fallback.get("api").cloned(),
            "componentPath": fallback.get("componentPath").cloned(),
            "suggestion": fallback.get("suggestion").cloned().unwrap_or_else(|| json!("Review this fallback before release.")),
            "severity": fallback.get("severity").cloned().unwrap_or_else(|| json!("warning")),
        }));
    }

    suggestions
}

fn validate_release_readiness(skill: &LoadedSkill, release_blockers: &[Value]) -> Value {
    let checks = vec![
        json!({
            "code": "supply_chain",
            "status": skill.integrity.status.as_str(),
            "productionReady": skill.integrity.production_ready,
            "issueCodes": skill.integrity.issue_codes,
            "suggestion": if skill.integrity.production_ready {
                "Supply-chain metadata passed the local validate gate."
            } else {
                "Attach trusted publisher DID, sha256 digest, signature, and publisher allowlist before production release."
            },
        }),
        json!({
            "code": "host_provider_policy",
            "status": "not-evaluated-by-validate",
            "productionReady": false,
            "suggestion": "Run dock-cli doctor with the target Host/runtime config before production release.",
        }),
        json!({
            "code": "persistence_backends",
            "status": "not-evaluated-by-validate",
            "productionReady": false,
            "suggestion": "Use production Host secure token, storage, audit, and cache backends; local/in-memory backends remain dev-only.",
        }),
        json!({
            "code": "render_ir_snapshots",
            "status": "requires-fixture-gate",
            "productionReady": Value::Null,
            "suggestion": "Run fixture and snapshot gates for components that render user-visible cards.",
        }),
    ];

    json!({
        "status": if release_blockers.is_empty() { "requires-environment-gates" } else { "blocked" },
        "checks": checks,
    })
}

fn inspect(skill_path: &Path) -> Result<Value, CliError> {
    let skill = load_skill(skill_path)?;
    let skill_id = skill_id_for_path(&skill, skill_path);
    let registration = validate_api_registration(&skill);
    let registered_apis = registration
        .clone()
        .unwrap_or_else(|_| inspect_registered_api_scan(&skill));
    let registration_source = if registration.is_ok() {
        "api-vm-registration-trace"
    } else if registered_apis.is_empty() {
        "unknown-with-reason"
    } else {
        "static-register-api-scan"
    };
    let file_tree = inspect_file_tree(&skill.root)?;
    let wx_usage = inspect_wx_usage(&skill);
    let component_reports = validate_component_reports(&skill);
    let permissions = validate_permissions(&component_reports);
    let risks = validate_risks(&skill);
    let api_reports = inspect_api_reports(
        &skill,
        &registered_apis,
        registration_source,
        registration.as_ref().err(),
    );
    let warnings = inspect_warnings(&skill, registration.as_ref().err());
    let skill_ref = validate_skill_ref(skill_path);
    let package = inspect_package_summary(&skill, &file_tree);

    Ok(json!({
        "schemaVersion": "dock.inspect-report.v1",
        "status": if warnings.is_empty() { "ok" } else { "warning" },
        "commandStatus": "ok",
        "skillId": skill_id,
        "skillRef": skill_ref,
        "package": package,
        "files": file_tree,
        "apis": api_reports,
        "registeredApis": registered_apis,
        "registeredApisSource": registration_source,
        "components": component_reports,
        "permissions": permissions,
        "risks": risks,
        "wxApiUsage": wx_usage,
        "warnings": warnings,
        "validation": validation_summary(&skill.validation),
    }))
}

fn inspect_file_tree(root: &Path) -> Result<Vec<Value>, CliError> {
    let mut files = Vec::new();
    inspect_file_tree_inner(root, root, &mut files)?;
    files.sort_by(|left, right| {
        left.get("path")
            .and_then(Value::as_str)
            .cmp(&right.get("path").and_then(Value::as_str))
    });
    Ok(files)
}

fn inspect_file_tree_inner(
    root: &Path,
    dir: &Path,
    files: &mut Vec<Value>,
) -> Result<(), CliError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        let relative = safe_relative_path(root, &path)?;
        let kind = if metadata.file_type().is_symlink() {
            "symlink"
        } else if metadata.is_dir() {
            "directory"
        } else {
            "file"
        };
        files.push(json!({
            "path": relative,
            "kind": kind,
            "sizeBytes": if metadata.is_file() { Some(metadata.len()) } else { None },
        }));
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            inspect_file_tree_inner(root, &path, files)?;
        }
    }
    Ok(())
}

fn safe_relative_path(root: &Path, path: &Path) -> Result<String, CliError> {
    let relative = path.strip_prefix(root).map_err(|_| {
        CliError::Demo(format!(
            "inspect path escaped skill root: {}",
            redact_text(&path.display().to_string())
        ))
    })?;
    let rendered = relative
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    if rendered.contains("..") || rendered.contains('\0') || rendered.trim().is_empty() {
        return Err(CliError::Demo(
            "inspect encountered an unsafe package path".to_owned(),
        ));
    }
    Ok(rendered)
}

fn inspect_package_summary(skill: &LoadedSkill, file_tree: &[Value]) -> Value {
    json!({
        "entry": skill.entry_js.relative_path.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/"),
        "skillMd": skill.skill_md.relative_path.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/"),
        "fileCount": file_tree.len(),
        "apiModuleCount": skill.api_modules.len(),
        "componentCount": skill.components.len(),
        "supplyChain": supply_chain_report(skill),
    })
}

fn inspect_api_reports(
    skill: &LoadedSkill,
    registered_apis: &[String],
    registration_source: &str,
    registration_error: Option<&String>,
) -> Vec<Value> {
    skill
        .manifest
        .apis
        .iter()
        .map(|api| {
            let registered = registered_apis.iter().any(|name| name == &api.name);
            let api_module_path = skill
                .api_modules
                .get(&api.name)
                .map(|module| module.relative_path.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/"));
            let input_formats = api
                .input_formats()
                .into_iter()
                .map(|field| json!({ "path": field.path, "format": field.format }))
                .collect::<Vec<_>>();
            let compatibility_status = if registration_error.is_some() {
                if registered {
                    api_compatibility_status(api, registered, &input_formats)
                } else {
                    "unsupported"
                }
            } else {
                api_compatibility_status(api, registered, &input_formats)
            };
            json!({
                "name": api.name,
                "description": api.description,
                "registered": registered,
                "registrationStatus": if registered && registration_error.is_some() {
                    "registered-static-with-vm-error"
                } else if registered {
                    "declared-and-registered"
                } else {
                    "declared-only"
                },
                "registrationSource": registration_source,
                "registrationReason": registration_error.map(|error| redact_text(error)),
                "apiModule": api_module_path,
                "componentPath": api.component_path(),
                "risk": api.meta.as_ref().and_then(|meta| meta.anp.as_ref()).and_then(|anp| anp.get("risk")).cloned(),
                "consentRequired": api_risk_requires_consent(api),
                "inputFormats": input_formats,
                "hasOutputSchema": api.output_schema.is_some(),
                "compatibilityStatus": compatibility_status,
                "suggestion": api_report_suggestion(api, registered, &input_formats),
            })
        })
        .collect()
}

fn inspect_registered_api_scan(skill: &LoadedSkill) -> Vec<String> {
    let mut names = BTreeSet::new();
    scan_registered_api_names(&skill.entry_js.source, &mut names);
    for module in skill.api_modules.values() {
        scan_registered_api_names(&module.source, &mut names);
    }
    names.into_iter().collect()
}

fn scan_registered_api_names(source: &str, names: &mut BTreeSet<String>) {
    let bytes = source.as_bytes();
    let mut offset = 0;
    while let Some(found) = source[offset..].find("registerAPI") {
        let mut index = offset + found + "registerAPI".len();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b'(') {
            offset = index;
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let Some(quote @ (b'\'' | b'"' | b'`')) = bytes.get(index).copied() else {
            offset = index;
            continue;
        };
        index += 1;
        let start = index;
        while index < bytes.len() && bytes[index] != quote {
            if bytes[index] == b'\\' {
                index = index.saturating_add(2);
            } else {
                index += 1;
            }
        }
        if index <= bytes.len() && index > start {
            let name = &source[start..index];
            if !name.trim().is_empty() {
                names.insert(name.to_owned());
            }
        }
        offset = index.saturating_add(1);
    }
}

fn inspect_warnings(skill: &LoadedSkill, registration_error: Option<&String>) -> Vec<Value> {
    let mut warnings = Vec::new();
    if let Some(error) = registration_error {
        warnings.push(json!({
            "code": "api_registration_unknown",
            "message": redact_text(error),
            "suggestion": "Fix API VM registration errors before relying on inspect registrationStatus.",
        }));
    }
    for warning in &skill.validation.warnings {
        let mut warning_json = validation_issue_json(warning);
        if let Value::Object(fields) = &mut warning_json {
            fields.insert("code".to_owned(), json!("validation_warning"));
        }
        warnings.push(warning_json);
    }
    warnings
}

fn inspect_wx_usage(skill: &LoadedSkill) -> Value {
    let mut usages = Vec::new();
    scan_wx_usage(&mut usages, "index.js", &skill.entry_js.source);
    for module in skill.api_modules.values() {
        scan_wx_usage(
            &mut usages,
            &module
                .relative_path
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/"),
            &module.source,
        );
    }
    for component in skill.components.values() {
        for source in [
            component.index_js.as_ref(),
            component.index_wxml.as_ref(),
            component.index_wxss.as_ref(),
            component.index_json.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            scan_wx_usage(
                &mut usages,
                &source
                    .relative_path
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
                &source.source,
            );
        }
    }
    usages.sort_by(|left, right| {
        (
            left.get("api").and_then(Value::as_str),
            left.get("file").and_then(Value::as_str),
            left.get("line").and_then(Value::as_u64),
        )
            .cmp(&(
                right.get("api").and_then(Value::as_str),
                right.get("file").and_then(Value::as_str),
                right.get("line").and_then(Value::as_u64),
            ))
    });
    json!({
        "status": if usages.is_empty() { "unknown-with-reason" } else { "scanned" },
        "reason": "Static string scan over loaded Skill JS/component files; dynamic property access is reported as unknown and should be verified with test-skill.",
        "items": usages,
    })
}

fn scan_wx_usage(usages: &mut Vec<Value>, file: &str, source: &str) {
    for (line_index, line) in source.lines().enumerate() {
        let mut offset = 0;
        while let Some(found) = line[offset..].find("wx.") {
            let start = offset + found + 3;
            let Some((api, consumed)) = parse_wx_api(&line[start..]) else {
                offset = start;
                continue;
            };
            usages.push(json!({
                "api": format!("wx.{api}"),
                "file": file,
                "line": line_index + 1,
                "confidence": "static-string-scan",
            }));
            offset = start + consumed;
        }
    }
}

fn parse_wx_api(input: &str) -> Option<(String, usize)> {
    let mut consumed = 0;
    let mut parts = Vec::new();
    for segment in input.split('.') {
        let ident: String = segment
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect();
        if ident.is_empty() {
            break;
        }
        consumed += ident.len();
        parts.push(ident);
        if parts.len() == 2 {
            break;
        }
        if input.as_bytes().get(consumed) == Some(&b'.') {
            consumed += 1;
        } else {
            break;
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some((parts.join("."), consumed))
    }
}

fn test_skill(skill_path: &Path) -> Result<Value, CliError> {
    let skill = load_skill(skill_path)?;
    let fixture_plan = FixturePlan::from_skill(skill_path, &skill)?;
    let mut cases = Vec::new();
    for case in &fixture_plan.cases {
        cases.push(run_fixture_case(skill_path, &skill, case)?);
    }
    let failed_count = cases
        .iter()
        .filter(|case| case.get("status").and_then(Value::as_str) != Some("pass"))
        .count();

    Ok(json!({
        "schemaVersion": "dock.test-skill-report.v1",
        "status": if failed_count == 0 { "ok" } else { "failed" },
        "commandStatus": "ok",
        "skillId": skill_id_for_path(&skill, skill_path),
        "skillRef": validate_skill_ref(skill_path),
        "fixtureSet": fixture_plan.name,
        "mockProvider": {
            "status": "dev-only",
            "productionReady": false,
            "provider": "dock-cli-headless-fixture",
            "consentProvider": DEV_HEADLESS_CONSENT_PROVIDER,
            "decisionActor": DEV_HEADLESS_DECISION_ACTOR,
            "note": "test-skill uses headless/dev-only providers and does not certify production Host providers."
        },
        "summary": {
            "total": cases.len(),
            "passed": cases.len().saturating_sub(failed_count),
            "failed": failed_count,
        },
        "cases": cases,
    }))
}

fn perf(skill_path: &Path, full: bool, iterations: Option<usize>) -> Result<Value, CliError> {
    let iterations = iterations
        .unwrap_or(if full { 20 } else { 3 })
        .clamp(1, if full { 200 } else { 10 });
    let mode = if full { "full" } else { "smoke" };
    let skill = load_skill(skill_path)?;
    let fixture_plan = FixturePlan::from_skill(skill_path, &skill)?;
    let mut samples = Vec::new();
    let initial_rss = current_rss_kib();

    samples.push(perf_skill_load_sample(skill_path, iterations)?);
    samples.extend(perf_runtime_fixture_samples(
        skill_path,
        &skill,
        &fixture_plan,
        iterations,
    )?);
    samples.extend(perf_micro_storage_and_token_samples(iterations)?);
    samples.push(perf_resource_limit_sample(skill_path)?);

    let final_rss = current_rss_kib();
    let status = if samples.iter().all(|sample| sample.status == "pass") {
        "ok"
    } else {
        "failed"
    };
    let sample_values = samples.iter().map(PerfSample::to_json).collect::<Vec<_>>();
    let max_rss = [initial_rss, final_rss].into_iter().flatten().max();
    let baseline = json!({
        "cases": sample_values.len(),
        "passed": samples.iter().filter(|sample| sample.status == "pass").count(),
        "failed": samples.iter().filter(|sample| sample.status != "pass").count(),
        "iterationsPerCase": iterations,
        "measurement": "local-dev-ci-friendly",
        "note": "Hardware-dependent baseline evidence only; not a production SLO."
    });

    let report = json!({
        "schemaVersion": PERF_REPORT_SCHEMA_VERSION,
        "status": status,
        "commandStatus": "ok",
        "mode": mode,
        "full": full,
        "skillId": skill_id_for_path(&skill, skill_path),
        "skillRef": validate_skill_ref(skill_path),
        "environment": perf_environment(),
        "baseline": baseline,
        "resource": {
            "memoryPerVm": {
                "measurement": "process-rss-sample",
                "unit": "KiB",
                "initial": initial_rss,
                "final": final_rss,
                "max": max_rss,
                "productionSlo": false
            }
        },
        "stress": perf_stress_summary(&samples),
        "samples": sample_values,
        "redaction": {
            "appliedByDefault": true,
            "localPaths": "redacted",
            "credentialMaterial": "omitted",
            "payloadPolicy": "mock/dev-only fixture summaries only"
        }
    });
    assert_perf_report_has_no_sensitive_strings(&report)?;
    Ok(report)
}

#[derive(Debug)]
struct PerfSample {
    name: String,
    category: String,
    status: &'static str,
    iterations: usize,
    unit: &'static str,
    values: Vec<u128>,
    details: Value,
}

impl PerfSample {
    fn duration(
        name: impl Into<String>,
        category: impl Into<String>,
        values: Vec<u128>,
        details: Value,
    ) -> Self {
        Self {
            name: name.into(),
            category: category.into(),
            status: "pass",
            iterations: values.len(),
            unit: "us",
            values,
            details,
        }
    }

    fn gauge(
        name: impl Into<String>,
        category: impl Into<String>,
        unit: &'static str,
        value: u128,
        details: Value,
    ) -> Self {
        Self {
            name: name.into(),
            category: category.into(),
            status: "pass",
            iterations: 1,
            unit,
            values: vec![value],
            details,
        }
    }

    fn pass_fail(
        name: impl Into<String>,
        category: impl Into<String>,
        passed: bool,
        details: Value,
    ) -> Self {
        Self {
            name: name.into(),
            category: category.into(),
            status: if passed { "pass" } else { "fail" },
            iterations: 1,
            unit: "count",
            values: vec![u128::from(passed)],
            details,
        }
    }

    fn to_json(&self) -> Value {
        let stats = perf_stats(&self.values);
        json!({
            "name": self.name,
            "category": self.category,
            "status": self.status,
            "iterations": self.iterations,
            "unit": self.unit,
            "p50": stats.p50,
            "p95": stats.p95,
            "max": stats.max,
            "details": self.details,
        })
    }
}

#[derive(Debug)]
struct PerfStats {
    p50: u128,
    p95: u128,
    max: u128,
}

fn perf_stats(values: &[u128]) -> PerfStats {
    if values.is_empty() {
        return PerfStats {
            p50: 0,
            p95: 0,
            max: 0,
        };
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    PerfStats {
        p50: percentile_value(&sorted, 50),
        p95: percentile_value(&sorted, 95),
        max: *sorted.last().unwrap_or(&0),
    }
}

fn percentile_value(sorted: &[u128], percentile: usize) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let index = ((sorted.len() - 1) * percentile).div_ceil(100);
    sorted[index.min(sorted.len() - 1)]
}

fn perf_skill_load_sample(skill_path: &Path, iterations: usize) -> Result<PerfSample, CliError> {
    let values = measure_iterations(iterations, || {
        load_skill(skill_path).map(|_| ()).map_err(CliError::from)
    })?;
    Ok(PerfSample::duration(
        "skill_load",
        "skill_load",
        values,
        json!({
            "loader": "skill-loader",
            "safePathChecks": true
        }),
    ))
}

fn perf_runtime_fixture_samples(
    skill_path: &Path,
    skill: &LoadedSkill,
    fixture_plan: &FixturePlan,
    iterations: usize,
) -> Result<Vec<PerfSample>, CliError> {
    let mut samples = Vec::new();
    for case in &fixture_plan.cases {
        samples.push(perf_api_call_sample(skill_path, skill, case, iterations)?);
        samples.push(perf_component_render_sample(
            skill_path, skill, case, iterations,
        )?);
        samples.push(perf_render_ir_size_sample(skill_path, skill, case)?);
    }
    samples.push(perf_concurrent_sessions_sample(
        skill_path,
        skill,
        fixture_plan,
        iterations,
    )?);
    samples.push(perf_multi_skill_sample(skill_path, iterations)?);
    samples.push(perf_multi_component_sample(
        skill_path,
        skill,
        fixture_plan,
        iterations,
    )?);
    samples.push(perf_dynamic_component_sample(iterations)?);
    Ok(samples)
}

fn perf_api_call_sample(
    skill_path: &Path,
    skill: &LoadedSkill,
    case: &FixtureCase,
    iterations: usize,
) -> Result<PerfSample, CliError> {
    let values = measure_iterations(iterations, || {
        let runtime = RuntimeHarness::load(
            skill_path,
            RuntimeIdentity::default_for_skill(skill, skill_path),
            Option::<&DemoAuthConfig>::None,
        )?;
        let call = runtime.call(&case.api_name, case.arguments.clone())?;
        if call.result.is_error {
            return Err(CliError::Demo(format!(
                "perf API fixture `{}` returned isError",
                case.name
            )));
        }
        Ok(())
    })?;
    Ok(PerfSample::duration(
        format!("api_vm_call.{}", case.name),
        "api_vm_call",
        values,
        json!({
            "apiName": case.api_name,
            "fixture": case.name,
            "mode": "cold-runtime-per-iteration"
        }),
    ))
}

fn perf_component_render_sample(
    skill_path: &Path,
    skill: &LoadedSkill,
    case: &FixtureCase,
    iterations: usize,
) -> Result<PerfSample, CliError> {
    let Some(component_path) = case.component_path.as_deref() else {
        return Ok(PerfSample::pass_fail(
            format!("component_render.{}", case.name),
            "component_render",
            false,
            json!({"reason": "fixture has no component path"}),
        ));
    };
    let runtime = RuntimeHarness::load(
        skill_path,
        RuntimeIdentity::default_for_skill(skill, skill_path),
        Option::<&DemoAuthConfig>::None,
    )?;
    let call = runtime.call(&case.api_name, case.arguments.clone())?;
    let values = measure_iterations(iterations, || {
        let mounted = mount_fixture_for_outcome(
            skill_path,
            skill,
            &case.api_name,
            case.arguments.clone(),
            call.result.clone(),
            component_path,
        )?;
        if mounted.mount.render.root.id.trim().is_empty() {
            return Err(CliError::Demo(format!(
                "perf component fixture `{}` rendered empty root id",
                case.name
            )));
        }
        Ok(())
    })?;
    Ok(PerfSample::duration(
        format!("component_render.{}", case.name),
        "component_render",
        values,
        json!({
            "componentPath": component_path,
            "fixture": case.name
        }),
    ))
}

fn perf_render_ir_size_sample(
    skill_path: &Path,
    skill: &LoadedSkill,
    case: &FixtureCase,
) -> Result<PerfSample, CliError> {
    let Some(component_path) = case.component_path.as_deref() else {
        return Ok(PerfSample::pass_fail(
            format!("render_ir_size.{}", case.name),
            "render_ir_size",
            false,
            json!({"reason": "fixture has no component path"}),
        ));
    };
    let runtime = RuntimeHarness::load(
        skill_path,
        RuntimeIdentity::default_for_skill(skill, skill_path),
        Option::<&DemoAuthConfig>::None,
    )?;
    let call = runtime.call(&case.api_name, case.arguments.clone())?;
    let mounted = mount_fixture_for_outcome(
        skill_path,
        skill,
        &case.api_name,
        case.arguments.clone(),
        call.result,
        component_path,
    )?;
    let bytes = serde_json::to_vec(&mounted.mount.render)
        .map_err(|source| CliError::Json {
            label: "Render IR perf sample".to_owned(),
            source,
        })?
        .len() as u128;
    Ok(PerfSample::gauge(
        format!("render_ir_size.{}", case.name),
        "render_ir_size",
        "bytes",
        bytes,
        json!({
            "componentPath": component_path,
            "schemaVersion": mounted.mount.render.schema_version,
            "fixture": case.name
        }),
    ))
}

fn perf_micro_storage_and_token_samples(iterations: usize) -> Result<Vec<PerfSample>, CliError> {
    let scope = StorageScope::new(DEFAULT_USER_DID, DEFAULT_MERCHANT_DID, DEFAULT_SKILL_ID);
    let storage = InMemoryScopedStorage::new();
    let write_values = measure_iterations(iterations, || {
        storage
            .set_storage(&scope, "cart", json!({"drinkId": "latte", "qty": 1}))
            .map_err(storage_cli_error)
    })?;
    let read_values = measure_iterations(iterations, || {
        storage
            .get_storage(&scope, "cart")
            .map(|_| ())
            .map_err(storage_cli_error)
    })?;

    let cache = InMemoryTokenCache::new();
    let token_scope = CapabilityTokenScope::for_subject(
        DEFAULT_MERCHANT_DID,
        DEFAULT_USER_DID,
        Some(DEFAULT_AGENT_DID.to_owned()),
        DEFAULT_SKILL_ID,
        Some(DEFAULT_SESSION_ID.to_owned()),
    );
    cache.put(
        token_scope.clone(),
        CapabilityToken::new("perf-token-redacted", None),
    );
    let token_values = measure_iterations(iterations, || {
        cache
            .get(&token_scope)
            .ok_or_else(|| CliError::Demo("perf token cache miss".to_owned()))
            .map(|_| ())
    })?;

    Ok(vec![
        PerfSample::duration(
            "storage_write.in_memory",
            "storage_write",
            write_values,
            json!({
                "backend": "in-memory",
                "scope": "mock-default",
                "productionReady": false
            }),
        ),
        PerfSample::duration(
            "storage_read.in_memory",
            "storage_read",
            read_values,
            json!({
                "backend": "in-memory",
                "scope": "mock-default",
                "productionReady": false
            }),
        ),
        PerfSample::duration(
            "token_lookup.in_memory",
            "token_lookup",
            token_values,
            json!({
                "cache": "in-memory",
                "tokenVisible": false,
                "productionReady": false
            }),
        ),
    ])
}

fn perf_concurrent_sessions_sample(
    skill_path: &Path,
    skill: &LoadedSkill,
    fixture_plan: &FixturePlan,
    iterations: usize,
) -> Result<PerfSample, CliError> {
    let Some(case) = fixture_plan.cases.first() else {
        return Ok(PerfSample::pass_fail(
            "stress.concurrent_sessions",
            "stress",
            false,
            json!({"reason": "fixture plan has no cases"}),
        ));
    };
    let count = iterations.clamp(2, 16);
    let skill_id = skill_id_for_path(skill, skill_path);
    let mut handles = Vec::with_capacity(count);
    for index in 0..count {
        let path = skill_path.to_path_buf();
        let api_name = case.api_name.clone();
        let arguments = case.arguments.clone();
        let session_id = format!("{DEFAULT_SESSION_ID}-perf-{index}");
        let skill_id = skill_id.clone();
        handles.push(std::thread::spawn(move || -> Result<u128, String> {
            let started = Instant::now();
            let loaded = load_skill(&path).map_err(|error| error.to_string())?;
            let runtime = RuntimeHarness::load(
                &path,
                RuntimeIdentity {
                    skill_id,
                    session_id,
                    ..RuntimeIdentity::default_demo()
                },
                Option::<&DemoAuthConfig>::None,
            )
            .map_err(|error| error.to_string())?;
            let call = runtime
                .call(&api_name, arguments)
                .map_err(|error| error.to_string())?;
            if call.result.is_error {
                return Err(format!(
                    "concurrent session fixture `{}` returned isError",
                    loaded
                        .manifest
                        .apis
                        .first()
                        .map(|api| api.name.as_str())
                        .unwrap_or("unknown")
                ));
            }
            Ok(started.elapsed().as_micros())
        }));
    }
    let mut values = Vec::with_capacity(count);
    for handle in handles {
        match handle.join() {
            Ok(Ok(value)) => values.push(value),
            Ok(Err(error)) => return Err(CliError::Demo(redact_text(&error))),
            Err(_) => {
                return Err(CliError::Demo(
                    "concurrent session worker panicked".to_owned(),
                ))
            }
        }
    }
    Ok(PerfSample::duration(
        "stress.concurrent_sessions",
        "stress",
        values,
        json!({
            "sessions": count,
            "isolation": "unique-session-id-per-iteration",
            "failClosed": true
        }),
    ))
}

fn perf_multi_skill_sample(skill_path: &Path, iterations: usize) -> Result<PerfSample, CliError> {
    let project_root = find_project_root_from(skill_path)
        .or_else(|| default_project_root().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let mut skill_paths = vec![skill_path.to_path_buf()];
    for name in [
        "address-form",
        "media-review",
        "dynamic-status",
        "location-map-preview",
    ] {
        let path = project_root.join("examples/fixtures").join(name);
        if path.join("mcp.json").is_file() {
            skill_paths.push(path);
        }
    }
    let values = measure_iterations(iterations, || {
        for path in &skill_paths {
            let skill = load_skill(path)?;
            let plan = FixturePlan::from_skill(path, &skill)?;
            if let Some(case) = plan.cases.first() {
                let runtime = RuntimeHarness::load(
                    path,
                    RuntimeIdentity::default_for_skill(&skill, path),
                    Option::<&DemoAuthConfig>::None,
                )?;
                let call = runtime.call(&case.api_name, case.arguments.clone())?;
                if call.result.is_error {
                    return Err(CliError::Demo(format!(
                        "multi-skill perf fixture `{}` returned isError",
                        case.name
                    )));
                }
            }
        }
        Ok(())
    })?;
    Ok(PerfSample::duration(
        "stress.multi_skill",
        "stress",
        values,
        json!({
            "skillCount": skill_paths.len(),
            "skillRefs": skill_paths.iter().map(|path| validate_skill_ref(path)).collect::<Vec<_>>()
        }),
    ))
}

fn perf_multi_component_sample(
    skill_path: &Path,
    skill: &LoadedSkill,
    fixture_plan: &FixturePlan,
    iterations: usize,
) -> Result<PerfSample, CliError> {
    let cases = fixture_plan
        .cases
        .iter()
        .filter(|case| case.component_path.is_some())
        .collect::<Vec<_>>();
    let values = measure_iterations(iterations, || {
        let runtime = RuntimeHarness::load(
            skill_path,
            RuntimeIdentity::default_for_skill(skill, skill_path),
            Option::<&DemoAuthConfig>::None,
        )?;
        for case in &cases {
            let call = runtime.call(&case.api_name, case.arguments.clone())?;
            let component_path = case
                .component_path
                .as_deref()
                .or_else(|| {
                    call.render
                        .as_ref()
                        .and_then(|render| render.component_path.as_deref())
                })
                .ok_or_else(|| CliError::Demo("component path missing".to_owned()))?;
            let _mounted = mount_fixture_for_outcome(
                skill_path,
                skill,
                &case.api_name,
                case.arguments.clone(),
                call.result,
                component_path,
            )?;
        }
        Ok(())
    })?;
    Ok(PerfSample::duration(
        "stress.multi_component_render",
        "stress",
        values,
        json!({
            "componentCount": cases.len(),
            "fixtureSet": fixture_plan.name
        }),
    ))
}

fn perf_dynamic_component_sample(iterations: usize) -> Result<PerfSample, CliError> {
    let root = default_project_root()
        .map(|root| root.join("examples/fixtures/dynamic-status"))
        .map_err(|error| CliError::Demo(format!("project root unavailable: {error}")))?;
    let skill = load_skill(&root)?;
    let plan = FixturePlan::from_skill(&root, &skill)?;
    let case = plan
        .cases
        .first()
        .ok_or_else(|| CliError::Demo("dynamic fixture plan has no cases".to_owned()))?;
    let values = measure_iterations(iterations, || {
        let report = run_fixture_case(&root, &skill, case)?;
        if report["status"] != "pass" {
            return Err(CliError::Demo("dynamic fixture perf run failed".to_owned()));
        }
        Ok(())
    })?;
    Ok(PerfSample::duration(
        "stress.dynamic_timer_request",
        "stress",
        values,
        json!({
            "fixture": "dynamic-status",
            "covers": ["dynamic request broker", "timer cleanup", "expire cleanup"],
            "mockOnly": true
        }),
    ))
}

fn perf_resource_limit_sample(skill_path: &Path) -> Result<PerfSample, CliError> {
    let skill = load_skill(skill_path)?;
    let skill_id = skill_id_for_path(&skill, skill_path);
    let fixture_plan = FixturePlan::from_skill(skill_path, &skill)?;
    let case = fixture_plan
        .cases
        .first()
        .ok_or_else(|| CliError::Demo("perf fixture plan has no cases".to_owned()))?;
    let api_name = case.api_name.clone();
    let arguments = case.arguments.clone();
    let vm = ApiVm::load_skill_with_config(
        skill,
        ApiVmConfig {
            max_result_json_bytes: 1,
            ..ApiVmConfig::default()
        },
    )?;
    let error = vm
        .call(ApiCall::new(
            skill_id,
            DEFAULT_SESSION_ID,
            api_name.clone(),
            arguments,
        ))
        .expect_err("resource limit should fail closed");
    let passed = matches!(error, ApiVmError::ResultTooLarge(_, 1));
    Ok(PerfSample::pass_fail(
        "resource_limit.result_size_fail_closed",
        "resource_limit",
        passed,
        json!({
            "apiName": api_name,
            "expected": "result_too_large",
            "actual": redact_text(&error.to_string()),
            "otherSessionImpact": "none",
            "failClosed": passed
        }),
    ))
}

fn measure_iterations_indexed(
    iterations: usize,
    mut operation: impl FnMut(usize) -> Result<(), CliError>,
) -> Result<Vec<u128>, CliError> {
    let mut values = Vec::with_capacity(iterations);
    for index in 0..iterations {
        let started = Instant::now();
        operation(index)?;
        values.push(started.elapsed().as_micros());
    }
    Ok(values)
}

fn measure_iterations(
    iterations: usize,
    mut operation: impl FnMut() -> Result<(), CliError>,
) -> Result<Vec<u128>, CliError> {
    measure_iterations_indexed(iterations, |_| operation())
}

fn perf_environment() -> Value {
    json!({
        "commit": git_head_short().unwrap_or_else(|| "unknown".to_owned()),
        "workingTreeDirty": git_working_tree_dirty(),
        "rustc": command_stdout("rustc", ["--version"]).unwrap_or_else(|| "unknown".to_owned()),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "timestampMs": current_time_ms(),
        "productionSlo": false
    })
}

fn perf_stress_summary(samples: &[PerfSample]) -> Value {
    let stress = samples
        .iter()
        .filter(|sample| sample.category == "stress" || sample.category == "resource_limit")
        .map(PerfSample::to_json)
        .collect::<Vec<_>>();
    json!({
        "status": if stress.iter().all(|sample| sample["status"] == "pass") { "pass" } else { "fail" },
        "cases": stress
    })
}

fn storage_cli_error(error: StorageError) -> CliError {
    CliError::Demo(format!("storage perf sample failed: {error}"))
}

fn command_stdout<const N: usize>(command: &str, args: [&str; N]) -> Option<String> {
    std::process::Command::new(command)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn git_head_short() -> Option<String> {
    command_stdout("git", ["rev-parse", "--short", "HEAD"])
}

fn git_working_tree_dirty() -> Option<bool> {
    command_stdout("git", ["status", "--short"]).map(|status| !status.trim().is_empty())
}

fn current_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn current_rss_kib() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
        let pages = statm.split_whitespace().nth(1)?.parse::<u64>().ok()?;
        Some(pages.saturating_mul(4096) / 1024)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn assert_perf_report_has_no_sensitive_strings(value: &Value) -> Result<(), CliError> {
    let rendered = value.to_string();
    for forbidden in [
        "Bearer ",
        "Authorization",
        "Signature",
        "Signature-Input",
        "capabilityToken",
        "private key",
        "fixture-token",
        "perf-token-redacted",
        "/home/",
        "/Users/",
    ] {
        if rendered.contains(forbidden) {
            return Err(CliError::Demo(format!(
                "perf report contains forbidden string `{forbidden}`"
            )));
        }
    }
    Ok(())
}

struct FixturePlan {
    name: String,
    cases: Vec<FixtureCase>,
}

#[derive(Clone)]
struct FixtureCase {
    name: String,
    api_name: String,
    arguments: Value,
    component_path: Option<String>,
    snapshot_name: Option<String>,
    action_method: Option<String>,
    expire: bool,
    audit_summary: Value,
    expected_render_root_kind: Option<String>,
}

impl FixturePlan {
    fn from_skill(skill_path: &Path, skill: &LoadedSkill) -> Result<Self, CliError> {
        let fixture_name = skill_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(DEFAULT_SKILL_ID);
        match fixture_name {
            "address-form" => Ok(Self {
                name: "address-form".to_owned(),
                cases: vec![FixtureCase {
                    name: "address-form.prepareAddressForm".to_owned(),
                    api_name: "prepareAddressForm".to_owned(),
                    arguments: json!({"addressHandle": "addr_handle_demo_001"}),
                    component_path: Some("components/address-form/index".to_owned()),
                    snapshot_name: Some("address-form.prepareAddressForm".to_owned()),
                    action_method: Some("submit".to_owned()),
                    expire: true,
                    audit_summary: json!({
                        "provider": "wx.chooseAddress",
                        "riskLevel": "L4",
                        "boundary": "host-provider-consent-required",
                        "dataPolicy": "opaque-address-handle-only"
                    }),
                    expected_render_root_kind: Some("view".to_owned()),
                }],
            }),
            "media-review" => Ok(Self {
                name: "media-review".to_owned(),
                cases: vec![FixtureCase {
                    name: "media-review.reviewMedia".to_owned(),
                    api_name: "reviewMedia".to_owned(),
                    arguments: json!({
                        "imageHandle": "image_handle_demo_001",
                        "fileHandle": "file_handle_demo_001"
                    }),
                    component_path: Some("components/media-review/index".to_owned()),
                    snapshot_name: Some("media-review.reviewMedia".to_owned()),
                    action_method: Some("approve".to_owned()),
                    expire: true,
                    audit_summary: json!({
                        "provider": "wx.chooseMedia",
                        "riskLevel": "L4",
                        "boundary": "host-media-provider-required",
                        "dataPolicy": "opaque-file-and-image-handles-only"
                    }),
                    expected_render_root_kind: Some("view".to_owned()),
                }],
            }),
            "dynamic-status" => Ok(Self {
                name: "dynamic-status".to_owned(),
                cases: vec![FixtureCase {
                    name: "dynamic-status.refreshDynamicStatus".to_owned(),
                    api_name: "refreshDynamicStatus".to_owned(),
                    arguments: json!({"orderId": "order_demo_001"}),
                    component_path: Some("components/dynamic-status/index".to_owned()),
                    snapshot_name: Some("dynamic-status.refreshDynamicStatus".to_owned()),
                    action_method: Some("refresh".to_owned()),
                    expire: true,
                    audit_summary: json!({
                        "provider": "RequestBroker",
                        "riskLevel": "L2",
                        "boundary": "dynamic-request-timer-gated",
                        "dataPolicy": "response-auth-headers-redacted"
                    }),
                    expected_render_root_kind: Some("view".to_owned()),
                }],
            }),
            "location-map-preview" => Ok(Self {
                name: "location-map-preview".to_owned(),
                cases: vec![FixtureCase {
                    name: "location-map-preview.prepareLocationMap".to_owned(),
                    api_name: "prepareLocationMap".to_owned(),
                    arguments: json!({"locationToken": "location_handle_demo_001"}),
                    component_path: Some("components/location-map-preview/index".to_owned()),
                    snapshot_name: Some("location-map-preview.prepareLocationMap".to_owned()),
                    action_method: Some("requestLocation".to_owned()),
                    expire: true,
                    audit_summary: json!({
                        "provider": "wx.getLocation",
                        "riskLevel": "L4",
                        "boundary": "host-location-provider-fail-closed",
                        "dataPolicy": "opaque-location-token-only"
                    }),
                    expected_render_root_kind: Some("view".to_owned()),
                }],
            }),
            "coffee-skill" | "coffee" => Ok(coffee_fixture_plan()),
            _ => {
                if is_coffee_fixture_shape(skill) {
                    return Ok(coffee_fixture_plan());
                }
                let cases = skill
                    .manifest
                    .apis
                    .iter()
                    .map(|api| FixtureCase {
                        name: format!("{}.{}", skill_id_for_path(skill, skill_path), api.name),
                        api_name: api.name.clone(),
                        arguments: Value::Object(Map::new()),
                        component_path: api.component_path().map(ToOwned::to_owned),
                        snapshot_name: None,
                        action_method: None,
                        expire: false,
                        audit_summary: json!({
                            "provider": "unknown",
                            "riskLevel": api.meta.as_ref().and_then(|meta| meta.anp.as_ref()).and_then(|anp| anp.get("risk")).cloned().unwrap_or_else(|| json!("unknown")),
                            "boundary": "generated-empty-arguments",
                            "dataPolicy": "developer-fixture-required"
                        }),
                        expected_render_root_kind: Some("view".to_owned()),
                    })
                    .collect::<Vec<_>>();
                Ok(Self {
                    name: skill_id_for_path(skill, skill_path),
                    cases,
                })
            }
        }
    }
}

fn is_coffee_fixture_shape(skill: &LoadedSkill) -> bool {
    let api_names = skill
        .manifest
        .apis
        .iter()
        .map(|api| api.name.as_str())
        .collect::<BTreeSet<_>>();
    let component_paths = skill
        .manifest
        .components
        .iter()
        .map(|component| component.path.as_str())
        .collect::<BTreeSet<_>>();
    ["searchDrinks", "confirmOrder", "payOrder"]
        .into_iter()
        .all(|name| api_names.contains(name))
        && [
            "components/drink-list/index",
            "components/order-confirm/index",
            "components/payment-result/index",
        ]
        .into_iter()
        .all(|path| component_paths.contains(path))
}

fn coffee_fixture_plan() -> FixturePlan {
    FixturePlan {
        name: "coffee".to_owned(),
        cases: vec![
            FixtureCase {
                name: "coffee.searchDrinks".to_owned(),
                api_name: "searchDrinks".to_owned(),
                arguments: json!({"query": "latte"}),
                component_path: Some("components/drink-list/index".to_owned()),
                snapshot_name: None,
                action_method: Some("confirmDrink".to_owned()),
                expire: false,
                audit_summary: json!({
                    "provider": "mock-coffee",
                    "riskLevel": "low",
                    "boundary": "demo-only-local-fixture",
                    "dataPolicy": "mock-merchant-data"
                }),
                expected_render_root_kind: Some("view".to_owned()),
            },
            FixtureCase {
                name: "coffee.confirmOrder".to_owned(),
                api_name: "confirmOrder".to_owned(),
                arguments: json!({
                    "drinkId": "latte",
                    "size": "medium",
                    "sugar": "less"
                }),
                component_path: Some("components/order-confirm/index".to_owned()),
                snapshot_name: None,
                action_method: Some("payOrder".to_owned()),
                expire: false,
                audit_summary: json!({
                    "provider": "mock-coffee",
                    "riskLevel": "order",
                    "boundary": "dev-headless-consent-approved",
                    "dataPolicy": "mock-order-only"
                }),
                expected_render_root_kind: Some("view".to_owned()),
            },
            FixtureCase {
                name: "coffee.payOrder".to_owned(),
                api_name: "payOrder".to_owned(),
                arguments: json!({"orderId": "order_demo_001"}),
                component_path: Some("components/payment-result/index".to_owned()),
                snapshot_name: None,
                action_method: None,
                expire: true,
                audit_summary: json!({
                    "provider": "mock-coffee",
                    "riskLevel": "payment",
                    "boundary": "dev-headless-consent-approved",
                    "dataPolicy": "mock-payment-only"
                }),
                expected_render_root_kind: Some("view".to_owned()),
            },
        ],
    }
}

fn run_fixture_case(
    skill_path: &Path,
    skill: &LoadedSkill,
    case: &FixtureCase,
) -> Result<Value, CliError> {
    let mut failures = Vec::new();
    let runtime = RuntimeHarness::load(
        skill_path,
        RuntimeIdentity::default_for_skill(skill, skill_path),
        Option::<&DemoAuthConfig>::None,
    )?;
    let call = match runtime.call(&case.api_name, case.arguments.clone()) {
        Ok(call) => call,
        Err(error) => {
            failures.push(json!({
                "stage": "api",
                "message": redact_text(&error.to_string()),
            }));
            return Ok(fixture_case_report(
                case,
                FixtureCaseArtifacts {
                    failures,
                    api: Value::Null,
                    component: Value::Null,
                    event_actions: Value::Null,
                    snapshot_compare: Value::Null,
                    audit_events: Vec::new(),
                    expire: Value::Null,
                },
            ));
        }
    };

    if call.result.is_error {
        failures.push(json!({
            "stage": "api",
            "message": "Atomic API returned isError = true",
        }));
    }

    let mut event_actions = Vec::new();
    let mut expire_summary = Value::Null;
    let mut snapshot_compare = Value::Null;
    let mut component_summary = Value::Null;
    let component_path = case.component_path.as_deref().or_else(|| {
        call.render
            .as_ref()
            .and_then(|render| render.component_path.as_deref())
    });
    if let Some(component_path) = component_path {
        let mut mounted = mount_fixture_for_outcome(
            skill_path,
            skill,
            &case.api_name,
            case.arguments.clone(),
            call.result.clone(),
            component_path,
        )?;
        component_summary = json!({
            "componentPath": component_path,
            "render": component_render_json(&mounted.mount.render),
            "actions": mounted.mount.actions,
            "metadata": mounted.mount.metadata,
            "state": mounted.mount.state,
        });
        if let Some(expected_kind) = &case.expected_render_root_kind {
            let actual_kind = serde_json::to_value(&mounted.mount.render.root.kind)
                .unwrap_or(Value::Null)
                .as_str()
                .unwrap_or_default()
                .to_owned();
            if actual_kind != *expected_kind {
                failures.push(json!({
                    "stage": "render",
                    "path": "render.root.kind",
                    "expected": expected_kind,
                    "actual": actual_kind,
                }));
            }
        }
        if let Some(method) = &case.action_method {
            match find_tap_event(&mounted.mount.render.root, method) {
                Some(event) => {
                    let outcome = mounted.instance.dispatch_event(&event)?;
                    event_actions = outcome.actions;
                    if !event_actions
                        .iter()
                        .any(|action| matches!(action, ComponentVmAction::ApiCall { .. }))
                    {
                        failures.push(json!({
                            "stage": "action",
                            "message": "event did not emit api/call",
                            "method": method,
                        }));
                    }
                }
                None => failures.push(json!({
                    "stage": "action",
                    "message": "tap event not found",
                    "method": method,
                })),
            }
        }
        if case.expire {
            let expired = mounted.instance.expire(json!({"reason": "test-skill"}));
            expire_summary = json!({
                "ok": expired.is_ok(),
                "expired": mounted.instance.is_expired(),
            });
            if expired.is_err() || !mounted.instance.is_expired() {
                failures.push(json!({
                    "stage": "expire",
                    "message": "component did not expire cleanly",
                }));
            }
        }
        if let Some(snapshot_name) = &case.snapshot_name {
            let actual_snapshot =
                fixture_snapshot_value(case, &mounted, &event_actions, &expire_summary);
            snapshot_compare =
                compare_fixture_snapshot(skill_path, snapshot_name, &actual_snapshot)?;
            if snapshot_compare.get("status").and_then(Value::as_str) != Some("match") {
                failures.push(json!({
                    "stage": "snapshot",
                    "snapshot": snapshot_name,
                    "diff": snapshot_compare,
                }));
            }
        }
    } else {
        failures.push(json!({
            "stage": "render",
            "message": "fixture case has no component path",
        }));
    }

    assert_fixture_report_has_no_sensitive_strings(&json!({
        "result": call.result,
        "render": component_summary,
        "eventActions": event_actions,
        "audit": audit_events_json(&runtime.audit_events()),
        "snapshotCompare": snapshot_compare,
    }))?;

    Ok(fixture_case_report(
        case,
        FixtureCaseArtifacts {
            failures,
            api: json!({
                "apiName": case.api_name,
                "arguments": redact_metadata_value(&case.arguments),
                "result": call.result,
                "modelVisible": call.model_visible,
            }),
            component: component_summary,
            event_actions: serde_json::to_value(&event_actions).unwrap_or(Value::Null),
            snapshot_compare,
            audit_events: runtime.audit_events(),
            expire: expire_summary,
        },
    ))
}

struct FixtureCaseArtifacts {
    failures: Vec<Value>,
    api: Value,
    component: Value,
    event_actions: Value,
    snapshot_compare: Value,
    audit_events: Vec<AuditEvent>,
    expire: Value,
}

fn fixture_case_report(case: &FixtureCase, artifacts: FixtureCaseArtifacts) -> Value {
    json!({
        "name": case.name,
        "status": if artifacts.failures.is_empty() { "pass" } else { "fail" },
        "api": artifacts.api,
        "component": artifacts.component,
        "eventActions": artifacts.event_actions,
        "snapshotCompare": artifacts.snapshot_compare,
        "auditSummary": {
            "expected": case.audit_summary,
            "events": audit_events_json(&artifacts.audit_events),
            "eventCount": artifacts.audit_events.len(),
        },
        "expire": artifacts.expire,
        "failures": artifacts.failures,
    })
}

fn mount_fixture_for_outcome(
    skill_path: &Path,
    skill: &LoadedSkill,
    api_name: &str,
    arguments: Value,
    result: AtomicApiResult,
    component_path: &str,
) -> Result<MountedComponent, CliError> {
    let package = load_component_package(skill_path, component_path)?;
    let metadata = manifest_component_metadata(&skill.manifest, component_path)?;
    let broker = metadata.dynamic.then(FixtureRequestBroker::new);
    let config = fixture_component_config(&metadata, broker.clone());
    let mut instance = ComponentInstance::with_config(package, config)?;
    let input = ComponentInput {
        component_metadata: metadata,
        ..component_input(api_name, arguments, &result)
    };
    let mount = instance.mount(input)?;
    Ok(MountedComponent {
        instance,
        mount,
        broker,
    })
}

fn fixture_component_config(
    metadata: &ComponentMetadata,
    broker: Option<Rc<FixtureRequestBroker>>,
) -> component_runtime::ComponentVmConfig {
    if metadata.dynamic {
        component_runtime::ComponentVmConfig {
            dynamic: DynamicComponentConfig::default()
                .with_request_broker(broker.unwrap_or_else(FixtureRequestBroker::new)),
            ..component_runtime::ComponentVmConfig::default()
        }
    } else {
        component_runtime::ComponentVmConfig::default()
    }
}

#[derive(Debug)]
struct FixtureRequestBroker {
    calls: RefCell<Vec<WxRequest>>,
}

impl FixtureRequestBroker {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            calls: RefCell::new(Vec::new()),
        })
    }
}

impl RequestBroker for FixtureRequestBroker {
    fn request(
        &self,
        profile: &CapabilityProfile,
        request: WxRequest,
    ) -> Result<WxResponse, WxRequestError> {
        if !profile.check(wx_compat::Capability::Request).is_allowed() {
            return Err(WxRequestError::Denied(
                "fixture request capability denied".to_owned(),
            ));
        }
        self.calls.borrow_mut().push(request);
        let mut headers = BTreeMap::new();
        headers.insert("x-fixture-safe".to_owned(), "broker-ok".to_owned());
        headers.insert(
            "Authorization".to_owned(),
            "Bearer fixture-token".to_owned(),
        );
        headers.insert("x-token-id".to_owned(), "fixture-token".to_owned());
        Ok(WxResponse {
            status_code: 200,
            headers,
            data: json!({
                "status": "ready",
                "source": "fixture-broker"
            }),
        })
    }
}

fn fixture_snapshot_value(
    case: &FixtureCase,
    mounted: &MountedComponent,
    event_actions: &[ComponentVmAction],
    expire: &Value,
) -> Value {
    let (fixture, _) = case
        .snapshot_name
        .as_deref()
        .and_then(|name| name.split_once('.'))
        .unwrap_or((case.name.as_str(), case.name.as_str()));
    let component = case
        .component_path
        .as_deref()
        .and_then(|path| {
            path.strip_suffix("/index")
                .unwrap_or(path)
                .rsplit('/')
                .next()
        })
        .unwrap_or(fixture);
    let mut audit_summary = case.audit_summary.clone();
    if let Some(broker) = &mounted.broker {
        audit_summary["brokerCalls"] = json!(broker.calls.borrow().len());
    }
    let state = normalized_fixture_snapshot_state(case, &mounted.mount.state);
    json!({
        "fixture": fixture,
        "component": component,
        "render": mounted.mount.render,
        "actions": mounted.mount.actions,
        "eventActions": event_actions,
        "warnings": mounted.mount.render.warnings,
        "metadata": mounted.mount.metadata,
        "state": state,
        "auditSummary": audit_summary,
        "expire": expire,
    })
}

fn normalized_fixture_snapshot_state(case: &FixtureCase, state: &Value) -> Value {
    let mut state = state.clone();
    if let Value::Object(fields) = &mut state {
        fields.insert(
            "content".to_owned(),
            json!([{
                "type": "text",
                "text": format!("{} fixture result", case.api_name),
            }]),
        );
        let meta = fields
            .entry("_meta".to_owned())
            .or_insert_with(|| json!({}));
        if let Value::Object(meta_fields) = meta {
            meta_fields.insert("fixture".to_owned(), json!(case.api_name));
            meta_fields.insert("mockOnly".to_owned(), json!(true));
            meta_fields.remove("risk");
        }
    }
    state
}

fn compare_fixture_snapshot(
    skill_path: &Path,
    snapshot_name: &str,
    actual: &Value,
) -> Result<Value, CliError> {
    let snapshot_path = fixture_snapshot_path(skill_path, snapshot_name)?;
    let expected = read_json_file(&snapshot_path)?;
    if &expected == actual {
        return Ok(json!({
            "status": "match",
            "snapshot": relative_display(&snapshot_path),
        }));
    }

    Ok(json!({
        "status": "mismatch",
        "snapshot": relative_display(&snapshot_path),
        "diff": first_json_diff("", &expected, actual).unwrap_or_else(|| json!({
            "path": "",
            "expected": expected,
            "actual": actual,
        })),
    }))
}

fn fixture_snapshot_path(skill_path: &Path, snapshot_name: &str) -> Result<PathBuf, CliError> {
    let project_root = find_project_root_from(skill_path)
        .or_else(|| default_project_root().ok())
        .ok_or_else(|| {
            CliError::Demo("could not locate project root for fixture snapshots".to_owned())
        })?;
    Ok(project_root
        .join("testdata/render-ir")
        .join(format!("{snapshot_name}.json")))
}

fn first_json_diff(path: &str, expected: &Value, actual: &Value) -> Option<Value> {
    if expected == actual {
        return None;
    }
    match (expected, actual) {
        (Value::Object(expected_map), Value::Object(actual_map)) => {
            let mut keys = expected_map
                .keys()
                .chain(actual_map.keys())
                .cloned()
                .collect::<BTreeSet<_>>();
            for key in std::mem::take(&mut keys) {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                match (expected_map.get(&key), actual_map.get(&key)) {
                    (Some(expected_child), Some(actual_child)) => {
                        if let Some(diff) =
                            first_json_diff(&child_path, expected_child, actual_child)
                        {
                            return Some(diff);
                        }
                    }
                    (expected_child, actual_child) => {
                        return Some(json!({
                            "path": child_path,
                            "expected": expected_child.cloned().unwrap_or(Value::Null),
                            "actual": actual_child.cloned().unwrap_or(Value::Null),
                        }));
                    }
                }
            }
            None
        }
        (Value::Array(expected_items), Value::Array(actual_items)) => {
            for index in 0..expected_items.len().max(actual_items.len()) {
                let child_path = format!("{path}[{index}]");
                match (expected_items.get(index), actual_items.get(index)) {
                    (Some(expected_child), Some(actual_child)) => {
                        if let Some(diff) =
                            first_json_diff(&child_path, expected_child, actual_child)
                        {
                            return Some(diff);
                        }
                    }
                    (expected_child, actual_child) => {
                        return Some(json!({
                            "path": child_path,
                            "expected": expected_child.cloned().unwrap_or(Value::Null),
                            "actual": actual_child.cloned().unwrap_or(Value::Null),
                        }));
                    }
                }
            }
            None
        }
        _ => Some(json!({
            "path": path,
            "expected": expected,
            "actual": actual,
        })),
    }
}

fn read_json_file(path: &Path) -> Result<Value, CliError> {
    let source = std::fs::read_to_string(path)?;
    serde_json::from_str(&source).map_err(|source| CliError::Json {
        label: relative_display(path),
        source,
    })
}

fn relative_display(path: &Path) -> String {
    if let Some(project_root) = find_project_root_from(path) {
        if let Ok(relative) = path.strip_prefix(&project_root) {
            return relative
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
        }
    }
    report_path(path).0
}

fn assert_fixture_report_has_no_sensitive_strings(value: &Value) -> Result<(), CliError> {
    let rendered = value.to_string();
    for forbidden in [
        "Bearer ",
        "Authorization",
        "Signature",
        "Signature-Input",
        "capabilityToken",
        "private key",
        "fixture-token",
        "/home/",
        "/Users/",
    ] {
        if rendered.contains(forbidden) {
            return Err(CliError::Demo(format!(
                "fixture report contains forbidden string `{forbidden}`"
            )));
        }
    }
    Ok(())
}

fn call_api(skill_path: &Path, api_name: &str, json_args: &str) -> Result<Value, CliError> {
    let args = parse_json(json_args, "jsonArgs")?;
    let auth_config = if requires_remote_auth(&args) {
        DemoAuthConfig::from_inputs_optional(None, None, None, None, None, None, EnvConfigSource)?
    } else {
        None
    };
    let identity = auth_config
        .as_ref()
        .map(|auth_config| RuntimeIdentity {
            user_did: auth_config.user_did.clone(),
            agent_did: auth_config.agent_did.clone(),
            merchant_did: DEFAULT_MERCHANT_DID.to_owned(),
            skill_id: DEFAULT_SKILL_ID.to_owned(),
            session_id: DEFAULT_SESSION_ID.to_owned(),
        })
        .unwrap_or_else(RuntimeIdentity::default_demo);
    let runtime = RuntimeHarness::load(skill_path, identity, auth_config.as_ref())?;
    let outcome = runtime.call(api_name, args)?;
    Ok(json!({
        "status": "ok",
        "apiName": api_name,
        "result": outcome.result,
        "modelVisible": outcome.model_visible,
        "render": render_outcome_json(outcome.render.as_ref()),
        "audit": audit_events_json(&runtime.audit_events())
    }))
}

fn requires_remote_auth(args: &Value) -> bool {
    args.get("serverUrl")
        .or_else(|| args.get("remoteBaseUrl"))
        .and_then(Value::as_str)
        .is_some_and(|url| !url.trim().is_empty())
}

fn preview_component(
    skill_path: &Path,
    component_path: &str,
    json_input: &str,
) -> Result<Value, CliError> {
    let input = parse_component_input(json_input)?;
    let package = load_component_package(skill_path, component_path)?;
    let mut instance = ComponentInstance::new(package)?;
    let metadata = load_component_metadata(skill_path, component_path)?;
    let input = ComponentInput {
        component_metadata: metadata,
        ..input
    };
    let outcome = instance.mount(input)?;
    Ok(json!({
        "status": "ok",
        "componentPath": component_path,
        "render": component_render_json(&outcome.render),
        "actions": outcome.actions,
        "metadata": outcome.metadata,
        "trace": outcome.trace,
        "state": outcome.state
    }))
}

fn preview_card(result_json: &str) -> Result<Value, CliError> {
    let result = parse_atomic_result(result_json)?;
    let reason = if result.is_error {
        FallbackReason::ApiError
    } else if result
        .structured_content
        .as_ref()
        .is_some_and(Map::is_empty)
        || result.structured_content.is_none()
    {
        FallbackReason::EmptyStructuredContent
    } else {
        FallbackReason::HostRendererUnavailable
    };
    let card = fallback_from_result(&result, reason);
    Ok(json!({
        "status": "ok",
        "card": card
    }))
}

fn runtime_json(skill_path: &Path, request_json: &str) -> Result<Value, CliError> {
    let request: RuntimeIpcRequest = match serde_json::from_str(request_json) {
        Ok(request) => request,
        Err(_) => {
            let response = RuntimeIpcResponse::error(
                "",
                "runtime.parseRequest",
                RuntimeErrorResponse::invalid_params(
                    "runtime IPC request JSON is invalid or does not match the envelope schema",
                ),
            );
            return serde_json::to_value(response).map_err(|source| CliError::Json {
                label: "runtime JSON response".to_owned(),
                source,
            });
        }
    };
    let runtime = RuntimeHarness::load(
        skill_path,
        RuntimeIdentity::default_demo(),
        Option::<&DemoAuthConfig>::None,
    )?;
    serde_json::to_value(runtime.handle_ipc_request(request)).map_err(|source| CliError::Json {
        label: "runtime JSON response".to_owned(),
        source,
    })
}

fn run_demo(
    skill_path: &Path,
    server: &str,
    auth_config: Option<&DemoAuthConfig>,
) -> Result<Value, CliError> {
    let auth_config = auth_config.ok_or(DidCredentialError::Unavailable)?;
    let auth = DemoHttpClient::new(server).login(auth_config)?;
    let server_business =
        DemoHttpClient::new(server).coffee_business_check(&auth.capability_token)?;
    let runtime = RuntimeHarness::load(
        skill_path,
        RuntimeIdentity {
            user_did: auth_config.user_did.clone(),
            agent_did: auth_config.agent_did.clone(),
            merchant_did: auth.merchant_did.clone(),
            skill_id: DEFAULT_SKILL_ID.to_owned(),
            session_id: DEFAULT_SESSION_ID.to_owned(),
        },
        Some(auth_config),
    )?;

    let search_args = json!({"query": "latte", "serverUrl": server.trim_end_matches('/')});
    let search = runtime.call("searchDrinks", search_args.clone())?;
    let mut drink_component = mount_for_outcome(
        skill_path,
        runtime.skill(),
        "searchDrinks",
        search_args,
        search.result.clone(),
        required_component_path(&search, "searchDrinks")?,
    )?;
    let drink_event = find_tap_event(&drink_component.mount.render.root, "confirmDrink")
        .ok_or_else(|| CliError::Demo("drink-list confirmDrink event not found".to_owned()))?;
    let drink_action = dispatch_first_api_call(&mut drink_component.instance, &drink_event)?;

    let confirm_args = api_call_args(&drink_action, "confirmOrder")?;
    let confirm = runtime.call("confirmOrder", confirm_args.clone())?;
    let mut order_component = mount_for_outcome(
        skill_path,
        runtime.skill(),
        "confirmOrder",
        confirm_args,
        confirm.result.clone(),
        required_component_path(&confirm, "confirmOrder")?,
    )?;
    let pay_event = find_tap_event(&order_component.mount.render.root, "payOrder")
        .ok_or_else(|| CliError::Demo("order-confirm payOrder event not found".to_owned()))?;
    let pay_action = dispatch_first_api_call(&mut order_component.instance, &pay_event)?;

    let pay_args = api_call_args(&pay_action, "payOrder")?;
    let payment = runtime.call("payOrder", pay_args.clone())?;
    let mut payment_component = mount_for_outcome(
        skill_path,
        runtime.skill(),
        "payOrder",
        pay_args,
        payment.result.clone(),
        required_component_path(&payment, "payOrder")?,
    )?;
    let expire = payment_component
        .instance
        .expire(json!({"reason": "payment_completed"}))?;

    let server_health = DemoHttpClient::new(server).get_json("/health", None)?;
    Ok(json!({
        "status": "ok",
        "server": {
            "baseUrl": server.trim_end_matches('/'),
            "health": server_health,
            "auth": {
                "merchantDid": auth.merchant_did,
                "userDid": auth_config.user_did,
                "agentDid": auth_config.agent_did,
                "capabilityToken": "[REDACTED]",
                "tokenReceived": auth.token_received,
                "authProvider": "did-challenge",
                "challengeVerified": auth.token_received,
                "tokenScopes": [
                    "coffee:drinks:read",
                    "coffee:order:confirm",
                    "coffee:order:pay",
                    "coffee:order:read"
                ],
                "wxLoginStatus": "container-managed",
                "requestAuthMode": "container-attached-bearer",
                "credential": auth_config.redacted_summary()
            },
            "business": server_business
        },
        "flow": [
            step_summary("searchDrinks", &search.result, &drink_component.mount.render.root, &drink_component.mount.actions),
            step_summary("confirmOrder", &confirm.result, &order_component.mount.render.root, &order_component.mount.actions),
            step_summary("payOrder", &payment.result, &payment_component.mount.render.root, &payment_component.mount.actions),
            json!({
                "name": "expire",
                "state": expire.state,
                "actions": expire.actions,
                "trace": expire.trace
            })
        ],
        "componentActions": {
            "drinkList": drink_action,
            "orderConfirm": pay_action
        },
        "audit": audit_events_json(&runtime.audit_events())
    }))
}

struct RuntimeHarness {
    service: RuntimeService<
        AllowHost,
        ApproveConsent,
        js_runtime_quickjs::QuickJsApiExecutor,
        ComponentRenderRouter,
        CollectAudit,
        CollectAudit,
    >,
    audit: CollectAudit,
    identity: RuntimeIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeIdentity {
    user_did: String,
    agent_did: Option<String>,
    merchant_did: String,
    skill_id: String,
    session_id: String,
}

impl RuntimeIdentity {
    fn default_demo() -> Self {
        Self {
            user_did: DEFAULT_USER_DID.to_owned(),
            agent_did: Some(DEFAULT_AGENT_DID.to_owned()),
            merchant_did: DEFAULT_MERCHANT_DID.to_owned(),
            skill_id: DEFAULT_SKILL_ID.to_owned(),
            session_id: DEFAULT_SESSION_ID.to_owned(),
        }
    }

    fn default_for_skill(skill: &LoadedSkill, skill_path: &Path) -> Self {
        Self {
            skill_id: skill_id_for_path(skill, skill_path),
            ..Self::default_demo()
        }
    }
}

impl RuntimeHarness {
    fn load(
        skill_path: &Path,
        identity: RuntimeIdentity,
        auth_config: Option<&DemoAuthConfig>,
    ) -> Result<Self, CliError> {
        let skill = load_skill(skill_path)?;
        let api_vm = ApiVm::load_skill(skill.clone())?;
        let mut executor = api_vm.executor();
        if let Some(auth_config) = auth_config {
            executor = executor.with_host_did_auth(
                HostDidAuthConfig::new(
                    auth_config.credential.did_document_path.clone(),
                    auth_config.credential.private_key_path.clone(),
                )
                .without_private_key_permission_check(),
            );
        }
        let audit = CollectAudit::default();
        let service = RuntimeService::load_skill(
            skill.clone(),
            AllowHost,
            ApproveConsent,
            executor,
            ComponentRenderRouter {
                skill_root: skill.root,
                manifest: skill.manifest,
            },
            audit.clone(),
            audit.clone(),
        );
        Ok(Self {
            service,
            audit,
            identity,
        })
    }

    fn call(
        &self,
        api_name: impl Into<String>,
        arguments: Value,
    ) -> Result<dock_core::CallOutcome, CliError> {
        let response = self
            .service
            .call_api(RuntimeCallRequest {
                session: RuntimeSessionContext {
                    user_did: Some(self.identity.user_did.clone()),
                    agent_did: self.identity.agent_did.clone(),
                    merchant_did: Some(self.identity.merchant_did.clone()),
                    skill_id: self.identity.skill_id.clone(),
                    session_id: self.identity.session_id.clone(),
                },
                api_name: api_name.into(),
                arguments,
                capability_token: None,
                operation: None,
            })
            .map_err(CliError::Runtime)?;
        Ok(response.data.into_call_outcome())
    }

    fn skill(&self) -> &LoadedSkill {
        self.service.skill()
    }

    fn audit_events(&self) -> Vec<AuditEvent> {
        self.audit.events.borrow().clone()
    }

    fn handle_ipc_request(&self, request: RuntimeIpcRequest) -> dock_core::RuntimeIpcResponse {
        self.service.handle_ipc_request(request)
    }
}

#[derive(Clone)]
struct AllowHost;

impl RuntimeHost for AllowHost {
    fn check_permission(
        &self,
        _context: &ApiCallContext,
    ) -> Result<PermissionDecision, DockCoreError> {
        Ok(PermissionDecision::Allow)
    }
}

#[derive(Clone)]
struct ApproveConsent;

impl ConsentGate for ApproveConsent {
    fn check_consent(
        &self,
        _context: &ApiCallContext,
        _request: &ConsentRequest,
    ) -> Result<ConsentDecision, DockCoreError> {
        Ok(ConsentDecision::approved(
            DEV_HEADLESS_CONSENT_PROVIDER,
            DEV_HEADLESS_DECISION_ACTOR,
        ))
    }
}

#[derive(Clone)]
struct ComponentRenderRouter {
    skill_root: PathBuf,
    manifest: mcp_schema::SkillManifest,
}

impl RenderRouter for ComponentRenderRouter {
    fn render(
        &self,
        _context: &ApiCallContext,
        input: &ComponentRenderInput,
    ) -> Result<RenderOutcome, DockCoreError> {
        let component_input = ComponentInput {
            api_name: input.api_name.clone(),
            arguments: input.arguments.clone(),
            properties: Map::new(),
            content: input
                .content
                .iter()
                .map(|content| serde_json::to_value(content).unwrap_or(Value::Null))
                .collect(),
            structured_content: input.structured_content.clone(),
            meta: input.meta.clone(),
            component_metadata: ComponentMetadata::default(),
        };
        let package = load_component_package(&self.skill_root, &input.component_path)
            .map_err(|error| DockCoreError::core(ErrorCode::RenderFailed, error.to_string()))?;
        let mut instance = ComponentInstance::new(package)
            .map_err(|error| DockCoreError::core(ErrorCode::RenderFailed, error.to_string()))?;
        let metadata = manifest_component_metadata(&self.manifest, &input.component_path).map_err(
            |error| DockCoreError::core(ErrorCode::RenderFailed, redact_text(&error.to_string())),
        )?;
        let component_input = ComponentInput {
            component_metadata: metadata,
            ..component_input
        };
        let outcome = instance
            .mount(component_input)
            .map_err(|error| DockCoreError::core(ErrorCode::RenderFailed, error.to_string()))?;

        Ok(RenderOutcome {
            renderer: "component-runtime".to_owned(),
            component_path: Some(input.component_path.clone()),
            payload: json!({
                "render": component_render_json(&outcome.render),
                "actions": outcome.actions,
                "metadata": outcome.metadata,
                "trace": outcome.trace,
                "state": outcome.state
            }),
            fallback_reason: None,
        })
    }

    fn fallback(
        &self,
        _context: &ApiCallContext,
        result: &AtomicApiResult,
        reason: &str,
    ) -> RenderOutcome {
        let fallback_reason = fallback_reason_from_str(reason);
        let stable_reason = fallback_reason.as_str().to_owned();
        RenderOutcome {
            renderer: "card-spec".to_owned(),
            component_path: None,
            payload: json!(fallback_from_result(result, fallback_reason)),
            fallback_reason: Some(stable_reason),
        }
    }
}

#[derive(Clone, Default)]
struct CollectAudit {
    events: std::rc::Rc<RefCell<Vec<AuditEvent>>>,
}

impl AuditSink for CollectAudit {
    fn record(&self, event: AuditEvent) -> Result<(), DockCoreError> {
        self.events.borrow_mut().push(event);
        Ok(())
    }
}

impl RuntimeAuditReader for CollectAudit {
    fn runtime_audit_records(&self) -> Result<Vec<AuditEvent>, DockCoreError> {
        Ok(self.events.borrow().clone())
    }
}

struct MountedComponent {
    instance: ComponentInstance,
    mount: component_runtime::ComponentOperationOutcome,
    broker: Option<Rc<FixtureRequestBroker>>,
}

fn mount_for_outcome(
    skill_path: &Path,
    skill: &LoadedSkill,
    api_name: &str,
    arguments: Value,
    result: AtomicApiResult,
    component_path: &str,
) -> Result<MountedComponent, CliError> {
    let package = load_component_package(skill_path, component_path)?;
    let mut instance = ComponentInstance::new(package)?;
    let metadata = manifest_component_metadata(&skill.manifest, component_path)?;
    let input = ComponentInput {
        component_metadata: metadata,
        ..component_input(api_name, arguments, &result)
    };
    let mount = instance.mount(input)?;
    Ok(MountedComponent {
        instance,
        mount,
        broker: None,
    })
}

fn component_input(api_name: &str, arguments: Value, result: &AtomicApiResult) -> ComponentInput {
    ComponentInput {
        api_name: api_name.to_owned(),
        arguments,
        properties: Map::new(),
        content: result
            .content
            .iter()
            .map(|content| serde_json::to_value(content).unwrap_or(Value::Null))
            .collect(),
        structured_content: result.structured_content.clone(),
        meta: result.meta.clone(),
        component_metadata: ComponentMetadata::default(),
    }
}

fn load_component_metadata(
    skill_path: &Path,
    component_path: &str,
) -> Result<ComponentMetadata, CliError> {
    let skill = load_skill(skill_path)?;
    manifest_component_metadata(&skill.manifest, component_path)
}

fn manifest_component_metadata(
    manifest: &mcp_schema::SkillManifest,
    component_path: &str,
) -> Result<ComponentMetadata, CliError> {
    let component = manifest
        .components
        .iter()
        .find(|component| component_paths_match(&component.path, component_path))
        .ok_or_else(|| CliError::Demo(format!("component `{component_path}` is not declared")))?;
    let mut metadata = ComponentMetadata::new(component.path.clone());
    metadata.related_page = component.related_page.as_ref().and_then(safe_related_page);
    metadata.dynamic = component.dynamic_permission().is_some();
    metadata.scope_dynamic = component.dynamic_permission().map(redact_metadata_value);
    metadata.expirable = component.expirable.unwrap_or(false);
    metadata.expired_text = component.expired_text.as_deref().map(redact_metadata_text);
    Ok(metadata)
}

fn safe_related_page(related_page: &Value) -> Option<Value> {
    let object = related_page.as_object()?;
    let path = object.get("path")?.as_str()?.trim();
    if path.is_empty() || path.starts_with('/') || path.contains("..") || path.contains('\0') {
        return None;
    }

    let mut safe = Map::new();
    safe.insert("path".to_owned(), Value::String(path.to_owned()));
    if let Some(query) = object.get("query").and_then(Value::as_object) {
        safe.insert(
            "query".to_owned(),
            Value::Object(
                query
                    .iter()
                    .map(|(key, value)| {
                        if is_sensitive_metadata_key(key) {
                            (key.clone(), Value::String("[REDACTED]".to_owned()))
                        } else {
                            (key.clone(), redact_metadata_value(value))
                        }
                    })
                    .collect(),
            ),
        );
    }
    Some(Value::Object(safe))
}

fn component_paths_match(declared: &str, requested: &str) -> bool {
    declared == requested || strip_index_suffix(declared) == strip_index_suffix(requested)
}

fn strip_index_suffix(path: &str) -> &str {
    path.strip_suffix("/index").unwrap_or(path)
}

fn redact_metadata_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    if is_sensitive_metadata_key(key) {
                        (key.clone(), Value::String("[REDACTED]".to_owned()))
                    } else {
                        (key.clone(), redact_metadata_value(value))
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(redact_metadata_value).collect()),
        Value::String(text) => Value::String(redact_metadata_text(text)),
        _ => value.clone(),
    }
}

fn redact_metadata_text(text: &str) -> String {
    if looks_sensitive_metadata_text(text) {
        "[REDACTED]".to_owned()
    } else {
        text.to_owned()
    }
}

fn is_sensitive_metadata_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    [
        "token",
        "authorization",
        "signature",
        "secret",
        "private",
        "credential",
        "password",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn looks_sensitive_metadata_text(text: &str) -> bool {
    text.starts_with('/')
        || text.starts_with("\\\\")
        || text.contains(":\\")
        || text.starts_with("file:")
}

fn load_component_package(
    skill_path: &Path,
    component_path: &str,
) -> Result<ComponentPackage, CliError> {
    ComponentPackage::load(component_directory(skill_path, component_path)?).map_err(Into::into)
}

fn component_directory(skill_path: &Path, component_path: &str) -> Result<PathBuf, CliError> {
    resolve_component_path(skill_path, component_path).map_err(Into::into)
}

fn find_tap_event(root: &RenderNode, method: &str) -> Option<ComponentEvent> {
    let binding = root
        .events
        .iter()
        .find(|event| event.event == RenderEventKind::Tap && event.method.as_str() == method);
    if let Some(binding) = binding {
        return Some(ComponentEvent::from_binding(binding));
    }
    root.children
        .iter()
        .find_map(|child| find_tap_event(child, method))
}

fn dispatch_first_api_call(
    instance: &mut ComponentInstance,
    event: &ComponentEvent,
) -> Result<ComponentVmAction, CliError> {
    let outcome = instance.dispatch_event(event)?;
    outcome
        .actions
        .into_iter()
        .find(|action| matches!(action, ComponentVmAction::ApiCall { .. }))
        .ok_or_else(|| CliError::Demo("component event did not emit api/call".to_owned()))
}

fn api_call_args(action: &ComponentVmAction, expected_name: &str) -> Result<Value, CliError> {
    match action {
        ComponentVmAction::ApiCall { name, arguments } if name == expected_name => {
            Ok(arguments.clone())
        }
        ComponentVmAction::ApiCall { name, .. } => Err(CliError::Demo(format!(
            "expected api/call `{expected_name}`, got `{name}`"
        ))),
        _ => Err(CliError::Demo("expected api/call action".to_owned())),
    }
}

fn required_component_path<'a>(
    outcome: &'a dock_core::CallOutcome,
    api_name: &str,
) -> Result<&'a str, CliError> {
    outcome
        .render
        .as_ref()
        .and_then(|render| render.component_path.as_deref())
        .ok_or_else(|| CliError::Demo(format!("API `{api_name}` did not render a component")))
}

fn step_summary(
    name: &str,
    result: &AtomicApiResult,
    root: &RenderNode,
    actions: &[ComponentVmAction],
) -> Value {
    json!({
        "name": name,
        "content": result.content,
        "structuredContent": result.structured_content,
        "renderRootKind": root.kind,
        "renderRootId": root.id,
        "actions": actions
    })
}

#[derive(Debug)]
struct DemoAuth {
    merchant_did: String,
    capability_token: String,
    token_received: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DemoDidChallenge {
    challenge_id: String,
    merchant_did: String,
    nonce: String,
    issued_at_ms: u64,
    expires_at_ms: Option<u64>,
    audience: String,
}

struct DemoHttpClient {
    base_url: String,
}

impl DemoHttpClient {
    fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }

    fn login(&self, auth_config: &DemoAuthConfig) -> Result<DemoAuth, CliError> {
        let challenge: DemoDidChallenge = serde_json::from_value(self.post_json(
            "/agents/coffee/auth/challenge",
            None,
            json!({
                "sessionId": DEFAULT_SESSION_ID,
                "skillId": DEFAULT_SKILL_ID,
                "userDid": auth_config.user_did,
                "agentDid": auth_config.agent_did
            }),
        )?)
        .map_err(|source| CliError::Json {
            label: "auth challenge response".to_owned(),
            source,
        })?;
        let session = IdentitySession::new(
            auth_config.user_did.clone(),
            auth_config.agent_did.clone(),
            challenge.merchant_did.clone(),
            DEFAULT_SKILL_ID,
            DEFAULT_SESSION_ID,
        );
        let payload = ChallengeProofPayload::from_challenge(
            &AdapterDidChallenge {
                challenge_id: challenge.challenge_id.clone(),
                merchant_did: challenge.merchant_did.clone(),
                nonce: challenge.nonce.clone(),
                expires_at_ms: challenge.expires_at_ms,
            },
            &session,
            challenge.audience.clone(),
            challenge.issued_at_ms,
        );
        let provider = FileDidCredentialProvider::from_config(auth_config.credential.clone())?;
        let proof = sign_challenge_proof(&payload, &provider, &session, AuthMode::HttpSignatures)?;
        let login_request = ChallengeLoginRequest {
            session_id: DEFAULT_SESSION_ID.to_owned(),
            skill_id: DEFAULT_SKILL_ID.to_owned(),
            user_did: auth_config.user_did.clone(),
            agent_did: auth_config.agent_did.clone(),
            merchant_did: challenge.merchant_did.clone(),
            challenge_id: challenge.challenge_id,
            signed_challenge: serde_json::to_value(proof).map_err(|source| CliError::Json {
                label: "signedChallenge".to_owned(),
                source,
            })?,
        };
        let login: ChallengeLoginResponse = serde_json::from_value(self.post_json(
            "/agents/coffee/auth/login",
            None,
            serde_json::to_value(login_request).map_err(|source| CliError::Json {
                label: "auth login request".to_owned(),
                source,
            })?,
        )?)
        .map_err(|source| CliError::Json {
            label: "auth login response".to_owned(),
            source,
        })?;
        Ok(DemoAuth {
            merchant_did: challenge.merchant_did,
            capability_token: login.capability_token.clone(),
            token_received: !login.capability_token.is_empty(),
        })
    }

    fn coffee_business_check(&self, token: &str) -> Result<Value, CliError> {
        let drinks = self.get_json("/api/drinks?query=latte", Some(token))?;
        let order = self.post_json(
            "/api/order/confirm",
            Some(token),
            json!({
                "drinkId": "latte",
                "size": "medium",
                "sugar": "less"
            }),
        )?;
        let order_id = order["orderId"]
            .as_str()
            .ok_or_else(|| CliError::Http("confirm order response missing orderId".to_owned()))?;
        let paid = self.post_json(
            "/api/order/pay",
            Some(token),
            json!({
                "orderId": order_id
            }),
        )?;

        Ok(json!({
            "firstDrinkId": drinks["drinks"].as_array().and_then(|items| items.first()).and_then(|item| item.get("id")).cloned().unwrap_or(Value::Null),
            "orderId": order["orderId"],
            "payable": order["payable"],
            "paymentStatus": paid["status"]
        }))
    }

    fn get_json(&self, path: &str, bearer: Option<&str>) -> Result<Value, CliError> {
        self.request_json("GET", path, bearer, None)
    }

    fn post_json(&self, path: &str, bearer: Option<&str>, body: Value) -> Result<Value, CliError> {
        self.request_json("POST", path, bearer, Some(body))
    }

    fn request_json(
        &self,
        method: &str,
        path: &str,
        bearer: Option<&str>,
        body: Option<Value>,
    ) -> Result<Value, CliError> {
        let (status, body) = http_request(&self.base_url, method, path, bearer, body)?;
        if !(200..300).contains(&status) {
            return Err(CliError::Http(format!(
                "{method} {path} returned {status}: {}",
                redact_text(&body)
            )));
        }
        serde_json::from_str(&body).map_err(|source| CliError::Json {
            label: format!("{method} {path} response"),
            source,
        })
    }
}

fn http_request(
    base_url: &str,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> Result<(u16, String), CliError> {
    let parsed = ParsedHttpUrl::parse(base_url)?;
    let body = body.map(|value| value.to_string()).unwrap_or_default();
    let mut stream = TcpStream::connect((parsed.host.as_str(), parsed.port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let full_path = if parsed.path_prefix == "/" {
        path.to_owned()
    } else {
        format!("{}{}", parsed.path_prefix.trim_end_matches('/'), path)
    };
    let auth = bearer
        .map(|token| format!("Authorization: Bearer {token}\r\n"))
        .unwrap_or_default();
    let request = format!(
        "{method} {full_path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{auth}\r\n{body}",
        parsed.host_header(),
        body.len()
    );
    stream.write_all(request.as_bytes())?;
    let response = read_http_response(&mut stream)?;
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| CliError::Http("HTTP response missing header separator".to_owned()))?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or_else(|| CliError::Http("HTTP response missing status code".to_owned()))?;
    Ok((status, body.to_owned()))
}

fn read_http_response(stream: &mut TcpStream) -> Result<String, CliError> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    let header_end = loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            return Err(CliError::Http(
                "connection closed before response headers".to_owned(),
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index;
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..header_end]).to_string();
    let body_start = header_end + 4;
    let content_length = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    });
    if let Some(content_length) = content_length {
        while bytes.len().saturating_sub(body_start) < content_length {
            let read = stream.read(&mut buffer)?;
            if read == 0 {
                return Err(CliError::Http(
                    "connection closed before full response body".to_owned(),
                ));
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
    } else {
        loop {
            let read = stream.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
    }
    String::from_utf8(bytes).map_err(|error| CliError::Http(error.to_string()))
}

#[derive(Debug)]
struct ParsedHttpUrl {
    host: String,
    port: u16,
    path_prefix: String,
}

impl ParsedHttpUrl {
    fn parse(url: &str) -> Result<Self, CliError> {
        let rest = url.strip_prefix("http://").ok_or_else(|| {
            CliError::Http("only http:// demo server URLs are supported".to_owned())
        })?;
        let (authority, path_prefix) = rest
            .split_once('/')
            .map(|(authority, path)| (authority, format!("/{path}")))
            .unwrap_or((rest, "/".to_owned()));
        let (host, port) = authority
            .rsplit_once(':')
            .map(|(host, port)| {
                let port = port
                    .parse::<u16>()
                    .map_err(|error| CliError::Http(format!("invalid server port: {error}")))?;
                Ok::<_, CliError>((host.to_owned(), port))
            })
            .transpose()?
            .unwrap_or_else(|| (authority.to_owned(), 80));
        if host.is_empty() {
            return Err(CliError::Http("server URL missing host".to_owned()));
        }
        Ok(Self {
            host,
            port,
            path_prefix,
        })
    }

    fn host_header(&self) -> String {
        if self.port == 80 {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

fn parse_json(source: &str, label: &str) -> Result<Value, CliError> {
    serde_json::from_str(source).map_err(|source| CliError::Json {
        label: label.to_owned(),
        source,
    })
}

fn parse_atomic_result(source: &str) -> Result<AtomicApiResult, CliError> {
    serde_json::from_str(source).map_err(|source| CliError::Json {
        label: "resultJson".to_owned(),
        source,
    })
}

fn parse_component_input(source: &str) -> Result<ComponentInput, CliError> {
    let value = parse_json(source, "jsonInput")?;
    match serde_json::from_value::<ComponentInput>(value.clone()) {
        Ok(input) => Ok(input),
        Err(_) => Ok(ComponentInput {
            api_name: value
                .get("apiName")
                .and_then(Value::as_str)
                .unwrap_or("preview")
                .to_owned(),
            arguments: value
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new())),
            properties: value
                .get("properties")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default(),
            content: component_content_from_json(&value),
            structured_content: value
                .get("structuredContent")
                .or_else(|| value.get("structured_content"))
                .and_then(Value::as_object)
                .cloned(),
            meta: value
                .get("_meta")
                .or_else(|| value.get("meta"))
                .and_then(Value::as_object)
                .cloned(),
            component_metadata: ComponentMetadata::default(),
        }),
    }
}

fn component_content_from_json(value: &Value) -> Vec<Value> {
    value
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn write_json(writer: &mut impl Write, output: &impl Serialize) -> Result<(), CliError> {
    serde_json::to_writer_pretty(&mut *writer, output).map_err(|source| CliError::Json {
        label: "output".to_owned(),
        source,
    })?;
    writeln!(writer)?;
    Ok(())
}

fn validation_summary(report: &ValidationReport) -> Value {
    json!({
        "valid": report.is_valid(),
        "errors": report
            .errors
            .iter()
            .map(validation_issue_json)
            .collect::<Vec<_>>(),
        "warnings": report
            .warnings
            .iter()
            .map(validation_issue_json)
            .collect::<Vec<_>>()
    })
}

fn validation_issue_json(issue: &ValidationIssue) -> Value {
    json!({
        "level": issue.level,
        "category": issue.category,
        "path": redact_text(&issue.path),
        "message": redact_text(&issue.message),
        "suggestion": issue.suggestion.as_ref().map(|suggestion| redact_text(suggestion)),
    })
}

fn skill_id(skill: &LoadedSkill) -> String {
    skill
        .manifest
        .extra
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_SKILL_ID)
        .to_owned()
}

fn skill_id_for_path(skill: &LoadedSkill, skill_path: &Path) -> String {
    skill
        .manifest
        .extra
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            if is_coffee_fixture_shape(skill) {
                Some(DEFAULT_SKILL_ID.to_owned())
            } else {
                None
            }
        })
        .or_else(|| {
            skill_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| DEFAULT_SKILL_ID.to_owned())
}

fn fallback_reason_from_str(reason: &str) -> FallbackReason {
    FallbackReason::normalize(reason)
}

fn render_outcome_json(render: Option<&RenderOutcome>) -> Value {
    let Some(render) = render else {
        return Value::Null;
    };
    json!({
        "renderer": render.renderer,
        "componentPath": render.component_path,
        "payload": render.payload,
        "fallbackReason": render.fallback_reason
    })
}

fn audit_events_json(events: &[AuditEvent]) -> Value {
    Value::Array(
        events
            .iter()
            .map(|event| {
                json!({
                    "userDid": event.user_did,
                    "agentDid": event.agent_did,
                    "merchantDid": event.merchant_did,
                    "sessionId": event.session_id,
                    "skillId": event.skill_id,
                    "apiName": event.api_name,
                    "riskLevel": event.risk_level,
                    "parameterSummary": event.parameter_summary,
                    "permissionDecision": event.permission_decision,
                    "consentProof": event.consent_proof,
                    "outcome": event.outcome
                })
            })
            .collect(),
    )
}

fn component_render_json(render: &ComponentRenderOutput) -> Value {
    json!({
        "schemaVersion": render.schema_version,
        "root": render.root,
        "warnings": render.warnings
    })
}

fn redact_text(value: &str) -> String {
    for marker in [
        "capabilityToken",
        "Authorization",
        "Signature",
        "token",
        "secret",
        "private",
    ] {
        if value
            .to_ascii_lowercase()
            .contains(&marker.to_ascii_lowercase())
        {
            return "[REDACTED]".to_owned();
        }
    }
    value.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcp_schema::ValidationIssueLevel;
    use std::fs;

    #[test]
    fn parses_validate_args() {
        let cli = Cli::try_parse_from_args(["dock-cli", "validate", "examples/coffee-skill"])
            .expect("args parse");
        assert!(matches!(cli.command, Command::Validate { .. }));
    }

    #[test]
    fn parses_inspect_args() {
        let cli = Cli::try_parse_from_args(["dock-cli", "inspect", "examples/coffee-skill"])
            .expect("args parse");
        assert!(matches!(cli.command, Command::Inspect { .. }));
    }

    #[test]
    fn parses_test_skill_args() {
        let cli = Cli::try_parse_from_args(["dock-cli", "test-skill", "examples/coffee-skill"])
            .expect("args parse");
        assert!(matches!(cli.command, Command::TestSkill { .. }));
    }

    #[test]
    fn parses_import_wechat_mcp_args() {
        let cli = Cli::try_parse_from_args([
            "dock-cli",
            "import-wechat-mcp",
            "examples/coffee-skill",
            "--dest",
            "examples/imported/coffee-skill",
            "--write",
        ])
        .expect("args parse");
        assert!(matches!(cli.command, Command::ImportWechatMcp { .. }));
    }

    #[test]
    fn parses_doctor_args() {
        let cli = Cli::try_parse_from_args([
            "dock-cli",
            "doctor",
            "--skill",
            "examples/coffee-skill",
            "--server",
            "http://127.0.0.1:3000",
            "--ci",
        ])
        .expect("args parse");
        assert!(matches!(cli.command, Command::Doctor { ci: true, .. }));
    }

    #[test]
    fn parses_perf_args() {
        let cli = Cli::try_parse_from_args([
            "dock-cli",
            "perf",
            "examples/coffee-skill",
            "--iterations",
            "1",
        ])
        .expect("args parse");
        assert!(matches!(
            cli.command,
            Command::Perf {
                full: false,
                iterations: Some(1),
                ..
            }
        ));
    }

    #[test]
    fn parses_runtime_json_args() {
        let cli = Cli::try_parse_from_args([
            "dock-cli",
            "runtime-json",
            "examples/coffee-skill",
            r#"{"apiVersion":"dock.runtime.v1","requestId":"req-1","method":"runtime.loadSkill","params":{}}"#,
        ])
        .expect("args parse");
        assert!(matches!(cli.command, Command::RuntimeJson { .. }));
    }

    #[test]
    fn preview_card_renders_fallback_card() {
        let output = preview_card(
            r#"{"content":[{"type":"text","text":"hello"}],"structuredContent":{"orderId":"1"}}"#,
        )
        .expect("preview card");

        assert_eq!(output["status"], "ok");
        assert_eq!(output["card"]["version"], "card-spec/v0");
        assert_eq!(
            output["card"]["fallbackReason"],
            "host_renderer_unavailable"
        );
    }

    #[test]
    fn redacts_http_errors() {
        let redacted = redact_text(r#"{"capabilityToken":"demo-token"}"#);
        assert_eq!(redacted, "[REDACTED]");
    }

    #[test]
    fn doctor_reports_required_checks_and_redacts_default_fixture_paths() {
        let credential = CredentialFixture::new();
        let skill = SkillFixture::new();
        let output = doctor(DoctorOptions {
            skill: Some(&skill.root),
            server: None,
            runtime_config: None,
            did_document: Some(&credential.did_path),
            private_key: Some(&credential.key_path),
            user_did: Some("did:wba:user.example"),
            agent_did: Some("did:wba:agent.example"),
            identity_handle: None,
            identity_root: None,
            ci: false,
        })
        .expect("doctor report");

        assert_eq!(output["schemaVersion"], DOCTOR_REPORT_SCHEMA_VERSION);
        assert_eq!(output["commandStatus"], "ok");
        assert_eq!(output["summary"]["skipCountsAsPass"], false);
        assert!(doctor_has_check(&output, "rust_toolchain"));
        assert!(doctor_has_check(&output, "did_identity"));
        assert!(doctor_has_check(&output, "credential_permissions"));
        assert!(doctor_has_check(&output, "trusted_resolver"));
        assert!(doctor_has_check(&output, "network_allowlist"));
        assert!(doctor_has_check(&output, "storage_backend"));
        assert!(doctor_has_check(&output, "audit_backend"));
        assert!(doctor_has_check(&output, "host_providers"));
        assert!(doctor_has_check(&output, "sandbox_gates"));
        assert!(doctor_has_check(&output, "server_health"));
        assert_eq!(doctor_check_status(&output, "server_health"), Some("skip"));

        let rendered = output.to_string();
        assert!(!rendered.contains(&credential.dir_path().display().to_string()));
        assert!(!rendered.contains(&skill.root.display().to_string()));
        assert!(!rendered.contains("test-only-key"));
        assert!(!rendered.contains("Authorization"));
        assert!(!rendered.contains("Bearer "));
        assert!(!rendered.contains("capabilityToken"));
    }

    #[test]
    fn doctor_runtime_config_file_reports_production_ready_backends() {
        let dir = TempDir::new("dock-cli-doctor-config").expect("temp dir");
        let config_path = dir.path().join("runtime.json");
        fs::write(
            &config_path,
            r#"{
              "schemaVersion": "dock.runtime.config.v1",
              "profile": "production",
              "identity": {
                "provider": {"handle": "host-identity"}
              },
              "resolver": {
                "provider": {"handle": "host-resolver"}
              },
              "allowlist": {
                "networkRules": [{
                  "name": "coffee-api",
                  "source": {"kind": "runtimeData", "path": "allowlist/coffee.json"}
                }]
              },
              "tokenIssuer": {
                "issuer": "did:wba:issuer.example",
                "secretRef": {"kind": "secretStore", "key": "dock/token-issuer"}
              },
              "storage": {
                "backend": "encryptedSqlite",
                "pathRef": {"kind": "runtimeData", "path": "state/storage.sqlite3"}
              },
              "audit": {
                "backend": "hostProvider",
                "provider": {"handle": "host-audit"},
                "retentionDays": 30
              },
              "cache": {
                "backend": "hostProvider",
                "provider": {"handle": "host-cache"}
              },
              "hostProviders": [{
                "handle": "host-runtime",
                "capabilities": ["render", "consent", "payment"]
              }]
            }"#,
        )
        .expect("write config");

        let output = doctor(DoctorOptions {
            skill: None,
            server: None,
            runtime_config: Some(&config_path),
            did_document: None,
            private_key: None,
            user_did: None,
            agent_did: None,
            identity_handle: None,
            identity_root: None,
            ci: false,
        })
        .expect("doctor report");

        assert_eq!(
            doctor_check_status(&output, "trusted_resolver"),
            Some("pass")
        );
        assert_eq!(
            doctor_check_status(&output, "network_allowlist"),
            Some("pass")
        );
        assert_eq!(
            doctor_check_status(&output, "storage_backend"),
            Some("pass")
        );
        assert_eq!(doctor_check_status(&output, "audit_backend"), Some("pass"));
        assert_eq!(doctor_check_status(&output, "host_providers"), Some("pass"));
        let rendered = output.to_string();
        assert!(!rendered.contains(&dir.path().display().to_string()));
        assert!(!rendered.contains("secretStore"));
    }

    #[test]
    fn doctor_ci_reports_failed_command_status_without_hiding_json() {
        let mut cli = Cli::try_parse_from_args([
            "dock-cli",
            "doctor",
            "--did-document",
            "missing-did.json",
            "--private-key",
            "missing-key.pem",
            "--ci",
        ])
        .expect("args parse");
        let output = cli.execute().expect("doctor executes");
        assert_eq!(output["schemaVersion"], DOCTOR_REPORT_SCHEMA_VERSION);
        assert_eq!(output["commandStatus"], "failed");
        assert_eq!(doctor_check_status(&output, "did_identity"), Some("fail"));

        let cli = Cli::try_parse_from_args([
            "dock-cli",
            "doctor",
            "--did-document",
            "missing-did.json",
            "--private-key",
            "missing-key.pem",
            "--ci",
        ])
        .expect("args parse");
        let mut writer = Vec::new();
        let error = run_with_writer(cli, &mut writer).expect_err("ci failure exits non-zero");
        assert!(error.to_string().contains("failing checks"));
        let rendered = String::from_utf8(writer).expect("utf8 output");
        assert!(rendered.contains(DOCTOR_REPORT_SCHEMA_VERSION));
    }

    #[test]
    fn validate_reports_api_registration_mismatch_as_release_blocker() {
        let fixture = SkillFixture::new();
        let output = validate(&fixture.root).expect("validate reports compatibility");

        assert_eq!(output["schemaVersion"], VALIDATE_REPORT_SCHEMA_VERSION);
        assert_eq!(output["status"], "warning");
        assert_eq!(output["commandStatus"], "ok");
        assert_eq!(output["reportStatus"], "warning");
        assert_eq!(output["compatibilityLevel"], "demo-only");
        assert_eq!(
            output["compatibilityReport"]["schemaVersion"],
            VALIDATE_REPORT_SCHEMA_VERSION
        );
        assert_eq!(output["compatibilityReport"]["status"], "warning");
        assert_eq!(output["skillRef"]["redacted"], true);
        assert_eq!(output["skillRoot"], "[REDACTED]");
        assert!(output["compatibilityReport"]["apis"]
            .as_array()
            .expect("api reports")
            .iter()
            .any(|api| {
                api["name"] == "missing"
                    && api["registered"] == false
                    && api["compatibilityStatus"] == "unsupported"
                    && api["status"] == "registration-unverified"
                    && api["suggestion"]
                        .as_str()
                        .is_some_and(|suggestion| suggestion.contains("Register this API"))
            }));
        assert!(output["apis"]
            .as_array()
            .expect("top-level api reports")
            .iter()
            .any(|api| api["name"] == "missing" && api["compatibilityStatus"] == "unsupported"));
        assert!(output["apiNames"]
            .as_array()
            .expect("api names")
            .iter()
            .any(|api| api == "declared"));
        assert!(output["compatibilityReport"]["releaseBlockers"]
            .as_array()
            .expect("release blockers")
            .iter()
            .any(|blocker| {
                blocker["code"] == "api_registration_mismatch"
                    && blocker["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("declared but not registered"))
            }));
        assert_eq!(
            output["compatibilityReport"]["supplyChain"]["status"],
            "demo-unsigned"
        );
        assert_eq!(
            output["compatibilityReport"]["supplyChain"]["productionReady"],
            false
        );
        assert!(output["compatibilityReport"]["releaseBlockers"]
            .as_array()
            .expect("release blockers")
            .iter()
            .any(
                |blocker| blocker["code"] == "supply_chain" && blocker["status"] == "demo-unsigned"
            ));
        assert_eq!(output["releaseReadiness"]["status"], "blocked");
        assert!(output["releaseReadiness"]["checks"]
            .as_array()
            .expect("release checks")
            .iter()
            .any(|check| check["code"] == "persistence_backends"
                && check["status"] == "not-evaluated-by-validate"
                && check["productionReady"] == false));
        assert!(output["repairSuggestions"]
            .as_array()
            .expect("repair suggestions")
            .iter()
            .any(|suggestion| suggestion["source"] == "releaseBlockers"
                && suggestion["severity"] == "blocker"));
        assert!(!output.to_string().contains("fixture-signature-secret"));
        assert!(!output
            .to_string()
            .contains(&fixture.root.display().to_string()));
    }

    #[test]
    fn inspect_reports_package_graph_and_wx_usage_without_absolute_paths() {
        let fixture = InspectSkillFixture::new();
        let output = inspect(&fixture.root).expect("inspect reports package graph");

        assert_eq!(output["schemaVersion"], "dock.inspect-report.v1");
        assert_eq!(output["status"], "warning");
        assert_eq!(output["commandStatus"], "ok");
        assert_eq!(output["skillRef"]["redacted"], true);
        assert_eq!(output["skillId"], "inspect-fixture");
        assert_eq!(output["package"]["entry"], "index.js");
        assert_eq!(output["package"]["skillMd"], "SKILL.md");
        assert!(output["files"]
            .as_array()
            .expect("files")
            .iter()
            .any(|file| file["path"] == "mcp.json" && file["kind"] == "file"));
        assert!(output["files"]
            .as_array()
            .expect("files")
            .iter()
            .all(|file| file["path"]
                .as_str()
                .is_some_and(|path| !path.starts_with('/'))));
        assert!(output["registeredApis"]
            .as_array()
            .expect("registered apis")
            .iter()
            .any(|api| api == "registered"));
        assert!(matches!(
            output["registeredApisSource"].as_str(),
            Some("api-vm-registration-trace" | "static-register-api-scan")
        ));
        assert!(output["apis"]
            .as_array()
            .expect("apis")
            .iter()
            .any(|api| api["name"] == "registered"
                && api["registered"] == true
                && matches!(
                    api["registrationStatus"].as_str(),
                    Some("declared-and-registered" | "registered-static-with-vm-error")
                )
                && api["risk"] == "payment"
                && api["consentRequired"] == true));
        assert!(output["apis"]
            .as_array()
            .expect("apis")
            .iter()
            .any(|api| api["name"] == "declaredOnly"
                && api["registered"] == false
                && api["registrationStatus"] == "declared-only"));
        assert!(output["components"]
            .as_array()
            .expect("components")
            .iter()
            .any(
                |component| component["path"] == "components/inspect-card/index"
                    && component["compatibilityStatus"] == "host-boundary"
                    && component["permissions"]["dynamic"] == true
            ));
        assert!(output["permissions"]["requiredHostCapabilities"]
            .as_array()
            .expect("host capabilities")
            .iter()
            .any(|capability| capability["componentPath"] == "components/inspect-card/index"));
        assert!(output["wxApiUsage"]["items"]
            .as_array()
            .expect("wx usage")
            .iter()
            .any(|usage| usage["api"] == "wx.login" && usage["file"] == "index.js"));
        assert!(output["wxApiUsage"]["items"]
            .as_array()
            .expect("wx usage")
            .iter()
            .any(|usage| usage["api"] == "wx.request"
                && usage["file"] == "components/inspect-card/index.js"));
        assert!(output["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning["code"] == "validation_warning"));
        let rendered = output.to_string();
        assert!(!rendered.contains(&fixture.root.display().to_string()));
        assert!(!rendered.contains("super-secret-token"));
        assert!(!rendered.contains("Authorization"));
    }

    #[test]
    fn validation_summary_redacts_sensitive_issue_text() {
        let summary = validation_summary(&ValidationReport {
            errors: vec![ValidationIssue {
                level: ValidationIssueLevel::Error,
                category: ValidationIssueCategory::Spec,
                path: "private/source/path".to_owned(),
                message: "Authorization header leaked".to_owned(),
                suggestion: Some("move secret to Host provider".to_owned()),
            }],
            warnings: Vec::new(),
        });

        let rendered = summary.to_string();
        assert!(!rendered.contains("private/source/path"));
        assert!(!rendered.contains("Authorization header leaked"));
        assert!(!rendered.contains("secret"));
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn import_wechat_mcp_dry_run_reports_structure_patch_and_redacts() {
        let fixture = ImportSkillFixture::new();
        let output = import_wechat_mcp(ImportOptions {
            source: &fixture.root,
            dest: None,
            dry_run: true,
            overwrite: false,
            generate_patch: true,
            include_fixtures: true,
        })
        .expect("import dry-run");

        assert_eq!(output["schemaVersion"], IMPORT_REPORT_SCHEMA_VERSION);
        assert_eq!(output["status"], "dry-run");
        assert_eq!(output["commandStatus"], "ok");
        assert_eq!(output["mode"]["dryRun"], true);
        assert_eq!(output["skillId"], "import-fixture");
        assert!(output["structure"]["requiredFiles"]
            .as_array()
            .expect("required files")
            .iter()
            .all(|file| file["present"] == true));
        assert_eq!(output["appJson"]["status"], "found");
        assert_eq!(output["appJson"]["items"][0]["secretToken"], "[REDACTED]");
        assert_eq!(output["migrationPatch"]["status"], "suggested");
        assert_eq!(output["migrationPatch"]["productionReady"], false);
        assert!(output["migrationPatch"]["changes"]
            .as_array()
            .expect("changes")
            .iter()
            .any(|change| change["path"] == "mcp.json/apis/registered"));
        assert_eq!(
            output["compatibilityReport"]["supplyChain"]["productionReady"],
            false
        );
        assert!(output["nextCommands"]
            .as_array()
            .expect("next commands")
            .iter()
            .any(|command| command["label"] == "safe-copy"));

        let rendered = output.to_string();
        assert!(!rendered.contains(&fixture.root.display().to_string()));
        assert!(!rendered.contains("import-secret-token"));
        assert!(!rendered.contains("Authorization"));
    }

    #[test]
    fn perf_smoke_report_covers_baselines_stress_and_redacts() {
        let project_root = default_project_root().expect("project root");
        let output =
            perf(&project_root.join("examples/coffee-skill"), false, Some(1)).expect("perf report");

        assert_eq!(output["schemaVersion"], PERF_REPORT_SCHEMA_VERSION);
        assert_eq!(output["status"], "ok");
        assert_eq!(output["commandStatus"], "ok");
        assert_eq!(output["mode"], "smoke");
        assert_eq!(output["baseline"]["iterationsPerCase"], 1);
        assert!(perf_has_category(&output, "skill_load"));
        assert!(perf_has_category(&output, "api_vm_call"));
        assert!(perf_has_category(&output, "component_render"));
        assert!(perf_has_category(&output, "render_ir_size"));
        assert!(perf_has_category(&output, "storage_read"));
        assert!(perf_has_category(&output, "storage_write"));
        assert!(perf_has_category(&output, "token_lookup"));
        assert!(perf_has_sample(&output, "stress.concurrent_sessions"));
        assert!(perf_has_sample(&output, "stress.multi_skill"));
        assert!(perf_has_sample(&output, "stress.multi_component_render"));
        assert!(perf_has_sample(&output, "stress.dynamic_timer_request"));
        assert!(perf_has_sample(
            &output,
            "resource_limit.result_size_fail_closed"
        ));
        assert_eq!(output["stress"]["status"], "pass");
        assert_eq!(
            output["resource"]["memoryPerVm"]["measurement"],
            "process-rss-sample"
        );

        let rendered = output.to_string();
        for forbidden in [
            "/home/",
            "/Users/",
            "Authorization",
            "Signature",
            "capabilityToken",
            "fixture-token",
            "perf-token-redacted",
            "Bearer ",
        ] {
            assert!(
                !rendered.contains(forbidden),
                "perf report leaked forbidden marker {forbidden}"
            );
        }
    }

    #[test]
    fn import_wechat_mcp_safe_copy_preserves_original_fields() {
        let fixture = ImportSkillFixture::new();
        let dest_dir = TempDir::new("dock-cli-import-dest").expect("temp dir");
        let dest = dest_dir.path().join("imported-skill");

        let output = import_wechat_mcp(ImportOptions {
            source: &fixture.root,
            dest: Some(&dest),
            dry_run: false,
            overwrite: false,
            generate_patch: true,
            include_fixtures: true,
        })
        .expect("import copy");

        assert_eq!(output["status"], "copied");
        assert!(output["blockers"].as_array().expect("blockers").is_empty());
        assert!(dest.join("SKILL.md").is_file());
        assert!(dest.join("mcp.json").is_file());
        assert!(dest.join("components/import-card/index.wxml").is_file());

        let copied_manifest =
            fs::read_to_string(dest.join("mcp.json")).expect("read copied manifest");
        assert!(copied_manifest.contains("\"wechatOriginalField\""));
        let validate_output = validate(&dest).expect("copied package validates");
        assert_eq!(
            validate_output["schemaVersion"],
            VALIDATE_REPORT_SCHEMA_VERSION
        );
        assert_eq!(validate_output["commandStatus"], "ok");
    }

    #[test]
    fn imported_coffee_skill_keeps_fixture_runner_shape_after_rename() {
        let fixture = ProjectIdentityFixture::new();
        let source = fixture.root.join("examples/coffee-skill");
        let project_root = default_project_root().expect("project root");
        copy_dir_all(&project_root.join("examples/coffee-skill"), &source)
            .expect("copy coffee fixture");
        let imported = fixture.root.join("imported/renamed-skill");

        let output = import_wechat_mcp(ImportOptions {
            source: &source,
            dest: Some(&imported),
            dry_run: false,
            overwrite: false,
            generate_patch: true,
            include_fixtures: true,
        })
        .expect("import coffee");

        assert_eq!(output["status"], "copied");
        let test_report = test_skill(&imported).expect("run copied coffee fixture");
        assert_eq!(test_report["status"], "ok");
        assert_eq!(test_report["fixtureSet"], "coffee");
        assert_eq!(test_report["summary"]["total"], 3);
        assert_eq!(test_report["summary"]["failed"], 0);
    }

    #[test]
    fn import_wechat_mcp_overwrite_requires_explicit_flag() {
        let fixture = ImportSkillFixture::new();
        let dest_dir = TempDir::new("dock-cli-import-overwrite").expect("temp dir");
        fs::write(dest_dir.path().join("mcp.json"), "{}").expect("write existing file");

        let output = import_wechat_mcp(ImportOptions {
            source: &fixture.root,
            dest: Some(dest_dir.path()),
            dry_run: false,
            overwrite: false,
            generate_patch: true,
            include_fixtures: true,
        })
        .expect("import blocks overwrite");

        assert_eq!(output["status"], "blocked");
        assert!(output["blockers"]
            .as_array()
            .expect("blockers")
            .iter()
            .any(
                |blocker| blocker["code"] == "overwrite_required" && blocker["path"] == "mcp.json"
            ));
    }

    #[test]
    fn import_wechat_mcp_missing_files_report_blockers() {
        let dir = TempDir::new("dock-cli-import-missing").expect("temp dir");
        fs::write(dir.path().join("SKILL.md"), "# Missing").expect("write skill");

        let output = import_wechat_mcp(ImportOptions {
            source: dir.path(),
            dest: None,
            dry_run: true,
            overwrite: false,
            generate_patch: true,
            include_fixtures: true,
        })
        .expect("import missing report");

        assert_eq!(output["status"], "blocked");
        assert!(output["blockers"]
            .as_array()
            .expect("blockers")
            .iter()
            .any(|blocker| blocker["code"] == "missing_required_file"
                && blocker["path"] == "mcp.json"));
        assert!(output["blockers"]
            .as_array()
            .expect("blockers")
            .iter()
            .any(|blocker| blocker["code"] == "load_skill_failed"));
    }

    #[cfg(unix)]
    #[test]
    fn import_wechat_mcp_denies_symlink_copy() {
        use std::os::unix::fs as unix_fs;

        let fixture = ImportSkillFixture::new();
        unix_fs::symlink(
            fixture.root.join("index.js"),
            fixture.root.join("linked-index.js"),
        )
        .expect("create symlink");

        let output = import_wechat_mcp(ImportOptions {
            source: &fixture.root,
            dest: None,
            dry_run: true,
            overwrite: false,
            generate_patch: true,
            include_fixtures: true,
        })
        .expect("import symlink report");

        assert_eq!(output["status"], "blocked");
        assert!(output["structure"]["symlinks"]
            .as_array()
            .expect("symlinks")
            .iter()
            .any(|link| link["path"] == "linked-index.js"));
        assert!(output["blockers"]
            .as_array()
            .expect("blockers")
            .iter()
            .any(|blocker| blocker["code"] == "symlink_denied"));
    }

    #[test]
    fn first_json_diff_reports_stable_path() {
        let diff = first_json_diff(
            "",
            &json!({"render": {"root": {"kind": "view"}}}),
            &json!({"render": {"root": {"kind": "text"}}}),
        )
        .expect("diff");

        assert_eq!(diff["path"], "render.root.kind");
        assert_eq!(diff["expected"], "view");
        assert_eq!(diff["actual"], "text");
    }

    #[test]
    fn validate_reports_component_runtime_metadata() {
        let dir = TempDir::new("dock-cli-metadata-fixture").expect("temp dir");
        let root = dir.path().to_path_buf();
        fs::create_dir_all(root.join("components/status-card")).expect("component dir");
        fs::write(root.join("SKILL.md"), "# Metadata Skill").expect("write SKILL.md");
        fs::write(
            root.join("index.js"),
            "const skill = wx.modelContext.createSkill(__dirname)\n\
             skill.registerAPI('status', async () => ({ content: [{ type: 'text', text: 'ok' }] }))\n\
             module.exports = skill\n",
        )
        .expect("write index.js");
        fs::write(
            root.join("components/status-card/index.wxml"),
            "<view><text>{{ apiName }}</text></view>",
        )
        .expect("write wxml");
        fs::write(
            root.join("mcp.json"),
            r#"{
              "apis": [{
                "name": "status",
                "description": "status API",
                "inputSchema": {},
                "_meta": { "ui": { "componentPath": "components/status-card/index" } }
              }],
              "components": [{
                "path": "components/status-card/index",
                "permissions": {
                  "scope.dynamic": { "desc": "refresh status" }
                },
                "relatedPage": {
                  "path": "pages/status/detail",
                  "query": {
                    "source": "card",
                    "secretToken": "should-not-leak"
                  }
                },
                "expirable": true,
                "expiredText": "Status expired"
              }]
            }"#,
        )
        .expect("write manifest");

        let output = validate(&root).expect("validate metadata");
        let component = output["compatibilityReport"]["components"][0].clone();

        assert_eq!(component["loaded"], true);
        assert_eq!(component["compatibilityStatus"], "host-boundary");
        assert_eq!(
            component["suggestion"],
            "Review dynamic component request/timer policy and Host production boundary before release."
        );
        assert_eq!(
            component["runtimeMetadata"]["componentPath"],
            "components/status-card/index"
        );
        assert_eq!(component["runtimeMetadata"]["dynamic"], true);
        assert_eq!(component["runtimeMetadata"]["expirable"], true);
        assert_eq!(
            component["runtimeMetadata"]["expiredText"],
            "Status expired"
        );
        assert_eq!(
            component["runtimeMetadata"]["relatedPage"]["path"],
            "pages/status/detail"
        );
        assert_eq!(
            component["runtimeMetadata"]["relatedPage"]["query"]["secretToken"],
            "[REDACTED]"
        );
        assert!(
            output["compatibilityReport"]["permissions"]["requiredHostCapabilities"]
                .as_array()
                .expect("required host capabilities")
                .iter()
                .any(|capability| {
                    capability["capability"] == "component.dynamic"
                        && capability["componentPath"] == "components/status-card/index"
                })
        );
        assert!(!component.to_string().contains("should-not-leak"));
    }

    #[test]
    fn validate_redacts_package_signature_value() {
        let fixture = SignedSkillFixture::new();
        let output = validate(&fixture.root).expect("validate signed fixture");

        assert_eq!(
            output["compatibilityReport"]["supplyChain"]["signature"]["keyId"],
            "did:wba:publisher.example#package-key-1"
        );
        assert_eq!(
            output["compatibilityReport"]["supplyChain"]["status"],
            "quarantined"
        );
        assert!(!output.to_string().contains("fixture-signature-secret"));
    }

    #[test]
    fn component_metadata_matches_index_path_alias() {
        let manifest: mcp_schema::SkillManifest = serde_json::from_value(json!({
            "apis": [],
            "components": [{
                "path": "components/status-card/index",
                "expirable": true
            }]
        }))
        .expect("manifest");

        let metadata =
            manifest_component_metadata(&manifest, "components/status-card").expect("metadata");

        assert_eq!(
            metadata.component_path.as_deref(),
            Some("components/status-card/index")
        );
        assert!(metadata.expirable);
    }

    #[test]
    fn finds_nested_tap_event() {
        let mut binding =
            component_runtime::RenderEventBinding::new(RenderEventKind::Tap, "confirmDrink");
        binding.dataset.insert("id".to_owned(), json!("latte"));
        let root = RenderNode::new("root", component_runtime::RenderNodeKind::View).with_child(
            RenderNode::new("button", component_runtime::RenderNodeKind::Button)
                .with_event(binding),
        );

        let event = find_tap_event(&root, "confirmDrink").expect("event");

        assert_eq!(event.kind, component_runtime::ComponentEventKind::Tap);
        assert_eq!(event.dataset["id"], "latte");
    }

    fn doctor_has_check(report: &Value, id: &str) -> bool {
        doctor_check_status(report, id).is_some()
    }

    fn doctor_check_status<'a>(report: &'a Value, id: &str) -> Option<&'a str> {
        report["checks"]
            .as_array()?
            .iter()
            .find(|check| check["id"] == id)?
            .get("status")?
            .as_str()
    }

    fn perf_has_category(report: &Value, category: &str) -> bool {
        report["samples"]
            .as_array()
            .expect("perf samples")
            .iter()
            .any(|sample| sample["category"] == category)
    }

    fn perf_has_sample(report: &Value, name: &str) -> bool {
        report["samples"]
            .as_array()
            .expect("perf samples")
            .iter()
            .any(|sample| sample["name"] == name)
    }

    #[test]
    fn parses_run_demo_explicit_credential_config() {
        let fixture = CredentialFixture::new();
        let cli = Cli::try_parse_from_args([
            "dock-cli".to_owned(),
            "run-demo".to_owned(),
            "--skill".to_owned(),
            "examples/coffee-skill".to_owned(),
            "--server".to_owned(),
            "http://127.0.0.1:3000".to_owned(),
            "--did-document".to_owned(),
            fixture.did_path.display().to_string(),
            "--private-key".to_owned(),
            fixture.key_path.display().to_string(),
            "--user-did".to_owned(),
            "did:wba:user.example".to_owned(),
        ])
        .expect("CLI args parse");

        let Command::RunDemo {
            did_document,
            private_key,
            user_did,
            ..
        } = cli.command
        else {
            panic!("expected run-demo");
        };
        let config = DemoAuthConfig::from_inputs(
            did_document,
            private_key,
            user_did,
            None,
            None,
            None,
            EmptyConfigSource,
        )
        .expect("credential config parses");

        assert_eq!(config.user_did, "did:wba:user.example");
        assert_eq!(config.credential.did_document_path, fixture.did_path);
    }

    #[test]
    fn derives_user_did_from_did_document_for_path_credentials() {
        let fixture = CredentialFixture::new();
        let config = DemoAuthConfig::from_inputs(
            Some(fixture.did_path.clone()),
            Some(fixture.key_path),
            None,
            Some("did:wba:agent.example".to_owned()),
            None,
            None,
            EmptyConfigSource,
        )
        .expect("credential config parses");

        assert_eq!(config.user_did, "did:wba:user.example");
        assert_eq!(config.agent_did.as_deref(), Some("did:wba:agent.example"));
        assert_eq!(config.credential.did_document_path, fixture.did_path);
    }

    #[test]
    fn incomplete_run_demo_credential_config_fails_closed() {
        let fixture = CredentialFixture::new();

        let error = DemoAuthConfig::from_inputs(
            Some(fixture.did_path),
            None,
            Some("did:wba:user.example".to_owned()),
            None,
            None,
            None,
            EmptyConfigSource,
        )
        .expect_err("missing private key must fail");

        assert_eq!(error, DidCredentialError::InvalidIdentity);
    }

    #[test]
    fn resolves_identity_handle_from_store_without_exposing_key_content() {
        let fixture = IdentityStoreFixture::new("miniapp-test.awiki.ai");
        let config = DemoAuthConfig::from_inputs(
            None,
            None,
            None,
            None,
            Some("miniapp-test.awiki.ai".to_owned()),
            Some(fixture.root.clone()),
            EmptyConfigSource,
        )
        .expect("identity handle resolves");

        assert_eq!(config.user_did, "did:wba:miniapp-test.example");
        assert_eq!(config.credential.private_key_path, fixture.key_path);
        let summary = config.redacted_summary().to_string();
        assert!(!summary.contains("test-only-key"));
        assert!(!summary.contains("key-1-private.pem"));
        assert!(summary.contains("[REDACTED]"));
    }

    #[test]
    fn resolves_default_project_identity_from_examples_identity() {
        let fixture = ProjectIdentityFixture::new();
        let config = DemoAuthConfig::from_default_project_identity(&fixture.root, None)
            .expect("default project identity resolves");

        assert_eq!(config.user_did, "did:wba:default-user.example");
        assert_eq!(
            config.credential.did_document_path,
            fixture
                .root
                .join(DEFAULT_IDENTITY_DIR)
                .join(DEFAULT_DID_DOCUMENT_FILE)
        );
        assert_eq!(
            config.credential.private_key_path,
            fixture
                .root
                .join(DEFAULT_IDENTITY_DIR)
                .join(DEFAULT_PRIVATE_KEY_FILE)
        );
    }

    #[test]
    fn finds_project_root_from_child_directory() {
        let fixture = ProjectIdentityFixture::new();
        let child = fixture.root.join("crates/dock-cli/src");
        fs::create_dir_all(&child).expect("create child dir");

        assert_eq!(
            find_project_root_from(&child).as_deref(),
            Some(fixture.root.as_path())
        );
    }

    #[derive(Clone, Copy)]
    struct EmptyConfigSource;

    impl ConfigSource for EmptyConfigSource {
        fn string(self, _name: &str) -> Option<String> {
            None
        }
    }

    struct CredentialFixture {
        _dir: TempDir,
        did_path: PathBuf,
        key_path: PathBuf,
    }

    impl CredentialFixture {
        fn new() -> Self {
            let dir = TempDir::new("dock-cli-credential").expect("temp dir");
            let did_path = dir.path().join("did_document.json");
            let key_path = dir.path().join("key-1-private.pem");
            fs::write(&did_path, br#"{"id":"did:wba:user.example"}"#).expect("write DID");
            fs::write(&key_path, "test-only-key").expect("write key");
            set_private_key_permissions(&key_path);
            Self {
                _dir: dir,
                did_path,
                key_path,
            }
        }

        fn dir_path(&self) -> &Path {
            self._dir.path()
        }
    }

    struct SkillFixture {
        _dir: TempDir,
        root: PathBuf,
    }

    impl SkillFixture {
        fn new() -> Self {
            let dir = TempDir::new("dock-cli-skill-fixture").expect("temp dir");
            let root = dir.path().to_path_buf();
            fs::write(root.join("SKILL.md"), "# Test Skill").expect("write SKILL.md");
            fs::write(
                root.join("index.js"),
                "const skill = wx.modelContext.createSkill(__dirname)\n\
                 skill.registerAPI('declared', async () => ({ content: [{ type: 'text', text: 'ok' }] }))\n\
                 module.exports = skill\n",
            )
            .expect("write index.js");
            fs::write(
                root.join("mcp.json"),
                r#"{
                  "apis": [
                    {
                      "name": "declared",
                      "description": "registered API",
                      "inputSchema": {}
                    },
                    {
                      "name": "missing",
                      "description": "declared but not registered API",
                      "inputSchema": {}
                    }
                  ],
                  "components": []
                }"#,
            )
            .expect("write mcp.json");

            Self { _dir: dir, root }
        }
    }

    struct InspectSkillFixture {
        _dir: TempDir,
        root: PathBuf,
    }

    impl InspectSkillFixture {
        fn new() -> Self {
            let dir = TempDir::new("dock-cli-inspect-skill-fixture").expect("temp dir");
            let root = dir.path().to_path_buf();
            fs::create_dir_all(root.join("components/inspect-card")).expect("component dir");
            fs::write(root.join("SKILL.md"), "# Inspect Skill").expect("write SKILL.md");
            fs::write(
                root.join("index.js"),
                "const skill = wx.modelContext.createSkill(__dirname)\n\
                 async function login() { return wx.login() }\n\
                 skill.registerAPI('registered', async () => {\n\
                   await login()\n\
                   return { content: [{ type: 'text', text: 'ok' }] }\n\
                 })\n\
                 module.exports = skill\n",
            )
            .expect("write index.js");
            fs::write(
                root.join("components/inspect-card/index.js"),
                "Component({ methods: { async refresh() { return wx.request({ url: 'https://example.invalid/status' }) } } })\n\
                 const secret = 'super-secret-token'\n",
            )
            .expect("write component js");
            fs::write(
                root.join("components/inspect-card/index.wxml"),
                "<view><text>{{ apiName }}</text></view>",
            )
            .expect("write component wxml");
            fs::write(
                root.join("mcp.json"),
                r#"{
                  "id": "inspect-fixture",
                  "apis": [
                    {
                      "name": "registered",
                      "description": "registered API",
                      "inputSchema": {},
                      "_meta": {
                        "ui": { "componentPath": "components/inspect-card/index" },
                        "anp": { "risk": "payment" }
                      }
                    },
                    {
                      "name": "declaredOnly",
                      "description": "declared only API",
                      "inputSchema": {
                        "type": "object",
                        "properties": {
                          "receipt": {
                            "type": "string",
                            "format": "file"
                          }
                        }
                      }
                    }
                  ],
                  "components": [{
                    "path": "components/inspect-card/index",
                    "permissions": {
                      "scope.dynamic": {
                        "desc": "refresh via wx.request"
                      }
                    }
                  }]
                }"#,
            )
            .expect("write mcp.json");

            Self { _dir: dir, root }
        }
    }

    struct ImportSkillFixture {
        _dir: TempDir,
        root: PathBuf,
    }

    impl ImportSkillFixture {
        fn new() -> Self {
            let dir = TempDir::new("dock-cli-import-skill-fixture").expect("temp dir");
            let root = dir.path().to_path_buf();
            fs::create_dir_all(root.join("apis")).expect("api dir");
            fs::create_dir_all(root.join("components/import-card")).expect("component dir");
            fs::write(root.join("SKILL.md"), "# Import Skill").expect("write SKILL.md");
            fs::write(
                root.join("index.js"),
                "const skill = wx.modelContext.createSkill(__dirname)\n\
                 skill.registerAPI('registered', require('./apis/registered'))\n\
                 module.exports = skill\n",
            )
            .expect("write index.js");
            fs::write(
                root.join("apis/registered.js"),
                "module.exports = async function registered() {\n\
                   await wx.login()\n\
                   return { content: [{ type: 'text', text: 'ok' }] }\n\
                 }\n",
            )
            .expect("write api");
            fs::write(
                root.join("components/import-card/index.js"),
                "Component({ methods: { refresh() { return wx.request({ url: 'https://example.invalid/status' }) } } })\n",
            )
            .expect("write component js");
            fs::write(
                root.join("components/import-card/index.wxml"),
                "<view><text>{{ apiName }}</text></view>",
            )
            .expect("write component wxml");
            fs::write(
                root.join("mcp.json"),
                r#"{
                  "id": "import-fixture",
                  "wechatOriginalField": { "keep": true },
                  "apis": [{
                    "name": "registered",
                    "description": "registered API",
                    "inputSchema": {
                      "type": "object",
                      "properties": {
                        "receipt": {
                          "type": "string",
                          "format": "file"
                        }
                      }
                    },
                    "_meta": {
                      "ui": { "componentPath": "components/import-card/index" },
                      "anp": { "risk": "payment" }
                    }
                  }],
                  "components": [{
                    "path": "components/import-card/index",
                    "permissions": {
                      "scope.dynamic": {
                        "desc": "refresh status"
                      }
                    }
                  }]
                }"#,
            )
            .expect("write manifest");
            fs::write(
                root.join("app.json"),
                r#"{
                  "agent": {
                    "skills": [{
                      "path": "./",
                      "secretToken": "import-secret-token"
                    }]
                  }
                }"#,
            )
            .expect("write app.json");

            Self { _dir: dir, root }
        }
    }

    struct SignedSkillFixture {
        _dir: TempDir,
        root: PathBuf,
    }

    impl SignedSkillFixture {
        fn new() -> Self {
            let dir = TempDir::new("dock-cli-signed-skill-fixture").expect("temp dir");
            let root = dir.path().to_path_buf();
            fs::write(root.join("SKILL.md"), "# Signed Test Skill").expect("write SKILL.md");
            fs::write(
                root.join("index.js"),
                "const skill = wx.modelContext.createSkill(__dirname)\n\
                 skill.registerAPI('declared', async () => ({ content: [{ type: 'text', text: 'ok' }] }))\n\
                 module.exports = skill\n",
            )
            .expect("write index.js");
            fs::write(
                root.join("mcp.json"),
                r#"{
                  "_meta": {
                    "anp": {
                      "supplyChain": {
                        "publisherDid": "did:wba:publisher.example",
                        "digest": {
                          "algorithm": "sha256",
                          "value": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        },
                        "signature": {
                          "algorithm": "dock.package.dev-sha256.v1",
                          "keyId": "did:wba:publisher.example#package-key-1",
                          "value": "fixture-signature-secret"
                        }
                      }
                    }
                  },
                  "apis": [
                    {
                      "name": "declared",
                      "description": "registered API",
                      "inputSchema": {}
                    }
                  ],
                  "components": []
                }"#,
            )
            .expect("write mcp.json");

            Self { _dir: dir, root }
        }
    }

    struct IdentityStoreFixture {
        _dir: TempDir,
        root: PathBuf,
        key_path: PathBuf,
    }

    impl IdentityStoreFixture {
        fn new(handle: &str) -> Self {
            let dir = TempDir::new("dock-cli-identity-store").expect("temp dir");
            let identities = dir.path().join("identities");
            let identity_dir = identities.join("e1_test");
            fs::create_dir_all(&identity_dir).expect("create identity dir");
            fs::write(
                identity_dir.join("identity.json"),
                format!(r#"{{"handle":"{handle}","did":"did:wba:miniapp-test.example"}}"#),
            )
            .expect("write identity");
            fs::write(
                identity_dir.join("did_document.json"),
                br#"{"id":"did:wba:miniapp-test.example"}"#,
            )
            .expect("write DID document");
            let key_path = identity_dir.join("key-1-private.pem");
            fs::write(&key_path, "test-only-key").expect("write key");
            set_private_key_permissions(&key_path);
            Self {
                root: dir.path().to_path_buf(),
                key_path,
                _dir: dir,
            }
        }
    }

    struct ProjectIdentityFixture {
        _dir: TempDir,
        root: PathBuf,
    }

    impl ProjectIdentityFixture {
        fn new() -> Self {
            let dir = TempDir::new("dock-cli-project-identity").expect("temp dir");
            let root = dir.path().join("repo");
            let identity = root.join(DEFAULT_IDENTITY_DIR);
            fs::create_dir_all(&identity).expect("create default identity dir");
            fs::create_dir_all(root.join("examples/coffee-skill")).expect("create skill dir");
            fs::write(root.join("Cargo.toml"), b"[workspace]\n").expect("write manifest");
            fs::write(
                identity.join(DEFAULT_DID_DOCUMENT_FILE),
                br#"{"id":"did:wba:default-user.example"}"#,
            )
            .expect("write DID document");
            let key_path = identity.join(DEFAULT_PRIVATE_KEY_FILE);
            fs::write(&key_path, "test-only-key").expect("write key");
            set_private_key_permissions(&key_path);
            Self { _dir: dir, root }
        }
    }

    #[cfg(unix)]
    fn set_private_key_permissions(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("set key permissions");
    }

    #[cfg(not(unix))]
    fn set_private_key_permissions(_path: &Path) {}

    fn copy_dir_all(source: &Path, dest: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dest)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let path = entry.path();
            let target = dest.join(entry.file_name());
            if path.is_dir() {
                copy_dir_all(&path, &target)?;
            } else {
                fs::copy(&path, &target)?;
            }
        }
        Ok(())
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> std::io::Result<Self> {
            let path = std::env::temp_dir().join(format!(
                "{}-{}-{}",
                prefix,
                std::process::id(),
                unique_suffix()
            ));
            fs::create_dir(&path)?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn unique_suffix() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        format!("{nanos}-{counter}")
    }
}
