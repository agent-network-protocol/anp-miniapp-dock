use consent_audit::{ConsentRequest, DEV_HEADLESS_CONSENT_PROVIDER, DEV_HEADLESS_DECISION_ACTOR};
use dock_core::{
    canonicalize_open_detail_page_target, ApiCallContext, ApiExecutor, AuditEvent, AuditSink,
    ConsentDecision, ConsentGate, DockCoreError, ErrorCode, HeadlessHostAdapter, HostActionOutcome,
    HostActionRedaction, HostActionRequest, HostActionStatus, HostAdapterContract,
    HostCapabilityRequirement, HostCapabilityStatus, PermissionDecision, RenderOutcome,
    RenderRouter, RuntimeComponentAction, RuntimeDispatchComponentActionRequest, RuntimeHost,
    RuntimeService, RuntimeSessionContext, HOST_ADAPTER_CONTRACT_VERSION,
};
use mcp_schema::{AtomicApiResult, TextContent};
use serde_json::{json, Map};
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

fn session() -> RuntimeSessionContext {
    RuntimeSessionContext {
        user_did: Some("did:wba:user.example".to_owned()),
        agent_did: Some("did:wba:agent.example".to_owned()),
        merchant_did: Some("did:wba:merchant.example".to_owned()),
        skill_id: "coffee".to_owned(),
        session_id: "session-host".to_owned(),
    }
}

fn service<H>(
    host: H,
    executor: MockExecutor,
) -> RuntimeService<H, ApproveConsent, MockExecutor, MockRenderer, MockAudit, MockAudit>
where
    H: RuntimeHost,
{
    let skill = load_skill(coffee_skill_root()).expect("coffee skill loads");
    let audit = MockAudit::default();
    RuntimeService::load_skill(
        skill,
        host,
        ApproveConsent,
        executor,
        MockRenderer,
        audit.clone(),
        audit,
    )
}

#[test]
fn host_adapter_contract_declares_required_optional_and_unsupported_capabilities() {
    let service = service(
        HeadlessHostAdapter,
        MockExecutor::with_result(AtomicApiResult {
            is_error: false,
            content: vec![TextContent::text("ok")],
            structured_content: None,
            meta: None,
            extra: Default::default(),
        }),
    );

    let response = service.host_contract();
    let contract = response.data.contract;

    assert_eq!(contract.version, HOST_ADAPTER_CONTRACT_VERSION);
    assert!(!contract.production_ready);
    assert_eq!(contract.adapter_name, "headless-mock-host");
    assert_capability(
        &contract,
        "renderIrRenderer",
        HostCapabilityRequirement::Required,
        HostCapabilityStatus::DevOnly,
    );
    assert_capability(
        &contract,
        "consentPrompt",
        HostCapabilityRequirement::Required,
        HostCapabilityStatus::DevOnly,
    );
    assert_capability(
        &contract,
        "providerPayment",
        HostCapabilityRequirement::Optional,
        HostCapabilityStatus::Unsupported,
    );
    assert_capability(
        &contract,
        "fullMiniappPageRouting",
        HostCapabilityRequirement::UnsupportedByDesign,
        HostCapabilityStatus::UnsupportedByDesign,
    );

    let custom = RecordingHost::default().adapter_contract();
    assert_eq!(custom.adapter_name, "custom-runtime-host");
    assert_capability(
        &custom,
        "fullMiniappPageRouting",
        HostCapabilityRequirement::UnsupportedByDesign,
        HostCapabilityStatus::UnsupportedByDesign,
    );
}

#[test]
fn api_call_action_routes_to_orchestrator_not_host_adapter() {
    let host = RecordingHost::default();
    let host_actions = host.actions.clone();
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("confirmed")],
        structured_content: Some(Map::from_iter([(
            "orderId".to_owned(),
            json!("order_demo_001"),
        )])),
        meta: None,
        extra: Default::default(),
    });
    let executor_calls = executor.calls.clone();
    let service = service(host, executor);

    let response = service
        .dispatch_component_action(RuntimeDispatchComponentActionRequest {
            session: session(),
            source_api_name: "searchDrinks".to_owned(),
            source_arguments: json!({"query": "latte"}),
            action: RuntimeComponentAction::ApiCall {
                name: "confirmOrder".to_owned(),
                arguments: json!({"drinkId": "latte"}),
            },
            capability_token: Some("capability-secret-token".to_owned()),
            operation: None,
        })
        .expect("api/call routes through runtime");

    assert!(response.data.handled);
    assert_eq!(response.data.boundary, "runtime-orchestrator");
    assert!(response.data.host_action.is_none());
    assert_eq!(
        response
            .data
            .call
            .as_ref()
            .map(|call| call.api_name.as_str()),
        Some("confirmOrder")
    );
    assert_eq!(*executor_calls.borrow(), 1);
    assert!(host_actions.borrow().is_empty());
}

#[test]
fn high_risk_component_api_call_still_requires_consent_and_audit_before_executor() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("paid")],
        structured_content: None,
        meta: None,
        extra: Default::default(),
    });
    let executor_calls = executor.calls.clone();
    let service = service(HeadlessHostAdapter, executor);

    let response = service
        .dispatch_component_action(RuntimeDispatchComponentActionRequest {
            session: session(),
            source_api_name: "confirmOrder".to_owned(),
            source_arguments: json!({"drinkId": "latte"}),
            action: RuntimeComponentAction::ApiCall {
                name: "payOrder".to_owned(),
                arguments: json!({
                    "orderId": "order_demo_001",
                    "capabilityToken": "capability-secret-token"
                }),
            },
            capability_token: Some("capability-secret-token".to_owned()),
            operation: None,
        })
        .expect("approved high-risk action routes through runtime");

    assert!(response.data.handled);
    assert_eq!(response.data.boundary, "runtime-orchestrator");
    assert_eq!(response.data.call.as_ref().unwrap().api_name, "payOrder");
    assert_eq!(*executor_calls.borrow(), 1);

    let audit = service
        .get_audit_records(Default::default())
        .expect("audit records available");
    assert_eq!(audit.data.records.len(), 1);
    let record = &audit.data.records[0];
    assert_eq!(record.api_name, "payOrder");
    assert_eq!(record.outcome, "ok");
    assert!(record.consent_proof.is_some());
    assert_eq!(record.parameter_summary["capabilityToken"], "[REDACTED]");
}

#[test]
fn host_action_accepts_safe_detail_page_and_redacts_follow_up_payload() {
    let service = service(
        HeadlessHostAdapter,
        MockExecutor::with_result(AtomicApiResult {
            is_error: false,
            content: vec![TextContent::text("unused")],
            structured_content: None,
            meta: None,
            extra: Default::default(),
        }),
    );

    let detail = service
        .dispatch_component_action(RuntimeDispatchComponentActionRequest {
            session: session(),
            source_api_name: "searchDrinks".to_owned(),
            source_arguments: json!({}),
            action: RuntimeComponentAction::OpenDetailPage {
                url: "pages/order/detail?orderId=order_demo_001".to_owned(),
            },
            capability_token: None,
            operation: None,
        })
        .expect("safe detail page action is accepted");
    assert!(detail.data.handled);
    assert_eq!(detail.data.boundary, "host-adapter");
    let host_action = detail.data.host_action.expect("host action outcome");
    assert_eq!(host_action.status, HostActionStatus::Accepted);
    assert_eq!(
        host_action.payload["canonicalUrl"],
        "/pages/order/detail?orderId=order_demo_001"
    );

    let follow_up = service
        .dispatch_component_action(RuntimeDispatchComponentActionRequest {
            session: session(),
            source_api_name: "searchDrinks".to_owned(),
            source_arguments: json!({}),
            action: RuntimeComponentAction::SendFollowUpMessage {
                content: vec![TextContent::text(
                    "Authorization Bearer capability-secret-token",
                )],
            },
            capability_token: None,
            operation: None,
        })
        .expect("follow-up action is accepted by headless boundary");
    assert!(follow_up.data.handled);
    let rendered = serde_json::to_string(&follow_up).expect("host action serializes");
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("Authorization Bearer"));
}

#[test]
fn open_detail_page_rejects_external_paths_and_sensitive_query() {
    for target in [
        "https://evil.example/path",
        "javascript:alert(1)",
        "file:///private/key.pem",
        "//evil.example/path",
        "../admin",
        "pages/detail?token=capability-secret-token",
        "pages/%2e%2e/private",
        "pages%2f..%2fprivate",
    ] {
        let error = canonicalize_open_detail_page_target(target)
            .expect_err("unsafe detail target should fail closed");
        assert_eq!(error.code(), ErrorCode::PermissionDenied);
    }

    assert_eq!(
        canonicalize_open_detail_page_target("pages/detail?id=order_demo_001")
            .expect("safe relative target"),
        "/pages/detail?id=order_demo_001"
    );
}

#[test]
fn unsupported_custom_host_action_fails_closed_without_executor_or_host_side_effect() {
    let host = RecordingHost::default();
    let actions = host.actions.clone();
    let service = service(
        host,
        MockExecutor::with_result(AtomicApiResult {
            is_error: false,
            content: vec![TextContent::text("unused")],
            structured_content: None,
            meta: None,
            extra: Default::default(),
        }),
    );

    let response = service
        .dispatch_component_action(RuntimeDispatchComponentActionRequest {
            session: session(),
            source_api_name: "searchDrinks".to_owned(),
            source_arguments: json!({}),
            action: RuntimeComponentAction::ExpirePreviousCards {
                component_paths: vec!["components/order-confirm/index".to_owned()],
                match_policy: Some("session".to_owned()),
            },
            capability_token: None,
            operation: None,
        })
        .expect("unsupported action returns stable outcome");

    assert!(!response.data.handled);
    assert_eq!(response.data.boundary, "host-adapter");
    let host_action = response.data.host_action.expect("host action outcome");
    assert_eq!(host_action.status, HostActionStatus::Unsupported);
    assert_eq!(host_action.reason_code, "host_action_not_supported");
    assert_eq!(actions.borrow().len(), 1);
}

#[test]
fn runtime_canonicalizes_detail_target_and_redacts_custom_host_outcome() {
    let host = LeakyHost::default();
    let actions = host.actions.clone();
    let service = service(
        host,
        MockExecutor::with_result(AtomicApiResult {
            is_error: false,
            content: vec![TextContent::text("unused")],
            structured_content: None,
            meta: None,
            extra: Default::default(),
        }),
    );

    let response = service
        .dispatch_component_action(RuntimeDispatchComponentActionRequest {
            session: session(),
            source_api_name: "searchDrinks".to_owned(),
            source_arguments: json!({}),
            action: RuntimeComponentAction::OpenDetailPage {
                url: "pages/order/detail?orderId=order_demo_001#ignored".to_owned(),
            },
            capability_token: None,
            operation: None,
        })
        .expect("safe detail page routes through runtime");

    assert!(response.data.handled);
    assert_eq!(actions.borrow().len(), 1);
    assert_eq!(
        actions.borrow()[0].payload["url"],
        "/pages/order/detail?orderId=order_demo_001"
    );
    let host_action = response.data.host_action.expect("host action outcome");
    assert_eq!(host_action.payload["token"], "[REDACTED]");
    assert_eq!(host_action.payload["Authorization"], "[REDACTED]");
    let rendered = serde_json::to_string(&host_action).expect("host outcome serializes");
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("Bearer"));
}

fn assert_capability(
    contract: &HostAdapterContract,
    name: &str,
    requirement: HostCapabilityRequirement,
    status: HostCapabilityStatus,
) {
    let capability = contract
        .capabilities
        .iter()
        .find(|capability| capability.name == name)
        .unwrap_or_else(|| panic!("missing capability {name}"));
    assert_eq!(capability.requirement, requirement);
    assert_eq!(capability.status, status);
}

#[derive(Clone, Default)]
struct RecordingHost {
    actions: std::rc::Rc<RefCell<Vec<HostActionRequest>>>,
}

impl RuntimeHost for RecordingHost {
    fn check_permission(
        &self,
        _context: &ApiCallContext,
    ) -> Result<PermissionDecision, DockCoreError> {
        Ok(PermissionDecision::Allow)
    }

    fn handle_host_action(
        &self,
        _context: &ApiCallContext,
        request: HostActionRequest,
    ) -> Result<HostActionOutcome, DockCoreError> {
        self.actions.borrow_mut().push(request.clone());
        Ok(HostActionOutcome::unsupported(
            request.action_type,
            "host_action_not_supported",
        ))
    }
}

#[derive(Clone, Default)]
struct LeakyHost {
    actions: std::rc::Rc<RefCell<Vec<HostActionRequest>>>,
}

impl RuntimeHost for LeakyHost {
    fn check_permission(
        &self,
        _context: &ApiCallContext,
    ) -> Result<PermissionDecision, DockCoreError> {
        Ok(PermissionDecision::Allow)
    }

    fn handle_host_action(
        &self,
        _context: &ApiCallContext,
        request: HostActionRequest,
    ) -> Result<HostActionOutcome, DockCoreError> {
        self.actions.borrow_mut().push(request.clone());
        Ok(HostActionOutcome {
            action_type: request.action_type,
            status: HostActionStatus::Accepted,
            boundary: "host-adapter".to_owned(),
            reason_code: "accepted".to_owned(),
            payload: json!({
                "Authorization": "Bearer capability-secret-token",
                "token": "capability-secret-token",
                "echoUrl": request.payload.get("url").cloned().unwrap_or_default(),
            }),
            redaction: HostActionRedaction {
                marker: "custom-host-forgot-redaction".to_owned(),
                policy: "custom-host".to_owned(),
                applied_by_default: false,
            },
        })
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
struct MockRenderer;

impl RenderRouter for MockRenderer {
    fn render(
        &self,
        _context: &ApiCallContext,
        input: &dock_core::ComponentRenderInput,
    ) -> Result<RenderOutcome, DockCoreError> {
        Ok(RenderOutcome {
            renderer: "mock-renderer".to_owned(),
            component_path: Some(input.component_path.clone()),
            payload: json!({ "apiName": input.api_name }),
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
            renderer: "mock-fallback".to_owned(),
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

impl dock_core::RuntimeAuditReader for MockAudit {
    fn runtime_audit_records(&self) -> Result<Vec<AuditEvent>, DockCoreError> {
        Ok(self.events.borrow().clone())
    }
}
