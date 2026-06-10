use demo_server::{app, DemoState};
use dock_cli::{run_with_writer, Cli};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under crates/dock-cli")
        .to_path_buf()
}

fn skill_root() -> PathBuf {
    repo_root().join("examples/coffee-skill")
}

async fn spawn_server() -> String {
    let state = DemoState::new(skill_root());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind demo server");
    let addr = listener.local_addr().expect("demo server addr");
    tokio::spawn(async move {
        axum::serve(listener, app(state))
            .await
            .expect("demo server runs");
    });
    format!("http://{addr}")
}

fn cli_json(args: impl IntoIterator<Item = String>) -> Value {
    let cli = Cli::try_parse_from_args(args).expect("CLI args parse");
    let mut output = Vec::new();
    run_with_writer(cli, &mut output).expect("CLI command succeeds");
    serde_json::from_slice(&output).expect("CLI prints JSON")
}

fn cli_json_result(args: impl IntoIterator<Item = String>) -> Result<Value, String> {
    let cli = Cli::try_parse_from_args(args).map_err(|error| error.to_string())?;
    let mut output = Vec::new();
    run_with_writer(cli, &mut output)
        .map_err(|error| error.to_string())
        .and_then(|_| serde_json::from_slice(&output).map_err(|error| error.to_string()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dock_cli_runs_coffee_order_flow_end_to_end() {
    let server = spawn_server().await;
    let skill = skill_root().display().to_string();

    let validate = cli_json(["dock-cli".to_owned(), "validate".to_owned(), skill.clone()]);
    assert_eq!(validate["status"], "ok");
    assert!(validate["apis"]
        .as_array()
        .expect("apis array")
        .iter()
        .any(|api| api == "payOrder"));

    let call = cli_json([
        "dock-cli".to_owned(),
        "call-api".to_owned(),
        skill.clone(),
        "searchDrinks".to_owned(),
        "{}".to_owned(),
    ]);
    assert_eq!(call["status"], "ok");
    assert_eq!(
        call["result"]["structuredContent"]["drinks"][0]["id"],
        "latte"
    );
    assert_eq!(call["render"]["renderer"], "component-runtime");
    assert!(call["modelVisible"].get("_meta").is_none());

    let component = cli_json([
        "dock-cli".to_owned(),
        "preview-component".to_owned(),
        skill.clone(),
        "components/drink-list/index".to_owned(),
        json!({
            "apiName": "searchDrinks",
            "structuredContent": {
                "drinks": [
                    { "id": "latte", "name": "Latte", "price": 18 }
                ]
            }
        })
        .to_string(),
    ]);
    assert_eq!(component["status"], "ok");
    assert_eq!(component["render"]["root"]["kind"], "view");

    let card = cli_json([
        "dock-cli".to_owned(),
        "preview-card".to_owned(),
        r#"{"content":[{"type":"text","text":"paid"}],"structuredContent":{"orderId":"order_demo_001","status":"paid"}}"#.to_owned(),
    ]);
    assert_eq!(card["card"]["version"], "card-spec/v0");

    let demo = cli_json([
        "dock-cli".to_owned(),
        "run-demo".to_owned(),
        "--skill".to_owned(),
        skill,
        "--server".to_owned(),
        server,
    ]);
    assert_eq!(demo["status"], "ok");
    assert_eq!(demo["server"]["auth"]["tokenReceived"], true);
    assert_eq!(demo["server"]["auth"]["capabilityToken"], "[REDACTED]");
    assert_eq!(demo["server"]["business"]["firstDrinkId"], "latte");
    assert_eq!(demo["server"]["business"]["paymentStatus"], "paid");
    assert_eq!(demo["flow"][0]["name"], "searchDrinks");
    assert_eq!(demo["flow"][1]["name"], "confirmOrder");
    assert_eq!(demo["flow"][2]["name"], "payOrder");
    assert_eq!(demo["flow"][2]["structuredContent"]["status"], "paid");
    assert_eq!(demo["flow"][3]["name"], "expire");
    assert_eq!(
        demo["componentActions"]["drinkList"]["name"],
        "confirmOrder"
    );
    assert_eq!(demo["componentActions"]["orderConfirm"]["name"], "payOrder");

    let rendered = demo.to_string();
    assert!(!rendered.contains("demo-token"));
    assert!(!rendered.contains("capability_"));
}

#[test]
fn call_api_reports_schema_errors_without_running_runtime() {
    let error = cli_json_result([
        "dock-cli".to_owned(),
        "call-api".to_owned(),
        skill_root().display().to_string(),
        "confirmOrder".to_owned(),
        "{}".to_owned(),
    ])
    .expect_err("missing drinkId should fail inputSchema");

    assert!(error.contains("validation_failed"));
}

#[test]
fn preview_card_falls_back_for_error_result() {
    let card = cli_json([
        "dock-cli".to_owned(),
        "preview-card".to_owned(),
        r#"{"isError":true,"content":[{"type":"text","text":"expired"}]}"#.to_owned(),
    ]);

    assert_eq!(card["status"], "ok");
    assert_eq!(card["card"]["status"], "error");
    assert_eq!(card["card"]["fallbackReason"], "api_error");
}
