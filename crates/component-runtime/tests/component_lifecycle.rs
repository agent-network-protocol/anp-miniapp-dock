use component_runtime::{
    ComponentEvent, ComponentEventKind, ComponentInput, ComponentInstance, ComponentPackage,
    ComponentTraceKind, ComponentVmAction, ComponentVmError, RenderNodeKind,
};
use serde_json::{json, Map};
use std::fs;
use std::path::{Path, PathBuf};

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
      this.setData({
        ok: typeof fetch === 'undefined'
          && typeof WebSocket === 'undefined'
          && typeof setTimeout === 'undefined'
          && typeof Function === 'undefined'
          && typeof functionEscape === 'undefined'
          && typeof asyncEscape === 'undefined'
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
