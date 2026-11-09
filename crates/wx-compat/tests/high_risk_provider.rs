use serde_json::json;
use wx_compat::{
    call_high_risk_api_with_boundary, high_risk_api_spec, high_risk_audit_summary,
    high_risk_error_json, high_risk_success_json, redact_high_risk_value,
    DevOnlyHighRiskHostProvider, HighRiskApiRequest, HighRiskHostProvider,
    UnavailableHighRiskHostProvider,
};

#[test]
fn unavailable_provider_fails_closed_without_mock_data() {
    let spec = high_risk_api_spec("requestPayment").expect("payment spec");
    let provider = UnavailableHighRiskHostProvider;
    let request = HighRiskApiRequest::new(
        "requestPayment",
        json!({ "paymentPassword": "123456", "amount": 18 }),
    );

    let error = provider
        .call_high_risk_api(spec, &request)
        .expect_err("unconfigured provider must fail closed");
    let payload = high_risk_error_json(spec.name, error);
    let encoded = payload.to_string();

    assert_eq!(
        payload["errMsg"],
        "requestPayment:fail provider_unavailable"
    );
    assert_eq!(payload["code"], "provider_unavailable");
    assert!(!encoded.contains("123456"));
}

#[test]
fn dev_only_provider_returns_marked_opaque_handles() {
    let spec = high_risk_api_spec("chooseMedia").expect("media spec");
    let provider = DevOnlyHighRiskHostProvider::default();
    let request = HighRiskApiRequest::new(
        "chooseMedia",
        json!({ "sourcePath": "/Users/alice/private-photo.png" }),
    );

    let success = provider
        .call_high_risk_api(spec, &request)
        .expect("dev-only provider");
    let payload = high_risk_success_json(spec.name, success);
    let encoded = payload.to_string();

    assert_eq!(provider.calls(), 1);
    assert_eq!(payload["errMsg"], "chooseMedia:ok");
    assert_eq!(payload["devOnly"], true);
    assert_eq!(payload["mock"], true);
    assert_eq!(payload["tempFiles"][0]["fileHandle"], "mock-media-handle");
    assert!(payload["tempFiles"][0].get("path").is_none());
    assert!(!encoded.contains("/Users/alice"));
}

#[test]
fn consent_required_blocks_provider_before_execution() {
    let spec = high_risk_api_spec("requestPayment").expect("payment spec");
    let provider = DevOnlyHighRiskHostProvider::default();
    let request = HighRiskApiRequest::new(
        "requestPayment",
        json!({ "paymentPassword": "123456", "amount": 18 }),
    );

    let error = call_high_risk_api_with_boundary(&provider, spec, &request, false)
        .expect_err("consent must block provider");
    let payload = high_risk_error_json(spec.name, error);
    let encoded = payload.to_string();

    assert_eq!(provider.calls(), 0);
    assert_eq!(payload["errMsg"], "requestPayment:fail consent_required");
    assert_eq!(payload["code"], "consent_required");
    assert!(!encoded.contains("123456"));
}

#[test]
fn high_risk_audit_summary_is_redacted() {
    let spec = high_risk_api_spec("getPhoneNumber").expect("phone spec");
    let request = HighRiskApiRequest::new(
        "getPhoneNumber",
        json!({
            "phoneNumber": "1234567890",
            "address": "1 Private Road",
            "filePath": "/tmp/private.txt",
            "safeLabel": "coffee"
        }),
    );

    let summary = high_risk_audit_summary(spec, &request);

    assert_eq!(summary["apiName"], "getPhoneNumber");
    assert_eq!(summary["riskLevel"], "L4");
    assert_eq!(summary["parameterSummary"]["phoneNumber"], "[REDACTED]");
    assert_eq!(summary["parameterSummary"]["address"], "[REDACTED]");
    assert_eq!(summary["parameterSummary"]["filePath"], "[REDACTED]");
    assert_eq!(summary["parameterSummary"]["safeLabel"], "coffee");
}

#[test]
fn high_risk_redaction_catches_local_paths_in_nested_values() {
    let redacted = redact_high_risk_value(&json!({
        "files": [
            { "path": "/tmp/secret.txt", "name": "safe-name" }
        ],
        "metadata": {
            "authorization": "Bearer secret-token"
        }
    }));

    assert_eq!(redacted["files"][0]["path"], "[REDACTED]");
    assert_eq!(redacted["files"][0]["name"], "safe-name");
    assert_eq!(redacted["metadata"]["authorization"], "[REDACTED]");
}
