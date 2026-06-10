use js_runtime_quickjs::{ApiCall, ApiVm, ApiVmError};
use serde_json::json;
use skill_loader::load_skill;

#[test]
fn registers_coffee_skill_apis() {
    let skill = load_skill("../../examples/coffee-skill").expect("load coffee skill");
    let vm = ApiVm::load_skill(skill).expect("load VM");
    let names: Vec<_> = vm
        .registered_apis()
        .iter()
        .map(|api| api.name.as_str())
        .collect();

    assert_eq!(names, ["searchDrinks", "confirmOrder", "payOrder"]);
}

#[test]
fn executes_async_coffee_api_result() {
    let skill = load_skill("../../examples/coffee-skill").expect("load coffee skill");
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new(
            "coffee",
            "session_001",
            "searchDrinks",
            json!({ "query": "latte" }),
        ))
        .expect("call searchDrinks");

    assert!(!result.is_error);
    assert_eq!(result.content[0].text, "Found drinks for latte");
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("drinks"))
            .and_then(|drinks| drinks.as_array())
            .map(Vec::len),
        Some(2)
    );
    assert!(result.meta.is_some());
}

#[test]
fn rejects_missing_api_call() {
    let skill = load_skill("../../examples/coffee-skill").expect("load coffee skill");
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let error = vm
        .call(ApiCall::new("coffee", "session_001", "missing", json!({})))
        .expect_err("missing API must fail");

    assert!(matches!(error, ApiVmError::MissingApi(name) if name == "missing"));
}
