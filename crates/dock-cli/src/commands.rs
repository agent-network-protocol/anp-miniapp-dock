use anp_adapter::{ChallengeLoginRequest, ChallengeLoginResponse, DidChallenge};
use card_spec::{fallback_from_result, FallbackReason};
use clap::{Parser, Subcommand};
use component_runtime::{
    ComponentEvent, ComponentInput, ComponentInstance, ComponentPackage, ComponentRenderOutput,
    ComponentVmAction, RenderEventKind, RenderNode,
};
use consent_audit::ConsentRequest;
use dock_core::{
    ApiCallContext, AuditEvent, AuditSink, ComponentRenderInput, ConsentDecision, ConsentGate,
    DockCoreError, ErrorCode, Orchestrator, PermissionDecision, RenderOutcome, RenderRouter,
    RuntimeHost,
};
use js_runtime_quickjs::ApiVm;
use mcp_schema::{AtomicApiResult, ValidationReport};
use serde::Serialize;
use serde_json::{json, Map, Value};
use skill_loader::{load_skill, resolve_component_path, LoadedSkill};
use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

const DEFAULT_SESSION_ID: &str = "session-cli";
const DEFAULT_SKILL_ID: &str = "coffee";
const DEFAULT_USER_DID: &str = "did:wba:user.example";
const DEFAULT_AGENT_DID: &str = "did:wba:agent.example";
const DEFAULT_MERCHANT_DID: &str = "did:wba:coffee-merchant.example";

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
    RunDemo {
        #[arg(long)]
        skill: PathBuf,
        #[arg(long)]
        server: String,
    },
}

pub fn run() -> Result<(), CliError> {
    let cli = Cli::parse();
    run_with_writer(cli, &mut std::io::stdout())
}

pub fn run_with_writer(mut cli: Cli, writer: &mut impl Write) -> Result<(), CliError> {
    let output = cli.execute()?;
    write_json(writer, &output)
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
            Command::RunDemo { skill, server } => run_demo(skill, server),
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
}

fn validate(skill_path: &Path) -> Result<Value, CliError> {
    let skill = load_skill(skill_path)?;
    Ok(json!({
        "status": "ok",
        "skillRoot": skill.root,
        "skillId": skill_id(&skill),
        "apis": skill.manifest.apis.iter().map(|api| api.name.as_str()).collect::<Vec<_>>(),
        "components": skill.components.keys().collect::<Vec<_>>(),
        "validation": validation_summary(&skill.validation)
    }))
}

fn call_api(skill_path: &Path, api_name: &str, json_args: &str) -> Result<Value, CliError> {
    let args = parse_json(json_args, "jsonArgs")?;
    let runtime = RuntimeHarness::load(skill_path)?;
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

fn preview_component(
    skill_path: &Path,
    component_path: &str,
    json_input: &str,
) -> Result<Value, CliError> {
    let input = parse_component_input(json_input)?;
    let package = load_component_package(skill_path, component_path)?;
    let mut instance = ComponentInstance::new(package)?;
    let outcome = instance.mount(input)?;
    Ok(json!({
        "status": "ok",
        "componentPath": component_path,
        "render": component_render_json(&outcome.render),
        "actions": outcome.actions,
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
        FallbackReason::RendererUnavailable
    };
    let card = fallback_from_result(&result, reason);
    Ok(json!({
        "status": "ok",
        "card": card
    }))
}

fn run_demo(skill_path: &Path, server: &str) -> Result<Value, CliError> {
    let auth = DemoHttpClient::new(server).login()?;
    let server_business =
        DemoHttpClient::new(server).coffee_business_check(&auth.capability_token)?;
    let runtime = RuntimeHarness::load(skill_path)?;

    let search = runtime.call("searchDrinks", json!({"query": "latte"}))?;
    let mut drink_component = mount_for_outcome(
        skill_path,
        "searchDrinks",
        json!({"query": "latte"}),
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
                "capabilityToken": "[REDACTED]",
                "tokenReceived": auth.token_received
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
    orchestrator: Orchestrator<
        AllowHost,
        ApproveConsent,
        js_runtime_quickjs::QuickJsApiExecutor,
        ComponentRenderRouter,
        CollectAudit,
    >,
    audit: CollectAudit,
}

impl RuntimeHarness {
    fn load(skill_path: &Path) -> Result<Self, CliError> {
        let skill = load_skill(skill_path)?;
        let api_vm = ApiVm::load_skill(skill.clone())?;
        let audit = CollectAudit::default();
        let orchestrator = Orchestrator::load_skill(
            skill.clone(),
            AllowHost,
            ApproveConsent,
            api_vm.executor(),
            ComponentRenderRouter {
                skill_root: skill.root,
            },
            audit.clone(),
        );
        Ok(Self {
            orchestrator,
            audit,
        })
    }

    fn call(
        &self,
        api_name: impl Into<String>,
        arguments: Value,
    ) -> Result<dock_core::CallOutcome, CliError> {
        self.orchestrator
            .call_api(ApiCallContext {
                user_did: Some(DEFAULT_USER_DID.to_owned()),
                agent_did: Some(DEFAULT_AGENT_DID.to_owned()),
                merchant_did: Some(DEFAULT_MERCHANT_DID.to_owned()),
                skill_id: DEFAULT_SKILL_ID.to_owned(),
                session_id: DEFAULT_SESSION_ID.to_owned(),
                api_name: api_name.into(),
                arguments,
                capability_token: None,
            })
            .map_err(Into::into)
    }

    fn audit_events(&self) -> Vec<AuditEvent> {
        self.audit.events.borrow().clone()
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
        Ok(ConsentDecision::Approved)
    }
}

#[derive(Clone)]
struct ComponentRenderRouter {
    skill_root: PathBuf,
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
        };
        let package = load_component_package(&self.skill_root, &input.component_path)
            .map_err(|error| DockCoreError::core(ErrorCode::RenderFailed, error.to_string()))?;
        let mut instance = ComponentInstance::new(package)
            .map_err(|error| DockCoreError::core(ErrorCode::RenderFailed, error.to_string()))?;
        let outcome = instance
            .mount(component_input)
            .map_err(|error| DockCoreError::core(ErrorCode::RenderFailed, error.to_string()))?;

        Ok(RenderOutcome {
            renderer: "component-runtime".to_owned(),
            component_path: Some(input.component_path.clone()),
            payload: json!({
                "render": component_render_json(&outcome.render),
                "actions": outcome.actions,
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
        RenderOutcome {
            renderer: "card-spec".to_owned(),
            component_path: None,
            payload: json!(fallback_from_result(result, fallback_reason)),
            fallback_reason: Some(reason.to_owned()),
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

struct MountedComponent {
    instance: ComponentInstance,
    mount: component_runtime::ComponentOperationOutcome,
}

fn mount_for_outcome(
    skill_path: &Path,
    api_name: &str,
    arguments: Value,
    result: AtomicApiResult,
    component_path: &str,
) -> Result<MountedComponent, CliError> {
    let package = load_component_package(skill_path, component_path)?;
    let mut instance = ComponentInstance::new(package)?;
    let mount = instance.mount(component_input(api_name, arguments, &result))?;
    Ok(MountedComponent { instance, mount })
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
    }
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

struct DemoHttpClient {
    base_url: String,
}

impl DemoHttpClient {
    fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }

    fn login(&self) -> Result<DemoAuth, CliError> {
        let challenge: DidChallenge = serde_json::from_value(self.post_json(
            "/agents/coffee/auth/challenge",
            None,
            json!({
                "sessionId": DEFAULT_SESSION_ID,
                "skillId": DEFAULT_SKILL_ID,
                "userDid": DEFAULT_USER_DID,
                "agentDid": DEFAULT_AGENT_DID
            }),
        )?)
        .map_err(|source| CliError::Json {
            label: "auth challenge response".to_owned(),
            source,
        })?;
        let login_request = ChallengeLoginRequest {
            session_id: DEFAULT_SESSION_ID.to_owned(),
            skill_id: DEFAULT_SKILL_ID.to_owned(),
            user_did: DEFAULT_USER_DID.to_owned(),
            agent_did: Some(DEFAULT_AGENT_DID.to_owned()),
            merchant_did: challenge.merchant_did.clone(),
            challenge_id: challenge.challenge_id,
            signed_challenge: json!({
                "proof": "demo-signature",
                "nonce": challenge.nonce
            }),
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
        "errors": report.errors,
        "warnings": report.warnings
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

fn fallback_reason_from_str(reason: &str) -> FallbackReason {
    if reason.contains("component_load") {
        FallbackReason::ComponentLoadFailed
    } else if reason.contains("render_failed") {
        FallbackReason::ComponentRenderFailed
    } else if reason.contains("no_component_path") {
        FallbackReason::NoComponentPath
    } else {
        FallbackReason::RendererUnavailable
    }
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
                    "consentProof": event.consent_proof,
                    "outcome": event.outcome
                })
            })
            .collect(),
    )
}

fn component_render_json(render: &ComponentRenderOutput) -> Value {
    json!({
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
            return format!("{marker}=[REDACTED]");
        }
    }
    value.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_validate_args() {
        let cli = Cli::try_parse_from_args(["dock-cli", "validate", "examples/coffee-skill"])
            .expect("args parse");
        assert!(matches!(cli.command, Command::Validate { .. }));
    }

    #[test]
    fn preview_card_renders_fallback_card() {
        let output = preview_card(
            r#"{"content":[{"type":"text","text":"hello"}],"structuredContent":{"orderId":"1"}}"#,
        )
        .expect("preview card");

        assert_eq!(output["status"], "ok");
        assert_eq!(output["card"]["version"], "card-spec/v0");
    }

    #[test]
    fn redacts_http_errors() {
        let redacted = redact_text(r#"{"capabilityToken":"demo-token"}"#);
        assert_eq!(redacted, "capabilityToken=[REDACTED]");
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
}
