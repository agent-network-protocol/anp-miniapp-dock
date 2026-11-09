use anp_adapter::{DidAuthSession, DidAuthSessionKey, DidAuthSessionManager};
use dock_core::{ApiCallContext, ApiExecutor, InMemoryMetricsSink, TraceContext, TraceSpanKind};
use js_runtime_quickjs::{ApiCall, ApiVm, ApiVmConfig, ApiVmError, HostDidAuthConfig};
use mcp_schema::{ApiDeclaration, ComponentDeclaration, SkillManifest, ValidationReport};
use serde_json::json;
use skill_loader::{
    LoadedComponent, LoadedSkill, PackageDigest, PackageIntegrityReport, SourceFile,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[test]
fn middleware_runs_in_onion_order() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.use(async (ctx, next) => {
  ctx.arguments.events.push('outer-before')
  await next()
  ctx.arguments.events.push('outer-after')
})
skill.use(async (ctx, next) => {
  ctx.arguments.events.push('inner-before')
  await next()
  ctx.arguments.events.push('inner-after')
})
skill.registerAPI('ordered', async (ctx) => {
  ctx.arguments.events.push('handler')
  return {
    content: [{ type: 'text', text: ctx.arguments.events.join(',') }],
    structuredContent: { events: ctx.arguments.events }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["ordered"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new(
            "skill",
            "session",
            "ordered",
            json!({ "events": [] }),
        ))
        .expect("call ordered");

    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("events"))
            .and_then(|events| events.as_array())
            .cloned(),
        Some(vec![
            json!("outer-before"),
            json!("inner-before"),
            json!("handler"),
            json!("inner-after"),
            json!("outer-after"),
        ])
    );
}

#[test]
fn async_handler_promise_is_resolved() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('asyncValue', async (ctx) => {
  const suffix = await Promise.resolve(ctx.arguments.suffix)
  return { content: [{ type: 'text', text: 'async-' + suffix }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["asyncValue"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new(
            "skill",
            "session",
            "asyncValue",
            json!({ "suffix": "ok" }),
        ))
        .expect("call asyncValue");

    assert_eq!(result.content[0].text, "async-ok");
}

#[test]
fn wx_login_is_available_to_atomic_api_code() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('login', async () => {
  const login = await wx.login()
  return {
    content: [{ type: 'text', text: login.errMsg }],
    structuredContent: { code: login.code }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["login"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "login", json!({})))
        .expect("call login");

    assert_eq!(result.content[0].text, "login:ok");
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("code"))
            .and_then(|code| code.as_str()),
        Some("dock-login-code-localhost")
    );
}

#[test]
fn wx_login_returns_redacted_receipt_for_cached_did_session() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('login', async () => {
  const login = await wx.login()
  return {
    content: [{ type: 'text', text: login.errMsg }],
    structuredContent: {
      code: login.code,
      tokenVisibleToSkill: login.didAuth.tokenVisibleToSkill,
      tokenReceived: login.didAuth.tokenReceived,
      scopes: login.didAuth.scopes,
      leaked: JSON.stringify(login).includes('cached-token')
    }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["login"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");
    let session_manager = DidAuthSessionManager::new();
    session_manager
        .put_session(
            did_session_key("http://127.0.0.1:3000"),
            DidAuthSession::new("cached-token", None, ["coffee:drinks:read"]),
        )
        .expect("seed session");
    let host_auth = HostDidAuthConfig::new("missing-did.json", "missing-key.pem")
        .with_session_manager(session_manager);

    let result = vm
        .call_with_host_did_auth(
            did_api_call("login", json!({ "serverUrl": "http://127.0.0.1:3000" })),
            Some(host_auth),
        )
        .expect("call login");

    assert_eq!(result.content[0].text, "login:ok");
    let structured = result.structured_content.as_ref().expect("structured");
    assert_eq!(
        structured.get("code").and_then(|code| code.as_str()),
        Some("dock-login-receipt-session-1")
    );
    assert_eq!(
        structured
            .get("tokenVisibleToSkill")
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    assert_eq!(
        structured
            .get("tokenReceived")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        structured.get("leaked").and_then(|value| value.as_bool()),
        Some(false)
    );
}

#[test]
fn wx_callback_exception_does_not_change_original_outcome() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('login', async () => {
  const events = []
  const login = await wx.login({
    success() {
      events.push('success')
      throw new Error('callback secret should not reject')
    },
    complete() {
      events.push('complete')
    }
  })
  return {
    content: [{ type: 'text', text: login.errMsg }],
    structuredContent: { events }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["login"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "login", json!({})))
        .expect("call login");

    assert_eq!(result.content[0].text, "login:ok");
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("events"))
            .and_then(|events| events.as_array())
            .cloned(),
        Some(vec![json!("success"), json!("complete")])
    );
}

#[test]
fn wx_check_session_reports_cached_did_session() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('check', async () => {
  const check = await wx.checkSession()
  return { content: [{ type: 'text', text: check.errMsg }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["check"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");
    let session_manager = DidAuthSessionManager::new();
    session_manager
        .put_session(
            did_session_key("http://127.0.0.1:3000"),
            DidAuthSession::new("cached-token", None, ["coffee:drinks:read"]),
        )
        .expect("seed session");
    let host_auth = HostDidAuthConfig::new("missing-did.json", "missing-key.pem")
        .with_session_manager(session_manager);

    let result = vm
        .call_with_host_did_auth(
            did_api_call("check", json!({ "serverUrl": "http://127.0.0.1:3000" })),
            Some(host_auth),
        )
        .expect("call check");

    assert_eq!(result.content[0].text, "checkSession:ok");
}

#[test]
fn wx_check_session_rejects_missing_did_session() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('check', async () => {
  try {
    await wx.checkSession()
  } catch (error) {
    return {
      content: [{ type: 'text', text: error.errMsg }],
      structuredContent: { code: error.code }
    }
  }
  return { content: [{ type: 'text', text: 'unexpected' }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["check"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");
    let host_auth = HostDidAuthConfig::new("missing-did.json", "missing-key.pem")
        .with_session_manager(DidAuthSessionManager::new());

    let result = vm
        .call_with_host_did_auth(
            did_api_call("check", json!({ "serverUrl": "http://127.0.0.1:3000" })),
            Some(host_auth),
        )
        .expect("call check");

    assert_eq!(result.content[0].text, "checkSession:fail auth_failed");
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("code"))
            .and_then(|code| code.as_str()),
        Some("auth_failed")
    );
}

#[test]
fn wx_request_rejects_js_provided_authorization_header() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('request', async () => {
  try {
    await wx.request({
      url: 'http://127.0.0.1:3000/api/drinks',
      header: { Authorization: 'Bearer attacker-token' }
    })
  } catch (error) {
    return {
      content: [{ type: 'text', text: error.errMsg }],
      structuredContent: {
        code: error.code,
        reason: error.reason,
        leaked: String(error.reason || '').includes('attacker-token')
      }
    }
  }
  return { content: [{ type: 'text', text: 'unexpected' }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["request"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "request", json!({})))
        .expect("call request");

    assert_eq!(result.content[0].text, "request:fail permission_denied");
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("code"))
            .and_then(|code| code.as_str()),
        Some("permission_denied")
    );
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("leaked"))
            .and_then(|leaked| leaked.as_bool()),
        Some(false)
    );
}

#[test]
fn wx_request_rejects_non_loopback_url() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('request', async () => {
  try {
    await wx.request({ url: 'https://merchant.example/api/drinks' })
  } catch (error) {
    return {
      content: [{ type: 'text', text: error.errMsg }],
      structuredContent: { code: error.code, reason: error.reason }
    }
  }
  return { content: [{ type: 'text', text: 'unexpected' }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["request"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "request", json!({})))
        .expect("call request");

    assert_eq!(result.content[0].text, "request:fail network_denied");
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("code"))
            .and_then(|code| code.as_str()),
        Some("network_denied")
    );
}

#[test]
fn quickjs_executor_records_vm_request_and_token_metrics_with_trace() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('request', async () => {
  await wx.login()
  try {
    await wx.checkSession()
  } catch (_) {}
  try {
    await wx.request({
      url: 'https://merchant.example/api/drinks?Authorization=Bearer-secret'
    })
  } catch (error) {
    return {
      content: [{ type: 'text', text: error.errMsg }],
      structuredContent: { code: error.code }
    }
  }
  return { content: [{ type: 'text', text: 'unexpected' }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["request"],
    );
    let metrics = InMemoryMetricsSink::default();
    let executor = ApiVm::load_skill(skill)
        .expect("load VM")
        .executor()
        .with_metrics(metrics.clone());
    let context = ApiCallContext {
        user_did: Some("did:wba:user.example".to_owned()),
        agent_did: Some("did:wba:agent.example".to_owned()),
        merchant_did: Some("did:wba:coffee-merchant.example".to_owned()),
        skill_id: "coffee".to_owned(),
        session_id: "session-1".to_owned(),
        api_name: "request".to_owned(),
        arguments: json!({}),
        capability_token: Some("capability-secret-token".to_owned()),
        trace: Some(TraceContext::new("trace-quickjs-001", "host-span-001")),
    };

    let result = executor
        .execute(&context, None)
        .expect("executor returns AtomicApiResult");

    assert_eq!(result.content[0].text, "request:fail network_denied");
    let recorded_metrics = metrics.metrics();
    let recorded_spans = metrics.spans();
    let metric_names = recorded_metrics
        .iter()
        .map(|metric| metric.name.as_str())
        .collect::<Vec<_>>();
    for expected in [
        "dock.api_vm_execution_ms",
        "dock.request.total",
        "dock.request_latency_ms",
        "dock.token_refresh.total",
        "dock.token_refresh_latency_ms",
    ] {
        assert!(
            metric_names.contains(&expected),
            "{expected} metric must be recorded"
        );
    }
    assert!(recorded_metrics
        .iter()
        .filter(|metric| metric.name == "dock.request.total")
        .any(|metric| {
            metric.labels.get("api").map(String::as_str) == Some("wx.request")
                && metric.labels.get("outcome").map(String::as_str) == Some("network_denied")
                && metric.trace_id.as_deref() == Some("trace-quickjs-001")
        }));
    assert!(recorded_spans
        .iter()
        .any(|span| span.kind == TraceSpanKind::WxApiCall && span.trace_id == "trace-quickjs-001"));
    assert!(recorded_spans
        .iter()
        .any(|span| span.kind == TraceSpanKind::Request && span.trace_id == "trace-quickjs-001"));
    assert!(recorded_spans
        .iter()
        .any(|span| span.kind == TraceSpanKind::Token && span.trace_id == "trace-quickjs-001"));

    let rendered =
        serde_json::to_string(&(recorded_metrics, recorded_spans)).expect("metrics serialize");
    assert!(!rendered.contains("merchant.example/api/drinks"));
    assert!(!rendered.contains("Bearer-secret"));
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("did:wba:user.example"));
    assert!(!rendered.contains("Authorization"));
}

#[test]
fn wx_request_redacts_host_owned_response_headers() {
    let (server_url, request_rx, server_handle) = spawn_sensitive_header_server();
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('request', async (ctx) => {
  const response = await wx.request({ url: ctx.arguments.url })
  return {
    content: [{ type: 'text', text: response.errMsg }],
    structuredContent: {
      statusCode: response.statusCode,
      header: response.header,
      data: response.data,
      leaked: JSON.stringify(response).includes('cached-token') ||
        JSON.stringify(response).includes('response-token') ||
        JSON.stringify(response).includes('session-secret') ||
        JSON.stringify(response).includes('sig-secret')
    }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["request"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");
    let session_manager = DidAuthSessionManager::new();
    session_manager
        .put_session(
            did_session_key(&server_url),
            DidAuthSession::new("cached-token", None, ["coffee:drinks:read"]),
        )
        .expect("seed session");
    let host_auth = HostDidAuthConfig::new("missing-did.json", "missing-key.pem")
        .with_session_manager(session_manager);

    let result = vm
        .call_with_host_did_auth(
            did_api_call(
                "request",
                json!({ "url": format!("{server_url}/api/drinks") }),
            ),
            Some(host_auth),
        )
        .expect("call request");
    let raw_request = request_rx.recv().expect("server observed request");
    server_handle.join().expect("server thread joins");

    assert!(raw_request.contains("Authorization: Bearer cached-token"));
    assert_eq!(result.content[0].text, "request:ok");
    let structured = result.structured_content.as_ref().expect("structured");
    assert_eq!(
        structured
            .get("statusCode")
            .and_then(|value| value.as_u64()),
        Some(200)
    );
    assert_eq!(
        structured
            .get("header")
            .and_then(|header| header.get("X-Safe"))
            .and_then(|value| value.as_str()),
        Some("visible")
    );
    for sensitive in [
        "Authorization",
        "Set-Cookie",
        "Signature",
        "Signature-Input",
        "X-Access-Token",
    ] {
        assert!(
            structured
                .get("header")
                .and_then(|header| header.get(sensitive))
                .is_none(),
            "{sensitive} must be redacted from JS-visible response headers"
        );
    }
    assert_eq!(
        structured.get("leaked").and_then(|value| value.as_bool()),
        Some(false)
    );
}

#[test]
fn high_risk_request_payment_fails_closed_with_callbacks_and_safe_shape() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('pay', async () => {
  const events = []
  try {
    await wx.requestPayment({
      timeStamp: 'secret-time',
      fail(error) {
        events.push('fail:' + error.errMsg)
        throw new Error('callback exception should not change outcome')
      },
      complete(error) {
        events.push('complete:' + error.errMsg)
      }
    })
  } catch (error) {
    return {
      content: [{ type: 'text', text: error.errMsg }],
      structuredContent: {
        code: error.code,
        reason: error.reason,
        suggestion: error.suggestion,
        events,
        leaked: JSON.stringify(error).includes('secret-time')
      }
    }
  }
  return { content: [{ type: 'text', text: 'unexpected' }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["pay"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "pay", json!({})))
        .expect("call pay");

    assert_eq!(
        result.content[0].text,
        "requestPayment:fail provider_unavailable"
    );
    let structured = result.structured_content.as_ref().expect("structured");
    assert_eq!(
        structured.get("code").and_then(|value| value.as_str()),
        Some("provider_unavailable")
    );
    assert!(structured
        .get("reason")
        .and_then(|value| value.as_str())
        .is_some_and(|reason| reason.contains("No Host provider")));
    assert_eq!(
        structured
            .get("events")
            .and_then(|value| value.as_array())
            .cloned(),
        Some(vec![
            json!("fail:requestPayment:fail provider_unavailable"),
            json!("complete:requestPayment:fail provider_unavailable"),
        ])
    );
    assert_eq!(
        structured.get("leaked").and_then(|value| value.as_bool()),
        Some(false)
    );
}

#[test]
fn high_risk_consent_required_blocks_before_provider() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('phone', async () => {
  const events = []
  try {
    await wx.getPhoneNumber({
      __dockConsentRequired: true,
      phoneNumber: '1234567890',
      fail(error) {
        events.push('fail:' + error.errMsg)
      },
      complete(error) {
        events.push('complete:' + error.errMsg)
      }
    })
  } catch (error) {
    return {
      content: [{ type: 'text', text: error.errMsg }],
      structuredContent: {
        code: error.code,
        reason: error.reason,
        events,
        leaked: JSON.stringify(error).includes('1234567890')
      }
    }
  }
  return { content: [{ type: 'text', text: 'unexpected' }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["phone"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "phone", json!({})))
        .expect("call phone");

    assert_eq!(
        result.content[0].text,
        "getPhoneNumber:fail consent_required"
    );
    let structured = result.structured_content.as_ref().expect("structured");
    assert_eq!(
        structured.get("code").and_then(|value| value.as_str()),
        Some("consent_required")
    );
    assert_eq!(
        structured
            .get("events")
            .and_then(|value| value.as_array())
            .cloned(),
        Some(vec![
            json!("fail:getPhoneNumber:fail consent_required"),
            json!("complete:getPhoneNumber:fail consent_required"),
        ])
    );
    assert_eq!(
        structured.get("leaked").and_then(|value| value.as_bool()),
        Some(false)
    );
}

#[test]
fn high_risk_media_rejects_local_file_path_without_echoing_it() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('media', async () => {
  try {
    await wx.chooseMedia({ filePath: '/Users/alice/private-photo.png' })
  } catch (error) {
    return {
      content: [{ type: 'text', text: error.errMsg }],
      structuredContent: {
        code: error.code,
        reason: error.reason,
        leaked: JSON.stringify(error).includes('/Users/alice')
      }
    }
  }
  return { content: [{ type: 'text', text: 'unexpected' }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["media"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "media", json!({})))
        .expect("call media");

    assert_eq!(result.content[0].text, "chooseMedia:fail invalid_options");
    let structured = result.structured_content.as_ref().expect("structured");
    assert_eq!(
        structured.get("code").and_then(|value| value.as_str()),
        Some("invalid_options")
    );
    assert_eq!(
        structured.get("leaked").and_then(|value| value.as_bool()),
        Some(false)
    );
}

#[test]
fn unsupported_sync_wx_api_throws_redacted_error() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('account', () => {
  try {
    wx.getAccountInfoSync({ key: 'private-key' })
  } catch (error) {
    return {
      content: [{ type: 'text', text: error.errMsg }],
      structuredContent: {
        code: error.code,
        reason: error.reason,
        suggestion: error.suggestion,
        leaked: JSON.stringify(error).includes('private-key')
      }
    }
  }
  return { content: [{ type: 'text', text: 'unexpected' }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["account"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "account", json!({})))
        .expect("call account");

    assert_eq!(
        result.content[0].text,
        "getAccountInfoSync:fail unsupported"
    );
    let structured = result.structured_content.as_ref().expect("structured");
    assert_eq!(
        structured.get("code").and_then(|value| value.as_str()),
        Some("unsupported")
    );
    assert_eq!(
        structured.get("leaked").and_then(|value| value.as_bool()),
        Some(false)
    );
}

#[test]
fn wx_storage_async_callbacks_and_promise_work() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('storageAsync', async () => {
  const events = []
  const setResult = await wx.setStorage({
    key: 'cart',
    data: { drinkId: 'latte' },
    success(payload) {
      events.push('set-success:' + payload.errMsg)
    },
    complete(payload) {
      events.push('set-complete:' + payload.errMsg)
    }
  })
  const getResult = await wx.getStorage({
    key: 'cart',
    success(payload) {
      events.push('get-success:' + payload.data.drinkId)
    },
    complete(payload) {
      events.push('get-complete:' + payload.errMsg)
    }
  })
  await wx.removeStorage({ key: 'cart' })
  let missing
  try {
    await wx.getStorage({
      key: 'cart',
      fail(error) {
        events.push('missing-fail:' + error.errMsg)
      },
      complete(error) {
        events.push('missing-complete:' + error.errMsg)
      }
    })
  } catch (error) {
    missing = { errMsg: error.errMsg, code: error.code }
  }
  return {
    content: [{ type: 'text', text: getResult.data.drinkId }],
    structuredContent: {
      setErrMsg: setResult.errMsg,
      events,
      missing
    }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["storageAsync"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(did_api_call("storageAsync", json!({})))
        .expect("call storageAsync");

    assert_eq!(result.content[0].text, "latte");
    let structured = result.structured_content.as_ref().expect("structured");
    assert_eq!(
        structured.get("setErrMsg").and_then(|value| value.as_str()),
        Some("setStorage:ok")
    );
    assert_eq!(
        structured
            .get("events")
            .and_then(|value| value.as_array())
            .cloned(),
        Some(vec![
            json!("set-success:setStorage:ok"),
            json!("set-complete:setStorage:ok"),
            json!("get-success:latte"),
            json!("get-complete:getStorage:ok"),
            json!("missing-fail:getStorage:fail invalid_options"),
            json!("missing-complete:getStorage:fail invalid_options"),
        ])
    );
    assert_eq!(
        structured
            .get("missing")
            .and_then(|value| value.get("code"))
            .and_then(|value| value.as_str()),
        Some("invalid_options")
    );
}

#[test]
fn wx_storage_sync_returns_and_throws_redacted_errors() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('storageSync', () => {
  wx.setStorageSync('cart', { drinkId: 'latte' })
  const first = wx.getStorageSync('cart')
  wx.removeStorageSync('cart')
  let missing
  try {
    wx.getStorageSync('cart')
  } catch (error) {
    missing = { errMsg: error.errMsg, code: error.code }
  }
  wx.setStorageSync('cart', { drinkId: 'tea' })
  wx.clearStorageSync()
  let cleared
  try {
    wx.getStorageSync('cart')
  } catch (error) {
    cleared = { errMsg: error.errMsg, code: error.code }
  }
  let sensitive
  try {
    wx.setStorageSync('Authorization', 'Bearer private-token')
  } catch (error) {
    sensitive = {
      errMsg: error.errMsg,
      code: error.code,
      leaked: JSON.stringify(error).includes('private-token') ||
        JSON.stringify(error).includes('Authorization')
    }
  }
  return {
    content: [{ type: 'text', text: first.drinkId }],
    structuredContent: { missing, cleared, sensitive }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["storageSync"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(did_api_call("storageSync", json!({})))
        .expect("call storageSync");

    assert_eq!(result.content[0].text, "latte");
    let structured = result.structured_content.as_ref().expect("structured");
    for field in ["missing", "cleared"] {
        assert_eq!(
            structured
                .get(field)
                .and_then(|value| value.get("errMsg"))
                .and_then(|value| value.as_str()),
            Some("getStorageSync:fail invalid_options")
        );
        assert_eq!(
            structured
                .get(field)
                .and_then(|value| value.get("code"))
                .and_then(|value| value.as_str()),
            Some("invalid_options")
        );
    }
    assert_eq!(
        structured
            .get("sensitive")
            .and_then(|value| value.get("errMsg"))
            .and_then(|value| value.as_str()),
        Some("setStorageSync:fail permission_denied")
    );
    assert_eq!(
        structured
            .get("sensitive")
            .and_then(|value| value.get("code"))
            .and_then(|value| value.as_str()),
        Some("permission_denied")
    );
    assert_eq!(
        structured
            .get("sensitive")
            .and_then(|value| value.get("leaked"))
            .and_then(|value| value.as_bool()),
        Some(false)
    );
}

#[test]
fn wx_storage_scope_uses_user_merchant_and_skill_without_session_id() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('storageScope', async (ctx) => {
  if (ctx.arguments.mode === 'set') {
    await wx.setStorage({ key: 'cart', data: { drinkId: ctx.arguments.drinkId } })
  }
  try {
    const result = await wx.getStorage({ key: 'cart' })
    return {
      content: [{ type: 'text', text: result.data.drinkId }],
      structuredContent: { drinkId: result.data.drinkId }
    }
  } catch (error) {
    return {
      content: [{ type: 'text', text: error.errMsg }],
      structuredContent: { code: error.code }
    }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["storageScope"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    vm.call(scoped_api_call(
        "coffee",
        "session-1",
        "storageScope",
        "did:wba:alice.example",
        "did:wba:merchant-a.example",
        json!({ "mode": "set", "drinkId": "latte" }),
    ))
    .expect("seed storage");

    let same_scope_other_session = vm
        .call(scoped_api_call(
            "coffee",
            "session-2",
            "storageScope",
            "did:wba:alice.example",
            "did:wba:merchant-a.example",
            json!({ "mode": "get" }),
        ))
        .expect("same scope");
    let other_user = vm
        .call(scoped_api_call(
            "coffee",
            "session-1",
            "storageScope",
            "did:wba:bob.example",
            "did:wba:merchant-a.example",
            json!({ "mode": "get" }),
        ))
        .expect("other user");
    let other_merchant = vm
        .call(scoped_api_call(
            "coffee",
            "session-1",
            "storageScope",
            "did:wba:alice.example",
            "did:wba:merchant-b.example",
            json!({ "mode": "get" }),
        ))
        .expect("other merchant");
    let other_skill = vm
        .call(scoped_api_call(
            "tea",
            "session-1",
            "storageScope",
            "did:wba:alice.example",
            "did:wba:merchant-a.example",
            json!({ "mode": "get" }),
        ))
        .expect("other skill");

    assert_eq!(same_scope_other_session.content[0].text, "latte");
    for result in [other_user, other_merchant, other_skill] {
        assert_eq!(result.content[0].text, "getStorage:fail invalid_options");
        assert_eq!(
            result
                .structured_content
                .as_ref()
                .and_then(|value| value.get("code"))
                .and_then(|value| value.as_str()),
            Some("invalid_options")
        );
    }
}

#[test]
fn wx_storage_requires_did_scope() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('storageScopeRequired', async () => {
  const events = []
  try {
    await wx.setStorage({
      key: 'cart',
      data: { drinkId: 'latte' },
      fail(error) {
        events.push('fail:' + error.errMsg)
      },
      complete(error) {
        events.push('complete:' + error.errMsg)
      }
    })
  } catch (error) {
    return {
      content: [{ type: 'text', text: error.errMsg }],
      structuredContent: { code: error.code, events }
    }
  }
  return { content: [{ type: 'text', text: 'unexpected' }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["storageScopeRequired"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new(
            "skill",
            "session",
            "storageScopeRequired",
            json!({}),
        ))
        .expect("call storageScopeRequired");

    assert_eq!(
        result.content[0].text,
        "setStorage:fail provider_unavailable"
    );
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|value| value.get("code"))
            .and_then(|value| value.as_str()),
        Some("provider_unavailable")
    );
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|value| value.get("events"))
            .and_then(|value| value.as_array())
            .cloned(),
        Some(vec![
            json!("fail:setStorage:fail provider_unavailable"),
            json!("complete:setStorage:fail provider_unavailable"),
        ])
    );
}

#[test]
fn wx_storage_rejects_non_json_safe_values() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('storageJsonSafe', async () => {
  const cyclic = {}
  cyclic.self = cyclic
  const events = []
  let asyncError
  try {
    await wx.setStorage({
      key: 'cart',
      data: cyclic,
      fail(error) {
        events.push('fail:' + error.errMsg)
      },
      complete(error) {
        events.push('complete:' + error.errMsg)
      }
    })
  } catch (error) {
    asyncError = { errMsg: error.errMsg, code: error.code, reason: error.reason }
  }
  let syncError
  try {
    wx.setStorageSync('cart', function secretValue() {})
  } catch (error) {
    syncError = { errMsg: error.errMsg, code: error.code, reason: error.reason }
  }
  return {
    content: [{ type: 'text', text: asyncError.errMsg }],
    structuredContent: { events, asyncError, syncError }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["storageJsonSafe"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(did_api_call("storageJsonSafe", json!({})))
        .expect("call storageJsonSafe");

    assert_eq!(result.content[0].text, "setStorage:fail invalid_options");
    let structured = result.structured_content.as_ref().expect("structured");
    assert_eq!(
        structured
            .get("events")
            .and_then(|value| value.as_array())
            .cloned(),
        Some(vec![
            json!("fail:setStorage:fail invalid_options"),
            json!("complete:setStorage:fail invalid_options"),
        ])
    );
    assert_eq!(
        structured
            .get("syncError")
            .and_then(|value| value.get("errMsg"))
            .and_then(|value| value.as_str()),
        Some("setStorageSync:fail invalid_options")
    );
}

#[test]
fn wx_storage_does_not_enter_model_visible_output_unless_returned() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('storageModelVisible', async () => {
  await wx.setStorage({ key: 'cart', data: { receipt: 'stored-secret-value' } })
  return {
    content: [{ type: 'text', text: 'stored' }],
    structuredContent: { status: 'stored' }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["storageModelVisible"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(did_api_call("storageModelVisible", json!({})))
        .expect("call storageModelVisible");
    let model_visible = serde_json::to_string(&result.model_visible()).expect("model visible");

    assert_eq!(result.content[0].text, "stored");
    assert!(!model_visible.contains("stored-secret-value"));
    assert!(result.meta.is_none());
}

#[test]
fn wx_device_and_app_info_sync_apis_return_minimized_frozen_snapshots() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('info', () => {
  const device = wx.getDeviceInfo()
  const app = wx.getAppBaseInfo()
  let mutationBlocked = false
  try {
    device.deviceId = 'fingerprint'
  } catch (_error) {
    mutationBlocked = true
  }
  const mutationPersisted = device.deviceId === 'fingerprint'
  return {
    content: [{ type: 'text', text: device.platform + '/' + app.version }],
    structuredContent: {
      device,
      app,
      deviceFrozen: Object.isFrozen(device),
      appFrozen: Object.isFrozen(app),
      mutationBlocked,
      mutationPersisted,
      leaked: JSON.stringify({ device, app }).includes('fingerprint') ||
        JSON.stringify({ device, app }).includes('credential') ||
        JSON.stringify({ device, app }).includes('localIp') ||
        JSON.stringify({ device, app }).includes('mac')
    }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["info"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "info", json!({})))
        .expect("call info");

    assert_eq!(result.content[0].text, "anp-miniapp-dock/0.1.0");
    let structured = result.structured_content.as_ref().expect("structured");
    assert_eq!(
        structured
            .get("device")
            .and_then(|value| value.get("platform"))
            .and_then(|value| value.as_str()),
        Some("anp-miniapp-dock")
    );
    assert_eq!(
        structured
            .get("device")
            .and_then(|value| value.get("model"))
            .and_then(|value| value.as_str()),
        Some("host-runtime")
    );
    assert_eq!(
        structured
            .get("app")
            .and_then(|value| value.get("SDKVersion"))
            .and_then(|value| value.as_str()),
        Some("0.1.0")
    );
    assert_eq!(
        structured
            .get("deviceFrozen")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        structured
            .get("appFrozen")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        structured
            .get("mutationBlocked")
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    assert_eq!(
        structured
            .get("mutationPersisted")
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    assert_eq!(
        structured.get("leaked").and_then(|value| value.as_bool()),
        Some(false)
    );
    for forbidden in [
        "deviceId",
        "localIp",
        "mac",
        "advertisingId",
        "account",
        "credentialPath",
    ] {
        assert!(
            structured
                .get("device")
                .and_then(|value| value.get(forbidden))
                .is_none(),
            "{forbidden} must not be exposed in Atomic API device info"
        );
    }
}

#[test]
fn wx_device_info_is_not_overwritten_by_unsupported_registry() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('infoType', () => ({
  content: [{ type: 'text', text: [
    typeof wx.getDeviceInfo,
    typeof wx.getAppBaseInfo,
    wx.getDeviceInfo().model,
    wx.getAppBaseInfo().version
  ].join(',') }]
}))
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["infoType"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "infoType", json!({})))
        .expect("call infoType");

    assert_eq!(
        result.content[0].text,
        "function,function,host-runtime,0.1.0"
    );
}

#[test]
fn unsupported_nested_wx_api_is_available() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('cloud', async () => {
  try {
    await wx.cloud.callFunction({ name: 'secretFunction' })
  } catch (error) {
    return {
      content: [{ type: 'text', text: error.errMsg }],
      structuredContent: {
        code: error.code,
        reason: error.reason,
        cloudType: typeof wx.cloud,
        leaked: JSON.stringify(error).includes('secretFunction')
      }
    }
  }
  return { content: [{ type: 'text', text: 'unexpected' }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["cloud"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "cloud", json!({})))
        .expect("call cloud");

    assert_eq!(
        result.content[0].text,
        "wx.cloud.callFunction:fail unsupported"
    );
    let structured = result.structured_content.as_ref().expect("structured");
    assert_eq!(
        structured.get("code").and_then(|value| value.as_str()),
        Some("unsupported")
    );
    assert_eq!(
        structured.get("cloudType").and_then(|value| value.as_str()),
        Some("object")
    );
    assert_eq!(
        structured.get("leaked").and_then(|value| value.as_bool()),
        Some(false)
    );
}

#[test]
fn unsupported_unknown_root_wx_api_fails_deterministically() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('unknown', async () => {
  const events = []
  try {
    await wx.unlistedApi({
      Authorization: 'Bearer secret-token',
      fail(error) {
        events.push('fail:' + error.errMsg)
      },
      complete(error) {
        events.push('complete:' + error.errMsg)
      }
    })
  } catch (error) {
    return {
      content: [{ type: 'text', text: error.errMsg }],
      structuredContent: {
        code: error.code,
        reason: error.reason,
        events,
        leaked: JSON.stringify(error).includes('secret-token') ||
          JSON.stringify(error).includes('unlistedApi')
      }
    }
  }
  return { content: [{ type: 'text', text: 'unexpected' }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["unknown"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "unknown", json!({})))
        .expect("call unknown");

    assert_eq!(result.content[0].text, "unknownWxApi:fail unsupported");
    let structured = result.structured_content.as_ref().expect("structured");
    assert_eq!(
        structured.get("code").and_then(|value| value.as_str()),
        Some("unsupported")
    );
    assert_eq!(
        structured
            .get("events")
            .and_then(|value| value.as_array())
            .cloned(),
        Some(vec![
            json!("fail:unknownWxApi:fail unsupported"),
            json!("complete:unknownWxApi:fail unsupported"),
        ])
    );
    assert_eq!(
        structured.get("leaked").and_then(|value| value.as_bool()),
        Some(false)
    );
}

#[test]
fn supported_wx_request_is_not_overwritten_by_unsupported_registry() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('types', () => ({
  content: [{ type: 'text', text: [
    typeof wx.request,
    typeof wx.login,
    typeof wx.requestPayment,
    typeof wx.getDeviceInfo
  ].join(',') }]
}))
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["types"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "types", json!({})))
        .expect("call types");

    assert_eq!(
        result.content[0].text,
        "function,function,function,function"
    );
}

#[test]
fn model_context_get_session_id_is_available_to_atomic_api_code() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('session', () => {
  const sessionId = wx.modelContext.getSessionId()
  return {
    content: [{ type: 'text', text: sessionId }],
    structuredContent: { sessionId }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["session"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session-123", "session", json!({})))
        .expect("call session");

    assert_eq!(result.content[0].text, "session-123");
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("sessionId"))
            .and_then(|session_id| session_id.as_str()),
        Some("session-123")
    );
}

#[test]
fn model_context_expire_all_cards_records_private_meta_event() {
    let skill = test_skill_with_components(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('expire', async () => {
  const outcome = await wx.modelContext.expireAllCards({
    componentPaths: ['components/result/index'],
    match: 'latest'
  })
  return {
    content: [{ type: 'text', text: outcome.errMsg }],
    structuredContent: { expiredCount: outcome.expiredCount }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["expire"],
        vec![("components/result/index", true)],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "expire", json!({})))
        .expect("call expire");

    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("expiredCount"))
            .and_then(|expired_count| expired_count.as_u64()),
        Some(1)
    );
    assert_eq!(
        result
            .meta
            .as_ref()
            .and_then(|meta| meta.get("modelContext"))
            .and_then(|model_context| model_context.get("cardEvents"))
            .and_then(|events| events.as_array())
            .and_then(|events| events.first())
            .and_then(|event| event.get("type"))
            .and_then(|event_type| event_type.as_str()),
        Some("expireAllCards")
    );
    assert_eq!(
        result
            .meta
            .as_ref()
            .and_then(|meta| meta.get("modelContext"))
            .and_then(|model_context| model_context.get("cardEvents"))
            .and_then(|events| events.as_array())
            .and_then(|events| events.first())
            .and_then(|event| event.get("matchPolicy"))
            .and_then(|match_policy| match_policy.as_str()),
        Some("latest")
    );
    let model_visible = serde_json::to_value(result.model_visible()).expect("model visible");
    assert!(model_visible.get("_meta").is_none());
}

#[test]
fn model_context_expire_all_cards_defaults_to_all_expirable_components() {
    let skill = test_skill_with_components(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('expire', async () => {
  const outcome = await wx.modelContext.expireAllCards({ match: 'all' })
  return {
    content: [{ type: 'text', text: outcome.errMsg }],
    structuredContent: { expiredCount: outcome.expiredCount }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["expire"],
        vec![
            ("components/result/index", true),
            ("components/receipt/index", true),
            ("components/static/index", false),
        ],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "expire", json!({})))
        .expect("call expire");

    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("expiredCount"))
            .and_then(|expired_count| expired_count.as_u64()),
        Some(2)
    );
    assert_eq!(
        result
            .meta
            .as_ref()
            .and_then(|meta| meta.get("modelContext"))
            .and_then(|model_context| model_context.get("cardEvents"))
            .and_then(|events| events.as_array())
            .and_then(|events| events.first())
            .and_then(|event| event.get("componentPaths"))
            .and_then(|paths| paths.as_array())
            .cloned(),
        Some(vec![
            json!("components/receipt/index"),
            json!("components/result/index"),
        ])
    );
    assert_eq!(
        result
            .meta
            .as_ref()
            .and_then(|meta| meta.get("modelContext"))
            .and_then(|model_context| model_context.get("cardEvents"))
            .and_then(|events| events.as_array())
            .and_then(|events| events.first())
            .and_then(|event| event.get("matchPolicy"))
            .and_then(|match_policy| match_policy.as_str()),
        Some("all")
    );
}

#[test]
fn model_context_expire_all_cards_rejects_invalid_component_path() {
    let skill = test_skill_with_components(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('expire', async () => {
  try {
    await wx.modelContext.expireAllCards({ componentPaths: ['../outside'] })
  } catch (error) {
    return {
      content: [{ type: 'text', text: error.errMsg }],
      structuredContent: { code: error.code }
    }
  }
  return { content: [{ type: 'text', text: 'unexpected' }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["expire"],
        vec![("components/result/index", true)],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "expire", json!({})))
        .expect("call expire");

    assert!(result.content[0]
        .text
        .contains("modelContext.expireAllCards:fail"));
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("code"))
            .and_then(|code| code.as_str()),
        Some("invalid_options")
    );
    assert!(result.meta.is_none());
}

#[test]
fn model_context_expire_all_cards_requires_expirable_component() {
    let skill = test_skill_with_components(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('expire', async () => {
  try {
    await wx.modelContext.expireAllCards({ componentPaths: ['components/result/index'] })
  } catch (error) {
    return { content: [{ type: 'text', text: error.reason }] }
  }
  return { content: [{ type: 'text', text: 'unexpected' }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["expire"],
        vec![("components/result/index", false)],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "expire", json!({})))
        .expect("call expire");

    assert!(result.content[0]
        .text
        .contains("component without expirable: true"));
    assert!(result.meta.is_none());
}

#[test]
fn model_context_notification_type_matches_component_runtime() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('notificationTypes', () => ({
  content: [{ type: 'text', text: [
    wx.modelContext.NotificationType.Input,
    wx.modelContext.NotificationType.Result,
    wx.modelContext.NotificationType.Expire,
    wx.modelContext.NotificationType.Overflow
  ].join(',') }]
}))
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["notificationTypes"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new(
            "skill",
            "session",
            "notificationTypes",
            json!({}),
        ))
        .expect("call notificationTypes");

    assert_eq!(result.content[0].text, "input,result,expire,overflow");
}

#[test]
fn create_skill_rejects_path_outside_skill_package() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill('../outside')
skill.registerAPI('escape', () => ({ content: [{ type: 'text', text: 'never' }] }))
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["escape"],
    );

    let error = ApiVm::load_skill(skill).expect_err("unsafe skillPath must fail during load");
    assert!(
        matches!(error, ApiVmError::QuickJs(message) if message.contains("createSkill path outside skill package"))
    );
}

#[test]
fn timeout_interrupts_long_running_handler() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('loop', () => {
  while (true) {}
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["loop"],
    );
    let vm = ApiVm::load_skill_with_config(
        skill,
        ApiVmConfig {
            timeout: Duration::from_millis(20),
            ..Default::default()
        },
    )
    .expect("load VM");

    let error = vm
        .call(ApiCall::new("skill", "session", "loop", json!({})))
        .expect_err("loop should time out");

    assert!(matches!(error, ApiVmError::Timeout(name, _) if name == "loop"));
}

#[test]
fn require_parent_escape_is_rejected() {
    let mut modules = BTreeMap::new();
    modules.insert(
        "safe".to_owned(),
        r#"
module.exports = () => ({ content: [{ type: 'text', text: 'never' }] })
"#
        .to_owned(),
    );
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('escape', require('../secret'))
module.exports = skill
"#,
        modules,
        vec!["escape"],
    );

    let error = ApiVm::load_skill(skill).expect_err("escape require must fail");
    assert!(
        matches!(error, ApiVmError::QuickJs(message) if message.contains("outside skill package"))
    );
}

#[test]
fn remote_require_is_rejected() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('escape', require('https://example.invalid/remote.js'))
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["escape"],
    );

    let error = ApiVm::load_skill(skill).expect_err("remote require must fail");
    assert!(
        matches!(error, ApiVmError::QuickJs(message) if message.contains("outside skill package"))
    );
}

#[test]
fn sandbox_globals_are_not_available_to_skill_code() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('globals', () => ({
  content: [{ type: 'text', text: [
    typeof process,
    typeof fetch,
    typeof WebSocket,
    typeof setTimeout,
    typeof setInterval,
    typeof clearTimeout,
    typeof clearInterval,
    typeof eval,
    typeof Function,
    typeof Proxy,
    typeof (() => {}).constructor,
    typeof ({}).constructor.constructor,
    typeof (async function() {}).constructor,
    typeof (function* () {}).constructor,
    typeof (async function* () {}).constructor
  ].join(',') }]
}))
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["globals"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "globals", json!({})))
        .expect("call globals");

    let observed = result.content[0].text.split(',').collect::<Vec<_>>();
    assert_eq!(observed.len(), 15);
    assert!(observed.iter().all(|value| *value == "undefined"));
}

#[test]
fn console_trace_is_bounded_and_truncated() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
console.log('short')
console.log('very-sensitive-token-should-be-truncated')
console.error('third entry should collapse into truncation marker')
skill.registerAPI('noop', () => ({ content: [{ type: 'text', text: 'ok' }] }))
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["noop"],
    );
    let vm = ApiVm::load_skill_with_config(
        skill,
        ApiVmConfig {
            max_console_entries: 2,
            max_console_message_chars: 18,
            ..Default::default()
        },
    )
    .expect("load VM");

    assert_eq!(vm.trace().console.len(), 2);
    assert_eq!(vm.trace().console[0].message, "short");
    assert_eq!(vm.trace().console[1].message, "[console output truncated]");
    assert!(!vm
        .trace()
        .console
        .iter()
        .any(|entry| entry.message.contains("very-sensitive-token")));
}

#[test]
fn result_json_size_limit_rejects_large_atomic_api_result() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('large', () => ({
  content: [{ type: 'text', text: 'x'.repeat(512) }]
}))
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["large"],
    );
    let vm = ApiVm::load_skill_with_config(
        skill,
        ApiVmConfig {
            max_result_json_bytes: 128,
            ..Default::default()
        },
    )
    .expect("load VM");

    let error = vm
        .call(ApiCall::new("skill", "session", "large", json!({})))
        .expect_err("large result should fail closed");

    assert!(matches!(&error, ApiVmError::ResultTooLarge(name, 128) if name == "large"));
    assert!(!error.to_string().contains(&"x".repeat(32)));
}

#[test]
fn invalid_atomic_api_result_error_does_not_echo_payload() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('invalid', () => ({
  content: 'secret-token-should-not-be-echoed'
}))
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["invalid"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let error = vm
        .call(ApiCall::new("skill", "session", "invalid", json!({})))
        .expect_err("invalid result should fail closed");

    assert!(matches!(&error, ApiVmError::InvalidResult(name, _) if name == "invalid"));
    let message = error.to_string();
    assert!(!message.contains("secret-token-should-not-be-echoed"));
    assert!(!message.contains("payload="));
}

#[test]
fn pending_job_drain_limit_fails_registration() {
    let skill = test_skill(
        r#"
Promise.resolve()
  .then(() => {})
  .then(() => {})
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('noop', () => ({ content: [{ type: 'text', text: 'ok' }] }))
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["noop"],
    );

    let error = ApiVm::load_skill_with_config(
        skill,
        ApiVmConfig {
            max_pending_jobs: 1,
            ..Default::default()
        },
    )
    .expect_err("excess pending jobs should fail closed");

    assert!(matches!(error, ApiVmError::PendingJobLimitExceeded));
}

fn test_skill(
    entry_js: &str,
    api_modules: BTreeMap<String, String>,
    api_names: Vec<&str>,
) -> LoadedSkill {
    test_skill_with_components(entry_js, api_modules, api_names, Vec::new())
}

fn did_api_call(api_name: &str, arguments: serde_json::Value) -> ApiCall {
    let mut call = ApiCall::new("coffee", "session-1", api_name, arguments);
    call.user_did = Some("did:wba:user.example".to_owned());
    call.agent_did = Some("did:wba:agent.example".to_owned());
    call.merchant_did = Some("did:wba:coffee-merchant.example".to_owned());
    call
}

fn scoped_api_call(
    skill_id: &str,
    session_id: &str,
    api_name: &str,
    user_did: &str,
    merchant_did: &str,
    arguments: serde_json::Value,
) -> ApiCall {
    let mut call = ApiCall::new(skill_id, session_id, api_name, arguments);
    call.user_did = Some(user_did.to_owned());
    call.agent_did = Some("did:wba:agent.example".to_owned());
    call.merchant_did = Some(merchant_did.to_owned());
    call
}

fn did_session_key(base_url: &str) -> DidAuthSessionKey {
    DidAuthSessionKey::new(
        base_url,
        "did:wba:coffee-merchant.example",
        "did:wba:user.example",
        Some("did:wba:agent.example".to_owned()),
        "coffee",
        "session-1",
    )
}

fn spawn_sensitive_header_server() -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("test server addr");
    let (request_tx, request_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buffer).expect("read request");
            assert!(read > 0, "connection closed before request headers");
            bytes.extend_from_slice(&buffer[..read]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let raw_request = String::from_utf8_lossy(&bytes).to_string();
        request_tx.send(raw_request).expect("send request");
        let body = r#"{"ok":true}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAuthorization: Bearer response-token\r\nSet-Cookie: sid=session-secret\r\nSignature: sig-secret\r\nSignature-Input: sig-input-secret\r\nX-Access-Token: response-token\r\nX-Safe: visible\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });
    (format!("http://{addr}"), request_rx, handle)
}

fn test_skill_with_components(
    entry_js: &str,
    api_modules: BTreeMap<String, String>,
    api_names: Vec<&str>,
    component_specs: Vec<(&str, bool)>,
) -> LoadedSkill {
    let root = test_root(&component_specs);
    let components = component_specs
        .iter()
        .map(|(path, expirable)| ComponentDeclaration {
            path: (*path).to_owned(),
            permissions: None,
            related_page: None,
            expirable: Some(*expirable),
            expired_text: None,
            meta: None,
            extra: BTreeMap::new(),
        })
        .collect::<Vec<_>>();
    let loaded_components = component_specs
        .iter()
        .map(|(path, _)| {
            (
                (*path).to_owned(),
                LoadedComponent {
                    route: (*path).to_owned(),
                    directory: component_directory(&root, path),
                    index_js: None,
                    index_wxml: None,
                    index_wxss: None,
                    index_json: None,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    LoadedSkill {
        root: root.clone(),
        skill_md: source_at(&root, "SKILL.md", "Test skill"),
        manifest: SkillManifest {
            apis: api_names
                .into_iter()
                .map(|name| ApiDeclaration {
                    name: name.to_owned(),
                    description: format!("{name} test API"),
                    input_schema: json!({ "type": "object" }),
                    output_schema: None,
                    meta: None,
                    extra: BTreeMap::new(),
                })
                .collect(),
            components,
            meta: None,
            extra: BTreeMap::new(),
        },
        integrity: PackageIntegrityReport::development_unsigned(PackageDigest::sha256("test")),
        entry_js: source_at(&root, "index.js", entry_js),
        api_modules: api_modules
            .into_iter()
            .map(|(name, body)| {
                (
                    name.clone(),
                    source_at(&root, format!("apis/{name}.js"), body),
                )
            })
            .collect(),
        components: loaded_components,
        component_routes: BTreeMap::new(),
        validation: ValidationReport::ok(),
    }
}

fn test_root(component_specs: &[(&str, bool)]) -> PathBuf {
    let mut root = std::env::temp_dir();
    root.push(format!(
        "anp-miniapp-dock-js-runtime-tests-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let _ = fs::remove_dir_all(&root);
    let mut required_dirs = component_specs
        .iter()
        .map(|(path, _)| component_directory(&root, path))
        .collect::<BTreeSet<_>>();
    required_dirs.insert(root.clone());
    for dir in required_dirs {
        fs::create_dir_all(dir).expect("create test component dir");
    }
    root
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

fn component_directory(root: &Path, component_path: &str) -> PathBuf {
    let relative = Path::new(component_path);
    let dir = if relative.file_name().is_some_and(|name| name == "index") {
        relative.parent().unwrap_or(relative)
    } else {
        relative
    };
    root.join(dir)
}

fn source_at(root: &Path, path: impl AsRef<Path>, source: impl Into<String>) -> SourceFile {
    let relative_path = path.as_ref().to_path_buf();
    SourceFile {
        absolute_path: root.join(&relative_path),
        relative_path,
        source: source.into(),
    }
}
