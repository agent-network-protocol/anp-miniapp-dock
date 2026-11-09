use component_runtime::{
    ComponentEvent, ComponentEventKind, ComponentInput, ComponentInstance, ComponentMetadata,
    ComponentPackage, ComponentTraceKind, ComponentVmAction, ComponentVmConfig, ComponentVmError,
    DynamicComponentConfig, RenderNodeKind,
};
use serde_json::{json, Map};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use wx_compat::{CapabilityProfile, RequestBroker, WxRequest, WxRequestError, WxResponse};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under crates/component-runtime")
        .to_path_buf()
}

fn write_component(name: &str, js: &str, wxml: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "anp-miniapp-dock-component-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("component temp dir");
    fs::write(root.join("index.js"), js).expect("write index.js");
    fs::write(root.join("index.wxml"), wxml).expect("write index.wxml");
    fs::write(root.join("index.wxss"), ".card { padding: 12px; }").expect("write index.wxss");
    root
}

fn component_input() -> ComponentInput {
    ComponentInput {
        api_name: "searchDrinks".to_owned(),
        arguments: json!({"query": "latte"}),
        properties: Map::new(),
        content: vec![json!({"type": "text", "text": "result"})],
        structured_content: Some(Map::from_iter([(
            "drinks".to_owned(),
            json!([{ "id": "latte", "name": "Latte" }]),
        )])),
        meta: None,
        component_metadata: ComponentMetadata::default(),
    }
}

#[derive(Debug)]
struct MockRequestBroker {
    calls: RefCell<Vec<WxRequest>>,
}

impl MockRequestBroker {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            calls: RefCell::new(Vec::new()),
        })
    }

    fn calls(&self) -> Vec<WxRequest> {
        self.calls.borrow().clone()
    }
}

impl RequestBroker for MockRequestBroker {
    fn request(
        &self,
        profile: &CapabilityProfile,
        request: WxRequest,
    ) -> Result<WxResponse, WxRequestError> {
        if !profile.check(wx_compat::Capability::Request).is_allowed() {
            return Err(WxRequestError::Denied(
                "request capability denied in test broker".to_owned(),
            ));
        }
        self.calls.borrow_mut().push(request.clone());
        let mut headers = BTreeMap::new();
        headers.insert("x-safe".to_owned(), "ok".to_owned());
        headers.insert("Authorization".to_owned(), "Bearer test-token".to_owned());
        headers.insert("x-token-id".to_owned(), "secret-token".to_owned());
        Ok(WxResponse {
            status_code: 200,
            headers,
            data: json!({"status": "fresh"}),
        })
    }
}

#[test]
fn lifecycle_and_result_notification_update_initial_render() {
    let root = write_component(
        "lifecycle",
        r#"
Component({
  data: { title: 'Loading' },
  lifetimes: {
    created() {
      const modelCtx = wx.modelContext.getContext(this)
      modelCtx.on(wx.modelContext.NotificationType.Result, (data) => {
        this.setData({
          title: data.result.structuredContent.drinks[0].name,
          count: data.result.structuredContent.drinks.length
        })
      })
    },
    attached() {
      this.setData({ attached: true })
    }
  }
})
"#,
        r#"<view class="card"><text>{{ title }}</text><text>{{ count }}</text><text>{{ attached }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let mut instance = ComponentInstance::new(package).expect("create vm");

    let outcome = instance.mount(component_input()).expect("mount component");

    assert_eq!(outcome.render.root.kind, RenderNodeKind::View);
    assert_eq!(
        outcome.render.root.children[0].text.as_deref(),
        Some("Latte")
    );
    assert_eq!(outcome.render.root.children[1].text.as_deref(), Some("1"));
    assert_eq!(
        outcome.render.root.children[2].text.as_deref(),
        Some("true")
    );
    assert_eq!(
        outcome
            .trace
            .iter()
            .map(|entry| (&entry.kind, entry.name.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (&ComponentTraceKind::Lifecycle, "created"),
            (&ComponentTraceKind::Notification, "input"),
            (&ComponentTraceKind::Notification, "result"),
            (&ComponentTraceKind::Lifecycle, "attached"),
        ]
    );
    assert!(instance.is_mounted());
}

#[test]
fn tap_event_passes_dataset_and_set_data_rerenders() {
    let root = write_component(
        "tap",
        r#"
Component({
  data: { selected: 'none' },
  methods: {
    selectDrink(e) {
      this.setData({ selected: e.currentTarget.dataset.id })
    }
  }
})
"#,
        r#"<view><button bindtap="selectDrink" data-id="latte">Choose</button><text>{{ selected }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let mut instance = ComponentInstance::new(package).expect("create vm");
    instance
        .mount(ComponentInput::new("searchDrinks"))
        .expect("mount");
    let button = instance
        .render()
        .expect("render")
        .root
        .children
        .into_iter()
        .find(|node| node.kind == RenderNodeKind::Button)
        .expect("button exists");
    let event = ComponentEvent::from_binding(&button.events[0]);

    let outcome = instance.dispatch_event(&event).expect("dispatch");

    assert_eq!(outcome.state.get("selected"), Some(&json!("latte")));
    assert_eq!(
        outcome.render.root.children[1].text.as_deref(),
        Some("latte")
    );
    assert_eq!(outcome.trace[0].kind, ComponentTraceKind::Event);
    assert_eq!(outcome.trace[0].name, "selectDrink");
}

#[test]
fn component_actions_are_returned_to_host_without_direct_execution() {
    let root = write_component(
        "actions",
        r#"
Component({
  methods: {
    confirmDrink(e) {
      const ctx = wx.modelContext.getContext(this)
      ctx.sendFollowUpMessage({
        content: [
          { type: 'text', text: '选择 ' + e.currentTarget.dataset.id },
          { type: 'api/call', data: { name: 'confirmOrder', arguments: { drinkId: e.currentTarget.dataset.id } } }
        ]
      })
    }
  }
})
"#,
        r#"<view><button bindtap="confirmDrink" data-id="latte">Choose</button></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let mut instance = ComponentInstance::new(package).expect("create vm");
    instance
        .mount(ComponentInput::new("searchDrinks"))
        .expect("mount");
    let button = instance.render().expect("render").root.children.remove(0);
    let event = ComponentEvent::from_binding(&button.events[0]);

    let outcome = instance.dispatch_event(&event).expect("dispatch");

    assert!(matches!(
        &outcome.actions[0],
        ComponentVmAction::SendFollowUpMessage { content } if content.len() == 2
    ));
    assert!(matches!(
        &outcome.actions[1],
        ComponentVmAction::ApiCall { name, arguments }
            if name == "confirmOrder" && arguments.get("drinkId") == Some(&json!("latte"))
    ));
}

#[test]
fn expire_notification_detaches_and_blocks_later_events() {
    let root = write_component(
        "expire",
        r#"
Component({
  data: { expired: false },
  lifetimes: {
    created() {
      wx.modelContext.getViewContext(this).on(wx.modelContext.NotificationType.Expire, () => {
        this.setData({ expired: true })
      })
    },
    detached() {
      this.setData({ detached: true })
    }
  },
  methods: {
    tap() {
      this.setData({ tapped: true })
    }
  }
})
"#,
        r#"<view><button bindtap="tap">Tap</button><text>{{ expired }}</text><text>{{ detached }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let mut instance = ComponentInstance::new(package).expect("create vm");
    instance
        .mount(ComponentInput::new("searchDrinks"))
        .expect("mount");

    let outcome = instance
        .expire(json!({"reason": "replaced"}))
        .expect("expire");

    assert!(instance.is_expired());
    assert_eq!(outcome.state.get("expired"), Some(&json!(true)));
    assert_eq!(outcome.state.get("detached"), Some(&json!(true)));
    let event = ComponentEvent::new(ComponentEventKind::Tap, "tap");
    assert!(matches!(
        instance.dispatch_event(&event),
        Err(ComponentVmError::Expired)
    ));
}

#[test]
fn component_vm_does_not_expose_network_timer_or_function_escape() {
    let root = write_component(
        "sandbox",
        r#"
Component({
  data: { ok: false },
  lifetimes: {
    created() {
      const functionEscape = (function() {}).constructor
      const asyncEscape = (async function() {}).constructor
      const generatorEscape = (function* () {}).constructor
      const asyncGeneratorEscape = (async function* () {}).constructor
      const prototypeEscape = ({}).constructor.constructor
      this.setData({
        ok: typeof fetch === 'undefined'
          && typeof WebSocket === 'undefined'
          && typeof setTimeout === 'undefined'
          && typeof setInterval === 'undefined'
          && typeof clearTimeout === 'undefined'
          && typeof clearInterval === 'undefined'
          && typeof require === 'undefined'
          && typeof eval === 'undefined'
          && typeof process === 'undefined'
          && typeof Function === 'undefined'
          && typeof functionEscape === 'undefined'
          && typeof asyncEscape === 'undefined'
          && typeof generatorEscape === 'undefined'
          && typeof asyncGeneratorEscape === 'undefined'
          && typeof prototypeEscape === 'undefined'
      })
    }
  }
})
"#,
        r#"<view><text>{{ ok }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let mut instance = ComponentInstance::new(package).expect("create vm");

    let outcome = instance
        .mount(ComponentInput::new("searchDrinks"))
        .expect("mount");

    assert_eq!(outcome.state.get("ok"), Some(&json!(true)));
    assert_eq!(
        outcome.render.root.children[0].text.as_deref(),
        Some("true")
    );
}

#[test]
fn component_snapshot_size_limit_fails_closed_without_echoing_payload() {
    let root = write_component(
        "snapshot-size",
        r#"
Component({
  data: { blob: '' },
  lifetimes: {
    created() {
      this.setData({ blob: 'private-token-should-not-be-echoed'.repeat(16) })
    }
  }
})
"#,
        r#"<view><text>{{ blob }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let config = ComponentVmConfig {
        max_snapshot_json_bytes: 128,
        ..ComponentVmConfig::default()
    };
    let mut instance = ComponentInstance::with_config(package, config).expect("create vm");

    let error = instance
        .mount(ComponentInput::new("searchDrinks"))
        .expect_err("oversized snapshot should fail closed");

    assert!(matches!(
        error,
        ComponentVmError::OutputTooLarge { limit: 128 }
    ));
    assert!(!error
        .to_string()
        .contains("private-token-should-not-be-echoed"));
}

#[test]
fn sandbox_gate_blocks_escape_and_remote_globals_before_dynamic() {
    let root = write_component(
        "sandbox-gate",
        r#"
Component({
  data: { blocked: false },
  lifetimes: {
    created() {
      let constructorBlocked = false
      try {
        constructorBlocked = typeof ({}).constructor.constructor === 'undefined'
      } catch (_) {
        constructorBlocked = true
      }
      this.setData({
        blocked: constructorBlocked
          && typeof eval === 'undefined'
          && typeof Function === 'undefined'
          && typeof process === 'undefined'
          && typeof fetch === 'undefined'
          && typeof WebSocket === 'undefined'
          && typeof require === 'undefined'
          && typeof __dockDynamicRequestJson === 'undefined'
          && typeof setTimeout === 'undefined'
          && typeof setInterval === 'undefined'
      })
    }
  }
})
"#,
        r#"<view><text>{{ blocked }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let mut instance = ComponentInstance::new(package).expect("create vm");

    let outcome = instance
        .mount(ComponentInput::new("searchDrinks"))
        .expect("mount");

    assert_eq!(outcome.state.get("blocked"), Some(&json!(true)));
}

#[test]
fn sandbox_gate_interrupts_long_running_component_code() {
    let root = write_component(
        "sandbox-timeout",
        r#"
Component({
  lifetimes: {
    created() {
      while (true) {}
    }
  }
})
"#,
        r#"<view><text>timeout</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let config = ComponentVmConfig {
        timeout: Duration::from_millis(1),
        ..ComponentVmConfig::default()
    };
    let mut instance = ComponentInstance::with_config(package, config).expect("create vm");

    assert!(matches!(
        instance.mount(ComponentInput::new("searchDrinks")),
        Err(ComponentVmError::Timeout(_))
    ));
}

#[test]
fn dynamic_request_and_timer_default_deny_without_scope_dynamic() {
    let root = write_component(
        "dynamic-default-deny",
        r#"
Component({
  data: { requestDenied: false, timerDenied: false },
  lifetimes: {
    async created() {
      try {
        await wx.request({ url: 'https://merchant.example/status' })
      } catch (error) {
        this.setData({ requestDenied: error.code === 'permission_denied' })
      }
      this.setData({ timerDenied: typeof setTimeout === 'undefined' })
    }
  }
})
"#,
        r#"<view><text>{{ requestDenied }}</text><text>{{ timerDenied }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let mut instance = ComponentInstance::new(package).expect("create vm");

    let outcome = instance
        .mount(ComponentInput::new("searchDrinks"))
        .expect("mount");

    assert_eq!(outcome.state.get("requestDenied"), Some(&json!(true)));
    assert_eq!(outcome.state.get("timerDenied"), Some(&json!(true)));
}

#[test]
fn dynamic_component_request_uses_broker_and_redacts_response_headers() {
    let root = write_component(
        "dynamic-request",
        r#"
Component({
  data: { status: '', safeHeader: '', leaked: false, failed: false, events: [] },
  lifetimes: {
    async created() {
      const events = []
      try {
        const response = await wx.request({
          url: 'https://merchant.example/status',
          method: 'POST',
          data: { orderId: 'order_demo_001' },
          header: { 'x-client': 'component' },
          success(payload) {
            events.push('success:' + payload.errMsg)
            throw new Error('callback error must be redacted')
          },
          complete(payload) {
            events.push('complete:' + payload.errMsg)
          }
        })
        events.push('resolved:' + response.errMsg)
        this.setData({
          status: response.data.status,
          safeHeader: response.header['x-safe'],
          leaked: Boolean(response.header.Authorization || response.header['x-token-id'] || response.header['authentication-info']),
          events
        })
      } catch (_) {
        this.setData({ failed: true })
      }
    }
  }
})
"#,
        r#"<view><text>{{ status }}</text><text>{{ safeHeader }}</text><text>{{ leaked }}</text><text>{{ failed }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let broker = MockRequestBroker::new();
    let config = ComponentVmConfig {
        dynamic: DynamicComponentConfig::default().with_request_broker(broker.clone()),
        ..ComponentVmConfig::default()
    };
    let mut input = ComponentInput::new("status");
    input.component_metadata.dynamic = true;
    let mut instance = ComponentInstance::with_config(package, config).expect("create vm");

    let outcome = instance.mount(input).expect("mount");

    assert_eq!(outcome.state.get("status"), Some(&json!("fresh")));
    assert_eq!(outcome.state.get("safeHeader"), Some(&json!("ok")));
    assert_eq!(outcome.state.get("leaked"), Some(&json!(false)));
    assert_eq!(outcome.state.get("failed"), Some(&json!(false)));
    assert_eq!(
        outcome.state.get("events"),
        Some(&json!([
            "success:request:ok",
            "complete:request:ok",
            "resolved:request:ok"
        ]))
    );
    assert_eq!(broker.calls().len(), 1);
    assert_eq!(broker.calls()[0].url, "https://merchant.example/status");
    assert_eq!(
        broker.calls()[0]
            .headers
            .get("x-client")
            .map(String::as_str),
        Some("component")
    );
}

#[test]
fn dynamic_request_rejects_host_owned_auth_headers() {
    let root = write_component(
        "dynamic-request-auth-deny",
        r#"
Component({
  data: { denied: false, reason: '' },
  lifetimes: {
    async created() {
      try {
        await wx.request({
          url: 'https://merchant.example/status',
          header: { Authorization: 'Bearer should-not-leave-js' }
        })
      } catch (error) {
        this.setData({ denied: error.code === 'permission_denied', reason: error.reason })
      }
    }
  }
})
"#,
        r#"<view><text>{{ denied }}</text><text>{{ reason }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let broker = MockRequestBroker::new();
    let config = ComponentVmConfig {
        dynamic: DynamicComponentConfig::default().with_request_broker(broker.clone()),
        ..ComponentVmConfig::default()
    };
    let mut input = ComponentInput::new("status");
    input.component_metadata.dynamic = true;
    let mut instance = ComponentInstance::with_config(package, config).expect("create vm");

    let outcome = instance.mount(input).expect("mount");

    assert_eq!(outcome.state.get("denied"), Some(&json!(true)));
    assert!(outcome
        .state
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .contains("Authorization"));
    assert!(broker.calls().is_empty());
}

#[test]
fn dynamic_timers_have_limit_clear_and_expire_cleanup() {
    let root = write_component(
        "dynamic-timers",
        r#"
Component({
  data: { count: 0, limitHit: false, detached: false },
    lifetimes: {
    attached() {
      const cleared = setTimeout(() => this.setData({ count: 99 }), 0)
      clearTimeout(cleared)
      setTimeout(() => this.setData({ count: this.data.count + 1 }), 0)
      setInterval(() => this.setData({ count: this.data.count + 1 }), 0)
      try {
        setTimeout(() => {}, 0)
        setTimeout(() => {}, 0)
        setTimeout(() => {}, 0)
      } catch (_) {
        this.setData({ limitHit: true })
      }
    },
    detached() {
      this.setData({ detached: true })
      try {
        setTimeout(() => this.setData({ count: 500 }), 0)
      } catch (_) {}
    }
  },
  methods: {
    noop() {}
  }
})
"#,
        r#"<view><text>{{ count }}</text><text>{{ limitHit }}</text><text>{{ detached }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let config = ComponentVmConfig {
        dynamic: DynamicComponentConfig::default().with_max_timers(2),
        ..ComponentVmConfig::default()
    };
    let mut input = ComponentInput::new("status");
    input.component_metadata.dynamic = true;
    let mut instance = ComponentInstance::with_config(package, config).expect("create vm");

    let mounted = instance.mount(input).expect("mount");
    assert_eq!(mounted.state.get("count"), Some(&json!(2)));
    assert_eq!(mounted.state.get("limitHit"), Some(&json!(true)));

    let flushed = instance
        .dispatch_event(&ComponentEvent::new(ComponentEventKind::Tap, "noop"))
        .expect("dispatch");
    assert_eq!(flushed.state.get("count"), Some(&json!(3)));

    let expired = instance.expire(json!({})).expect("expire");
    assert_eq!(expired.state.get("detached"), Some(&json!(true)));
    assert_eq!(expired.state.get("count"), Some(&json!(3)));
    assert!(matches!(
        instance.dispatch_event(&ComponentEvent::new(ComponentEventKind::Tap, "noop")),
        Err(ComponentVmError::Expired)
    ));
}

#[test]
fn component_vm_device_info_and_app_info_match_shared_defaults() {
    let root = write_component(
        "device-info",
        r#"
Component({
  data: { platform: '', model: '', sdk: '', version: '', leaked: false },
  lifetimes: {
    created() {
      const device = wx.getDeviceInfo()
      const app = wx.getAppBaseInfo()
      this.setData({
        platform: device.platform,
        model: device.model,
        sdk: app.SDKVersion,
        version: app.version,
        leaked: JSON.stringify({ device, app }).includes('deviceId') ||
          JSON.stringify({ device, app }).includes('localIp') ||
          JSON.stringify({ device, app }).includes('credential')
      })
    }
  }
})
"#,
        r#"<view><text>{{ platform }}</text><text>{{ model }}</text><text>{{ sdk }}</text><text>{{ version }}</text><text>{{ leaked }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let mut instance = ComponentInstance::new(package).expect("create vm");

    let outcome = instance
        .mount(ComponentInput::new("searchDrinks"))
        .expect("mount");

    assert_eq!(
        outcome.state.get("platform"),
        Some(&json!("anp-miniapp-dock"))
    );
    assert_eq!(outcome.state.get("model"), Some(&json!("host-runtime")));
    assert_eq!(outcome.state.get("sdk"), Some(&json!("0.1.0")));
    assert_eq!(outcome.state.get("version"), Some(&json!("0.1.0")));
    assert_eq!(outcome.state.get("leaked"), Some(&json!(false)));
}

#[test]
fn properties_are_available_to_component_instance() {
    let root = write_component(
        "properties",
        r#"
Component({
  properties: { label: String },
  data: { copied: '' },
  lifetimes: {
    created() {
      this.setData({ copied: this.properties.label })
    }
  }
})
"#,
        r#"<view><text>{{ label }}</text><text>{{ copied }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let mut input = ComponentInput::new("searchDrinks");
    input
        .properties
        .insert("label".to_owned(), json!("Featured"));
    let mut instance = ComponentInstance::new(package).expect("create vm");

    let outcome = instance.mount(input).expect("mount");

    assert_eq!(outcome.state.get("copied"), Some(&json!("Featured")));
    assert_eq!(
        outcome.render.root.children[0].text.as_deref(),
        Some("Featured")
    );
    assert_eq!(
        outcome.render.root.children[1].text.as_deref(),
        Some("Featured")
    );
}

#[test]
fn component_metadata_is_runtime_visible_without_entering_js_state() {
    let root = write_component(
        "metadata",
        r#"
Component({
  data: { leaked: false },
  lifetimes: {
    created() {
      this.setData({ leaked: Boolean(this.data.componentMetadata) })
    }
  }
})
"#,
        r#"<view><text>{{ leaked }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let mut input = ComponentInput::new("searchDrinks");
    input.component_metadata = ComponentMetadata {
        component_path: Some("components/drink-list/index".to_owned()),
        related_page: Some(json!({
            "path": "pages/drink/detail",
            "query": { "source": "card" }
        })),
        dynamic: true,
        scope_dynamic: Some(json!({ "desc": "刷新饮品库存" })),
        expirable: true,
        expired_text: Some("饮品列表已过期，请重新搜索".to_owned()),
    };
    let mut instance = ComponentInstance::new(package).expect("create vm");

    let outcome = instance.mount(input).expect("mount");

    assert_eq!(
        outcome.metadata.component_path.as_deref(),
        Some("components/drink-list/index")
    );
    assert_eq!(
        outcome
            .metadata
            .related_page
            .as_ref()
            .and_then(|page| page.get("path"))
            .and_then(serde_json::Value::as_str),
        Some("pages/drink/detail")
    );
    assert!(outcome.metadata.dynamic);
    assert!(outcome.metadata.expirable);
    assert_eq!(
        outcome.metadata.expired_text.as_deref(),
        Some("饮品列表已过期，请重新搜索")
    );
    assert_eq!(outcome.state.get("componentMetadata"), None);
    assert_eq!(outcome.state.get("leaked"), Some(&json!(false)));
}

#[test]
fn image_load_and_error_events_dispatch_to_methods() {
    let root = write_component(
        "image-events",
        r#"
Component({
  data: { imageState: 'pending' },
  methods: {
    onLoad() {
      this.setData({ imageState: 'loaded' })
    },
    onError() {
      this.setData({ imageState: 'failed' })
    }
  }
})
"#,
        r#"<view><image src="/latte.png" bindload="onLoad" binderror="onError" /><text>{{ imageState }}</text></view>"#,
    );
    let package = ComponentPackage::load(root).expect("load component");
    let mut instance = ComponentInstance::new(package).expect("create vm");
    let mounted = instance
        .mount(ComponentInput::new("searchDrinks"))
        .expect("mount");
    let image = mounted.render.root.children[0].clone();

    let load = image
        .events
        .iter()
        .find(|event| event.method == "onLoad")
        .map(ComponentEvent::from_binding)
        .expect("load binding");
    let loaded = instance.dispatch_event(&load).expect("dispatch load");
    assert_eq!(loaded.state.get("imageState"), Some(&json!("loaded")));

    let error = image
        .events
        .iter()
        .find(|event| event.method == "onError")
        .map(ComponentEvent::from_binding)
        .expect("error binding");
    let failed = instance.dispatch_event(&error).expect("dispatch error");
    assert_eq!(failed.state.get("imageState"), Some(&json!("failed")));
}

#[test]
fn coffee_components_run_result_notifications_and_emit_actions() {
    let drink_package =
        ComponentPackage::load(repo_root().join("examples/coffee-skill/components/drink-list"))
            .expect("load drink-list");
    let mut drink = ComponentInstance::new(drink_package).expect("create drink vm");
    let drink_render = drink
        .mount(ComponentInput {
            api_name: "searchDrinks".to_owned(),
            arguments: json!({"query": "latte"}),
            properties: Map::new(),
            content: vec![json!({"type": "text", "text": "Found drinks"})],
            structured_content: Some(Map::from_iter([(
                "drinks".to_owned(),
                json!([{ "id": "latte", "name": "Latte", "price": 18, "image": "/latte.png" }]),
            )])),
            meta: None,
            component_metadata: ComponentMetadata::default(),
        })
        .expect("mount drink-list")
        .render;
    let button = drink_render
        .root
        .children
        .iter()
        .find(|node| node.kind == RenderNodeKind::ScrollView)
        .and_then(|scroll| scroll.children.first())
        .and_then(|item| {
            item.children
                .iter()
                .find(|node| node.kind == RenderNodeKind::Button)
        })
        .expect("confirm button");
    let action = drink
        .dispatch_event(&ComponentEvent::from_binding(&button.events[0]))
        .expect("dispatch drink action");
    assert!(matches!(
        &action.actions[1],
        ComponentVmAction::ApiCall { name, arguments }
            if name == "confirmOrder" && arguments.get("drinkId") == Some(&json!("latte"))
    ));

    let order_package =
        ComponentPackage::load(repo_root().join("examples/coffee-skill/components/order-confirm"))
            .expect("load order-confirm");
    let mut order = ComponentInstance::new(order_package).expect("create order vm");
    let order_render = order
        .mount(ComponentInput {
            api_name: "confirmOrder".to_owned(),
            arguments: json!({"drinkId": "latte"}),
            properties: Map::new(),
            content: vec![json!({"type": "text", "text": "Confirm"})],
            structured_content: Some(Map::from_iter([
                ("orderId".to_owned(), json!("order_demo_001")),
                ("drinkId".to_owned(), json!("latte")),
                ("payable".to_owned(), json!(18)),
            ])),
            meta: None,
            component_metadata: ComponentMetadata::default(),
        })
        .expect("mount order-confirm")
        .render;
    let pay_button = order_render
        .root
        .children
        .iter()
        .find(|node| node.kind == RenderNodeKind::Button)
        .expect("pay button");
    let pay = order
        .dispatch_event(&ComponentEvent::from_binding(&pay_button.events[0]))
        .expect("dispatch pay action");
    assert!(matches!(
        &pay.actions[1],
        ComponentVmAction::ApiCall { name, arguments }
            if name == "payOrder" && arguments.get("orderId") == Some(&json!("order_demo_001"))
    ));

    let payment_package =
        ComponentPackage::load(repo_root().join("examples/coffee-skill/components/payment-result"))
            .expect("load payment-result");
    let mut payment = ComponentInstance::new(payment_package).expect("create payment vm");
    let payment_result = payment
        .mount(ComponentInput {
            api_name: "payOrder".to_owned(),
            arguments: json!({"orderId": "order_demo_001"}),
            properties: Map::new(),
            content: vec![json!({"type": "text", "text": "Paid"})],
            structured_content: Some(Map::from_iter([
                ("orderId".to_owned(), json!("order_demo_001")),
                ("status".to_owned(), json!("paid")),
            ])),
            meta: None,
            component_metadata: ComponentMetadata::default(),
        })
        .expect("mount payment-result");
    assert!(matches!(
        &payment_result.actions[0],
        ComponentVmAction::ExpirePreviousCards {
            component_paths,
            match_policy
        } if component_paths == &vec!["components/order-confirm/index".to_owned()]
            && match_policy.as_deref() == Some("latest")
    ));
}
