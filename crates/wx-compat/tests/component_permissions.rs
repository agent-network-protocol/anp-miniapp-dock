use wx_compat::{
    unsupported_api, unsupported_api_registry, AppBaseInfo, Capability, CapabilityProfile,
    CardEvent, DeviceInfo, HostPermissionOverride, InMemoryCardEventSink, ModelContext,
    PermissionDecision, PermissionPolicyEngine, PermissionPolicyInput, PermissionReasonCode,
    RequestBroker, RuntimeProfile, UnsupportedApiKind, UnsupportedRequestBroker, WxEnvironmentKind,
    WxRequest, WxRequestError,
};

#[test]
fn atomic_api_profile_allows_request_but_broker_is_step_08_unsupported() {
    let broker = UnsupportedRequestBroker;
    let profile = CapabilityProfile::atomic_api();

    let error = broker
        .request(
            &profile,
            WxRequest::get("https://merchant.example.invalid/drinks"),
        )
        .expect_err("Step 07 broker should not perform network");

    assert!(matches!(error, WxRequestError::Unsupported(message) if message.contains("Step 08")));
}

#[test]
fn atomic_api_profile_does_not_treat_payment_as_real_capability() {
    let profile = CapabilityProfile::atomic_api();

    assert!(!profile.check(Capability::Payment).is_allowed());
}

#[test]
fn component_profile_denies_request_and_timer_by_default() {
    let broker = UnsupportedRequestBroker;
    let profile = CapabilityProfile::component();

    let error = broker
        .request(
            &profile,
            WxRequest::get("https://merchant.example.invalid/drinks"),
        )
        .expect_err("component request must be denied");

    assert!(matches!(error, WxRequestError::Denied(reason) if reason.contains("request")));
    assert!(!profile.check(Capability::Timer).is_allowed());
}

#[test]
fn dynamic_component_profile_can_enable_request_broker_boundary() {
    let profile = CapabilityProfile::component()
        .with_dynamic_component_request()
        .with_dynamic_component_timer();

    assert!(profile.check(Capability::Request).is_allowed());
    assert!(profile.check(Capability::Timer).is_allowed());
}

#[test]
fn permission_policy_denies_undeclared_sensitive_capability_by_default() {
    let decision = PermissionPolicyEngine.decide(PermissionPolicyInput::new(
        Capability::Payment,
        WxEnvironmentKind::AtomicApi,
    ));

    assert!(matches!(
        decision,
        PermissionDecision::Deny {
            reason_code: PermissionReasonCode::CapabilityNotDeclared,
            ..
        }
    ));
    let summary = decision.summary(Capability::Payment);
    assert_eq!(summary.decision, "deny");
    assert_eq!(summary.reason_code, "capability_not_declared");
}

#[test]
fn permission_policy_host_deny_override_wins_over_manifest() {
    let decision = PermissionPolicyEngine.decide(
        PermissionPolicyInput::new(Capability::Request, WxEnvironmentKind::AtomicApi)
            .with_manifest_declared(true)
            .with_host_override(HostPermissionOverride::Deny),
    );

    assert!(matches!(
        decision,
        PermissionDecision::Deny {
            reason_code: PermissionReasonCode::HostOverrideDeny,
            ..
        }
    ));
}

#[test]
fn permission_policy_host_allow_does_not_declare_sensitive_capability() {
    let decision = PermissionPolicyEngine.decide(
        PermissionPolicyInput::new(Capability::Payment, WxEnvironmentKind::AtomicApi)
            .with_host_override(HostPermissionOverride::Allow),
    );

    assert!(matches!(
        decision,
        PermissionDecision::Deny {
            reason_code: PermissionReasonCode::CapabilityNotDeclared,
            ..
        }
    ));
}

#[test]
fn permission_policy_mock_provider_is_dev_headless_only() {
    let production = PermissionPolicyEngine.decide(
        PermissionPolicyInput::new(Capability::Payment, WxEnvironmentKind::AtomicApi)
            .with_manifest_declared(true)
            .with_mock_provider(true),
    );
    assert!(matches!(
        production,
        PermissionDecision::Deny {
            reason_code: PermissionReasonCode::MockProductionDenied,
            ..
        }
    ));

    let headless = PermissionPolicyEngine.decide(
        PermissionPolicyInput::new(Capability::Payment, WxEnvironmentKind::AtomicApi)
            .with_manifest_declared(true)
            .with_runtime_profile(RuntimeProfile::Headless)
            .with_mock_provider(true),
    );
    assert!(matches!(
        headless,
        PermissionDecision::MockAllowed {
            reason_code: PermissionReasonCode::MockDevOnlyAllowed,
            dev_only: true,
            ..
        }
    ));
}

#[test]
fn component_request_requires_dynamic_scope_even_with_meta_permission() {
    let denied = PermissionPolicyEngine.decide(
        PermissionPolicyInput::new(Capability::Request, WxEnvironmentKind::Component)
            .with_meta_anp_declared(true),
    );
    assert!(matches!(
        denied,
        PermissionDecision::Deny {
            reason_code: PermissionReasonCode::CapabilityNotDeclared,
            ..
        }
    ));

    let allowed = PermissionPolicyEngine.decide(
        PermissionPolicyInput::new(Capability::Request, WxEnvironmentKind::Component)
            .with_dynamic_scope_declared(true),
    );
    assert!(allowed.is_allowed());
}

#[test]
fn permission_policy_reads_manifest_meta_anp_and_x_anp_values() {
    let from_manifest = PermissionPolicyEngine.decide(
        PermissionPolicyInput::new(Capability::Request, WxEnvironmentKind::AtomicApi)
            .with_manifest_permissions_value(&serde_json::json!({
                "capabilities": ["wx.request"]
            })),
    );
    assert!(from_manifest.is_allowed());

    let from_meta_anp = PermissionPolicyEngine.decide(
        PermissionPolicyInput::new(Capability::Payment, WxEnvironmentKind::AtomicApi)
            .with_meta_anp_value(&serde_json::json!({
                "permissions": {
                    "wx.requestPayment": true
                }
            })),
    );
    assert!(from_meta_anp.is_allowed());

    let from_x_anp = PermissionPolicyEngine.decide(
        PermissionPolicyInput::new(Capability::Login, WxEnvironmentKind::AtomicApi)
            .with_x_anp_value(&serde_json::json!({
                "scopes": ["wx.login"]
            })),
    );
    assert!(from_x_anp.is_allowed());
}

#[test]
fn permission_policy_reads_component_dynamic_scope_value() {
    let decision = PermissionPolicyEngine.decide(
        PermissionPolicyInput::new(Capability::Timer, WxEnvironmentKind::Component)
            .with_component_dynamic_scope_value(&serde_json::json!({
                "desc": "refresh status"
            })),
    );

    assert!(decision.is_allowed());
}

#[test]
fn model_context_records_card_expiration_events() {
    let context = ModelContext::new(
        "session-1",
        "coffee",
        "did:example:alice",
        "did:example:merchant",
    );
    let sink = InMemoryCardEventSink::new();

    context.expire_all_cards(
        &sink,
        ["components/drink-list/index"],
        Some("session".to_owned()),
    );
    context.expire_previous_cards(&sink, ["components/order-confirm/index"], None);

    assert_eq!(
        sink.events(),
        vec![
            CardEvent::ExpireAllCards {
                component_paths: vec!["components/drink-list/index".to_owned()],
                match_policy: Some("session".to_owned()),
            },
            CardEvent::ExpirePreviousCards {
                component_paths: vec!["components/order-confirm/index".to_owned()],
                match_policy: None,
            },
        ]
    );
}

#[test]
fn device_and_app_info_defaults_are_minimized() {
    let context = ModelContext::new(
        "session-1",
        "coffee",
        "did:example:alice",
        "did:example:merchant",
    );

    assert_eq!(
        context.get_device_info(),
        DeviceInfo {
            platform: "anp-miniapp-dock".to_owned(),
            model: "host-runtime".to_owned(),
            language: "en".to_owned(),
        }
    );
    assert_eq!(
        context.get_app_base_info(),
        AppBaseInfo {
            sdk_version: "0.1.0".to_owned(),
            version: "0.1.0".to_owned(),
        }
    );

    let device_json = serde_json::to_value(context.get_device_info()).expect("device json");
    for forbidden in [
        "deviceId",
        "device_id",
        "mac",
        "localIp",
        "local_ip",
        "advertisingId",
        "account",
        "credentialPath",
    ] {
        assert!(
            device_json.get(forbidden).is_none(),
            "{forbidden} must not be exposed in default device info"
        );
    }
}

#[test]
fn unsupported_wx_apis_have_explicit_fail_shape() {
    let photo_save = unsupported_api("saveImageToPhotosAlbum");

    assert_eq!(
        photo_save.get("errMsg").and_then(|value| value.as_str()),
        Some("saveImageToPhotosAlbum:fail unsupported")
    );
    assert_eq!(
        photo_save.get("code").and_then(|value| value.as_str()),
        Some("unsupported")
    );
    assert!(photo_save
        .get("reason")
        .and_then(|value| value.as_str())
        .is_some_and(|reason| !reason.contains("4111111111111111")));
    assert!(photo_save
        .get("suggestion")
        .and_then(|value| value.as_str())
        .is_some_and(|suggestion| suggestion.contains("explicit native save action")));
}

#[test]
fn unsupported_registry_marks_sync_and_async_apis() {
    let registry = unsupported_api_registry();

    assert!(registry.iter().any(|api| {
        api.name == "getNetworkType" && matches!(api.kind, UnsupportedApiKind::Async)
    }));
    assert!(!registry.iter().any(|api| api.name == "requestPayment"
        || api.name == "getPhoneNumber"
        || api.name == "chooseMedia"));
    assert!(registry.iter().any(|api| {
        api.name == "getAccountInfoSync" && matches!(api.kind, UnsupportedApiKind::Sync)
    }));
    assert!(registry
        .iter()
        .any(|api| api.name == "wx.cloud.callFunction"));
    assert!(!registry.iter().any(|api| api.name == "getDeviceInfo"));
    assert!(!registry.iter().any(|api| api.name == "getAppBaseInfo"));
}

#[test]
fn unknown_unsupported_api_uses_safe_fallback_shape() {
    let result = unsupported_api("../../../Authorization: Bearer secret-token");

    assert_eq!(
        result.get("errMsg").and_then(|value| value.as_str()),
        Some("unknownWxApi:fail unsupported")
    );
    assert_eq!(
        result.get("code").and_then(|value| value.as_str()),
        Some("unsupported")
    );
    let result_json = serde_json::Value::Object(result).to_string();
    assert!(!result_json.contains("secret-token"));
}
