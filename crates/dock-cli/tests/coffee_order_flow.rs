use anp::authentication::{create_did_wba_document, DidDocumentOptions};
use demo_server::auth::ServerAuthConfig;
use demo_server::{app, DemoState};
use dock_cli::{run_with_writer, Cli};
use serde_json::{json, Value};
use std::fs;
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

fn fixture_skill_root(name: &str) -> PathBuf {
    repo_root().join("examples/fixtures").join(name)
}

fn read_json_file(path: impl AsRef<Path>) -> Value {
    let content = fs::read_to_string(path.as_ref()).expect("json file reads");
    serde_json::from_str(&content).expect("json file parses")
}

async fn spawn_server(fixture: &DidFixture) -> String {
    let auth_config = ServerAuthConfig::for_tests()
        .with_trusted_did_document(fixture.did(), fixture.did_path.clone());
    let state = DemoState::with_auth_config(skill_root(), auth_config);
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

fn runtime_ipc_request(method: &str, request_id: &str, params: Value) -> String {
    json!({
        "apiVersion": "dock.runtime.v1",
        "requestId": request_id,
        "method": method,
        "params": params
    })
    .to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dock_cli_runs_coffee_order_flow_end_to_end() {
    let fixture = DidFixture::new();
    let server = spawn_server(&fixture).await;
    let skill = skill_root().display().to_string();

    let validate = cli_json(["dock-cli".to_owned(), "validate".to_owned(), skill.clone()]);
    assert_eq!(validate["schemaVersion"], "dock.validate-report.v1");
    assert_eq!(validate["status"], "warning");
    assert_eq!(validate["commandStatus"], "ok");
    assert_eq!(validate["reportStatus"], "warning");
    assert_eq!(validate["compatibilityLevel"], "demo-only");
    assert_eq!(
        validate["compatibilityReport"]["status"], "warning",
        "validate should expose a machine-readable compatibility report"
    );
    assert_eq!(
        validate["compatibilityReport"]["schemaVersion"],
        "dock.validate-report.v1"
    );
    assert!(validate["compatibilityReport"]["apis"]
        .as_array()
        .expect("api reports")
        .iter()
        .any(|api| api["name"] == "payOrder"
            && api["registered"] == true
            && api["compatibilityStatus"] == "demo-only"
            && api["consentRequired"] == true));
    assert!(validate["compatibilityReport"]["components"]
        .as_array()
        .expect("component reports")
        .iter()
        .any(
            |component| component["path"] == "components/payment-result/index"
                && component["loaded"] == true
                && component["runtimeMetadata"]["dynamic"] == false
                && component["runtimeMetadata"]["expirable"] == false
        ));
    assert!(validate["compatibilityReport"]["risks"]
        .as_array()
        .expect("risk reports")
        .iter()
        .any(|risk| risk["api"] == "payOrder" && risk["consentRequired"] == true));
    assert!(validate["compatibilityReport"]["releaseBlockers"]
        .as_array()
        .expect("release blockers")
        .iter()
        .any(|blocker| blocker["code"] == "production_warning"));
    assert!(validate["releaseReadiness"]["checks"]
        .as_array()
        .expect("release readiness checks")
        .iter()
        .any(|check| check["code"] == "persistence_backends"
            && check["status"] == "not-evaluated-by-validate"));
    assert!(validate["repairSuggestions"]
        .as_array()
        .expect("repair suggestions")
        .iter()
        .any(|suggestion| suggestion["source"] == "releaseBlockers"
            && suggestion["severity"] == "blocker"));
    assert!(validate["apiNames"]
        .as_array()
        .expect("api names array")
        .iter()
        .any(|api| api == "payOrder"));
    assert!(!validate.to_string().contains("/home/"));

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
    assert_eq!(
        call["render"]["payload"]["render"]["schemaVersion"],
        "dock.render-ir.v1"
    );
    assert_eq!(
        call["render"]["payload"]["metadata"]["componentPath"],
        "components/drink-list/index"
    );
    assert_eq!(call["render"]["payload"]["metadata"]["dynamic"], false);
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
    assert_eq!(component["render"]["schemaVersion"], "dock.render-ir.v1");
    assert_eq!(
        component["metadata"]["componentPath"],
        "components/drink-list/index"
    );
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
        "--did-document".to_owned(),
        fixture.did_path.display().to_string(),
        "--private-key".to_owned(),
        fixture.key_path.display().to_string(),
        "--user-did".to_owned(),
        fixture.did(),
        "--agent-did".to_owned(),
        "did:wba:agent.example".to_owned(),
    ]);
    assert_eq!(demo["status"], "ok");
    assert_eq!(demo["server"]["auth"]["tokenReceived"], true);
    assert_eq!(demo["server"]["auth"]["capabilityToken"], "[REDACTED]");
    assert_eq!(
        demo["audit"][0]["userDid"],
        fixture.did(),
        "local runtime audit scope should match the signed DID credential"
    );
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
    assert!(!rendered.contains("Authorization"));
    assert!(!rendered.contains("Signature"));
    assert!(!rendered.contains("Signature-Input"));
    assert!(!rendered.contains(fixture.key_path.to_string_lossy().as_ref()));
    assert!(!rendered.contains(&fixture.key_material));
}

#[test]
fn inspect_coffee_skill_reports_package_graph() {
    let inspect = cli_json([
        "dock-cli".to_owned(),
        "inspect".to_owned(),
        skill_root().display().to_string(),
    ]);

    assert_eq!(inspect["schemaVersion"], "dock.inspect-report.v1");
    assert_eq!(inspect["commandStatus"], "ok");
    assert_eq!(inspect["skillId"], "coffee");
    assert_eq!(inspect["package"]["entry"], "index.js");
    assert!(inspect["files"]
        .as_array()
        .expect("files")
        .iter()
        .any(|file| file["path"] == "mcp.json" && file["kind"] == "file"));
    assert!(inspect["apis"]
        .as_array()
        .expect("apis")
        .iter()
        .any(|api| api["name"] == "payOrder"
            && api["registered"] == true
            && api["registrationStatus"] == "declared-and-registered"
            && api["risk"] == "payment"));
    assert!(inspect["components"]
        .as_array()
        .expect("components")
        .iter()
        .any(
            |component| component["path"] == "components/payment-result/index"
                && component["loaded"] == true
        ));
    assert!(inspect["wxApiUsage"]["items"]
        .as_array()
        .expect("wx usage")
        .iter()
        .any(|usage| usage["api"] == "wx.login" && usage["file"] == "index.js"));

    let rendered = inspect.to_string();
    assert!(!rendered.contains("/home/"));
    assert!(!rendered.contains("Authorization"));
    assert!(!rendered.contains("Signature"));
    assert!(!rendered.contains("capabilityToken"));
}

#[test]
fn test_skill_coffee_reports_fixture_passes() {
    let report = cli_json([
        "dock-cli".to_owned(),
        "test-skill".to_owned(),
        skill_root().display().to_string(),
    ]);

    assert_eq!(report["schemaVersion"], "dock.test-skill-report.v1");
    assert_eq!(report["status"], "ok");
    assert_eq!(report["commandStatus"], "ok");
    assert_eq!(report["fixtureSet"], "coffee");
    assert_eq!(report["summary"]["total"], 3);
    assert_eq!(report["summary"]["failed"], 0);
    assert_eq!(report["mockProvider"]["status"], "dev-only");
    assert_eq!(report["mockProvider"]["productionReady"], false);
    assert!(report["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .any(|case| case["name"] == "coffee.payOrder"
            && case["status"] == "pass"
            && case["expire"]["expired"] == true));

    let rendered = report.to_string();
    assert!(!rendered.contains("Authorization"));
    assert!(!rendered.contains("Signature"));
    assert!(!rendered.contains("capabilityToken"));
    assert!(!rendered.contains("/home/"));
}

#[test]
fn perf_smoke_reports_baselines_and_stress() {
    let report = cli_json([
        "dock-cli".to_owned(),
        "perf".to_owned(),
        skill_root().display().to_string(),
        "--iterations".to_owned(),
        "1".to_owned(),
    ]);

    assert_eq!(report["schemaVersion"], "dock.perf-baseline-report.v1");
    assert_eq!(report["status"], "ok");
    assert_eq!(report["commandStatus"], "ok");
    assert_eq!(report["mode"], "smoke");
    assert_eq!(report["baseline"]["iterationsPerCase"], 1);
    assert_eq!(report["stress"]["status"], "pass");
    assert!(report["samples"]
        .as_array()
        .expect("samples")
        .iter()
        .any(|sample| sample["category"] == "api_vm_call"));
    assert!(report["samples"]
        .as_array()
        .expect("samples")
        .iter()
        .any(|sample| sample["name"] == "stress.dynamic_timer_request"));
    assert!(report["samples"]
        .as_array()
        .expect("samples")
        .iter()
        .any(|sample| sample["name"] == "resource_limit.result_size_fail_closed"));

    let rendered = report.to_string();
    for forbidden in [
        "/home/",
        "/Users/",
        "Authorization",
        "Signature",
        "capabilityToken",
        "fixture-token",
        "Bearer ",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "perf report leaked forbidden marker {forbidden}"
        );
    }
}

#[test]
fn test_skill_dynamic_fixture_compares_snapshot() {
    let report = cli_json([
        "dock-cli".to_owned(),
        "test-skill".to_owned(),
        fixture_skill_root("dynamic-status").display().to_string(),
    ]);

    assert_eq!(report["schemaVersion"], "dock.test-skill-report.v1");
    assert_eq!(report["status"], "ok");
    assert_eq!(report["skillId"], "dynamic-status");
    assert_eq!(report["fixtureSet"], "dynamic-status");
    assert_eq!(report["summary"]["total"], 1);
    assert_eq!(report["cases"][0]["snapshotCompare"]["status"], "match");
    assert_eq!(
        report["cases"][0]["auditSummary"]["expected"]["boundary"],
        "dynamic-request-timer-gated"
    );
    assert_eq!(report["cases"][0]["component"]["metadata"]["dynamic"], true);
    assert_eq!(
        report["cases"][0]["auditSummary"]["events"][0]["skillId"],
        "dynamic-status"
    );

    let rendered = report.to_string();
    assert!(!rendered.contains("fixture-token"));
    assert!(!rendered.contains("Authorization"));
    assert!(!rendered.contains("Signature"));
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
fn ipc_runtime_json_call_uses_versioned_envelope_and_facade() {
    let skill = skill_root().display().to_string();
    let response = cli_json([
        "dock-cli".to_owned(),
        "runtime-json".to_owned(),
        skill,
        runtime_ipc_request(
            "runtime.callApi",
            "req-call-1",
            json!({
                "session": {
                    "userDid": "did:wba:user.example",
                    "agentDid": "did:wba:agent.example",
                    "merchantDid": "did:wba:coffee-merchant.example",
                    "skillId": "coffee",
                    "sessionId": "session-ipc"
                },
                "apiName": "searchDrinks",
                "arguments": { "query": "latte" },
                "capabilityToken": "capability-secret-token"
            }),
        ),
    ]);

    assert_eq!(response["apiVersion"], "dock.runtime.v1");
    assert_eq!(response["requestId"], "req-call-1");
    assert_eq!(response["method"], "runtime.callApi");
    assert_eq!(response["status"], "ok");
    assert_eq!(response["transport"]["mode"], "headless-cli-json");
    assert_eq!(response["transport"]["binding"], "local-process-stdio");
    assert_eq!(response["redaction"]["marker"], "[REDACTED]");
    assert_eq!(
        response["result"]["data"]["result"]["structuredContent"]["drinks"][0]["id"],
        "latte"
    );
    assert_eq!(
        response["result"]["data"]["render"]["renderer"],
        "component-runtime"
    );

    let rendered = response.to_string();
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("Authorization"));
    assert!(!rendered.contains("Signature"));
}

#[test]
fn ipc_runtime_json_rejects_version_and_method_with_redacted_errors() {
    let skill = skill_root().display().to_string();
    let version_error = cli_json([
        "dock-cli".to_owned(),
        "runtime-json".to_owned(),
        skill.clone(),
        json!({
            "apiVersion": "dock.runtime.v0",
            "requestId": "req-version",
            "method": "runtime.callApi",
            "params": {}
        })
        .to_string(),
    ]);
    assert_eq!(version_error["status"], "error");
    assert_eq!(version_error["error"]["code"], "unsupported_version");
    assert_eq!(version_error["requestId"], "req-version");

    let method_error = cli_json([
        "dock-cli".to_owned(),
        "runtime-json".to_owned(),
        skill,
        runtime_ipc_request(
            "runtime.unsupportedSecretMethod",
            "req-method",
            json!({
                "token": "capability-secret-token",
                "path": "/home/user/key-1-private.pem"
            }),
        ),
    ]);
    assert_eq!(method_error["status"], "error");
    assert_eq!(method_error["error"]["code"], "invalid_method");
    let rendered = method_error.to_string();
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("/home/user/key-1-private.pem"));
    assert!(rendered.contains("[REDACTED]") || rendered.contains("not supported"));
}

#[test]
fn ipc_runtime_json_redacts_invalid_params_errors() {
    let response = cli_json([
        "dock-cli".to_owned(),
        "runtime-json".to_owned(),
        skill_root().display().to_string(),
        runtime_ipc_request(
            "runtime.callApi",
            "req-invalid",
            json!({
                "session": {
                    "userDid": "did:wba:user.example",
                    "skillId": "coffee",
                    "sessionId": "session-ipc"
                },
                "apiName": 123,
                "arguments": {
                    "Authorization": "Bearer capability-secret-token",
                    "privateKey": "/home/user/key-1-private.pem"
                }
            }),
        ),
    ]);

    assert_eq!(response["status"], "error");
    assert_eq!(response["error"]["code"], "invalid_params");
    let rendered = response.to_string();
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("/home/user/key-1-private.pem"));
    assert!(rendered.contains("[REDACTED]"));
}

#[test]
fn ipc_runtime_json_parse_errors_use_redacted_envelope() {
    let response = cli_json([
        "dock-cli".to_owned(),
        "runtime-json".to_owned(),
        skill_root().display().to_string(),
        r#"{"apiVersion":1,"requestId":"req-parse","method":"runtime.callApi","params":{"capabilityToken":"capability-secret-token","privateKey":"/home/user/key-1-private.pem"}}"#.to_owned(),
    ]);

    assert_eq!(response["apiVersion"], "dock.runtime.v1");
    assert_eq!(response["method"], "runtime.parseRequest");
    assert_eq!(response["status"], "error");
    assert_eq!(response["error"]["code"], "invalid_params");
    assert_eq!(response["redaction"]["marker"], "[REDACTED]");
    assert_eq!(response["transport"]["binding"], "local-process-stdio");

    let rendered = response.to_string();
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("/home/user/key-1-private.pem"));
    assert!(!rendered.contains("Authorization"));
    assert!(!rendered.contains("Signature"));
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

#[test]
fn fixture_skills_validate_and_preview_component_snapshots() {
    for (fixture, api_name, component_path, input) in [
        (
            "address-form",
            "prepareAddressForm",
            "components/address-form/index",
            json!({
                "apiName": "prepareAddressForm",
                "arguments": { "addressHandle": "addr_handle_demo_001" },
                "structuredContent": {
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
                }
            }),
        ),
        (
            "media-review",
            "reviewMedia",
            "components/media-review/index",
            json!({
                "apiName": "reviewMedia",
                "structuredContent": {
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
                }
            }),
        ),
        (
            "location-map-preview",
            "prepareLocationMap",
            "components/location-map-preview/index",
            json!({
                "apiName": "prepareLocationMap",
                "structuredContent": {
                    "location": {
                        "region": "mock-region-downtown",
                        "locationToken": "location_handle_demo_001",
                        "providerStatus": "fail-closed",
                        "fallbackReason": "host_location_provider_required"
                    }
                }
            }),
        ),
    ] {
        let skill = fixture_skill_root(fixture).display().to_string();
        let validate = cli_json(["dock-cli".to_owned(), "validate".to_owned(), skill.clone()]);
        assert_eq!(validate["status"], "warning");
        assert_eq!(validate["commandStatus"], "ok");
        assert!(validate["compatibilityReport"]["apis"]
            .as_array()
            .expect("api reports")
            .iter()
            .any(|api| api["name"] == api_name && api["registered"] == true));
        assert!(validate["compatibilityReport"]["components"]
            .as_array()
            .expect("component reports")
            .iter()
            .any(|component| component["path"] == component_path && component["loaded"] == true));

        let preview = cli_json([
            "dock-cli".to_owned(),
            "preview-component".to_owned(),
            skill,
            component_path.to_owned(),
            input.to_string(),
        ]);
        assert_eq!(preview["status"], "ok");
        assert_eq!(preview["render"]["schemaVersion"], "dock.render-ir.v1");
        assert_eq!(preview["metadata"]["componentPath"], component_path);
        assert_eq!(preview["render"]["root"]["kind"], "view");
    }

    let dynamic_skill = fixture_skill_root("dynamic-status").display().to_string();
    let dynamic_validate = cli_json([
        "dock-cli".to_owned(),
        "validate".to_owned(),
        dynamic_skill.clone(),
    ]);
    assert_eq!(dynamic_validate["status"], "warning");
    assert_eq!(dynamic_validate["commandStatus"], "ok");
    assert!(
        dynamic_validate["compatibilityReport"]["permissions"]["dynamicComponents"]
            .as_array()
            .expect("dynamic components")
            .iter()
            .any(|component| component == "components/dynamic-status/index")
    );
    assert!(
        dynamic_validate["compatibilityReport"]["permissions"]["policy"]
            .as_str()
            .expect("policy string")
            .contains("Step 02-05")
    );

    let dynamic_preview = cli_json([
        "dock-cli".to_owned(),
        "preview-component".to_owned(),
        dynamic_skill,
        "components/dynamic-status/index".to_owned(),
        json!({
            "apiName": "refreshDynamicStatus",
            "structuredContent": {
                "orderId": "order_demo_001",
                "status": "pending"
            }
        })
        .to_string(),
    ]);
    assert_eq!(dynamic_preview["status"], "ok");
    assert_eq!(dynamic_preview["metadata"]["dynamic"], true);
    assert_eq!(
        dynamic_preview["render"]["root"]["children"][2]["text"],
        "request-denied"
    );

    let rendered = dynamic_preview.to_string();
    assert!(!rendered.contains("Authorization"));
    assert!(!rendered.contains("Signature"));
    assert!(!rendered.contains("private key"));
}

#[test]
fn example_compatibility_fixtures_validate_and_test_skill() {
    for fixture in [
        "address-form",
        "media-review",
        "dynamic-status",
        "location-map-preview",
    ] {
        let root = fixture_skill_root(fixture);
        let expected = read_json_file(root.join("expected-test-skill.json"));
        let readme = fs::read_to_string(root.join("README.md")).expect("fixture README reads");
        let skill = root.display().to_string();

        assert!(
            readme.contains("cargo run -p dock-cli -- validate"),
            "{fixture} README should include validate command"
        );
        assert!(
            readme.contains("cargo run -p dock-cli -- test-skill"),
            "{fixture} README should include test-skill command"
        );
        assert!(
            readme.contains("Risk Boundary") && readme.contains("mock-only"),
            "{fixture} README should explain mock-only risk boundary"
        );

        let validate = cli_json(["dock-cli".to_owned(), "validate".to_owned(), skill.clone()]);
        assert_eq!(validate["schemaVersion"], "dock.validate-report.v1");
        assert_eq!(validate["commandStatus"], "ok");
        assert_eq!(validate["skillId"], expected["skillId"]);
        assert_eq!(
            validate["compatibilityReport"]["skillId"],
            expected["skillId"]
        );
        assert!(
            validate["compatibilityReport"]["apis"]
                .as_array()
                .expect("api reports")
                .iter()
                .any(|api| api["name"] == expected["cases"][0]["apiName"]
                    && api["registered"] == true)
        );
        assert!(validate["compatibilityReport"]["components"]
            .as_array()
            .expect("component reports")
            .iter()
            .any(
                |component| component["path"] == expected["cases"][0]["componentPath"]
                    && component["loaded"] == true
            ));

        let report = cli_json(["dock-cli".to_owned(), "test-skill".to_owned(), skill]);
        assert_eq!(report["schemaVersion"], expected["schemaVersion"]);
        assert_eq!(report["status"], expected["status"]);
        assert_eq!(report["commandStatus"], expected["commandStatus"]);
        assert_eq!(report["skillId"], expected["skillId"]);
        assert_eq!(report["fixtureSet"], expected["fixtureSet"]);
        assert_eq!(
            report["mockProvider"]["productionReady"],
            expected["mockProvider"]["productionReady"]
        );
        assert_eq!(report["summary"]["total"], expected["summary"]["total"]);
        assert_eq!(report["summary"]["failed"], expected["summary"]["failed"]);
        assert_eq!(report["cases"][0]["name"], expected["cases"][0]["name"]);
        assert_eq!(report["cases"][0]["status"], expected["cases"][0]["status"]);
        assert_eq!(
            report["cases"][0]["api"]["apiName"],
            expected["cases"][0]["apiName"]
        );
        assert_eq!(
            report["cases"][0]["component"]["componentPath"],
            expected["cases"][0]["componentPath"]
        );
        assert_eq!(
            report["cases"][0]["snapshotCompare"]["status"],
            expected["cases"][0]["snapshotStatus"]
        );
        assert_eq!(
            report["cases"][0]["auditSummary"]["expected"]["boundary"],
            expected["cases"][0]["auditBoundary"]
        );
        assert!(
            repo_root()
                .join(
                    expected["cases"][0]["snapshot"]
                        .as_str()
                        .expect("snapshot path string")
                )
                .is_file(),
            "{fixture} snapshot should exist"
        );

        let rendered = report.to_string();
        for forbidden in [
            "/home/",
            "Authorization",
            "Signature",
            "capabilityToken",
            "fixture-token",
            "Bearer ",
            "private key material",
            "PEM",
            "latitude",
            "longitude",
        ] {
            assert!(
                !rendered.contains(forbidden),
                "{fixture} test-skill report leaked forbidden marker {forbidden}"
            );
        }
    }
}

struct DidFixture {
    _dir: TempDir,
    did_document: Value,
    did_path: PathBuf,
    key_path: PathBuf,
    key_material: String,
}

impl DidFixture {
    fn new() -> Self {
        let bundle = create_did_wba_document("user.example", DidDocumentOptions::default())
            .expect("DID fixture creates");
        let dir = TempDir::new("dock-cli-coffee-flow").expect("temp dir creates");
        let did_path = dir.path().join("did.json");
        let key_path = dir.path().join("key.pem");
        let key_material = bundle.keys["key-1"].private_key_pem.clone();
        std::fs::write(&did_path, serde_json::to_vec(&bundle.did_document).unwrap()).unwrap();
        std::fs::write(&key_path, &key_material).unwrap();
        set_private_key_permissions(&key_path);
        Self {
            _dir: dir,
            did_document: bundle.did_document,
            did_path,
            key_path,
            key_material,
        }
    }

    fn did(&self) -> String {
        self.did_document["id"]
            .as_str()
            .expect("fixture has DID")
            .to_owned()
    }
}

#[cfg(unix)]
fn set_private_key_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .expect("set key permissions");
}

#[cfg(not(unix))]
fn set_private_key_permissions(_path: &Path) {}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "{}-{}-{}",
            prefix,
            std::process::id(),
            unique_suffix()
        ));
        std::fs::create_dir(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn unique_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{nanos}-{counter}")
}
