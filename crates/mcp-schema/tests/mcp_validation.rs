use mcp_schema::{
    validate_api_result, validate_input, validate_manifest, validate_manifest_with_component_paths,
    validate_output_warning, AtomicApiResult, SkillManifest, TextContent,
};
use serde_json::{json, Value};

fn valid_manifest_json() -> Value {
    json!({
        "apis": [
            {
                "name": "searchDrinks",
                "description": "搜索饮品",
                "_meta": {
                    "ui": {
                        "componentPath": "components/drink-list/index"
                    },
                    "anp": {
                        "merchantDid": "did:wba:example"
                    }
                },
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "饮品关键词"
                        }
                    },
                    "required": ["query"]
                },
                "outputSchema": {
                    "type": "object",
                    "properties": {
                        "drinks": {
                            "type": "array"
                        }
                    },
                    "required": ["drinks"]
                },
                "x_anp": {
                    "trace": true
                }
            }
        ],
        "components": [
            {
                "path": "components/drink-list/index",
                "permissions": {
                    "scope.dynamic": {
                        "desc": "刷新饮品库存"
                    }
                },
                "unknownComponentField": "kept"
            }
        ],
        "unknownRootField": {
            "kept": true
        }
    })
}

#[test]
fn validates_legal_coffee_manifest_and_keeps_unknown_fields() {
    let manifest: SkillManifest = serde_json::from_value(valid_manifest_json()).unwrap();

    let report = validate_manifest(&manifest);

    assert!(report.is_valid(), "{report:#?}");
    assert!(manifest.extra.contains_key("unknownRootField"));
    assert!(manifest.apis[0].extra.contains_key("x_anp"));
    assert!(manifest.components[0]
        .extra
        .contains_key("unknownComponentField"));
    assert_eq!(
        manifest.apis[0].component_path(),
        Some("components/drink-list/index")
    );
}

#[test]
fn rejects_duplicate_api_names() {
    let mut manifest: SkillManifest = serde_json::from_value(valid_manifest_json()).unwrap();
    let duplicate = manifest.apis[0].clone();
    manifest.apis.push(duplicate);

    let report = validate_manifest(&manifest);

    assert!(!report.is_valid());
    assert!(report
        .errors
        .iter()
        .any(|issue| issue.message.contains("duplicate API name")));
}

#[test]
fn rejects_non_object_input_schema() {
    let mut manifest: SkillManifest = serde_json::from_value(valid_manifest_json()).unwrap();
    manifest.apis[0].input_schema = json!({"type": "string"});

    let report = validate_manifest(&manifest);

    assert!(!report.is_valid());
    assert!(report
        .errors
        .iter()
        .any(|issue| issue.path == "apis[0].inputSchema"));
}

#[test]
fn accepts_empty_object_input_schema() {
    let mut manifest: SkillManifest = serde_json::from_value(valid_manifest_json()).unwrap();
    manifest.apis[0].input_schema = json!({});

    let report = validate_manifest(&manifest);
    assert!(report.is_valid(), "{report:#?}");

    let report = validate_input(&manifest.apis[0].input_schema, &json!({}));
    assert!(report.is_valid(), "{report:#?}");
}

#[test]
fn rejects_missing_component_path_reference() {
    let mut manifest: SkillManifest = serde_json::from_value(valid_manifest_json()).unwrap();
    manifest.components.clear();

    let report = validate_manifest(&manifest);

    assert!(!report.is_valid());
    assert!(report
        .errors
        .iter()
        .any(|issue| issue.path == "apis[0]._meta.ui.componentPath"));
}

#[test]
fn accepts_component_path_from_external_component_set() {
    let mut manifest: SkillManifest = serde_json::from_value(valid_manifest_json()).unwrap();
    manifest.components.clear();

    let report = validate_manifest_with_component_paths(&manifest, ["components/drink-list/index"]);

    assert!(report.is_valid(), "{report:#?}");
}

#[test]
fn validates_input_arguments_with_json_schema() {
    let manifest: SkillManifest = serde_json::from_value(valid_manifest_json()).unwrap();
    let report = validate_input(&manifest.apis[0].input_schema, &json!({"query": "latte"}));

    assert!(report.is_valid(), "{report:#?}");

    let report = validate_input(&manifest.apis[0].input_schema, &json!({}));
    assert!(!report.is_valid());
    assert!(report.errors.iter().any(|issue| issue.path == "arguments"));
}

#[test]
fn output_schema_mismatch_is_warning_only() {
    let manifest: SkillManifest = serde_json::from_value(valid_manifest_json()).unwrap();
    let report = validate_output_warning(
        manifest.apis[0].output_schema.as_ref().unwrap(),
        Some(&json!({"items": []})),
    );

    assert!(report.is_valid());
    assert!(!report.warnings.is_empty());
}

#[test]
fn error_result_does_not_require_structured_content() {
    let result = AtomicApiResult {
        is_error: true,
        content: vec![TextContent::text("订单已过期")],
        structured_content: None,
        meta: None,
        extra: Default::default(),
    };

    let report = validate_api_result(&result);

    assert!(report.is_valid(), "{report:#?}");
    assert!(!result.should_render_component());
}

#[test]
fn meta_is_not_model_visible() {
    let result: AtomicApiResult = serde_json::from_value(json!({
        "isError": false,
        "content": [{"type": "text", "text": "找到 1 杯拿铁"}],
        "structuredContent": {"count": 1},
        "_meta": {"internalImageUrl": "https://example.invalid/private.png"}
    }))
    .unwrap();

    let visible = serde_json::to_value(result.model_visible()).unwrap();

    assert!(visible.get("_meta").is_none());
    assert_eq!(visible["structuredContent"]["count"], 1);
}
