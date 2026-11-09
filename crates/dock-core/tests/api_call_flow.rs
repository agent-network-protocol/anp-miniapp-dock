use consent_audit::{
    ConsentRequest, DecisionConsentProvider, RiskLevel, UnavailableConsentProvider,
    DEV_HEADLESS_CONSENT_PROVIDER, DEV_HEADLESS_DECISION_ACTOR,
};
use dock_core::{
    ApiCallContext, ApiExecutor, AuditEvent, AuditSink, ComponentAction, ComponentRenderInput,
    ConsentDecision, ConsentGate, DockCoreError, ErrorCode, HostConsentGateAdapter, Orchestrator,
    PermissionDecision, RenderOutcome, RenderRouter, RuntimeHost,
};
use mcp_schema::{AtomicApiResult, TextContent};
use serde_json::{json, Map, Value};
use skill_loader::load_skill;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under crates/dock-core")
        .to_path_buf()
}

fn coffee_skill_root() -> PathBuf {
    repo_root().join("examples/coffee-skill")
}

fn context(api_name: &str, arguments: Value) -> ApiCallContext {
    ApiCallContext {
        user_did: Some("did:wba:user.example".to_owned()),
        agent_did: Some("did:wba:agent.example".to_owned()),
        merchant_did: Some("did:wba:merchant.example".to_owned()),
        skill_id: "coffee".to_owned(),
        session_id: "session-1".to_owned(),
        api_name: api_name.to_owned(),
        arguments,
        capability_token: None,
        trace: None,
    }
}

fn orchestrator(
    executor: MockExecutor,
    renderer: MockRenderer,
) -> Orchestrator<AllowHost, ApproveConsent, MockExecutor, MockRenderer, MockAudit> {
    orchestrator_with(executor, renderer, ApproveConsent, MockAudit::default())
}

fn orchestrator_with<C, A>(
    executor: MockExecutor,
    renderer: MockRenderer,
    consent: C,
    audit: A,
) -> Orchestrator<AllowHost, C, MockExecutor, MockRenderer, A>
where
    C: ConsentGate,
    A: AuditSink,
{
    let skill = load_skill(coffee_skill_root()).expect("coffee skill loads");
    Orchestrator::load_skill(skill, AllowHost, consent, executor, renderer, audit)
}

fn orchestrator_with_host<H, C, A>(
    host: H,
    executor: MockExecutor,
    renderer: MockRenderer,
    consent: C,
    audit: A,
) -> Orchestrator<H, C, MockExecutor, MockRenderer, A>
where
    H: RuntimeHost,
    C: ConsentGate,
    A: AuditSink,
{
    let skill = load_skill(coffee_skill_root()).expect("coffee skill loads");
    Orchestrator::load_skill(skill, host, consent, executor, renderer, audit)
}

#[test]
fn successful_api_call_routes_to_renderer() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("found drinks")],
        structured_content: Some(Map::from_iter([("drinks".to_owned(), json!([]))])),
        meta: Some(Map::from_iter([(
            "private".to_owned(),
            json!("for-component"),
        )])),
        extra: Default::default(),
    });
    let orchestrator = orchestrator(executor, MockRenderer::ok());

    let outcome = orchestrator
        .call_api(context("searchDrinks", json!({"query": "latte"})))
        .expect("call succeeds");

    assert_eq!(outcome.result.content[0].text, "found drinks");
    assert!(outcome.model_visible.get("_meta").is_none());
    assert_eq!(
        outcome
            .render
            .as_ref()
            .and_then(|render| render.component_path.as_deref()),
        Some("components/drink-list/index")
    );
}

#[test]
fn input_schema_failure_does_not_execute_vm() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("should not run")],
        structured_content: None,
        meta: None,
        extra: Default::default(),
    });
    let calls = executor.calls.clone();
    let audit = MockAudit::default();
    let events = audit.events.clone();
    let orchestrator = orchestrator_with(executor, MockRenderer::ok(), ApproveConsent, audit);

    let error = orchestrator
        .call_api(context("confirmOrder", json!({})))
        .expect_err("required drinkId should fail");

    assert_eq!(error.code(), ErrorCode::ValidationFailed);
    assert_eq!(*calls.borrow(), 0);
    let events = events.borrow();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].api_name, "confirmOrder");
    assert_eq!(events[0].risk_level, RiskLevel::L3);
    assert_eq!(events[0].outcome, "validation_failed");
    assert_eq!(events[0].permission_decision.decision, "not_evaluated");
    assert_eq!(
        events[0].permission_decision.reason_code,
        "input_validation_failed"
    );
}

#[test]
fn error_result_does_not_render_component() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: true,
        content: vec![TextContent::text("expired")],
        structured_content: None,
        meta: Some(Map::from_iter([("private".to_owned(), json!("hidden"))])),
        extra: Default::default(),
    });
    let renderer = MockRenderer::ok();
    let render_calls = renderer.render_calls.clone();
    let orchestrator = orchestrator(executor, renderer);

    let outcome = orchestrator
        .call_api(context("searchDrinks", json!({"query": "latte"})))
        .expect("call succeeds");

    assert!(outcome.render.is_none());
    assert_eq!(*render_calls.borrow(), 0);
    assert!(outcome.model_visible.get("_meta").is_none());
}

#[test]
fn component_api_call_returns_to_orchestrator() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("confirmed")],
        structured_content: Some(Map::from_iter([
            ("orderId".to_owned(), json!("order_demo_001")),
            ("payable".to_owned(), json!(18)),
        ])),
        meta: None,
        extra: Default::default(),
    });
    let calls = executor.calls.clone();
    let orchestrator = orchestrator(executor, MockRenderer::ok());
    let base = context("searchDrinks", json!({"query": "latte"}));

    let outcome = orchestrator
        .handle_component_action(
            &base,
            ComponentAction::ApiCall {
                name: "confirmOrder".to_owned(),
                arguments: json!({"drinkId": "latte"}),
            },
        )
        .expect("component action should route")
        .expect("api/call returns call outcome");

    assert_eq!(*calls.borrow(), 1);
    assert_eq!(outcome.result.content[0].text, "confirmed");
}

#[test]
fn render_failure_uses_fallback() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("found drinks")],
        structured_content: Some(Map::from_iter([("drinks".to_owned(), json!([]))])),
        meta: None,
        extra: Default::default(),
    });
    let orchestrator = orchestrator(executor, MockRenderer::fail());

    let outcome = orchestrator
        .call_api(context("searchDrinks", json!({"query": "latte"})))
        .expect("fallback should keep call successful");

    let render = outcome.render.expect("fallback render exists");
    assert_eq!(render.renderer, "fallback");
    assert_eq!(
        render.fallback_reason.as_deref(),
        Some("component_vm_failed")
    );
}

#[test]
fn low_risk_query_skips_consent_gate() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("found drinks")],
        structured_content: Some(Map::from_iter([("drinks".to_owned(), json!([]))])),
        meta: None,
        extra: Default::default(),
    });
    let executor_calls = executor.calls.clone();
    let consent = RequireConsent::default();
    let consent_calls = consent.calls.clone();
    let audit = MockAudit::default();
    let events = audit.events.clone();
    let orchestrator = orchestrator_with(executor, MockRenderer::ok(), consent, audit);

    orchestrator
        .call_api(context("searchDrinks", json!({"query": "latte"})))
        .expect("L0 query executes without human consent");

    assert_eq!(*executor_calls.borrow(), 1);
    assert_eq!(*consent_calls.borrow(), 0);
    let events = events.borrow();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].risk_level, RiskLevel::L0);
    assert_eq!(events[0].outcome, "ok");
    assert!(events[0].consent_proof.is_none());
}

#[test]
fn high_risk_payment_requires_consent_before_executor() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("paid")],
        structured_content: Some(Map::from_iter([
            ("orderId".to_owned(), json!("order-1")),
            ("status".to_owned(), json!("paid")),
        ])),
        meta: None,
        extra: Default::default(),
    });
    let executor_calls = executor.calls.clone();
    let consent = RequireConsent::default();
    let consent_calls = consent.calls.clone();
    let consent_requests = consent.requests.clone();
    let audit = MockAudit::default();
    let events = audit.events.clone();
    let orchestrator = orchestrator_with(executor, MockRenderer::ok(), consent, audit);

    let error = orchestrator
        .call_api(context(
            "payOrder",
            json!({"orderId": "order-1", "capabilityToken": "real-token"}),
        ))
        .expect_err("payment should fail closed without approval");

    assert_eq!(error.code(), ErrorCode::ConsentRequired);
    assert_eq!(*executor_calls.borrow(), 0);
    assert_eq!(*consent_calls.borrow(), 1);
    let requests = consent_requests.borrow();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].risk_level, RiskLevel::L3);
    assert_eq!(
        requests[0].parameter_summary["capabilityToken"],
        "[REDACTED]"
    );
    drop(requests);

    let events = events.borrow();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].api_name, "payOrder");
    assert_eq!(events[0].risk_level, RiskLevel::L3);
    assert_eq!(events[0].outcome, "blocked_consent_required");
    assert_eq!(events[0].permission_decision.decision, "allow");
    assert_eq!(events[0].parameter_summary["capabilityToken"], "[REDACTED]");
    assert!(events[0].consent_proof.is_none());
}

#[test]
fn host_permission_deny_blocks_before_consent_and_executor_with_audit_summary() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("should not execute")],
        structured_content: None,
        meta: None,
        extra: Default::default(),
    });
    let executor_calls = executor.calls.clone();
    let consent = RequireConsent::default();
    let consent_calls = consent.calls.clone();
    let audit = MockAudit::default();
    let events = audit.events.clone();
    let orchestrator = orchestrator_with_host(
        StaticHost(PermissionDecision::deny(
            "Host policy denied payOrder Authorization: Bearer secret-token",
            "host_override_deny",
        )),
        executor,
        MockRenderer::ok(),
        consent,
        audit,
    );

    let error = orchestrator
        .call_api(context(
            "payOrder",
            json!({"orderId": "order-1", "capabilityToken": "real-token"}),
        ))
        .expect_err("Host deny override must block");

    assert_eq!(error.code(), ErrorCode::PermissionDenied);
    assert_eq!(*executor_calls.borrow(), 0);
    assert_eq!(*consent_calls.borrow(), 0);

    let events = events.borrow();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].outcome, "blocked_permission_denied");
    assert_eq!(events[0].permission_decision.decision, "deny");
    assert_eq!(
        events[0].permission_decision.reason_code,
        "host_override_deny"
    );
    assert!(!events[0]
        .permission_decision
        .reason
        .contains("secret-token"));
    assert_eq!(events[0].parameter_summary["capabilityToken"], "[REDACTED]");
}

#[test]
fn host_permission_prompt_flows_into_consent_gate_and_audit_summary() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("paid")],
        structured_content: None,
        meta: None,
        extra: Default::default(),
    });
    let executor_calls = executor.calls.clone();
    let audit = MockAudit::default();
    let events = audit.events.clone();
    let orchestrator = orchestrator_with_host(
        StaticHost(PermissionDecision::prompt(
            "Host policy requires prompt",
            "host_override_prompt",
        )),
        executor,
        MockRenderer::ok(),
        ApproveConsent,
        audit,
    );

    orchestrator
        .call_api(context("payOrder", json!({"orderId": "order-1"})))
        .expect("approved prompt should continue through consent");

    assert_eq!(*executor_calls.borrow(), 1);
    let events = events.borrow();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].outcome, "ok");
    assert_eq!(events[0].permission_decision.decision, "prompt");
    assert_eq!(
        events[0].permission_decision.reason_code,
        "host_override_prompt"
    );
    assert!(events[0].consent_proof.is_some());
}

#[test]
fn high_risk_payment_with_consent_records_proof() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("paid")],
        structured_content: Some(Map::from_iter([
            ("orderId".to_owned(), json!("order-1")),
            ("status".to_owned(), json!("paid")),
        ])),
        meta: None,
        extra: Default::default(),
    });
    let executor_calls = executor.calls.clone();
    let audit = MockAudit::default();
    let events = audit.events.clone();
    let orchestrator = orchestrator_with(executor, MockRenderer::ok(), ApproveConsent, audit);

    orchestrator
        .call_api(context(
            "payOrder",
            json!({
                "orderId": "order-1",
                "capabilityToken": "real-token",
                "deliveryAddress": "1 Private Road"
            }),
        ))
        .expect("approved payment executes");

    assert_eq!(*executor_calls.borrow(), 1);

    let events = events.borrow();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].risk_level, RiskLevel::L3);
    assert_eq!(events[0].outcome, "ok");
    assert_eq!(events[0].parameter_summary["capabilityToken"], "[REDACTED]");
    assert_eq!(events[0].parameter_summary["deliveryAddress"], "[REDACTED]");

    let proof = events[0]
        .consent_proof
        .as_ref()
        .expect("high-risk approval records proof");
    assert_eq!(proof.policy_version, consent_audit::CONSENT_POLICY_VERSION);
    assert!(!proof.prompt_digest.is_empty());
    assert_eq!(proof.provider, DEV_HEADLESS_CONSENT_PROVIDER);
    assert_eq!(proof.decision_actor, DEV_HEADLESS_DECISION_ACTOR);
    assert!(proof.granted_at_ms > 0);
    assert_eq!(proof.user_did.as_deref(), Some("did:wba:user.example"));
    assert_eq!(
        proof.merchant_did.as_deref(),
        Some("did:wba:merchant.example")
    );
    assert_eq!(proof.skill_id, "coffee");
    assert_eq!(proof.api_name, "payOrder");
    assert_eq!(proof.parameter_summary["capabilityToken"], "[REDACTED]");
    assert_eq!(proof.parameter_summary["deliveryAddress"], "[REDACTED]");
}

#[test]
fn high_risk_payment_fails_closed_before_executor_when_audit_sink_unavailable() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("should not execute")],
        structured_content: None,
        meta: None,
        extra: Default::default(),
    });
    let executor_calls = executor.calls.clone();
    let orchestrator =
        orchestrator_with(executor, MockRenderer::ok(), ApproveConsent, FailingAudit);

    let error = orchestrator
        .call_api(context("payOrder", json!({"orderId": "order-1"})))
        .expect_err("audit sink unavailable must block high-risk actions");

    assert_eq!(error.code(), ErrorCode::AuditUnavailable);
    assert_eq!(*executor_calls.borrow(), 0);
}

#[test]
fn host_consent_adapter_can_drive_core_consent_gate() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("paid")],
        structured_content: None,
        meta: None,
        extra: Default::default(),
    });
    let executor_calls = executor.calls.clone();
    let audit = MockAudit::default();
    let events = audit.events.clone();
    let orchestrator = orchestrator_with(
        executor,
        MockRenderer::ok(),
        HostConsentGateAdapter::new(DecisionConsentProvider::approved()),
        audit,
    );

    orchestrator
        .call_api(context("payOrder", json!({"orderId": "order-1"})))
        .expect("approved Host adapter should execute");

    assert_eq!(*executor_calls.borrow(), 1);
    let proof = events.borrow()[0]
        .consent_proof
        .clone()
        .expect("proof recorded");
    assert_eq!(proof.provider, DEV_HEADLESS_CONSENT_PROVIDER);
    assert_eq!(proof.decision_actor, DEV_HEADLESS_DECISION_ACTOR);
}

#[test]
fn consent_provider_unavailable_fails_closed_and_records_audit() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("should not execute")],
        structured_content: None,
        meta: None,
        extra: Default::default(),
    });
    let executor_calls = executor.calls.clone();
    let audit = MockAudit::default();
    let events = audit.events.clone();
    let orchestrator = orchestrator_with(executor, MockRenderer::ok(), UnavailableConsent, audit);

    let error = orchestrator
        .call_api(context("payOrder", json!({"orderId": "order-1"})))
        .expect_err("provider unavailable should fail closed");

    assert_eq!(error.code(), ErrorCode::ConsentRequired);
    assert_eq!(*executor_calls.borrow(), 0);
    let events = events.borrow();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].api_name, "payOrder");
    assert_eq!(events[0].outcome, "blocked_consent_required");
    assert_eq!(events[0].permission_decision.decision, "allow");
    assert!(events[0].consent_proof.is_none());
}

#[test]
fn host_consent_adapter_unavailable_fails_closed_with_audit() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("should not execute")],
        structured_content: None,
        meta: None,
        extra: Default::default(),
    });
    let executor_calls = executor.calls.clone();
    let audit = MockAudit::default();
    let events = audit.events.clone();
    let orchestrator = orchestrator_with(
        executor,
        MockRenderer::ok(),
        HostConsentGateAdapter::new(UnavailableConsentProvider::new("host-ui")),
        audit,
    );

    let error = orchestrator
        .call_api(context("payOrder", json!({"orderId": "order-1"})))
        .expect_err("unavailable Host adapter should fail closed");

    assert_eq!(error.code(), ErrorCode::ConsentRequired);
    assert_eq!(*executor_calls.borrow(), 0);
    assert_eq!(events.borrow()[0].outcome, "blocked_consent_required");
}

#[test]
fn host_consent_adapter_denial_fails_closed_with_audit() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("should not execute")],
        structured_content: None,
        meta: None,
        extra: Default::default(),
    });
    let executor_calls = executor.calls.clone();
    let audit = MockAudit::default();
    let events = audit.events.clone();
    let orchestrator = orchestrator_with(
        executor,
        MockRenderer::ok(),
        HostConsentGateAdapter::new(DecisionConsentProvider::denied()),
        audit,
    );

    let error = orchestrator
        .call_api(context("payOrder", json!({"orderId": "order-1"})))
        .expect_err("denied Host adapter should fail closed");

    assert_eq!(error.code(), ErrorCode::ConsentRequired);
    assert_eq!(*executor_calls.borrow(), 0);
    let events = events.borrow();
    assert_eq!(events[0].outcome, "blocked_consent_required");
    assert!(events[0].consent_proof.is_none());
}

#[test]
fn executor_failure_records_error_audit() {
    let executor = MockExecutor::fail(ErrorCode::VmFailed, "vm failed");
    let executor_calls = executor.calls.clone();
    let audit = MockAudit::default();
    let events = audit.events.clone();
    let orchestrator = orchestrator_with(executor, MockRenderer::ok(), ApproveConsent, audit);

    let error = orchestrator
        .call_api(context("searchDrinks", json!({"query": "latte"})))
        .expect_err("executor failure propagates");

    assert_eq!(error.code(), ErrorCode::VmFailed);
    assert_eq!(*executor_calls.borrow(), 1);

    let events = events.borrow();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].api_name, "searchDrinks");
    assert_eq!(events[0].risk_level, RiskLevel::L0);
    assert_eq!(events[0].outcome, "error");
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
struct StaticHost(PermissionDecision);

impl RuntimeHost for StaticHost {
    fn check_permission(
        &self,
        _context: &ApiCallContext,
    ) -> Result<PermissionDecision, DockCoreError> {
        Ok(self.0.clone())
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
struct UnavailableConsent;

impl ConsentGate for UnavailableConsent {
    fn check_consent(
        &self,
        _context: &ApiCallContext,
        _request: &ConsentRequest,
    ) -> Result<ConsentDecision, DockCoreError> {
        Err(DockCoreError::core(
            ErrorCode::ConsentRequired,
            "host consent provider unavailable",
        ))
    }
}

#[derive(Clone, Default)]
struct RequireConsent {
    calls: std::rc::Rc<RefCell<usize>>,
    requests: std::rc::Rc<RefCell<Vec<ConsentRequest>>>,
}

impl ConsentGate for RequireConsent {
    fn check_consent(
        &self,
        _context: &ApiCallContext,
        request: &ConsentRequest,
    ) -> Result<ConsentDecision, DockCoreError> {
        *self.calls.borrow_mut() += 1;
        self.requests.borrow_mut().push(request.clone());
        Ok(ConsentDecision::Required(
            "human approval required".to_owned(),
        ))
    }
}

#[derive(Clone)]
struct MockExecutor {
    result: AtomicApiResult,
    error: Option<(ErrorCode, String)>,
    calls: std::rc::Rc<RefCell<usize>>,
}

impl MockExecutor {
    fn with_result(result: AtomicApiResult) -> Self {
        Self {
            result,
            error: None,
            calls: Default::default(),
        }
    }

    fn fail(code: ErrorCode, message: &str) -> Self {
        Self {
            result: AtomicApiResult {
                is_error: false,
                content: vec![TextContent::text("unused")],
                structured_content: None,
                meta: None,
                extra: Default::default(),
            },
            error: Some((code, message.to_owned())),
            calls: Default::default(),
        }
    }
}

impl ApiExecutor for MockExecutor {
    fn execute(
        &self,
        _context: &ApiCallContext,
        _component_path: Option<&str>,
    ) -> Result<AtomicApiResult, DockCoreError> {
        *self.calls.borrow_mut() += 1;
        if let Some((code, message)) = &self.error {
            return Err(DockCoreError::core(*code, message.clone()));
        }
        Ok(self.result.clone())
    }
}

#[derive(Clone)]
struct MockRenderer {
    fail: bool,
    render_calls: std::rc::Rc<RefCell<usize>>,
}

impl MockRenderer {
    fn ok() -> Self {
        Self {
            fail: false,
            render_calls: Default::default(),
        }
    }

    fn fail() -> Self {
        Self {
            fail: true,
            render_calls: Default::default(),
        }
    }
}

impl RenderRouter for MockRenderer {
    fn render(
        &self,
        _context: &ApiCallContext,
        input: &ComponentRenderInput,
    ) -> Result<RenderOutcome, DockCoreError> {
        *self.render_calls.borrow_mut() += 1;
        if self.fail {
            return Err(DockCoreError::core(
                ErrorCode::RenderFailed,
                "renderer unavailable",
            ));
        }

        Ok(RenderOutcome {
            renderer: "mock".to_owned(),
            component_path: Some(input.component_path.clone()),
            payload: json!({
                "apiName": input.api_name,
                "meta": input.meta,
                "structuredContent": input.structured_content
            }),
            fallback_reason: None,
        })
    }

    fn fallback(
        &self,
        _context: &ApiCallContext,
        _result: &AtomicApiResult,
        reason: &str,
    ) -> RenderOutcome {
        RenderOutcome {
            renderer: "fallback".to_owned(),
            component_path: None,
            payload: json!({}),
            fallback_reason: Some(reason.to_owned()),
        }
    }
}

#[derive(Clone, Default)]
struct MockAudit {
    events: std::rc::Rc<RefCell<Vec<AuditEvent>>>,
}

impl AuditSink for MockAudit {
    fn record(&self, event: AuditEvent) -> Result<(), DockCoreError> {
        self.events.borrow_mut().push(event);
        Ok(())
    }
}

#[derive(Clone)]
struct FailingAudit;

impl AuditSink for FailingAudit {
    fn ensure_available(&self) -> Result<(), DockCoreError> {
        Err(DockCoreError::core(
            ErrorCode::AuditUnavailable,
            "audit backend unavailable",
        ))
    }

    fn record(&self, _event: AuditEvent) -> Result<(), DockCoreError> {
        Err(DockCoreError::core(
            ErrorCode::AuditUnavailable,
            "audit backend unavailable",
        ))
    }
}
