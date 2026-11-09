use component_runtime::{
    compile_component_to_render_ir, compile_wxml_to_render_ir, ComponentCompileError,
    ComponentPackage, RenderEventKind, RenderNodeKind, RENDER_IR_SCHEMA_VERSION,
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

    assert_eq!(output.schema_version, RENDER_IR_SCHEMA_VERSION);
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
fn wx_elif_else_condition_chain_renders_first_matching_branch() {
    let output = compile_wxml_to_render_ir(
        r#"
        <view>
          <text wx:if="{{ status === 'ready' }}">Ready</text>
          <text wx:elif="{{ status === 'pending' }}">Pending</text>
          <text wx:else>Fallback</text>
        </view>
        "#,
        "",
        &json!({"status": "pending"}),
    )
    .expect("component compiles");

    assert_eq!(output.root.children.len(), 1);
    assert_eq!(output.root.children[0].text.as_deref(), Some("Pending"));
    assert!(output.warnings.is_empty());
}

#[test]
fn wx_else_branch_renders_when_previous_conditions_do_not_match() {
    let output = compile_wxml_to_render_ir(
        r#"
        <view>
          <text wx:if="{{ count === 1 }}">One</text>
          <text wx:elif="{{ count === 2 }}">Two</text>
          <text wx:else>Many</text>
        </view>
        "#,
        "",
        &json!({"count": 3}),
    )
    .expect("component compiles");

    assert_eq!(output.root.children.len(), 1);
    assert_eq!(output.root.children[0].text.as_deref(), Some("Many"));
    assert!(output.warnings.is_empty());
}

#[test]
fn wx_catchtap_is_distinct_render_event_but_maps_to_tap_runtime_event() {
    let output = compile_wxml_to_render_ir(
        r#"<view><button catchtap="stopTap" data-order-id="{{ order.id }}">Stop</button></view>"#,
        "",
        &json!({"order": {"id": "order_demo_001"}}),
    )
    .expect("component compiles");

    let event = output.root.children[0]
        .events
        .iter()
        .find(|event| event.event == RenderEventKind::CatchTap)
        .expect("catchtap event exists");
    assert_eq!(event.method, "stopTap");
    assert_eq!(
        event.dataset.get("order-id"),
        Some(&json!("order_demo_001"))
    );

    let component_event = component_runtime::ComponentEvent::from_binding(event);
    assert_eq!(
        component_event.kind,
        component_runtime::ComponentEventKind::Tap
    );
    assert_eq!(
        component_event.dataset.get("orderId"),
        Some(&json!("order_demo_001"))
    );
}

#[test]
fn wx_disabled_button_sets_prop_and_suppresses_tap_events() {
    let output = compile_wxml_to_render_ir(
        r#"<view><button disabled="{{ !canSubmit }}" bindtap="submit" catchtap="stop">Submit</button></view>"#,
        "",
        &json!({"canSubmit": false}),
    )
    .expect("component compiles");

    let button = &output.root.children[0];
    assert_eq!(button.props.get("disabled"), Some(&json!(true)));
    assert!(button.events.is_empty());
    assert!(output.warnings.is_empty());
}

#[test]
fn wx_simple_expression_allowlist_supports_literals_negation_and_equality() {
    let output = compile_wxml_to_render_ir(
        r#"
        <view>
          <text wx:if="{{ !hidden }}">Visible</text>
          <text wx:if="{{ amount === 450 }}">Amount</text>
          <text wx:if="{{ status !== 'closed' }}">Open</text>
          <text wx:if="{{ false }}">Never</text>
        </view>
        "#,
        "",
        &json!({"hidden": false, "amount": 450, "status": "ready"}),
    )
    .expect("component compiles");

    let text = output
        .root
        .children
        .iter()
        .filter_map(|node| node.text.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(text, vec!["Visible", "Amount", "Open"]);
    assert!(output.warnings.is_empty());
}

#[test]
fn wx_unsupported_condition_expression_warns_and_fails_closed() {
    let output = compile_wxml_to_render_ir(
        r#"<view><text wx:if="{{ isReady() }}">Unsafe</text><text>Safe</text></view>"#,
        "",
        &json!({"ready": true}),
    )
    .expect("component compiles with warning");

    assert_eq!(output.root.children.len(), 1);
    assert_eq!(output.root.children[0].text.as_deref(), Some("Safe"));
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("unsupported wx:if expression")));
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
fn wxss_p1_selectors_and_properties_map_to_render_style() {
    let output = compile_wxml_to_render_ir(
        r#"
        <view id="root" class="card">
          <view class="row"><text class="price">Price</text></view>
        </view>
        "#,
        r#"
        view { display: flex; }
        #root { gap: 8rpx; justify-content: center; align-items: stretch; }
        .card .price { min-width: 120rpx; max-width: 240rpx; box-shadow: 0 2rpx 6rpx #000; overflow-x: hidden; }
        "#,
        &json!({}),
    )
    .expect("component compiles");

    assert_eq!(output.root.style.display.as_deref(), Some("flex"));
    assert_eq!(output.root.style.gap.as_deref(), Some("8px"));
    assert_eq!(output.root.style.justify_content.as_deref(), Some("center"));
    assert_eq!(output.root.style.align_items.as_deref(), Some("stretch"));

    let price = &output.root.children[0].children[0];
    assert_eq!(price.style.min_width.as_deref(), Some("120px"));
    assert_eq!(price.style.max_width.as_deref(), Some("240px"));
    assert_eq!(price.style.box_shadow.as_deref(), Some("0 2px 6px #000"));
    assert_eq!(price.style.overflow_x.as_deref(), Some("hidden"));
    assert!(output.warnings.is_empty());
}

#[test]
fn wxss_unsupported_complex_selector_is_warning() {
    let output = compile_wxml_to_render_ir(
        r#"<view class="card primary"><text class="price">Price</text></view>"#,
        ".card.primary { color: red; } .card .price:hover { color: blue; }",
        &json!({}),
    )
    .expect("component compiles with warnings");

    assert_eq!(output.warnings.len(), 2);
    assert!(output
        .warnings
        .iter()
        .all(|warning| warning.contains("unsupported selector")));
}

#[test]
fn node_form_controls_compile_to_p1_render_nodes_and_safe_props() {
    let output = compile_wxml_to_render_ir(
        r#"
        <view>
          <input name="recipient" value="{{ form.name }}" placeholder="Name" type="text" maxlength="32" bindinput="onName" />
          <textarea name="note" value="{{ form.note }}" placeholder="Note" bindinput="onNote"></textarea>
          <radio name="size" value="small" checked="{{ selected === 'small' }}" bindchange="onSize">Small</radio>
          <checkbox name="extras" value="milk" checked bindchange="onExtra">Milk</checkbox>
          <picker name="time" range="{{ slots }}" value="{{ selectedIndex }}" bindchange="onTime"></picker>
        </view>
        "#,
        "",
        &json!({
            "form": {"name": "Ada", "note": "Leave at desk"},
            "selected": "small",
            "slots": ["09:00", "10:00"],
            "selectedIndex": 1
        }),
    )
    .expect("component compiles");

    let input = &output.root.children[0];
    assert_eq!(input.kind, RenderNodeKind::Input);
    assert_eq!(input.props.get("name"), Some(&json!("recipient")));
    assert_eq!(input.props.get("value"), Some(&json!("Ada")));
    assert_eq!(input.props.get("placeholder"), Some(&json!("Name")));
    assert_eq!(input.props.get("inputType"), Some(&json!("text")));
    assert_eq!(input.props.get("maxLength"), Some(&json!(32)));
    assert_eq!(input.events[0].event, RenderEventKind::Input);

    let textarea = &output.root.children[1];
    assert_eq!(textarea.kind, RenderNodeKind::Textarea);
    assert_eq!(textarea.props.get("value"), Some(&json!("Leave at desk")));
    assert_eq!(textarea.events[0].event, RenderEventKind::Input);

    let radio = &output.root.children[2];
    assert_eq!(radio.kind, RenderNodeKind::Radio);
    assert_eq!(radio.props.get("checked"), Some(&json!(true)));
    assert_eq!(radio.events[0].event, RenderEventKind::Change);

    let checkbox = &output.root.children[3];
    assert_eq!(checkbox.kind, RenderNodeKind::Checkbox);
    assert_eq!(checkbox.props.get("checked"), Some(&json!(true)));
    assert_eq!(checkbox.events[0].event, RenderEventKind::Change);

    let picker = &output.root.children[4];
    assert_eq!(picker.kind, RenderNodeKind::Picker);
    assert_eq!(
        picker.props.get("options"),
        Some(&json!(["09:00", "10:00"]))
    );
    assert_eq!(picker.props.get("value"), Some(&json!(1)));
    assert_eq!(picker.events[0].event, RenderEventKind::Change);
    assert!(output.warnings.is_empty());
}

#[test]
fn node_disabled_form_controls_suppress_events() {
    let output = compile_wxml_to_render_ir(
        r#"
        <view>
          <input disabled="{{ locked }}" bindinput="onInput" bindchange="onChange" />
          <picker disabled bindchange="onPick"></picker>
        </view>
        "#,
        "",
        &json!({"locked": true}),
    )
    .expect("component compiles");

    let input = &output.root.children[0];
    assert_eq!(input.kind, RenderNodeKind::Input);
    assert_eq!(input.props.get("disabled"), Some(&json!(true)));
    assert!(input.events.is_empty());

    let picker = &output.root.children[1];
    assert_eq!(picker.kind, RenderNodeKind::Picker);
    assert_eq!(picker.props.get("disabled"), Some(&json!(true)));
    assert!(picker.events.is_empty());
}

#[test]
fn node_static_map_and_canvas_compile_without_interactive_apis() {
    let output = compile_wxml_to_render_ir(
        r#"
        <view>
          <map id="store-map" region="{{ region }}" location-token="{{ locationToken }}" latitude="31.2" longitude="121.4" markers="{{ markers }}" />
          <canvas canvas-id="receipt" poster="{{ poster }}" script="drawSensitive()" bindtouchstart="onTouch" />
        </view>
        "#,
        "",
        &json!({
            "region": "mock-region-shanghai",
            "locationToken": "opaque-location-demo",
            "markers": [{"latitude": 31.2, "longitude": 121.4}],
            "poster": "https://img.example/receipt.png"
        }),
    )
    .expect("component compiles with warnings");

    let map = &output.root.children[0];
    assert_eq!(map.kind, RenderNodeKind::MapPreview);
    assert_eq!(map.props.get("id"), Some(&json!("store-map")));
    assert_eq!(
        map.props.get("region"),
        Some(&json!("mock-region-shanghai"))
    );
    assert_eq!(
        map.props.get("locationToken"),
        Some(&json!("opaque-location-demo"))
    );
    assert!(!map.props.contains_key("latitude"));
    assert!(!map.props.contains_key("longitude"));
    assert!(!map.props.contains_key("markers"));

    let canvas = &output.root.children[1];
    assert_eq!(canvas.kind, RenderNodeKind::CanvasStatic);
    assert_eq!(canvas.props.get("canvasId"), Some(&json!("receipt")));
    assert_eq!(
        canvas.props.get("poster"),
        Some(&json!("https://img.example/receipt.png"))
    );
    assert!(canvas.events.is_empty());

    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("unsupported `map-preview` attribute `latitude`")));
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("unsupported `map-preview` attribute `longitude`")));
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("unsupported `map-preview` attribute `markers`")));
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("unsupported `canvas-static` attribute `script`")));
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("unsupported `canvas-static` attribute `bindtouchstart`")));
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
