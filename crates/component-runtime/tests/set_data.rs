use component_runtime::{ComponentEvent, ComponentInput, ComponentInstance, ComponentPackage};
use serde_json::json;
use std::fs;

#[test]
fn set_data_supports_dotted_paths() {
    let root =
        std::env::temp_dir().join(format!("anp-miniapp-dock-set-data-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("component temp dir");
    fs::write(
        root.join("index.js"),
        r#"
Component({
  data: { order: { status: 'new' } },
  methods: {
    pay() {
      this.setData({ 'order.status': 'paid' })
    }
  }
})
"#,
    )
    .expect("write index.js");
    fs::write(
        root.join("index.wxml"),
        r#"<view><button bindtap="pay">Pay</button><text>{{ order.status }}</text></view>"#,
    )
    .expect("write index.wxml");

    let package = ComponentPackage::load(root).expect("load component");
    let mut instance = ComponentInstance::new(package).expect("create vm");
    instance
        .mount(ComponentInput::new("confirmOrder"))
        .expect("mount");
    let button = instance.render().expect("render").root.children.remove(0);
    let event = ComponentEvent::from_binding(&button.events[0]);

    let outcome = instance.dispatch_event(&event).expect("dispatch");

    assert_eq!(outcome.state.pointer("/order/status"), Some(&json!("paid")));
    assert_eq!(
        outcome.render.root.children[1].text.as_deref(),
        Some("paid")
    );
}
