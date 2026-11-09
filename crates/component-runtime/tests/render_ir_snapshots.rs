use component_runtime::{
    ComponentEvent, ComponentEventKind, ComponentInput, ComponentInstance, ComponentMetadata,
    ComponentPackage, ComponentVmConfig, ComponentVmError, DynamicComponentConfig,
};
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use wx_compat::{CapabilityProfile, RequestBroker, WxRequest, WxRequestError, WxResponse};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under crates/component-runtime")
        .to_path_buf()
}

fn fixture_root(name: &str) -> PathBuf {
    repo_root().join("examples/fixtures").join(name)
}

fn component_root(fixture: &str, component: &str) -> PathBuf {
    fixture_root(fixture).join("components").join(component)
}

fn snapshot_path(name: &str) -> PathBuf {
    repo_root()
        .join("testdata/render-ir")
        .join(format!("{name}.json"))
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

#[test]
fn snapshot_address_form_fixture() {
    let mut input = component_input(
        "prepareAddressForm",
        json!({"addressHandle": "addr_handle_demo_001"}),
        json!({
            "form": {
                "recipient": "Demo Recipient",
                "note": "Use opaque address handle only",
                "slots": ["09:00-10:00", "10:00-11:00"],
                "selectedSlot": 1,
                "addressHandle": "addr_handle_demo_001"
            },
            "boundary": {
                "provider": "wx.chooseAddress",
                "status": "host-boundary",
                "consent": "required"
            }
        }),
    );
    input.component_metadata = ComponentMetadata {
        component_path: Some("components/address-form/index".to_owned()),
        related_page: Some(json!({
            "path": "pages/address/review",
            "query": { "fixture": "address-form" }
        })),
        expirable: true,
        expired_text: Some("Mock address form expired".to_owned()),
        ..ComponentMetadata::default()
    };

    let snapshot = mount_snapshot(
        "address-form",
        "address-form",
        input,
        ComponentVmConfig::default(),
        json!({
            "provider": "wx.chooseAddress",
            "riskLevel": "L4",
            "boundary": "host-provider-consent-required",
            "dataPolicy": "opaque-address-handle-only"
        }),
        Some(ComponentEvent::new(ComponentEventKind::Tap, "submit")),
    );

    assert_snapshot("address-form.prepareAddressForm", snapshot);
}

#[test]
fn snapshot_media_review_fixture() {
    let mut input = component_input(
        "reviewMedia",
        json!({
            "imageHandle": "image_handle_demo_001",
            "fileHandle": "file_handle_demo_001"
        }),
        json!({
            "media": {
                "imageHandle": "image_handle_demo_001",
                "fileHandle": "file_handle_demo_001",
                "previewImage": "https://static.example.invalid/fixtures/media-preview.png",
                "poster": "https://static.example.invalid/fixtures/media-poster.png"
            },
            "boundary": {
                "provider": "wx.chooseMedia",
                "status": "host-boundary",
                "handleType": "opaque"
            }
        }),
    );
    input.component_metadata = ComponentMetadata {
        component_path: Some("components/media-review/index".to_owned()),
        expirable: true,
        expired_text: Some("Mock media review expired".to_owned()),
        ..ComponentMetadata::default()
    };

    let snapshot = mount_snapshot(
        "media-review",
        "media-review",
        input,
        ComponentVmConfig::default(),
        json!({
            "provider": "wx.chooseMedia",
            "riskLevel": "L4",
            "boundary": "host-media-provider-required",
            "dataPolicy": "opaque-file-and-image-handles-only"
        }),
        Some(ComponentEvent::new(ComponentEventKind::Tap, "approve")),
    );

    assert_snapshot("media-review.reviewMedia", snapshot);
}

#[test]
fn snapshot_dynamic_status_fixture() {
    let broker = FixtureRequestBroker::new();
    let config = ComponentVmConfig {
        dynamic: DynamicComponentConfig::default().with_request_broker(broker.clone()),
        ..ComponentVmConfig::default()
    };
    let mut input = component_input(
        "refreshDynamicStatus",
        json!({"orderId": "order_demo_001"}),
        json!({
            "orderId": "order_demo_001",
            "status": "pending"
        }),
    );
    input.component_metadata = ComponentMetadata {
        component_path: Some("components/dynamic-status/index".to_owned()),
        dynamic: true,
        scope_dynamic: Some(json!({
            "desc": "Poll mock order status through RequestBroker",
            "reason": "fixture-only status refresh"
        })),
        expirable: true,
        expired_text: Some("Mock dynamic status expired".to_owned()),
        ..ComponentMetadata::default()
    };

    let mut snapshot = mount_snapshot(
        "dynamic-status",
        "dynamic-status",
        input,
        config,
        json!({
            "provider": "RequestBroker",
            "riskLevel": "L2",
            "boundary": "dynamic-request-timer-gated",
            "dataPolicy": "response-auth-headers-redacted"
        }),
        Some(ComponentEvent::new(ComponentEventKind::Tap, "refresh")),
    );
    let broker_calls = broker.calls.borrow().len();
    assert_eq!(broker_calls, 1);
    snapshot["auditSummary"]["brokerCalls"] = json!(broker_calls);

    assert_snapshot("dynamic-status.refreshDynamicStatus", snapshot);
}

#[test]
fn snapshot_location_map_preview_fixture() {
    let mut input = component_input(
        "prepareLocationMap",
        json!({"locationToken": "location_handle_demo_001"}),
        json!({
            "location": {
                "region": "mock-region-downtown",
                "locationToken": "location_handle_demo_001",
                "providerStatus": "fail-closed",
                "fallbackReason": "host_location_provider_required"
            }
        }),
    );
    input.component_metadata = ComponentMetadata {
        component_path: Some("components/location-map-preview/index".to_owned()),
        related_page: Some(json!({
            "path": "pages/location/preview",
            "query": { "fixture": "location-map-preview" }
        })),
        ..ComponentMetadata::default()
    };

    let snapshot = mount_snapshot(
        "location-map-preview",
        "location-map-preview",
        input,
        ComponentVmConfig::default(),
        json!({
            "provider": "wx.getLocation",
            "riskLevel": "L4",
            "boundary": "host-location-provider-fail-closed",
            "dataPolicy": "opaque-location-token-only"
        }),
        Some(ComponentEvent::new(
            ComponentEventKind::Tap,
            "requestLocation",
        )),
    );

    assert_snapshot("location-map-preview.prepareLocationMap", snapshot);
}

fn component_input(api_name: &str, arguments: Value, structured_content: Value) -> ComponentInput {
    ComponentInput {
        api_name: api_name.to_owned(),
        arguments,
        properties: Map::new(),
        content: vec![json!({"type": "text", "text": format!("{api_name} fixture result")})],
        structured_content: structured_content.as_object().cloned(),
        meta: Some(Map::from_iter([
            ("fixture".to_owned(), Value::String(api_name.to_owned())),
            ("mockOnly".to_owned(), Value::Bool(true)),
        ])),
        component_metadata: ComponentMetadata::default(),
    }
}

fn mount_snapshot(
    fixture: &str,
    component: &str,
    input: ComponentInput,
    config: ComponentVmConfig,
    audit_summary: Value,
    action_event: Option<ComponentEvent>,
) -> Value {
    let package = ComponentPackage::load(component_root(fixture, component)).expect("load fixture");
    let mut instance = ComponentInstance::with_config(package, config).expect("create vm");
    let mounted = instance.mount(input).expect("mount fixture");
    let action = action_event.map(|event| {
        instance
            .dispatch_event(&event)
            .expect("dispatch fixture action")
            .actions
    });
    let expired = instance.expire(json!({"reason": "snapshot"}));

    json!({
        "fixture": fixture,
        "component": component,
        "render": mounted.render,
        "actions": mounted.actions,
        "eventActions": action.unwrap_or_default(),
        "warnings": mounted.render.warnings,
        "metadata": mounted.metadata,
        "state": mounted.state,
        "auditSummary": audit_summary,
        "expire": {
            "ok": expired.is_ok(),
            "expired": instance.is_expired()
        }
    })
}

fn assert_snapshot(name: &str, actual: Value) {
    assert_snapshot_has_no_sensitive_strings(&actual);
    let path = snapshot_path(name);
    if std::env::var_os("ANP_DOCK_UPDATE_SNAPSHOTS").is_some() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create snapshot dir");
        }
        fs::write(&path, format!("{}\n", pretty(&actual))).expect("write snapshot");
        return;
    }

    let expected = read_json(&path);
    assert_eq!(actual, expected, "snapshot mismatch for {}", path.display());
}

fn read_json(path: &Path) -> Value {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read snapshot `{}`: {error}", path.display()));
    serde_json::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse snapshot `{}`: {error}", path.display()))
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).expect("snapshot serializes")
}

fn assert_snapshot_has_no_sensitive_strings(value: &Value) {
    let source = value.to_string();
    for forbidden in [
        "Bearer ",
        "Authorization",
        "Signature",
        "Signature-Input",
        "private key",
        "phoneNumber",
        "real_address",
        "latitude",
        "longitude",
        "/home/",
        "/Users/",
    ] {
        assert!(
            !source.contains(forbidden),
            "snapshot contains forbidden string `{forbidden}`"
        );
    }
    assert!(!source.contains("fixture-token"));
}

#[test]
fn fixture_packages_exist() {
    for (fixture, component) in [
        ("address-form", "address-form"),
        ("media-review", "media-review"),
        ("dynamic-status", "dynamic-status"),
        ("location-map-preview", "location-map-preview"),
    ] {
        assert!(fixture_root(fixture).join("SKILL.md").is_file());
        assert!(fixture_root(fixture).join("mcp.json").is_file());
        assert!(component_root(fixture, component)
            .join("index.wxml")
            .is_file());
    }
}

#[test]
fn expired_dynamic_fixture_rejects_later_actions() {
    let package =
        ComponentPackage::load(component_root("dynamic-status", "dynamic-status")).expect("load");
    let mut input = component_input(
        "refreshDynamicStatus",
        json!({"orderId": "order_demo_001"}),
        json!({"orderId": "order_demo_001", "status": "pending"}),
    );
    input.component_metadata.dynamic = true;
    let mut instance = ComponentInstance::new(package).expect("create vm");

    instance.mount(input).expect("mount");
    instance.expire(json!({})).expect("expire");

    assert!(matches!(
        instance.dispatch_event(&ComponentEvent::new(ComponentEventKind::Tap, "refresh")),
        Err(ComponentVmError::Expired)
    ));
}
