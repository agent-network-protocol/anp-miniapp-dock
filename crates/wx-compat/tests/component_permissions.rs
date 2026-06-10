use wx_compat::{
    unsupported_api, Capability, CapabilityProfile, CardEvent, InMemoryCardEventSink, ModelContext,
    RequestBroker, UnsupportedRequestBroker, WxRequest, WxRequestError,
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
    let profile = CapabilityProfile::component().with_dynamic_component_request();

    assert!(profile.check(Capability::Request).is_allowed());
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
fn unsupported_wx_apis_have_explicit_mock_shape() {
    let payment = unsupported_api("requestPayment");

    assert_eq!(
        payment.get("errMsg").and_then(|value| value.as_str()),
        Some("requestPayment:unsupported")
    );
}
