use anp::authentication::{create_did_wba_document, AuthMode, DidDocumentOptions};
use anp_adapter::{
    sign_challenge_proof, CapabilityTokenIssuer, CapabilityTokenIssuerConfig,
    CapabilityTokenRequest, ChallengeProofPayload, DidChallenge as AdapterDidChallenge,
    DidCredentialConfig, FileDidCredentialProvider, IdentitySession,
};
use demo_server::auth::ServerAuthConfig;
use demo_server::{app, DemoState};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under crates/demo-server")
        .to_path_buf()
}

fn skill_root() -> PathBuf {
    repo_root().join("examples/coffee-skill")
}

async fn spawn_server_with_fixture(fixture: &DidFixture) -> (SocketAddr, DemoState) {
    let auth_config = ServerAuthConfig::for_tests()
        .with_trusted_did_document(fixture.did(), fixture.did_path.clone());
    let state = DemoState::with_auth_config(skill_root(), auth_config);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let addr = listener.local_addr().expect("test server addr");
    let app = app(state.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("test server runs");
    });
    (addr, state)
}

fn request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (u16, String) {
    let body = body.map(|value| value.to_string()).unwrap_or_default();
    let mut stream = connect(addr);
    let auth = bearer
        .map(|token| format!("Authorization: Bearer {token}\r\n"))
        .unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{auth}\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).expect("write request");
    let response = read_http_response(&mut stream);
    let (head, body) = response.split_once("\r\n\r\n").expect("http response");
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .expect("status code");
    (status, body.to_owned())
}

fn read_http_response(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    let header_end = loop {
        let read = stream.read(&mut buffer).expect("read response headers");
        assert!(read > 0, "connection closed before headers");
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(index) = find_header_end(&bytes) {
            break index;
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..header_end]).to_string();
    let body_start = header_end + 4;
    let content_length = content_length(&headers).unwrap_or(0);
    while bytes.len().saturating_sub(body_start) < content_length {
        let read = stream.read(&mut buffer).expect("read response body");
        assert!(read > 0, "connection closed before full body");
        bytes.extend_from_slice(&buffer[..read]);
    }
    String::from_utf8(bytes).expect("response utf8")
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn content_length(headers: &str) -> Option<usize> {
    headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    })
}

fn connect(addr: SocketAddr) -> TcpStream {
    for _ in 0..100 {
        match TcpStream::connect(addr) {
            Ok(stream) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .expect("set read timeout");
                stream
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .expect("set write timeout");
                return stream;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    TcpStream::connect(addr).expect("connect test server")
}

fn json_response(
    addr: SocketAddr,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> Value {
    let (status, body) = request(addr, method, path, bearer, body);
    assert!(
        (200..300).contains(&status),
        "expected success status, got {status}: {body}"
    );
    serde_json::from_str(&body).expect("json body")
}

fn login(addr: SocketAddr, fixture: &DidFixture) -> String {
    let challenge = json_response(
        addr,
        "POST",
        "/agents/coffee/auth/challenge",
        None,
        Some(json!({
            "sessionId": "session-1",
            "skillId": "coffee",
            "userDid": fixture.did(),
            "agentDid": "did:wba:agent.example"
        })),
    );
    let proof = sign_login_proof(&challenge, fixture, "session-1", "coffee");

    let login = json_response(
        addr,
        "POST",
        "/agents/coffee/auth/login",
        None,
        Some(json!({
            "sessionId": "session-1",
            "skillId": "coffee",
            "userDid": fixture.did(),
            "agentDid": "did:wba:agent.example",
            "merchantDid": challenge["merchantDid"],
            "challengeId": challenge["challengeId"],
            "signedChallenge": proof
        })),
    );

    login["capabilityToken"].as_str().expect("token").to_owned()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn registry_manifest_and_skill_files_are_served() {
    let fixture = DidFixture::new();
    let (addr, _state) = spawn_server_with_fixture(&fixture).await;

    let registry = json_response(addr, "GET", "/registry/agents", None, None);
    assert_eq!(registry["agents"][0]["id"], "coffee");

    let manifest = json_response(addr, "GET", "/agents/coffee/manifest", None, None);
    assert_eq!(
        manifest["auth"]["challenge"],
        "/agents/coffee/auth/challenge"
    );

    let mcp = json_response(addr, "GET", "/agents/coffee/mcp.json", None, None);
    assert_eq!(mcp["apis"][0]["name"], "searchDrinks");

    let (status, skill) = request(addr, "GET", "/agents/coffee/SKILL.md", None, None);
    assert_eq!(status, 200);
    assert!(skill.contains("coffee ordering flow"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_challenge_login_and_coffee_order_flow_succeed() {
    let fixture = DidFixture::new();
    let (addr, state) = spawn_server_with_fixture(&fixture).await;
    let token = login(addr, &fixture);

    let drinks = json_response(addr, "GET", "/api/drinks?query=latte", Some(&token), None);
    assert_eq!(drinks["drinks"][0]["id"], "latte");
    assert!(drinks["drinks"][0]["image"]
        .as_str()
        .unwrap()
        .contains("latte"));
    let generic_coffee = json_response(
        addr,
        "GET",
        "/api/drinks?query=%E5%92%96%E5%95%A1",
        Some(&token),
        None,
    );
    assert!(generic_coffee["drinks"]
        .as_array()
        .expect("drinks array")
        .iter()
        .any(|drink| drink["id"] == "latte"));

    let order = json_response(
        addr,
        "POST",
        "/api/order/confirm",
        Some(&token),
        Some(json!({"drinkId": "latte", "size": "medium", "sugar": "less"})),
    );
    assert_eq!(order["drinkName"], "Latte");
    assert_eq!(order["payable"], 18);

    let paid = json_response(
        addr,
        "POST",
        "/api/order/pay",
        Some(&token),
        Some(json!({"orderId": order["orderId"]})),
    );
    assert_eq!(paid["status"], "paid");

    assert!(state
        .audit_records()
        .iter()
        .any(|record| record.event == "api.order.pay" && record.outcome == "ok"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn demo_signature_and_replayed_challenge_are_rejected() {
    let fixture = DidFixture::new();
    let (addr, _state) = spawn_server_with_fixture(&fixture).await;
    let challenge = json_response(
        addr,
        "POST",
        "/agents/coffee/auth/challenge",
        None,
        Some(json!({
            "sessionId": "session-1",
            "skillId": "coffee",
            "userDid": fixture.did(),
            "agentDid": "did:wba:agent.example"
        })),
    );

    let (status, body) = request(
        addr,
        "POST",
        "/agents/coffee/auth/login",
        None,
        Some(json!({
            "sessionId": "session-1",
            "skillId": "coffee",
            "userDid": fixture.did(),
            "agentDid": "did:wba:agent.example",
            "merchantDid": challenge["merchantDid"],
            "challengeId": challenge["challengeId"],
            "signedChallenge": {"proof": "demo-signature"}
        })),
    );
    assert_eq!(status, 401);
    assert!(body.contains("invalid_signature"));

    let proof = sign_login_proof(&challenge, &fixture, "session-1", "coffee");
    let body = json!({
        "sessionId": "session-1",
        "skillId": "coffee",
        "userDid": fixture.did(),
        "agentDid": "did:wba:agent.example",
        "merchantDid": challenge["merchantDid"],
        "challengeId": challenge["challengeId"],
        "signedChallenge": proof
    });
    let (first, _) = request(
        addr,
        "POST",
        "/agents/coffee/auth/login",
        None,
        Some(body.clone()),
    );
    assert_eq!(first, 200);
    let (replay, replay_body) =
        request(addr, "POST", "/agents/coffee/auth/login", None, Some(body));
    assert_eq!(replay, 401);
    assert!(replay_body.contains("unknown_challenge"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_scope_mismatch_is_rejected() {
    let fixture = DidFixture::new();
    let (addr, _state) = spawn_server_with_fixture(&fixture).await;
    let challenge = json_response(
        addr,
        "POST",
        "/agents/coffee/auth/challenge",
        None,
        Some(json!({
            "sessionId": "session-1",
            "skillId": "coffee",
            "userDid": fixture.did(),
            "agentDid": "did:wba:agent.example"
        })),
    );
    let proof = sign_login_proof(&challenge, &fixture, "session-1", "coffee");

    let (status, body) = request(
        addr,
        "POST",
        "/agents/coffee/auth/login",
        None,
        Some(json!({
            "sessionId": "session-2",
            "skillId": "coffee",
            "userDid": fixture.did(),
            "agentDid": "did:wba:agent.example",
            "merchantDid": challenge["merchantDid"],
            "challengeId": challenge["challengeId"],
            "signedChallenge": proof
        })),
    );

    assert_eq!(status, 401);
    assert!(body.contains("scope_mismatch"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn business_apis_fail_closed_without_or_with_expired_token() {
    let fixture = DidFixture::new();
    let (addr, state) = spawn_server_with_fixture(&fixture).await;

    let (missing, _body) = request(addr, "GET", "/api/drinks", None, None);
    assert_eq!(missing, 401);

    let expired_token = issue_token(
        state.merchant_did(),
        fixture.did(),
        "coffee",
        "session-1",
        ["coffee:drinks:read"],
        1_000,
    );
    let (expired, _body) = request(addr, "GET", "/api/drinks", Some("expired-token"), None);
    assert_eq!(expired, 403);
    let (expired, _body) = request(addr, "GET", "/api/drinks", Some(&expired_token), None);
    assert_eq!(expired, 403);

    let wrong_scope = issue_token(
        state.merchant_did(),
        fixture.did(),
        "coffee",
        "session-1",
        ["coffee:order:pay"],
        current_time_ms(),
    );
    let (wrong_scope, body) = request(addr, "GET", "/api/drinks", Some(&wrong_scope), None);
    assert_eq!(wrong_scope, 403);
    assert!(body.contains("insufficient_scope"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn audit_endpoint_never_exposes_authorization_values() {
    let fixture = DidFixture::new();
    let (addr, _state) = spawn_server_with_fixture(&fixture).await;
    let token = login(addr, &fixture);
    let _ = request(
        addr,
        "POST",
        "/api/order/confirm",
        Some(&token),
        Some(json!({"drinkId": "latte", "capabilityToken": "real-token"})),
    );

    let (status, audit) = request(addr, "GET", "/audit", None, None);
    assert_eq!(status, 200);

    assert!(!audit.contains("real-token"));
    assert!(audit.contains("[REDACTED]"));
}

fn sign_login_proof(
    challenge: &Value,
    fixture: &DidFixture,
    session_id: &str,
    skill_id: &str,
) -> Value {
    let session = IdentitySession::new(
        fixture.did(),
        Some("did:wba:agent.example".to_owned()),
        challenge["merchantDid"].as_str().expect("merchant DID"),
        skill_id,
        session_id,
    );
    let payload = ChallengeProofPayload::from_challenge(
        &AdapterDidChallenge {
            challenge_id: challenge["challengeId"]
                .as_str()
                .expect("challenge id")
                .to_owned(),
            merchant_did: challenge["merchantDid"]
                .as_str()
                .expect("merchant DID")
                .to_owned(),
            nonce: challenge["nonce"].as_str().expect("nonce").to_owned(),
            expires_at_ms: challenge["expiresAtMs"].as_u64(),
        },
        &session,
        challenge["audience"].as_str().expect("audience"),
        challenge["issuedAtMs"].as_u64().expect("issuedAtMs"),
    );
    let provider =
        FileDidCredentialProvider::from_config(fixture.credential()).expect("credential config");
    serde_json::to_value(
        sign_challenge_proof(&payload, &provider, &session, AuthMode::HttpSignatures)
            .expect("proof signs"),
    )
    .expect("proof serializes")
}

fn issue_token(
    merchant_did: &str,
    user_did: String,
    skill_id: &str,
    session_id: &str,
    scopes: impl IntoIterator<Item = &'static str>,
    now_ms: u64,
) -> String {
    CapabilityTokenIssuer::new(
        CapabilityTokenIssuerConfig::new(
            merchant_did,
            merchant_did,
            "test-only-token-issuer-secret",
        )
        .with_ttl_ms(300_000),
    )
    .expect("issuer config")
    .issue_at(
        CapabilityTokenRequest::new(
            merchant_did,
            user_did,
            Some("did:wba:agent.example".to_owned()),
            skill_id,
            session_id,
            scopes,
        ),
        now_ms,
    )
    .expect("token issues")
    .token
    .value
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or_default()
}

struct DidFixture {
    _dir: TempDir,
    did_document: Value,
    did_path: PathBuf,
    key_path: PathBuf,
}

impl DidFixture {
    fn new() -> Self {
        let bundle = create_did_wba_document("user.example", DidDocumentOptions::default())
            .expect("DID fixture creates");
        let dir = TempDir::new("anp-miniapp-dock-demo-api").expect("temp dir creates");
        let did_path = dir.path().join("did.json");
        let key_path = dir.path().join("key.pem");
        std::fs::write(&did_path, serde_json::to_vec(&bundle.did_document).unwrap()).unwrap();
        std::fs::write(&key_path, &bundle.keys["key-1"].private_key_pem).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        Self {
            _dir: dir,
            did_document: bundle.did_document,
            did_path,
            key_path,
        }
    }

    fn did(&self) -> String {
        self.did_document["id"]
            .as_str()
            .expect("fixture has DID")
            .to_owned()
    }

    fn credential(&self) -> DidCredentialConfig {
        DidCredentialConfig::new(&self.did_path, &self.key_path)
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
