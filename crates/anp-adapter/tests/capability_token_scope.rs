use anp::authentication::{create_did_wba_document, DidDocumentOptions};
use anp_adapter::{
    redact_for_log, AnpHttpClient, AnpRequestBroker, AnpRequestError, CapabilityTokenCache,
    CapabilityTokenIssuer, CapabilityTokenIssuerConfig, CapabilityTokenRequest, DidCredential,
    DidCredentialError, DidCredentialProvider, HttpTransport, IdentitySession, InMemoryTokenCache,
    SignedRequestPolicy, TransportRequest, TransportResponse,
};
use serde_json::json;
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use wx_compat::{CapabilityProfile, RequestBroker, WxMethod, WxRequest, WxRequestError};

#[test]
fn allowlist_miss_is_denied_without_transport() {
    let transport = MockTransport::new(vec![]);
    let calls = transport.calls.clone();
    let client = client_with_transport(
        transport,
        SignedRequestPolicy::new(["merchant.example"]),
        InMemoryTokenCache::new(),
    );

    let error = client
        .request(WxRequest::get("https://evil.example/orders"))
        .expect_err("allowlist miss must fail closed");

    assert!(matches!(error, AnpRequestError::Denied(message) if message.contains("allowlist")));
    assert!(calls.borrow().is_empty());
}

#[test]
fn empty_allowlist_denies_by_default() {
    let transport = MockTransport::new(vec![]);
    let client = client_with_transport(
        transport,
        SignedRequestPolicy::default(),
        InMemoryTokenCache::new(),
    );

    let error = client
        .request(WxRequest::get("https://merchant.example/orders"))
        .expect_err("default policy must deny network");

    assert!(matches!(error, AnpRequestError::Denied(_)));
}

#[test]
fn token_cache_hit_uses_bearer_and_keeps_scope_isolated() {
    let token_cache = InMemoryTokenCache::new();
    let session = identity_session();
    token_cache.put(
        token_scope(&session),
        anp_adapter::CapabilityToken::new("coffee-token", None),
    );
    token_cache.put(
        anp_adapter::CapabilityTokenScope::for_subject(
            session.merchant_did.clone(),
            session.user_did.clone(),
            session.agent_did.clone(),
            "tea",
            Some(session.session_id.clone()),
        ),
        anp_adapter::CapabilityToken::new("tea-token", None),
    );
    token_cache.put(
        anp_adapter::CapabilityTokenScope::for_subject(
            session.merchant_did.clone(),
            session.user_did.clone(),
            session.agent_did.clone(),
            session.skill_id.clone(),
            Some("session-2".to_owned()),
        ),
        anp_adapter::CapabilityToken::new("other-session-token", None),
    );
    let transport = MockTransport::new(vec![TransportResponse::json(
        200,
        BTreeMap::new(),
        json!({"ok": true}),
    )]);
    let calls = transport.calls.clone();
    let client = client_with_transport(
        transport,
        SignedRequestPolicy::new(["merchant.example"]),
        token_cache,
    );

    client
        .request(WxRequest::get("https://merchant.example/orders"))
        .expect("request succeeds");

    let calls = calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].headers.get("Authorization").map(String::as_str),
        Some("Bearer coffee-token")
    );
    assert!(!calls[0].headers.contains_key("Signature"));
}

#[test]
fn initial_request_uses_http_signature_when_token_missing() {
    let transport = MockTransport::new(vec![TransportResponse::json(
        200,
        BTreeMap::new(),
        json!({"ok": true}),
    )]);
    let calls = transport.calls.clone();
    let client = client_with_transport(
        transport,
        SignedRequestPolicy::new(["merchant.example"]),
        InMemoryTokenCache::new(),
    );

    client
        .request(WxRequest::get("https://merchant.example/orders"))
        .expect("request succeeds");

    let calls = calls.borrow();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].headers.contains_key("Signature"));
    assert!(calls[0].headers.contains_key("Signature-Input"));
    assert!(!calls[0].headers.contains_key("Authorization"));
}

#[test]
fn challenge_401_retries_with_server_nonce_and_caches_token() {
    let jwt = issue_test_token(4_000_000_000_000);
    let mut challenge_headers = BTreeMap::new();
    challenge_headers.insert(
        "WWW-Authenticate".to_owned(),
        "DIDWba realm=\"merchant.example\", nonce=\"server-nonce\"".to_owned(),
    );
    challenge_headers.insert(
        "Accept-Signature".to_owned(),
        "sig1=(\"@method\" \"@target-uri\" \"@authority\" \"content-digest\");created;expires;nonce;keyid".to_owned(),
    );
    let mut success_headers = BTreeMap::new();
    success_headers.insert(
        "Authentication-Info".to_owned(),
        format!(r#"access_token="{jwt}", token_type="Bearer""#),
    );
    let transport = MockTransport::new(vec![
        TransportResponse {
            status_code: 401,
            headers: challenge_headers,
            body: None,
        },
        TransportResponse::json(200, success_headers, json!({"ok": true})),
    ]);
    let calls = transport.calls.clone();
    let token_cache = InMemoryTokenCache::new();
    let client = client_with_transport(
        transport,
        SignedRequestPolicy::new(["merchant.example"]),
        token_cache.clone(),
    );
    let request = WxRequest {
        url: "https://merchant.example/orders".to_owned(),
        method: WxMethod::Post,
        headers: BTreeMap::from([("Content-Type".to_owned(), "application/json".to_owned())]),
        data: Some(json!({"orderId": "order-1"})),
    };

    let response = client.request(request).expect("challenge retry succeeds");

    assert_eq!(response.status_code, 200);
    let calls = calls.borrow();
    assert_eq!(calls.len(), 2);
    let retry_signature_input = calls[1]
        .headers
        .get("Signature-Input")
        .expect("retry signs request");
    assert!(retry_signature_input.contains("server-nonce"));
    assert!(calls[1].headers.contains_key("Content-Digest"));
    assert_eq!(
        token_cache.get(&token_scope(&identity_session())),
        Some(anp_adapter::CapabilityToken::new(
            jwt,
            Some(4_000_000_300_000)
        ))
    );
}

#[test]
fn cached_bearer_401_clears_token_and_retries_with_signature() {
    let session = identity_session();
    let token_cache = InMemoryTokenCache::new();
    let scope = token_scope(&session);
    token_cache.put(
        scope.clone(),
        anp_adapter::CapabilityToken::new("stale-token", None),
    );
    let mut challenge_headers = BTreeMap::new();
    challenge_headers.insert(
        "WWW-Authenticate".to_owned(),
        "DIDWba realm=\"merchant.example\", nonce=\"server-nonce\"".to_owned(),
    );
    let transport = MockTransport::new(vec![
        TransportResponse {
            status_code: 401,
            headers: challenge_headers,
            body: None,
        },
        TransportResponse::json(200, BTreeMap::new(), json!({"ok": true})),
    ]);
    let calls = transport.calls.clone();
    let client = client_with_transport(
        transport,
        SignedRequestPolicy::new(["merchant.example"]),
        token_cache.clone(),
    );

    client
        .request(WxRequest::get("https://merchant.example/orders"))
        .expect("challenge retry succeeds after stale token");

    let calls = calls.borrow();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].headers.get("Authorization").map(String::as_str),
        Some("Bearer stale-token")
    );
    assert!(calls[1].headers.contains_key("Signature"));
    assert!(token_cache.get(&scope).is_none());
}

#[test]
fn request_broker_respects_wx_capability_profile() {
    let transport = MockTransport::new(vec![TransportResponse::json(
        200,
        BTreeMap::new(),
        json!({"ok": true}),
    )]);
    let broker = AnpRequestBroker::new(client_with_transport(
        transport,
        SignedRequestPolicy::new(["merchant.example"]),
        InMemoryTokenCache::new(),
    ));

    let denied = broker
        .request(
            &CapabilityProfile::component(),
            WxRequest::get("https://merchant.example/orders"),
        )
        .expect_err("component cannot request by default");
    assert!(matches!(denied, WxRequestError::Denied(reason) if reason.contains("request")));

    let response = broker
        .request(
            &CapabilityProfile::atomic_api(),
            WxRequest::get("https://merchant.example/orders"),
        )
        .expect("atomic API request is allowed");
    assert_eq!(response.status_code, 200);
}

#[test]
fn redaction_removes_sensitive_values_from_errors() {
    let redacted = redact_for_log(
        "Authorization: Bearer real-token; Signature: abc; privateKey=/tmp/key.pem; secret=value",
    );

    assert!(!redacted.contains("real-token"));
    assert!(!redacted.contains("/tmp/key.pem"));
    assert!(redacted.contains("[REDACTED]"));
}

#[derive(Clone)]
struct StaticCredentialProvider {
    credential: DidCredential,
    _fixture: Rc<DidFixture>,
}

impl DidCredentialProvider for StaticCredentialProvider {
    fn credential_for(
        &self,
        _session: &IdentitySession,
    ) -> Result<DidCredential, DidCredentialError> {
        Ok(self.credential.clone())
    }
}

#[derive(Clone)]
struct MockTransport {
    responses: Rc<RefCell<VecDeque<TransportResponse>>>,
    calls: Rc<RefCell<Vec<TransportRequest>>>,
}

impl MockTransport {
    fn new(responses: Vec<TransportResponse>) -> Self {
        Self {
            responses: Rc::new(RefCell::new(VecDeque::from(responses))),
            calls: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

impl HttpTransport for MockTransport {
    fn send(&self, request: TransportRequest) -> Result<TransportResponse, AnpRequestError> {
        self.calls.borrow_mut().push(request);
        self.responses
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| AnpRequestError::Transport("missing mock response".to_owned()))
    }
}

fn client_with_transport(
    transport: MockTransport,
    policy: SignedRequestPolicy,
    token_cache: InMemoryTokenCache,
) -> AnpHttpClient<StaticCredentialProvider, InMemoryTokenCache, MockTransport> {
    let fixture = Rc::new(DidFixture::new());
    AnpHttpClient::new(
        identity_session(),
        StaticCredentialProvider {
            credential: fixture.credential(),
            _fixture: fixture,
        },
        token_cache,
        policy,
        transport,
    )
}

fn identity_session() -> IdentitySession {
    IdentitySession::new(
        "did:wba:user.example",
        Some("did:wba:agent.example".to_owned()),
        "did:wba:merchant.example",
        "coffee",
        "session-1",
    )
}

fn token_scope(session: &IdentitySession) -> anp_adapter::CapabilityTokenScope {
    anp_adapter::CapabilityTokenScope::for_subject(
        session.merchant_did.clone(),
        session.user_did.clone(),
        session.agent_did.clone(),
        session.skill_id.clone(),
        Some(session.session_id.clone()),
    )
}

fn issue_test_token(now_ms: u64) -> String {
    let session = identity_session();
    CapabilityTokenIssuer::new(
        CapabilityTokenIssuerConfig::new(
            "did:wba:merchant.example",
            "did:wba:merchant.example",
            "test-only-capability-token-secret-do-not-use-in-production",
        )
        .with_ttl_ms(300_000),
    )
    .expect("issuer config")
    .issue_at(
        CapabilityTokenRequest::new(
            session.merchant_did,
            session.user_did,
            session.agent_did,
            session.skill_id,
            session.session_id,
            ["coffee:drinks:read"],
        ),
        now_ms,
    )
    .expect("token issues")
    .token
    .value
}

struct DidFixture {
    _dir: TempDir,
    did_path: PathBuf,
    key_path: PathBuf,
}

impl DidFixture {
    fn new() -> Self {
        let bundle = create_did_wba_document("user.example", DidDocumentOptions::default())
            .expect("DID fixture creates");
        let dir = TempDir::new("anp-miniapp-dock-anp-adapter").expect("temp dir creates");
        let did_path = dir.path().join("did.json");
        let key_path = dir.path().join("key.pem");
        fs::write(&did_path, serde_json::to_vec(&bundle.did_document).unwrap()).unwrap();
        fs::write(&key_path, &bundle.keys["key-1"].private_key_pem).unwrap();

        Self {
            _dir: dir,
            did_path,
            key_path,
        }
    }

    fn credential(&self) -> DidCredential {
        DidCredential::new(self.did_path.clone(), self.key_path.clone())
    }
}

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
        fs::create_dir(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
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
