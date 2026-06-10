use dock_core::{
    ApiCallContext, ApiExecutor, AuditEvent, AuditSink, ComponentAction, ComponentRenderInput,
    ConsentDecision, ConsentGate, DockCoreError, ErrorCode, Orchestrator, PermissionDecision,
    RenderOutcome, RenderRouter, RuntimeHost,
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
    }
}

fn orchestrator(
    executor: MockExecutor,
    renderer: MockRenderer,
) -> Orchestrator<AllowHost, ApproveConsent, MockExecutor, MockRenderer, MockAudit> {
    let skill = load_skill(coffee_skill_root()).expect("coffee skill loads");
    Orchestrator::load_skill(
        skill,
        AllowHost,
        ApproveConsent,
        executor,
        renderer,
        MockAudit::default(),
    )
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
    let orchestrator = orchestrator(executor, MockRenderer::ok());

    let error = orchestrator
        .call_api(context("confirmOrder", json!({})))
        .expect_err("required drinkId should fail");

    assert_eq!(error.code(), ErrorCode::ValidationFailed);
    assert_eq!(*calls.borrow(), 0);
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
    assert!(render
        .fallback_reason
        .as_deref()
        .unwrap_or_default()
        .contains("render_failed"));
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
    fn check_consent(&self, _context: &ApiCallContext) -> Result<ConsentDecision, DockCoreError> {
        Ok(ConsentDecision::Approved)
    }
}

#[derive(Clone)]
struct MockExecutor {
    result: AtomicApiResult,
    calls: std::rc::Rc<RefCell<usize>>,
}

impl MockExecutor {
    fn with_result(result: AtomicApiResult) -> Self {
        Self {
            result,
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
