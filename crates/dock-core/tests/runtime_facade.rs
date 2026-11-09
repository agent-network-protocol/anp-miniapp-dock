use consent_audit::{ConsentRequest, DEV_HEADLESS_CONSENT_PROVIDER, DEV_HEADLESS_DECISION_ACTOR};
use dock_core::{
    negotiate_runtime_version, ApiCallContext, ApiExecutor, AuditEvent, AuditSink,
    ComponentRenderInput, ConsentDecision, ConsentGate, DockCoreError, ErrorCode,
    InMemoryMetricsSink, InMemoryObservabilitySink, ObservabilityEventKind, PermissionDecision,
    RenderOutcome, RenderRouter, RuntimeAuditReader, RuntimeAuditRecordsRequest,
    RuntimeCallRequest, RuntimeCancelOperationRequest, RuntimeCloseSessionRequest,
    RuntimeComponentAction, RuntimeDispatchComponentActionRequest, RuntimeExpireCardsRequest,
    RuntimeHost, RuntimeOperationOptions, RuntimePersistentAuditSink,
    RuntimeRenderComponentRequest, RuntimeService, RuntimeServiceObservabilityParts,
    RuntimeServiceParts, RuntimeSessionContext, RuntimeTraceContext, TraceContext, TraceSpanKind,
    RUNTIME_API_VERSION,
};
use mcp_schema::{AtomicApiResult, TextContent, ValidationReport};
use serde_json::{json, Map};
use skill_loader::load_skill;
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};

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
        session_id: "session-runtime".to_owned(),
    }
}

fn runtime_service(
    executor: MockExecutor,
) -> RuntimeService<AllowHost, ApproveConsent, MockExecutor, MockRenderer, MockAudit, MockAudit> {
    let skill = load_skill(coffee_skill_root()).expect("coffee skill loads");
    let audit = MockAudit::default();
    RuntimeService::load_skill(
        skill,
        AllowHost,
        ApproveConsent,
        executor,
        MockRenderer,
        audit.clone(),
        audit,
    )
}

fn threaded_runtime_service(
    executor: BlockingExecutor,
) -> Arc<
    RuntimeService<
        ThreadSafeAllowHost,
        ThreadSafeApproveConsent,
        BlockingExecutor,
        ThreadSafeRenderer,
        ThreadSafeAudit,
        ThreadSafeAudit,
    >,
> {
    let skill = load_skill(coffee_skill_root()).expect("coffee skill loads");
    let audit = ThreadSafeAudit::default();
    Arc::new(RuntimeService::load_skill(
        skill,
        ThreadSafeAllowHost,
        ThreadSafeApproveConsent,
        executor,
        ThreadSafeRenderer,
        audit.clone(),
        audit,
    ))
}

fn first_call_blocking_runtime_service(
    executor: FirstCallBlockingExecutor,
) -> Arc<
    RuntimeService<
        ThreadSafeAllowHost,
        ThreadSafeApproveConsent,
        FirstCallBlockingExecutor,
        ThreadSafeRenderer,
        ThreadSafeAudit,
        ThreadSafeAudit,
    >,
> {
    let skill = load_skill(coffee_skill_root()).expect("coffee skill loads");
    let audit = ThreadSafeAudit::default();
    Arc::new(RuntimeService::load_skill(
        skill,
        ThreadSafeAllowHost,
        ThreadSafeApproveConsent,
        executor,
        ThreadSafeRenderer,
        audit.clone(),
        audit,
    ))
}

fn observability_runtime_service(
    executor: MockExecutor,
    observability: InMemoryObservabilitySink,
) -> RuntimeService<
    AllowHost,
    ApproveConsent,
    MockExecutor,
    MockRenderer,
    MockAudit,
    MockAudit,
    InMemoryObservabilitySink,
> {
    let skill = load_skill(coffee_skill_root()).expect("coffee skill loads");
    let audit = MockAudit::default();
    RuntimeService::load_skill_with_observability(RuntimeServiceParts {
        skill,
        host: AllowHost,
        consent: ApproveConsent,
        executor,
        renderer: MockRenderer,
        audit: audit.clone(),
        audit_reader: audit,
        observability,
    })
}

fn metrics_runtime_service(
    executor: MockExecutor,
    observability: InMemoryObservabilitySink,
    metrics: InMemoryMetricsSink,
) -> RuntimeService<
    AllowHost,
    ApproveConsent,
    MockExecutor,
    MockRenderer,
    MockAudit,
    MockAudit,
    InMemoryObservabilitySink,
    InMemoryMetricsSink,
> {
    let skill = load_skill(coffee_skill_root()).expect("coffee skill loads");
    let audit = MockAudit::default();
    RuntimeService::load_skill_with_observability_and_metrics(RuntimeServiceObservabilityParts {
        skill,
        host: AllowHost,
        consent: ApproveConsent,
        executor,
        renderer: MockRenderer,
        audit: audit.clone(),
        audit_reader: audit,
        observability,
        metrics,
    })
}

#[test]
fn runtime_version_negotiation_reports_stable_version() {
    let version = negotiate_runtime_version(Some(RUNTIME_API_VERSION))
        .expect("current version is accepted")
        .data;

    assert_eq!(version.current, RUNTIME_API_VERSION);
    assert_eq!(version.supported, vec![RUNTIME_API_VERSION]);

    let error =
        negotiate_runtime_version(Some("dock.runtime.v0")).expect_err("unsupported version fails");
    assert_eq!(error.error.code, "unsupported_version");
    assert_eq!(error.version, RUNTIME_API_VERSION);
}

#[test]
fn runtime_facade_call_render_action_expire_audit_and_close_are_versioned() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("confirmed")],
        structured_content: Some(Map::from_iter([
            ("orderId".to_owned(), json!("order_demo_001")),
            ("payable".to_owned(), json!(18)),
        ])),
        meta: Some(Map::from_iter([(
            "private".to_owned(),
            json!("component-only"),
        )])),
        extra: Default::default(),
    });
    let calls = executor.calls.clone();
    let service = runtime_service(executor);

    let validate = service.validate_skill();
    assert_eq!(validate.version, RUNTIME_API_VERSION);
    assert_eq!(validate.data.skill.skill_id, "unknown");
    assert!(validate
        .data
        .skill
        .api_names
        .iter()
        .any(|api| api == "confirmOrder"));

    let loaded = service.load_skill_response();
    assert_eq!(loaded.version, RUNTIME_API_VERSION);
    assert!(loaded.data.skill.component_paths.iter().any(|path| {
        path == "components/order-confirm/index" || path == "components/order-confirm"
    }));

    let call = service
        .call_api(RuntimeCallRequest {
            session: session(),
            api_name: "confirmOrder".to_owned(),
            arguments: json!({"drinkId": "latte"}),
            capability_token: Some("capability-secret-token".to_owned()),
            operation: None,
        })
        .expect("facade call succeeds");
    assert_eq!(call.version, RUNTIME_API_VERSION);
    assert_eq!(call.data.api_name, "confirmOrder");
    assert_eq!(call.data.result.content[0].text, "confirmed");
    let model_visible_json =
        serde_json::to_value(&call.data.model_visible).expect("model-visible serializes");
    assert!(model_visible_json.get("_meta").is_none());
    assert_eq!(
        call.data
            .render
            .as_ref()
            .and_then(|render| render.component_path.as_deref()),
        Some("components/order-confirm/index")
    );

    let render = service
        .render_component(RuntimeRenderComponentRequest {
            session: session(),
            api_name: "confirmOrder".to_owned(),
            arguments: json!({"drinkId": "latte"}),
            content: vec![TextContent::text("confirmed")],
            structured_content: Some(Map::from_iter([("ok".to_owned(), json!(true))])),
            meta: None,
            component_path: "components/order-confirm/index".to_owned(),
        })
        .expect("explicit render succeeds");
    assert_eq!(render.data.render.renderer, "mock-renderer");

    let action = service
        .dispatch_component_action(RuntimeDispatchComponentActionRequest {
            session: session(),
            source_api_name: "searchDrinks".to_owned(),
            source_arguments: json!({"query": "latte"}),
            action: RuntimeComponentAction::ApiCall {
                name: "confirmOrder".to_owned(),
                arguments: json!({"drinkId": "latte"}),
            },
            capability_token: None,
            operation: None,
        })
        .expect("component api/call routes through facade");
    assert!(action.data.handled);
    assert_eq!(action.data.boundary, "runtime-orchestrator");
    assert!(action.data.host_action.is_none());
    assert_eq!(
        action.data.call.as_ref().map(|call| call.api_name.as_str()),
        Some("confirmOrder")
    );
    assert_eq!(*calls.borrow(), 2);

    let expire = service
        .expire_cards(RuntimeExpireCardsRequest {
            session: session(),
            filters: json!({"componentPath": "components/order-confirm/index"}),
        })
        .expect("expire boundary returns stable response");
    assert!(expire.data.accepted);
    assert_eq!(expire.data.boundary, "host-managed-card-store");

    let audit = service
        .get_audit_records(RuntimeAuditRecordsRequest {
            session_id: Some("session-runtime".to_owned()),
            skill_id: Some("coffee".to_owned()),
        })
        .expect("audit reader returns records");
    assert_eq!(audit.version, RUNTIME_API_VERSION);
    assert_eq!(audit.data.records.len(), 2);
    assert!(audit
        .data
        .records
        .iter()
        .all(|record| record.parameter_summary["drinkId"] == "latte"));

    let close = service
        .close_session(RuntimeCloseSessionRequest { session: session() })
        .expect("close response is stable");
    assert!(close.data.closed);
    assert_eq!(close.data.boundary, "runtime-session-manager");
}

#[test]
fn runtime_persistent_audit_sink_records_and_reads_redacted_events() {
    let fixture = TempDir::new("dock-core-runtime-audit");
    let audit = RuntimePersistentAuditSink::new(consent_audit::FileAuditSink::new(
        fixture.path().join("audit").join("records.jsonl"),
    ));
    let skill = load_skill(coffee_skill_root()).expect("coffee skill loads");
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("paid")],
        structured_content: Some(Map::from_iter([
            ("orderId".to_owned(), json!("order_demo_001")),
            ("status".to_owned(), json!("paid")),
        ])),
        meta: None,
        extra: Default::default(),
    });
    let service = RuntimeService::load_skill(
        skill,
        AllowHost,
        ApproveConsent,
        executor,
        MockRenderer,
        audit.clone(),
        audit,
    );

    service
        .call_api(RuntimeCallRequest {
            session: session(),
            api_name: "payOrder".to_owned(),
            arguments: json!({
                "orderId": "order_demo_001",
                "capabilityToken": "capability-secret-token",
                "deliveryAddress": "1 Private Road"
            }),
            capability_token: Some("capability-secret-token".to_owned()),
            operation: None,
        })
        .expect("payment call succeeds with persistent audit");

    let audit = service
        .get_audit_records(RuntimeAuditRecordsRequest {
            session_id: Some("session-runtime".to_owned()),
            skill_id: Some("coffee".to_owned()),
        })
        .expect("persistent audit reader returns records");
    assert_eq!(audit.data.records.len(), 1);
    let record = &audit.data.records[0];
    assert_eq!(record.api_name, "payOrder");
    assert_eq!(record.outcome, "ok");
    assert_eq!(record.permission_decision.decision, "allow");
    assert_eq!(record.parameter_summary["capabilityToken"], "[REDACTED]");
    assert_eq!(record.parameter_summary["deliveryAddress"], "[REDACTED]");

    let rendered = serde_json::to_string(&audit).expect("audit serializes");
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("1 Private Road"));
}

#[test]
fn runtime_persistent_audit_reader_reports_unavailable_when_backend_is_corrupt() {
    let fixture = TempDir::new("dock-core-runtime-audit-corrupt");
    let audit_path = fixture.path().join("audit").join("records.jsonl");
    fs::create_dir_all(audit_path.parent().expect("audit dir")).expect("create audit parent dir");
    fs::write(&audit_path, "{not-json}\n").expect("write corrupt audit file");
    let audit = RuntimePersistentAuditSink::new(consent_audit::FileAuditSink::new(audit_path));
    let skill = load_skill(coffee_skill_root()).expect("coffee skill loads");
    let service = RuntimeService::load_skill(
        skill,
        AllowHost,
        ApproveConsent,
        MockExecutor::with_result(AtomicApiResult {
            is_error: false,
            content: vec![TextContent::text("ok")],
            structured_content: None,
            meta: None,
            extra: Default::default(),
        }),
        MockRenderer,
        MockAudit::default(),
        audit,
    );

    let error = service
        .get_audit_records(RuntimeAuditRecordsRequest {
            session_id: None,
            skill_id: None,
        })
        .expect_err("corrupt persistent audit backend should surface an error");

    assert_eq!(error.error.code, "audit_unavailable");
    assert!(!error.error.message.contains("{not-json}"));
}

#[test]
fn runtime_observability_events_cover_core_flow_and_redact_sensitive_fields() {
    let observability = InMemoryObservabilitySink::default();
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("paid")],
        structured_content: Some(Map::from_iter([(
            "orderId".to_owned(),
            json!("order_demo_001"),
        )])),
        meta: Some(Map::from_iter([(
            "private".to_owned(),
            json!("component-only"),
        )])),
        extra: Default::default(),
    });
    let service = observability_runtime_service(executor, observability.clone());

    service
        .call_api(RuntimeCallRequest {
            session: session(),
            api_name: "payOrder".to_owned(),
            arguments: json!({
                "orderId": "order_demo_001",
                "capabilityToken": "capability-secret-token",
                "deliveryAddress": "1 Private Road"
            }),
            capability_token: Some("capability-secret-token".to_owned()),
            operation: None,
        })
        .expect("payment call succeeds");

    service
        .render_component(RuntimeRenderComponentRequest {
            session: session(),
            api_name: "confirmOrder".to_owned(),
            arguments: json!({"drinkId": "latte"}),
            content: vec![TextContent::text("confirmed")],
            structured_content: Some(Map::from_iter([("ok".to_owned(), json!(true))])),
            meta: None,
            component_path: "components/order-confirm/index".to_owned(),
        })
        .expect("explicit render emits events");

    service
        .dispatch_component_action(RuntimeDispatchComponentActionRequest {
            session: session(),
            source_api_name: "searchDrinks".to_owned(),
            source_arguments: json!({"query": "latte"}),
            action: RuntimeComponentAction::OpenDetailPage {
                url: "pages/order/detail".to_owned(),
            },
            capability_token: None,
            operation: None,
        })
        .expect("component action emits events");

    let events = observability.events();
    assert!(events
        .iter()
        .any(|event| event.kind == ObservabilityEventKind::SkillLoadStart));
    assert!(events
        .iter()
        .any(|event| event.kind == ObservabilityEventKind::SkillLoadEnd));
    assert!(events
        .iter()
        .any(|event| event.kind == ObservabilityEventKind::ApiCallStart));
    assert!(events
        .iter()
        .any(|event| event.kind == ObservabilityEventKind::ApiCallEnd));
    assert!(events
        .iter()
        .any(|event| event.kind == ObservabilityEventKind::ConsentPrompt));
    assert!(events
        .iter()
        .any(|event| event.kind == ObservabilityEventKind::ConsentDecision));
    assert!(events
        .iter()
        .any(|event| event.kind == ObservabilityEventKind::AuditRecordWritten));
    assert!(events
        .iter()
        .any(|event| event.kind == ObservabilityEventKind::ComponentRenderStart));
    assert!(events
        .iter()
        .any(|event| event.kind == ObservabilityEventKind::ComponentRenderEnd));
    assert!(events
        .iter()
        .any(|event| event.kind == ObservabilityEventKind::ComponentEvent));

    let api_end = events
        .iter()
        .find(|event| event.kind == ObservabilityEventKind::ApiCallEnd)
        .expect("api end event exists");
    assert_eq!(api_end.session_id.as_deref(), Some("session-runtime"));
    assert_eq!(api_end.skill_id.as_deref(), Some("coffee"));
    assert_eq!(api_end.api_name.as_deref(), Some("payOrder"));
    assert!(api_end
        .trace_id
        .as_deref()
        .is_some_and(|id| id.starts_with("trace-")));
    assert!(api_end
        .user_did_hash
        .as_deref()
        .is_some_and(|id| id.starts_with("sha256:")));
    assert_ne!(
        api_end.user_did_hash.as_deref(),
        Some("did:wba:user.example")
    );
    assert_eq!(
        api_end.merchant_did.as_deref(),
        Some("did:wba:merchant.example")
    );
    assert!(api_end.latency_ms.is_some());

    let rendered = serde_json::to_string(&events).expect("events serialize");
    assert!(rendered.contains("dock.observability.event.v1"));
    assert!(!rendered.contains("did:wba:user.example"));
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("1 Private Road"));
    assert!(!rendered.contains("Authorization"));
    assert!(!rendered.contains("private.pem"));
}

#[test]
fn runtime_observability_records_sandbox_limit_event_on_timeout() {
    let observability = InMemoryObservabilitySink::default();
    let service = observability_runtime_service(
        MockExecutor::with_result(AtomicApiResult {
            is_error: false,
            content: vec![TextContent::text("unused")],
            structured_content: None,
            meta: None,
            extra: Default::default(),
        }),
        observability.clone(),
    );

    let error = service
        .call_api(RuntimeCallRequest {
            session: session(),
            api_name: "payOrder".to_owned(),
            arguments: json!({"orderId": "order_demo_001"}),
            capability_token: None,
            operation: Some(RuntimeOperationOptions {
                timeout_ms: Some(0),
                ..Default::default()
            }),
        })
        .expect_err("timeout fails before dispatch");

    assert_eq!(error.error.code, "timeout");
    let events = observability.events();
    let limit_event = events
        .iter()
        .find(|event| event.kind == ObservabilityEventKind::SandboxLimitHit)
        .expect("sandbox limit event exists");
    assert_eq!(limit_event.outcome, "timeout");
    assert_eq!(limit_event.api_name.as_deref(), Some("payOrder"));
}

#[test]
fn runtime_metrics_and_trace_context_cover_core_flow_and_redact_labels() {
    let observability = InMemoryObservabilitySink::default();
    let metrics = InMemoryMetricsSink::default();
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("paid")],
        structured_content: Some(Map::from_iter([(
            "orderId".to_owned(),
            json!("order_demo_001"),
        )])),
        meta: Some(Map::from_iter([(
            "private".to_owned(),
            json!("component-only"),
        )])),
        extra: Default::default(),
    });
    let executor_traces = executor.traces.clone();
    let service = metrics_runtime_service(executor, observability, metrics.clone());
    let operation = Some(RuntimeOperationOptions {
        trace: Some(RuntimeTraceContext {
            trace_id: "trace-runtime-001".to_owned(),
            parent_span_id: Some("host-span-001".to_owned()),
        }),
        ..Default::default()
    });

    service
        .call_api(RuntimeCallRequest {
            session: session(),
            api_name: "payOrder".to_owned(),
            arguments: json!({
                "orderId": "order_demo_001",
                "capabilityToken": "capability-secret-token",
                "deliveryAddress": "1 Private Road",
                "header": { "Authorization": "Bearer secret" }
            }),
            capability_token: Some("capability-secret-token".to_owned()),
            operation: operation.clone(),
        })
        .expect("payment call succeeds");

    service
        .render_component(RuntimeRenderComponentRequest {
            session: session(),
            api_name: "confirmOrder".to_owned(),
            arguments: json!({"drinkId": "latte"}),
            content: vec![TextContent::text("confirmed")],
            structured_content: Some(Map::from_iter([("ok".to_owned(), json!(true))])),
            meta: None,
            component_path: "components/order-confirm/index".to_owned(),
        })
        .expect("explicit render records metrics");

    service
        .dispatch_component_action(RuntimeDispatchComponentActionRequest {
            session: session(),
            source_api_name: "searchDrinks".to_owned(),
            source_arguments: json!({"query": "latte"}),
            action: RuntimeComponentAction::OpenDetailPage {
                url: "pages/order/detail".to_owned(),
            },
            capability_token: None,
            operation: operation.clone(),
        })
        .expect("host action records component event metrics");

    service
        .dispatch_component_action(RuntimeDispatchComponentActionRequest {
            session: session(),
            source_api_name: "searchDrinks".to_owned(),
            source_arguments: json!({"query": "latte"}),
            action: RuntimeComponentAction::ApiCall {
                name: "confirmOrder".to_owned(),
                arguments: json!({"drinkId": "latte"}),
            },
            capability_token: None,
            operation,
        })
        .expect("component api/call propagates trace");

    let recorded_metrics = metrics.metrics();
    let recorded_spans = metrics.spans();
    let metric_names = recorded_metrics
        .iter()
        .map(|metric| metric.name.as_str())
        .collect::<Vec<_>>();
    for expected in [
        "dock.skill_load.total",
        "dock.api_call.total",
        "dock.api_latency_ms",
        "dock.render_latency_ms",
        "dock.component_event.total",
        "dock.audit_record.total",
        "dock.audit_latency_ms",
        "dock.consent.total",
    ] {
        assert!(
            metric_names.contains(&expected),
            "{expected} metric must be recorded"
        );
    }
    assert!(recorded_metrics
        .iter()
        .filter(|metric| metric.name == "dock.api_call.total")
        .any(
            |metric| metric.trace_id.as_deref() == Some("trace-runtime-001")
                && metric.labels.get("api").map(String::as_str) == Some("payOrder")
        ));
    assert!(recorded_spans
        .iter()
        .any(|span| span.kind == TraceSpanKind::ApiCall
            && span.trace_id == "trace-runtime-001"
            && span.parent_span_id.as_deref().is_some()));
    assert!(recorded_spans
        .iter()
        .any(|span| span.kind == TraceSpanKind::Render));
    assert!(recorded_spans
        .iter()
        .any(|span| span.kind == TraceSpanKind::ComponentAction
            && span.trace_id == "trace-runtime-001"));
    assert!(recorded_spans
        .iter()
        .any(|span| span.kind == TraceSpanKind::Audit && span.trace_id == "trace-runtime-001"));

    let traces = executor_traces.borrow();
    assert!(traces
        .iter()
        .flatten()
        .any(|trace| trace.trace_id == "trace-runtime-001"
            && trace.parent_span_id.as_deref().is_some()));

    let rendered =
        serde_json::to_string(&(recorded_metrics, recorded_spans)).expect("metrics serialize");
    assert!(!rendered.contains("did:wba:user.example"));
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("1 Private Road"));
    assert!(!rendered.contains("Authorization"));
    assert!(!rendered.contains("Bearer secret"));
    assert!(!rendered.contains("/home/"));
}

#[test]
fn runtime_metrics_record_sandbox_limit_on_timeout() {
    let observability = InMemoryObservabilitySink::default();
    let metrics = InMemoryMetricsSink::default();
    let service = metrics_runtime_service(
        MockExecutor::with_result(AtomicApiResult {
            is_error: false,
            content: vec![TextContent::text("unused")],
            structured_content: None,
            meta: None,
            extra: Default::default(),
        }),
        observability,
        metrics.clone(),
    );

    let error = service
        .call_api(RuntimeCallRequest {
            session: session(),
            api_name: "payOrder".to_owned(),
            arguments: json!({"orderId": "order_demo_001"}),
            capability_token: None,
            operation: Some(RuntimeOperationOptions {
                timeout_ms: Some(0),
                trace: Some(RuntimeTraceContext {
                    trace_id: "trace-timeout-001".to_owned(),
                    parent_span_id: None,
                }),
                ..Default::default()
            }),
        })
        .expect_err("timeout fails before dispatch");

    assert_eq!(error.error.code, "timeout");
    assert!(metrics.metrics().iter().any(|metric| {
        metric.name == "dock.sandbox_limit.total"
            && metric.trace_id.as_deref() == Some("trace-timeout-001")
            && metric.labels.get("limit").map(String::as_str) == Some("timeout")
    }));
}

#[test]
fn runtime_metrics_count_unsupported_api_without_sensitive_labels() {
    let observability = InMemoryObservabilitySink::default();
    let metrics = InMemoryMetricsSink::default();
    let service = metrics_runtime_service(
        MockExecutor::with_result(AtomicApiResult {
            is_error: false,
            content: vec![TextContent::text("unused")],
            structured_content: None,
            meta: None,
            extra: Default::default(),
        }),
        observability,
        metrics.clone(),
    );

    let error = service
        .call_api(RuntimeCallRequest {
            session: session(),
            api_name: "missingApi".to_owned(),
            arguments: json!({
                "url": "https://merchant.example/orders?Authorization=Bearer-secret"
            }),
            capability_token: Some("capability-secret-token".to_owned()),
            operation: Some(RuntimeOperationOptions {
                trace: Some(RuntimeTraceContext {
                    trace_id: "trace-unsupported-001".to_owned(),
                    parent_span_id: Some("host-span-001".to_owned()),
                }),
                ..Default::default()
            }),
        })
        .expect_err("missing API fails");

    assert_eq!(error.error.code, "api_not_found");
    let recorded_metrics = metrics.metrics();
    assert!(recorded_metrics.iter().any(|metric| {
        metric.name == "dock.unsupported_api.total"
            && metric.trace_id.as_deref() == Some("trace-unsupported-001")
            && metric.labels.get("api").map(String::as_str) == Some("missingApi")
            && metric.labels.get("outcome").map(String::as_str) == Some("api_not_found")
    }));
    let rendered = serde_json::to_string(&recorded_metrics).expect("metrics serialize");
    assert!(!rendered.contains("merchant.example/orders"));
    assert!(!rendered.contains("Bearer-secret"));
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("Authorization"));
}

#[test]
fn runtime_concurrency_policy_is_versioned_and_exposed() {
    let service = runtime_service(MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("unused")],
        structured_content: None,
        meta: None,
        extra: Default::default(),
    }));

    let policy = service.concurrency_policy();

    assert_eq!(policy.data.policy.version, "dock.runtime.concurrency.v1");
    assert!(policy.data.policy.low_risk_parallel_per_session);
    assert_eq!(
        policy
            .data
            .policy
            .retry_policy
            .non_idempotent_business_retry,
        "disabled-by-default"
    );
    assert!(!policy.data.policy.idempotency.required_for_high_risk);
}

#[test]
fn runtime_concurrency_cancel_and_close_block_before_executor_or_host_action() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("unused")],
        structured_content: None,
        meta: None,
        extra: Default::default(),
    });
    let calls = executor.calls.clone();
    let service = runtime_service(executor);

    service
        .cancel_operation(RuntimeCancelOperationRequest {
            session: session(),
            cancellation_token: "cancel-payment".to_owned(),
        })
        .expect("cancel token registers");

    let cancelled = service
        .call_api(RuntimeCallRequest {
            session: session(),
            api_name: "payOrder".to_owned(),
            arguments: json!({"orderId": "order_demo_001"}),
            capability_token: None,
            operation: Some(RuntimeOperationOptions {
                cancellation_token: Some("cancel-payment".to_owned()),
                ..Default::default()
            }),
        })
        .expect_err("cancelled operation fails before executor");
    assert_eq!(cancelled.error.code, "permission_denied");
    assert_eq!(*calls.borrow(), 0);

    let timed_out = service
        .dispatch_component_action(RuntimeDispatchComponentActionRequest {
            session: session(),
            source_api_name: "searchDrinks".to_owned(),
            source_arguments: json!({}),
            action: RuntimeComponentAction::OpenDetailPage {
                url: "pages/order/detail".to_owned(),
            },
            capability_token: None,
            operation: Some(RuntimeOperationOptions {
                timeout_ms: Some(0),
                ..Default::default()
            }),
        })
        .expect_err("timeout blocks before host action");
    assert_eq!(timed_out.error.code, "timeout");

    service
        .close_session(RuntimeCloseSessionRequest { session: session() })
        .expect("session closes");
    let closed = service
        .call_api(RuntimeCallRequest {
            session: session(),
            api_name: "searchDrinks".to_owned(),
            arguments: json!({"query": "latte"}),
            capability_token: None,
            operation: None,
        })
        .expect_err("closed session fails closed");
    assert_eq!(closed.error.code, "permission_denied");
    assert_eq!(*calls.borrow(), 0);
}

#[test]
fn runtime_idempotency_key_is_forwarded_redacted_and_replayed_without_reexecution() {
    let executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("paid")],
        structured_content: Some(Map::from_iter([(
            "orderId".to_owned(),
            json!("order_demo_001"),
        )])),
        meta: None,
        extra: Default::default(),
    });
    let calls = executor.calls.clone();
    let arguments = executor.arguments.clone();
    let service = runtime_service(executor);
    let request = || RuntimeCallRequest {
        session: session(),
        api_name: "payOrder".to_owned(),
        arguments: json!({"orderId": "order_demo_001"}),
        capability_token: Some("capability-secret-token".to_owned()),
        operation: Some(RuntimeOperationOptions {
            idempotency_key: Some("idem-order-001".to_owned()),
            ..Default::default()
        }),
    };

    let first = service.call_api(request()).expect("first payment succeeds");
    let second = service
        .call_api(request())
        .expect("idempotent replay succeeds");

    assert_eq!(first.data.result.content[0].text, "paid");
    assert_eq!(second.data.result.content[0].text, "paid");
    assert_eq!(*calls.borrow(), 1);
    assert_eq!(arguments.borrow()[0]["idempotencyKey"], "idem-order-001");

    let audit = service
        .get_audit_records(RuntimeAuditRecordsRequest {
            session_id: Some("session-runtime".to_owned()),
            skill_id: Some("coffee".to_owned()),
        })
        .expect("audit available");
    assert_eq!(audit.data.records.len(), 1);
    assert_eq!(
        audit.data.records[0].parameter_summary["idempotencyKey"],
        "idem-order-001"
    );
    let rendered = serde_json::to_string(&audit).expect("audit serializes");
    assert!(!rendered.contains("capability-secret-token"));
}

#[test]
fn runtime_concurrency_rejects_high_risk_same_session_and_api() {
    let blocker = BlockingExecutor::new();
    let calls = blocker.calls.clone();
    let service = threaded_runtime_service(blocker.clone());
    let worker_service = service.clone();
    let worker = std::thread::spawn(move || {
        worker_service.call_api(RuntimeCallRequest {
            session: session(),
            api_name: "payOrder".to_owned(),
            arguments: json!({"orderId": "order_demo_001"}),
            capability_token: None,
            operation: None,
        })
    });
    blocker.wait_until_started();

    let duplicate = service
        .call_api(RuntimeCallRequest {
            session: session(),
            api_name: "payOrder".to_owned(),
            arguments: json!({"orderId": "order_demo_001"}),
            capability_token: None,
            operation: None,
        })
        .expect_err("duplicate high-risk operation is rejected");
    assert_eq!(duplicate.error.code, "permission_denied");

    blocker.release();
    worker
        .join()
        .expect("worker joins")
        .expect("first high-risk call succeeds");
    assert_eq!(*calls.lock().expect("calls lock"), 1);

    let second_executor = MockExecutor::with_result(AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("paid")],
        structured_content: None,
        meta: None,
        extra: Default::default(),
    });
    let second_calls = second_executor.calls.clone();
    let second_service = runtime_service(second_executor);
    second_service
        .call_api(RuntimeCallRequest {
            session: RuntimeSessionContext {
                session_id: "session-runtime-2".to_owned(),
                ..session()
            },
            api_name: "payOrder".to_owned(),
            arguments: json!({"orderId": "order_demo_002"}),
            capability_token: None,
            operation: None,
        })
        .expect("different session is isolated");
    assert_eq!(*calls.lock().expect("calls lock"), 1);
    assert_eq!(*second_calls.borrow(), 1);
}

#[test]
fn runtime_concurrency_allows_high_risk_for_different_sessions() {
    let blocker = FirstCallBlockingExecutor::new();
    let calls = blocker.calls.clone();
    let service = first_call_blocking_runtime_service(blocker.clone());
    let worker_service = service.clone();
    let worker = std::thread::spawn(move || {
        worker_service.call_api(RuntimeCallRequest {
            session: session(),
            api_name: "payOrder".to_owned(),
            arguments: json!({"orderId": "order_demo_001"}),
            capability_token: None,
            operation: None,
        })
    });
    blocker.wait_until_started();

    service
        .call_api(RuntimeCallRequest {
            session: RuntimeSessionContext {
                session_id: "session-runtime-2".to_owned(),
                ..session()
            },
            api_name: "payOrder".to_owned(),
            arguments: json!({"orderId": "order_demo_002"}),
            capability_token: None,
            operation: None,
        })
        .expect("different session can run while first high-risk call is in flight");

    blocker.release();
    worker
        .join()
        .expect("worker joins")
        .expect("first high-risk call succeeds");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn runtime_concurrency_low_risk_calls_are_not_serialized_by_high_risk_policy() {
    let blocker = FirstCallBlockingExecutor::new();
    let calls = blocker.calls.clone();
    let service = first_call_blocking_runtime_service(blocker.clone());
    let worker_service = service.clone();
    let worker = std::thread::spawn(move || {
        worker_service.call_api(RuntimeCallRequest {
            session: session(),
            api_name: "searchDrinks".to_owned(),
            arguments: json!({"query": "latte"}),
            capability_token: None,
            operation: None,
        })
    });
    blocker.wait_until_started();

    service
        .call_api(RuntimeCallRequest {
            session: session(),
            api_name: "searchDrinks".to_owned(),
            arguments: json!({"query": "mocha"}),
            capability_token: None,
            operation: None,
        })
        .expect("low-risk calls are not serialized");

    blocker.release();
    worker
        .join()
        .expect("worker joins")
        .expect("first low-risk call succeeds");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn runtime_facade_errors_are_json_serializable_and_redacted() {
    let service = runtime_service(MockExecutor::fail(
        ErrorCode::VmFailed,
        "Authorization Bearer capability-secret-token",
    ));

    let error = service
        .call_api(RuntimeCallRequest {
            session: session(),
            api_name: "searchDrinks".to_owned(),
            arguments: json!({"query": "latte"}),
            capability_token: Some("capability-secret-token".to_owned()),
            operation: None,
        })
        .expect_err("executor failure is wrapped as runtime error");

    let json = serde_json::to_value(&error).expect("runtime error serializes");
    assert_eq!(json["version"], RUNTIME_API_VERSION);
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["code"], "vm_failed");
    let rendered = json.to_string();
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("Authorization Bearer"));
    assert!(rendered.contains("[REDACTED]"));
}

#[test]
fn runtime_facade_validation_errors_redact_reports() {
    let mut report = ValidationReport::ok();
    report.push_error(
        "/home/user/private/key-1-private.pem",
        "Authorization Bearer capability-secret-token",
    );
    report.push_compatibility_warning(
        "credential.path",
        "file:/Users/demo/private-key.pem",
        Some("remove secret token"),
    );

    let error = dock_core::RuntimeErrorResponse::from_core(DockCoreError::validation(
        "private validation error",
        report,
    ));

    let rendered = serde_json::to_string(&error).expect("runtime error serializes");
    assert_eq!(error.error.code, "validation_failed");
    assert!(!rendered.contains("/home/user/private/key-1-private.pem"));
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("/Users/demo/private-key.pem"));
    assert!(!rendered.contains("secret token"));
    assert!(rendered.contains("[REDACTED]"));
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
struct MockExecutor {
    result: AtomicApiResult,
    error: Option<(ErrorCode, String)>,
    calls: std::rc::Rc<RefCell<usize>>,
    arguments: std::rc::Rc<RefCell<Vec<serde_json::Value>>>,
    traces: std::rc::Rc<RefCell<Vec<Option<TraceContext>>>>,
}

impl MockExecutor {
    fn with_result(result: AtomicApiResult) -> Self {
        Self {
            result,
            error: None,
            calls: Default::default(),
            arguments: Default::default(),
            traces: Default::default(),
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
            arguments: Default::default(),
            traces: Default::default(),
        }
    }
}

impl ApiExecutor for MockExecutor {
    fn execute(
        &self,
        context: &ApiCallContext,
        _component_path: Option<&str>,
    ) -> Result<AtomicApiResult, DockCoreError> {
        *self.calls.borrow_mut() += 1;
        self.arguments.borrow_mut().push(context.arguments.clone());
        self.traces.borrow_mut().push(context.trace.clone());
        if let Some((code, message)) = &self.error {
            return Err(DockCoreError::core(*code, message.clone()));
        }
        Ok(self.result.clone())
    }
}

#[derive(Clone)]
struct BlockingExecutor {
    started: Arc<Barrier>,
    release: Arc<Barrier>,
    calls: Arc<Mutex<usize>>,
}

impl BlockingExecutor {
    fn new() -> Self {
        Self {
            started: Arc::new(Barrier::new(2)),
            release: Arc::new(Barrier::new(2)),
            calls: Arc::new(Mutex::new(0)),
        }
    }

    fn wait_until_started(&self) {
        self.started.wait();
    }

    fn release(&self) {
        self.release.wait();
    }
}

impl ApiExecutor for BlockingExecutor {
    fn execute(
        &self,
        _context: &ApiCallContext,
        _component_path: Option<&str>,
    ) -> Result<AtomicApiResult, DockCoreError> {
        *self.calls.lock().expect("calls lock") += 1;
        self.started.wait();
        self.release.wait();
        Ok(AtomicApiResult {
            is_error: false,
            content: vec![TextContent::text("paid")],
            structured_content: None,
            meta: None,
            extra: Default::default(),
        })
    }
}

#[derive(Clone)]
struct FirstCallBlockingExecutor {
    started: Arc<Barrier>,
    release: Arc<Barrier>,
    calls: Arc<AtomicUsize>,
}

impl FirstCallBlockingExecutor {
    fn new() -> Self {
        Self {
            started: Arc::new(Barrier::new(2)),
            release: Arc::new(Barrier::new(2)),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn wait_until_started(&self) {
        self.started.wait();
    }

    fn release(&self) {
        self.release.wait();
    }
}

impl ApiExecutor for FirstCallBlockingExecutor {
    fn execute(
        &self,
        _context: &ApiCallContext,
        _component_path: Option<&str>,
    ) -> Result<AtomicApiResult, DockCoreError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            self.started.wait();
            self.release.wait();
        }
        Ok(AtomicApiResult {
            is_error: false,
            content: vec![TextContent::text("ok")],
            structured_content: None,
            meta: None,
            extra: Default::default(),
        })
    }
}

#[derive(Clone)]
struct MockRenderer;

impl RenderRouter for MockRenderer {
    fn render(
        &self,
        _context: &ApiCallContext,
        input: &ComponentRenderInput,
    ) -> Result<RenderOutcome, DockCoreError> {
        Ok(RenderOutcome {
            renderer: "mock-renderer".to_owned(),
            component_path: Some(input.component_path.clone()),
            payload: json!({
                "apiName": input.api_name,
                "structuredContent": input.structured_content,
                "meta": input.meta
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
            renderer: "mock-fallback".to_owned(),
            component_path: None,
            payload: json!({}),
            fallback_reason: Some(reason.to_owned()),
        }
    }
}

#[derive(Clone)]
struct ThreadSafeRenderer;

impl RenderRouter for ThreadSafeRenderer {
    fn render(
        &self,
        _context: &ApiCallContext,
        input: &ComponentRenderInput,
    ) -> Result<RenderOutcome, DockCoreError> {
        Ok(RenderOutcome {
            renderer: "thread-safe-renderer".to_owned(),
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
            renderer: "thread-safe-fallback".to_owned(),
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

#[derive(Clone, Default)]
struct ThreadSafeAudit {
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl AuditSink for ThreadSafeAudit {
    fn record(&self, event: AuditEvent) -> Result<(), DockCoreError> {
        self.events.lock().expect("audit lock").push(event);
        Ok(())
    }
}

impl RuntimeAuditReader for ThreadSafeAudit {
    fn runtime_audit_records(&self) -> Result<Vec<AuditEvent>, dock_core::DockCoreError> {
        Ok(self.events.lock().expect("audit lock").clone())
    }
}

#[derive(Clone)]
struct ThreadSafeAllowHost;

impl RuntimeHost for ThreadSafeAllowHost {
    fn check_permission(
        &self,
        _context: &ApiCallContext,
    ) -> Result<PermissionDecision, DockCoreError> {
        Ok(PermissionDecision::Allow)
    }
}

#[derive(Clone)]
struct ThreadSafeApproveConsent;

impl ConsentGate for ThreadSafeApproveConsent {
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

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{prefix}-{}", unique_suffix()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("create temp dir");
        Self { path }
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
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    format!("{}-{nanos}", std::process::id())
}

impl AuditSink for MockAudit {
    fn record(&self, event: AuditEvent) -> Result<(), DockCoreError> {
        self.events.borrow_mut().push(event);
        Ok(())
    }
}

impl RuntimeAuditReader for MockAudit {
    fn runtime_audit_records(&self) -> Result<Vec<AuditEvent>, dock_core::DockCoreError> {
        Ok(self.events.borrow().clone())
    }
}
