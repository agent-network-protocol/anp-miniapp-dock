use card_spec::{fallback_from_result, CardItem, CardStatus, FallbackReason};
use mcp_schema::{AtomicApiResult, TextContent};
use serde_json::{json, Map, Value};

#[test]
fn structured_content_generates_stable_order_card() {
    let result = AtomicApiResult {
        is_error: false,
        content: vec![TextContent::text("Confirm order")],
        structured_content: Some(map([
            ("drinkId", json!("latte")),
            ("orderId", json!("order_demo_001")),
            ("payable", json!(18)),
        ])),
        meta: Some(map([("privateComponentState", json!("hidden"))])),
        extra: Default::default(),
    };

    let card = fallback_from_result(&result, FallbackReason::ComponentRenderFailed);
    let card_json = serde_json::to_value(&card).expect("serialize card");
    let rendered = serde_json::to_string(&card_json).expect("stringify card");

    assert_eq!(card.status, CardStatus::Normal);
    assert_eq!(card.fallback_reason, "component_render_failed");
    assert!(!rendered.contains("privateComponentState"));
    assert_eq!(
        card_json,
        json!({
            "version": "card-spec/v0",
            "title": "Response",
            "status": "normal",
            "fallbackReason": "component_render_failed",
            "sections": [{
                "title": "Structured content",
                "items": [
                    { "type": "field", "label": "drinkId", "value": "latte" },
                    { "type": "field", "label": "orderId", "value": "order_demo_001" },
                    { "type": "field", "label": "payable", "value": 18 }
                ]
            }]
        })
    );
}

#[test]
fn error_result_uses_text_only_card() {
    let result = AtomicApiResult {
        is_error: true,
        content: vec![TextContent::text("Payment denied")],
        structured_content: Some(map([("orderId", json!("order_demo_001"))])),
        meta: None,
        extra: Default::default(),
    };

    let card = fallback_from_result(&result, FallbackReason::NoComponentPath);

    assert_eq!(card.status, CardStatus::Error);
    assert_eq!(card.fallback_reason, "api_error");
    assert_eq!(
        card.sections[0].items,
        vec![CardItem::text("Payment denied")]
    );
}

#[test]
fn empty_structured_content_has_explicit_reason() {
    let result = AtomicApiResult {
        is_error: false,
        content: Vec::new(),
        structured_content: Some(Map::new()),
        meta: None,
        extra: Default::default(),
    };

    let card = fallback_from_result(&result, FallbackReason::EmptyStructuredContent);

    assert_eq!(card.status, CardStatus::Empty);
    assert_eq!(card.fallback_reason, "empty_structured_content");
}

fn map<const N: usize>(pairs: [(&str, Value); N]) -> Map<String, Value> {
    pairs
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}
