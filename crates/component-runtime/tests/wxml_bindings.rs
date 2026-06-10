use component_runtime::{
    compile_component_to_render_ir, compile_wxml_to_render_ir, ComponentCompileError,
    ComponentPackage, RenderEventKind, RenderNodeKind,
};
use serde_json::json;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under crates/component-runtime")
        .to_path_buf()
}

fn component_root(name: &str) -> PathBuf {
    repo_root()
        .join("examples/coffee-skill/components")
        .join(name)
}

#[test]
fn compiles_drink_list_fixture_with_for_image_button_and_scroll_view() {
    let package = ComponentPackage::load(component_root("drink-list")).expect("component loads");
    let output = compile_component_to_render_ir(
        &package,
        &json!({
            "title": "Choose a drink",
            "empty": false,
            "drinks": [
                {"id": "latte", "name": "Latte", "price": "$4.50", "image": "https://img.example/latte.png"},
                {"id": "mocha", "name": "Mocha", "price": "$5.00", "image": "https://img.example/mocha.png"}
            ]
        }),
    )
    .expect("component compiles");

    assert_eq!(output.root.kind, RenderNodeKind::View);
    assert_eq!(output.root.style.padding.as_deref(), Some("12px"));
    assert_eq!(
        output.root.children[0].text.as_deref(),
        Some("Choose a drink")
    );

    let scroll = output
        .root
        .children
        .iter()
        .find(|node| node.kind == RenderNodeKind::ScrollView)
        .expect("scroll-view compiles");
    assert_eq!(scroll.props.get("scrollX"), Some(&json!(true)));

    let drink_items = scroll
        .children
        .iter()
        .filter(|node| node.kind == RenderNodeKind::View)
        .collect::<Vec<_>>();
    assert_eq!(drink_items.len(), 2);
    assert_eq!(drink_items[0].props.get("key"), Some(&json!("latte")));
    assert_eq!(drink_items[1].props.get("key"), Some(&json!("mocha")));
    assert_eq!(drink_items[0].children[1].text.as_deref(), Some("Latte"));

    let image = &drink_items[0].children[0];
    assert_eq!(image.kind, RenderNodeKind::Image);
    assert_eq!(
        image.props.get("src"),
        Some(&json!("https://img.example/latte.png"))
    );
    assert!(image
        .events
        .iter()
        .any(|event| event.event == RenderEventKind::ImageLoad));
    assert!(image
        .events
        .iter()
        .any(|event| event.event == RenderEventKind::ImageError));

    let button = drink_items[0]
        .children
        .iter()
        .find(|node| node.kind == RenderNodeKind::Button)
        .expect("button compiles");
    let tap = button
        .events
        .iter()
        .find(|event| event.event == RenderEventKind::Tap)
        .expect("button tap event exists");
    assert_eq!(tap.method, "confirmDrink");
    assert_eq!(tap.dataset.get("id"), Some(&json!("latte")));
}

#[test]
fn wx_if_false_omits_node() {
    let output = compile_wxml_to_render_ir(
        r#"<view><text wx:if="{{ visible }}">Shown</text><text>Always</text></view>"#,
        "",
        &json!({"visible": false}),
    )
    .expect("component compiles");

    assert_eq!(output.root.children.len(), 1);
    assert_eq!(output.root.children[0].text.as_deref(), Some("Always"));
}

#[test]
fn class_and_inline_style_are_merged_with_inline_precedence() {
    let output = compile_wxml_to_render_ir(
        r#"<view class="card" style="padding: 8px; opacity: 0.5"></view>"#,
        ".card { padding: 12px; background-color: #fff; }",
        &json!({}),
    )
    .expect("component compiles");

    assert_eq!(output.root.style.padding.as_deref(), Some("8px"));
    assert_eq!(output.root.style.background.as_deref(), Some("#fff"));
    assert_eq!(output.root.style.opacity.as_deref(), Some("0.5"));
}

#[test]
fn unsupported_expression_generates_warning() {
    let output = compile_wxml_to_render_ir(
        "<view><text>{{ price + tax }}</text></view>",
        ".card { transform: scale(1); }",
        &json!({"price": 1, "tax": 2}),
    )
    .expect("component compiles with warning");

    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("unsupported binding expression")));
}

#[test]
fn parse_failure_can_drive_fallback_reason() {
    let error = compile_wxml_to_render_ir("<view><text>bad</view>", "", &json!({}))
        .expect_err("invalid WXML fails");

    assert!(matches!(error, ComponentCompileError::Wxml(_)));
    assert!(error.to_string().contains("WXML parse failed"));
}

#[test]
fn component_loader_reads_optional_files() {
    let package = ComponentPackage::load(component_root("order-confirm")).expect("component loads");

    assert!(Path::new(&package.root).ends_with("order-confirm"));
    assert!(package
        .js
        .as_deref()
        .unwrap_or_default()
        .contains("Component"));
    assert!(package
        .json
        .as_deref()
        .unwrap_or_default()
        .contains("component"));
    assert!(package
        .wxss
        .as_deref()
        .unwrap_or_default()
        .contains(".primary"));
}
