use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UnsupportedApiKind {
    Async,
    Sync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsupportedApi {
    pub name: &'static str,
    pub kind: UnsupportedApiKind,
    pub reason: &'static str,
    pub suggestion: &'static str,
}

const DEFAULT_REASON: &str = "This wx API is not supported by anp-miniapp-dock production runtime.";
const DEFAULT_SUGGESTION: &str =
    "Use an ANP merchant Agent API through wx.request or a Host-provided capability.";

const HOST_PROVIDER_REASON: &str =
    "This wx API requires an explicit Host provider and is currently fail-closed.";
const UNSUPPORTED_BY_DESIGN_REASON: &str =
    "This wx API is outside the Agentic MiniApp Container production boundary.";

pub const UNSUPPORTED_WX_APIS: &[UnsupportedApi] = &[
    UnsupportedApi {
        name: "getAccountInfoSync",
        kind: UnsupportedApiKind::Sync,
        reason: "WeChat account identity is not exposed by this runtime.",
        suggestion: "Use ANP DID or Host registry metadata instead of WeChat account identifiers.",
    },
    UnsupportedApi {
        name: "getNetworkType",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use a Host network snapshot provider that returns minimized network state.",
    },
    UnsupportedApi {
        name: "onNetworkStatusChange",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use a Host listener provider or poll a merchant Agent API through wx.request.",
    },
    UnsupportedApi {
        name: "offNetworkStatusChange",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use a Host listener provider or avoid network-status listeners in headless runtime.",
    },
    UnsupportedApi {
        name: "getLocalIPAddress",
        kind: UnsupportedApiKind::Async,
        reason: "Local IP address is device and network private information.",
        suggestion: "Ask the Host for minimized network status instead of exposing local IP.",
    },
    UnsupportedApi {
        name: "authorize",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use the Host permission policy interface instead of WeChat settings APIs.",
    },
    UnsupportedApi {
        name: "getSetting",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use the Host permission policy interface instead of WeChat settings APIs.",
    },
    UnsupportedApi {
        name: "openSetting",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use the Host permission UI instead of WeChat settings APIs.",
    },
    UnsupportedApi {
        name: "saveImageToPhotosAlbum",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Let the Host expose an explicit native save action if the user requests it.",
    },
    UnsupportedApi {
        name: "verifyPaymentPassword",
        kind: UnsupportedApiKind::Async,
        reason: "Payment passwords must never be collected by the container.",
        suggestion: "Delegate payment verification to the regulated payment provider or Host flow.",
    },
    UnsupportedApi {
        name: "startFacialRecognitionVerify",
        kind: UnsupportedApiKind::Async,
        reason: "Biometric verification is outside the default container boundary.",
        suggestion: "Delegate biometric verification to a compliant Host/provider flow.",
    },
    UnsupportedApi {
        name: "startFacialRecognitionVerifyAndUploadVideo",
        kind: UnsupportedApiKind::Async,
        reason: "Biometric video upload is outside the default container boundary.",
        suggestion: "Delegate biometric verification to a compliant Host/provider flow.",
    },
    UnsupportedApi {
        name: "chooseInvoiceTitle",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Use a merchant Agent API or Host business capability for invoice data.",
    },
    UnsupportedApi {
        name: "chooseInvoice",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Use a merchant Agent API or Host business capability for invoice data.",
    },
    UnsupportedApi {
        name: "getWeRunData",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Do not request WeChat fitness data from this container.",
    },
    UnsupportedApi {
        name: "shareAppMessage",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use a Host share sheet triggered by an explicit user action.",
    },
    UnsupportedApi {
        name: "requestSubscribeMessage",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use a Host notification permission provider with consent and audit.",
    },
    UnsupportedApi {
        name: "uploadFile",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use a file broker with opaque handles and RequestBroker allowlist enforcement.",
    },
    UnsupportedApi {
        name: "downloadFile",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use a file broker with opaque handles and RequestBroker allowlist enforcement.",
    },
    UnsupportedApi {
        name: "openDocument",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use a Host document viewer with trusted URLs or opaque handles.",
    },
    UnsupportedApi {
        name: "getImageInfo",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use a Host media provider that reads only opaque image handles.",
    },
    UnsupportedApi {
        name: "openLocation",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use a Host map/deeplink provider with explicit user action.",
    },
    UnsupportedApi {
        name: "showToast",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use Host renderer affordances or model-visible status text.",
    },
    UnsupportedApi {
        name: "hideToast",
        kind: UnsupportedApiKind::Async,
        reason: HOST_PROVIDER_REASON,
        suggestion: "Use Host renderer affordances or model-visible status text.",
    },
    UnsupportedApi {
        name: "getUserCryptoManager",
        kind: UnsupportedApiKind::Sync,
        reason: "Host crypto providers are not exposed to Skill JS yet.",
        suggestion: "Use ANP/Host crypto provider APIs that never expose private keys.",
    },
    UnsupportedApi {
        name: "wx.cloud.init",
        kind: UnsupportedApiKind::Async,
        reason: "wx.cloud.* is unsupported by anp-miniapp-dock production runtime.",
        suggestion: "Expose cloud-backed business logic as a merchant Agent API and call it through wx.request.",
    },
    UnsupportedApi {
        name: "wx.cloud.callFunction",
        kind: UnsupportedApiKind::Async,
        reason: "wx.cloud.* is unsupported by anp-miniapp-dock production runtime.",
        suggestion: "Expose cloud-backed business logic as a merchant Agent API and call it through wx.request.",
    },
    UnsupportedApi {
        name: "wx.cloud.database",
        kind: UnsupportedApiKind::Async,
        reason: "wx.cloud.* is unsupported by anp-miniapp-dock production runtime.",
        suggestion: "Expose cloud-backed business logic as a merchant Agent API and call it through wx.request.",
    },
    UnsupportedApi {
        name: "startWifi",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Do not control device WiFi from Skill JS; ask the Host for minimized network status.",
    },
    UnsupportedApi {
        name: "connectWifi",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Do not control device WiFi from Skill JS.",
    },
    UnsupportedApi {
        name: "openBluetoothAdapter",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Use a native Host provider with explicit authorization for Bluetooth workflows.",
    },
    UnsupportedApi {
        name: "connectSocket",
        kind: UnsupportedApiKind::Async,
        reason: "Socket APIs would bypass RequestBroker allowlist and audit.",
        suggestion: "Use wx.request through the RequestBroker instead of raw sockets.",
    },
    UnsupportedApi {
        name: "createTCPSocket",
        kind: UnsupportedApiKind::Sync,
        reason: "Raw TCP sockets would bypass RequestBroker allowlist and audit.",
        suggestion: "Use wx.request through the RequestBroker instead of raw sockets.",
    },
    UnsupportedApi {
        name: "createUDPSocket",
        kind: UnsupportedApiKind::Sync,
        reason: "Raw UDP sockets would bypass RequestBroker allowlist and audit.",
        suggestion: "Use wx.request through the RequestBroker instead of raw sockets.",
    },
    UnsupportedApi {
        name: "startLocalServiceDiscovery",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Do not perform LAN discovery from Skill JS.",
    },
    UnsupportedApi {
        name: "onAccelerometerChange",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Use explicit Host capabilities only for sensor workflows.",
    },
    UnsupportedApi {
        name: "onCompassChange",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Use explicit Host capabilities only for sensor workflows.",
    },
    UnsupportedApi {
        name: "onDeviceMotionChange",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Use explicit Host capabilities only for sensor workflows.",
    },
    UnsupportedApi {
        name: "onGyroscopeChange",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Use explicit Host capabilities only for sensor workflows.",
    },
    UnsupportedApi {
        name: "navigateToMiniProgram",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Use a Host-controlled detail page or merchant Agent API instead of mini program navigation.",
    },
    UnsupportedApi {
        name: "navigateTo",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Use Host adapter detail pages instead of full miniapp routing.",
    },
    UnsupportedApi {
        name: "switchTab",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Use Host adapter navigation instead of full miniapp routing.",
    },
    UnsupportedApi {
        name: "restartMiniProgram",
        kind: UnsupportedApiKind::Async,
        reason: UNSUPPORTED_BY_DESIGN_REASON,
        suggestion: "Restart or reload must be controlled by the Host runtime.",
    },
];

pub fn unsupported_api_registry() -> &'static [UnsupportedApi] {
    UNSUPPORTED_WX_APIS
}

pub fn unsupported_api_registry_js_literal() -> String {
    serde_json::to_string(UNSUPPORTED_WX_APIS).expect("unsupported wx API registry must serialize")
}

pub fn unsupported_api(name: &str) -> Map<String, Value> {
    let api = unsupported_api_registry()
        .iter()
        .find(|api| api.name == name);
    let api_name = api
        .map(|api| api.name.to_owned())
        .unwrap_or_else(|| safe_unknown_api_name(name));
    let reason = api.map(|api| api.reason).unwrap_or(DEFAULT_REASON);
    let suggestion = api.map(|api| api.suggestion).unwrap_or(DEFAULT_SUGGESTION);
    let mut value = Map::new();
    value.insert(
        "errMsg".to_owned(),
        Value::String(format!("{api_name}:fail unsupported")),
    );
    value.insert("code".to_owned(), Value::String("unsupported".to_owned()));
    value.insert("reason".to_owned(), Value::String(reason.to_owned()));
    value.insert(
        "suggestion".to_owned(),
        Value::String(suggestion.to_owned()),
    );
    value
}

fn safe_unknown_api_name(name: &str) -> String {
    let _ = name;
    "unknownWxApi".to_owned()
}
