# DID 身份、wx.login/wx.request 与 Python 远端服务整合方案

## 1. 背景与目标

远端最新代码已经引入真实 DID challenge、DID 签名验证、scoped capability token 等能力；本地已经完成 `wx.login`、`wx.request`、localhost FastAPI 咖啡服务以及 Mac Chatbot Demo。当前目标不是重写点单业务，而是把两条链路融合成一条完整演示链路：

1. 用户在 PC/Mac Chatbot 中输入「我要点咖啡」。
2. Chatbot 调用 OpenAI-compatible 模型做意图识别。
3. 模型/宿主调用本地 MiniApp 容器中的 Coffee Skill。
4. Skill 在原子 API 中触发 `wx.login()` 和 `wx.request()`。
5. 容器侧在 `wx.login`/首次 HTTP 请求边界触发 DID challenge、DID 签名和身份认证。
6. 远端 Python FastAPI 服务使用 ANP/AMP Python SDK 校验 DID 身份，签发/校验 scoped capability token。
7. 登录成功后业务 HTTP API 返回咖啡数据、订单确认、支付结果。
8. 容器渲染 Skill 返回的 MiniApp MCP 组件，Mac Chatbot 原生展示组件。

> 注：下文统一使用「ANP Python SDK」指当前 PyPI/GitHub 上的 `anp` SDK。如果后续确认内部命名为 AMP SDK，应在实现阶段把包名和 API import 调整为实际 SDK。

## 2. 实施状态（2026-06-11）

本方案已按当前迭代完成第一轮落地，后续章节保留原始 Review 和设计依据，便于追溯。已实现内容：

- **Demo / Mac 侧**：`DemoPipelineRunner` 优先启动 FastAPI 远端示例并注入 DID/auth 环境；FastAPI 不可用时 fallback 到 Rust `demo-server`；`PipelineSnapshot`、headless 输出和 SwiftUI 新增 DID/Auth Evidence。
- **容器侧身份**：QuickJS `wx.login()` / `wx.request()` 接入 host-side DID challenge 登录，使用 `examples/identity` 中的 DID 文档和私钥，通过 Rust ANP experimental SDK / `anp-adapter` 生成 `DockDidChallengeProof`；capability token 由容器缓存并在 `wx.request` 时自动附加 Bearer，Skill JS 不读取 raw token。
- **Skill JS**：Coffee Skill 保留本地 Skill 包加载；远端 HTTP 模式下调用 `wx.login()` 和 `/api/login` 只确认登录状态，后续业务请求由容器托管 Authorization；`_meta` 明确 `authBoundary=container-managed`、`tokenVisibleToSkill=false`。
- **Python FastAPI 远端示例**：`examples/coffee-fastapi-server/auth.py` 实现 Rust-compatible DID challenge proof 校验、ANP Python SDK 优先尝试、Ed25519/Multikey fallback verifier、HS256 scoped `dock.capability.v1` token；`/api/login` 改为要求 Bearer token 且不签发任意 code token；业务 API 按 route scope 校验。
- **Rust demo-server fallback**：`/api/login` 对齐容器托管 token 模式，要求 Bearer 后返回 redacted 登录状态；Rust fallback 与 FastAPI 共享同一 Skill 远端调用语义。
- **渲染链路**：点咖啡流程继续使用原有 `drink-list`、`order-confirm`、`payment-result` 原生组件渲染；Mac UI/headless 输出包含 auth evidence。

本轮已验证：

```bash
python3 -m json.tool examples/coffee-skill/mcp.json >/dev/null
node --check examples/coffee-skill/index.js
for f in examples/coffee-skill/apis/*.js examples/coffee-skill/components/*/index.js; do node --check "$f"; done
./examples/coffee-fastapi-server/.venv/bin/python -m py_compile examples/coffee-fastapi-server/auth.py examples/coffee-fastapi-server/app.py
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -q -p dock-cli -- run-demo --skill examples/coffee-skill --server http://127.0.0.1:8008
cargo run -q -p dock-cli -- run-demo --skill examples/coffee-skill --server http://127.0.0.1:3000
cd mac-app/AnpMiniappDockMac && swift build
cd mac-app/AnpMiniappDockMac && ANP_DOCK_MAC_HEADLESS=1 ANP_DOCK_CHAT_PROMPT='我要点咖啡' swift run AnpMiniappDockMac
cd mac-app/AnpMiniappDockMac && xcodebuild -project AnpMiniappDockMac.xcodeproj -scheme AnpMiniappDockMac -configuration Debug -destination 'platform=macOS' build
```

端到端输出确认：FastAPI 服务 `coffee-fastapi-server`、DID challenge verified、`requestAuthMode=container-attached-bearer`、支付状态 `paid`。

## 2. 当前代码现状 Review

### 2.1 Demo / Mac Chatbot 侧

已有能力：

- `mac-app/AnpMiniappDockMac` 已提供 SwiftUI Chatbot UI。
- `PipelineViewModel.swift` 已从 `source ~/.zshrc` 读取 `OPENAI_BASE_URL`、`OPENAI_API_KEY`、`OPENAI_MODEL`。
- `ChatbotTurnRunner` 在识别 coffee intent 后调用 `DemoPipelineRunner`。
- `DemoPipelineRunner` 目前优先启动 `examples/coffee-fastapi-server/.venv/bin/uvicorn`，否则 fallback 到 Rust `demo-server`。
- `dock-cli run-demo` 已支持从 `examples/identity/did_document.json` 和 `examples/identity/key-1-private.pem` 读取默认 DID 身份。

缺失/待调整：

- Mac Demo 没有显式展示「当前使用的 DID」「认证状态」「token scope」等身份链路证据。
- FastAPI 方案与 Rust `demo-server` 方案的认证能力不一致：Rust server 已是 DID proof + scoped token，FastAPI 仍是 mock challenge/login。
- `DemoPipelineRunner` 对 FastAPI 和 Rust server 的启动参数、identity 配置、认证能力假设需要统一。
- Headless 输出的 `PipelineSnapshot` 可看到 `tokenReceived`，但还不能区分 DID proof 登录成功、wx.login 兼容登录成功、业务 route scope 校验成功。

### 2.2 容器 / QuickJS / wx 兼容层

已有能力：

- `crates/js-runtime-quickjs/src/bridge.rs` 已暴露：
  - `wx.login(options)` → `__dock.login()`
  - `wx.request(options)` → `__dock.request(optionsJson)`
- `crates/js-runtime-quickjs/src/api_vm.rs` 已有本地 HTTP bridge：
  - `host_login_json()` 返回固定 `dock-login-code-localhost`。
  - `host_request_json()` 解析 `wx.request` 参数并通过 `TcpStream` 请求 localhost。
  - 限制只允许 loopback URL，避免 Skill 任意访问外网。
- `ApiCallContext` 已包含 `user_did`、`agent_did`、`merchant_did`、`skill_id`、`session_id`、`capability_token`。
- `dock-cli RuntimeHarness` 已能把 DID identity 注入运行时上下文。
- `crates/anp-adapter` 已提供 DID credential、challenge proof 签名、challenge proof 验证、capability token issuer/verifier 等 Rust 侧能力。

缺失/待调整：

- `host_login_json()` 仍是固定 mock code，未触发 DID challenge/sign/login。
- `host_request_json()` 目前只是裸 HTTP 转发，未接入 token cache，也不会自动附加 Bearer token。
- QuickJS `install_host_bridge()` 当前只捕获 modules/console，没有捕获当前 `ApiCall` identity、credential provider、auth session manager。
- `wx-compat::RequestBroker` trait 已存在，但 QuickJS bridge 当前没有通过该 trait 统一请求、权限、allowlist、DID auth、redaction。
- 当前 Coffee Skill JS 会把 `/api/login` 返回的 `accessToken` 读到 JS 里并设置 `Authorization`。这对 demo 可用，但不符合长期安全目标：原始 capability token 不应暴露给 Skill JS 或模型可见输出。

### 2.3 远端 Python FastAPI 示例

已有能力：

- `examples/coffee-fastapi-server/app.py` 提供 localhost FastAPI coffee server。
- 已有 endpoints：
  - `GET /health`
  - `GET /registry/agents`
  - `GET /agents/coffee/manifest`
  - `GET /agents/coffee/SKILL.md`
  - `GET /agents/coffee/mcp.json`
  - `POST /agents/coffee/auth/challenge`
  - `POST /agents/coffee/auth/login`
  - `POST /api/login`
  - `GET /api/drinks`
  - `POST /api/order/confirm`
  - `POST /api/order/pay`
  - `GET /audit`
- 业务 token 当前是内存 token，能保护 drinks/order/pay。

缺失/待调整：

- `/agents/coffee/auth/challenge` 返回字段不完整，缺少 Rust 侧 challenge proof 所需的 `issuedAtMs` 和 `audience`。
- `/agents/coffee/auth/login` 目前只检查 `signedChallenge` 中是否存在 `proof` 或 `signature`，不做真实 DID 签名验证。
- `/api/login` 当前只校验 `code` 非空，然后直接签发 token，未绑定 DID proof、session、skill、agent、merchant。
- capability token 是自定义内存 token，不是与远端 Rust 新能力一致的 scoped capability JWT 或 SDK token。
- 缺少 FastAPI middleware / dependency 层来统一校验 Bearer token、scope、session、skill、DID。
- 缺少配置项来声明 trusted DID document、merchant DID、token issuer secret、token TTL、challenge TTL。
- 缺少 Python SDK 依赖和 SDK API 使用示例。

### 2.4 渲染链路

已有能力：

- `component-runtime` 能把 `drink-list`、`order-confirm`、`payment-result` 组件渲染为 Render IR。
- Mac Chatbot 已把 pipeline summary 和 component steps 渲染成 SwiftUI 卡片。
- 组件 action 继续走 `api/call` → `confirmOrder` → `payOrder` 的现有流程。

缺失/待调整：

- UI 目前渲染的是 summary 风格卡片，不是完整逐节点 Render IR 映射。
- UI 缺少 DID/auth 证据区，例如 challenge id、user DID、merchant DID、scopes、token 是否 redacted、认证服务类型（FastAPI/Rust）。
- 需要确保 raw JSON、log、error 中不会显示 `Authorization`、`Signature`、`capabilityToken`、private key path 等敏感信息。

## 3. 目标端到端流程

### 3.1 推荐目标流程（token 由容器托管）

```text
Chatbot 用户输入
  -> LLM intent router: coffee_order
  -> DemoPipelineRunner / dock-cli run-demo
  -> dock-core Orchestrator.call_api(searchDrinks)
  -> QuickJS Coffee Skill API
  -> Skill calls wx.login()
  -> Container DidAuthSessionManager:
       1. Discover/resolve remote auth endpoints
       2. POST /agents/coffee/auth/challenge
       3. Use Rust ANP experimental SDK/anp-adapter with Example Identity private key to sign challenge
       4. POST /agents/coffee/auth/login
       5. Cache scoped capability token by merchant DID + user DID + agent DID + skill ID + session ID
  -> wx.login returns MiniApp-compatible success payload, but not raw token
  -> Skill calls wx.request(/api/login or /api/drinks)
  -> Container RequestBroker attaches Authorization: Bearer <cached token>
  -> FastAPI auth middleware verifies token/scope with ANP Python SDK or compatible verifier
  -> Business API returns data
  -> AtomicApiResult contains structuredContent + component path
  -> Component Runtime renders Render IR
  -> Mac Chatbot renders native cards/components
```

这种方式的关键安全边界是：

- JS Skill 只知道 `wx.login` 成功，不拿 DID private key。
- JS Skill 不直接保存 raw capability token；Authorization 由容器 `RequestBroker` 自动注入。
- DID challenge proof 和 token cache 属于 host/container side。
- 模型可见输出、UI log、audit 都只显示 redacted token/proof 状态。

### 3.2 兼容过渡流程（保留 `/api/login`）

为了最小化对当前 Coffee Skill JS 的改动，可以保留 `/api/login`：

1. `wx.login()` 在容器侧完成 DID challenge/login，并创建 session-bound login receipt/code。
2. Skill 仍调用 `wx.request({ url: /api/login, data: { code } })`。
3. `RequestBroker` 对 `/api/login` 也自动附加 Bearer token 或 signed login receipt。
4. Python `/api/login` 校验 token/receipt 后只返回登录状态、DID/account 信息，不再返回 raw token；如需短期兼容，可返回 `tokenReceived: true` 而不是 `accessToken`。
5. 后续业务 `wx.request` 均由容器自动加 `Authorization`。

如果必须保持现有 Skill JS 完全不变，则 `/api/login` 仍需返回 `accessToken`，但这应标记为 **demo-only fallback**，并在后续迭代中移除。

## 4. 详细修改方案

### 4.1 Demo 侧改造

#### 4.1.1 Mac Chatbot / PipelineRunner

建议修改文件：

- `mac-app/AnpMiniappDockMac/Sources/AnpMiniappDockMac/DemoPipelineRunner.swift`
- `mac-app/AnpMiniappDockMac/Sources/AnpMiniappDockMac/PipelineSnapshot.swift`
- `mac-app/AnpMiniappDockMac/Sources/AnpMiniappDockMac/ContentView.swift`
- `mac-app/AnpMiniappDockMac/README.md`

改造点：

1. 统一 identity 配置来源：
   - 优先读取 env：`ANP_DOCK_DID_DOCUMENT`、`ANP_DOCK_PRIVATE_KEY`、`ANP_DOCK_USER_DID`、`ANP_DOCK_AGENT_DID`。
   - 默认 fallback 到 `examples/identity/did_document.json` 和 `examples/identity/key-1-private.pem`。
   - 不在 UI/log 中打印 private key path 或 key content。

2. FastAPI server 启动参数增加 DID/auth 配置：
   - `ANP_COFFEE_MERCHANT_DID`
   - `ANP_COFFEE_TRUSTED_DID_DOCUMENT`
   - `ANP_COFFEE_TOKEN_ISSUER_SECRET`
   - `ANP_COFFEE_AUTH_MODE=anp-http-signature/v1`

3. `PipelineSnapshot` 增加认证证据字段：
   - `authProvider`: `fastapi-anp` / `rust-demo-server`
   - `userDid`
   - `agentDid`
   - `merchantDid`
   - `challengeVerified: Bool`
   - `tokenScopes: [String]`
   - `wxLoginStatus`
   - `requestAuthMode`: `container-attached-bearer`

4. UI 增加「DID/Auth Evidence」卡片：
   - 显示 DID、merchant、session、skill、scopes。
   - 显示 token/proof 只用 `[REDACTED]`。
   - 显示 FastAPI SDK 校验是否通过。

5. Headless smoke 输出扩展：
   - 输出 `auth.challengeVerified`、`auth.tokenReceived`、`auth.scopes`。
   - 保留对比 Rust server 与 FastAPI server 的能力。

#### 4.1.2 dock-cli run-demo

建议修改文件：

- `crates/dock-cli/src/commands.rs`

改造点：

1. 当前 `run-demo` 在 runtime 之外先做了一次 `/auth/challenge` + `/auth/login`，用于 server business check。整合后应变成：
   - `run-demo` 仍可做 server preflight auth，验证远端服务可用。
   - 真正的 Skill 内 HTTP 请求 auth 由 `wx.login`/`RequestBroker` 触发，覆盖用户要求的「小程序启动并调用容器内 HTTP 请求时触发登录」。

2. 新增/调整参数：
   - `--auth-mode did-challenge` 默认。
   - `--request-auth-mode container-managed-token` 默认。
   - `--server-kind fastapi-anp|rust-demo` 可选，用于输出证据。

3. RuntimeHarness 加载时注入身份配置和 auth client：
   - `RuntimeIdentity` 目前只有 DID 字符串，需要再持有 credential config 或 credential provider handle。
   - `QuickJsApiExecutor` 需要能拿到 `DidCredentialProvider` 或 `DidAuthSessionManager`。

### 4.2 容器侧 DID 身份与 wx bridge 改造

#### 4.2.1 新增 DidAuthSessionManager

建议新增/修改模块：

- `crates/anp-adapter/src/challenge.rs`
- `crates/anp-adapter/src/signed_request.rs`
- `crates/js-runtime-quickjs/src/api_vm.rs`
- `crates/wx-compat/src/request.rs`
- 可能新增 `crates/wx-compat/src/auth.rs` 或 `crates/anp-adapter/src/login.rs`

职责：

1. 以 `(merchant_did, user_did, agent_did, skill_id, session_id, server_base_url)` 为 key 管理登录状态。
2. 首次 `wx.login()` 或首次需要授权的 `wx.request()` 时：
   - POST `${server}/agents/coffee/auth/challenge`
   - 构造 `IdentitySession`
   - 调用 Rust ANP 实验 SDK / 当前 `anp-adapter::sign_challenge_proof`
   - POST `${server}/agents/coffee/auth/login`
   - 验证响应中 capability token 非空、未过期
   - 写入 token cache
3. token 未过期时直接复用。
4. 收到 401/403/expired token 时清 cache 并重试一次登录。
5. 所有错误 redacted 后返回给 JS，不包含私钥、raw token、signature。

#### 4.2.2 调整 `wx.login()` 语义

当前：

```rust
fn host_login_json() -> String {
    { "code": "dock-login-code-localhost", "errMsg": "login:ok" }
}
```

建议：

1. `host_login_json(call_context, auth_manager)` 触发真实 DID login。
2. 返回 MiniApp-compatible payload：

```json
{
  "code": "dock-login-code-<session>",
  "errMsg": "login:ok",
  "didAuth": {
    "status": "ok",
    "userDid": "did:wba:...",
    "merchantDid": "did:wba:coffee-merchant.example",
    "tokenReceived": true
  }
}
```

3. 不返回 `capabilityToken` 或 `Authorization`。
4. 对 callback 风格和 Promise 风格保持兼容。

#### 4.2.3 调整 `wx.request()` 语义

当前：裸 `TcpStream` + localhost allowlist。

建议：

1. 将 `host_request_json()` 改为调用统一 `RequestBroker`。
2. `RequestBroker` 处理：
   - URL allowlist（demo 只允许 `127.0.0.1`/`localhost`）。
   - 自动调用 `DidAuthSessionManager.ensure_login()`。
   - 自动附加 `Authorization: Bearer <capability-token>`。
   - 对 `/api/login`、`/api/drinks`、`/api/order/*` 执行不同 required scope。
   - 401/403 retry once。
3. JS 传入的 `header.Authorization` 不应覆盖 host-managed token；如存在，应忽略或报错。
4. 返回结构保持 `wx.request` 兼容：`statusCode`、`header`、`data`、`errMsg`。

#### 4.2.4 Coffee Skill JS 调整

建议修改文件：

- `examples/coffee-skill/index.js`
- `examples/coffee-skill/apis/searchDrinks.js`
- `examples/coffee-skill/apis/confirmOrder.js`
- `examples/coffee-skill/apis/payOrder.js`

改造方向：

1. `login(ctx)` 只触发 `wx.login()`，不读取 raw token。
2. `/api/login` 可保留为兼容登录/账户绑定接口，但返回值不应驱动 JS 设置 Authorization。
3. `request(ctx, path, options)` 调用 `wx.request()` 时不传 `Authorization`，由 host bridge 自动添加。
4. `_meta` 中可保留：
   - `remoteLogin: 'wx.login+did-challenge'`
   - `authBoundary: 'container-managed'`
   - `tokenVisibleToSkill: false`

### 4.3 Python FastAPI 远端示例改造

#### 4.3.1 依赖与配置

建议修改：

- `examples/coffee-fastapi-server/requirements.txt`
- `examples/coffee-fastapi-server/app.py`
- 新增 `examples/coffee-fastapi-server/auth.py`
- 新增 `examples/coffee-fastapi-server/README.md` 认证说明

建议依赖：

```text
fastapi>=0.111,<1
uvicorn[standard]>=0.30,<1
pydantic>=2,<3
anp>=0.8.7
PyJWT>=2,<3      # 如果 SDK 不直接提供 capability JWT issuer/verifier，则用于 demo token
```

配置项：

```bash
export ANP_COFFEE_MERCHANT_DID=did:wba:coffee-merchant.example
export ANP_COFFEE_PUBLIC_BASE_URL=http://127.0.0.1:8008
export ANP_COFFEE_TRUSTED_DID_DOCUMENT=../identity/did_document.json
export ANP_COFFEE_TOKEN_ISSUER_SECRET=test-only-local-secret
export ANP_COFFEE_CHALLENGE_TTL_MS=300000
export ANP_COFFEE_TOKEN_TTL_MS=900000
```

#### 4.3.2 Challenge/Login endpoints

`POST /agents/coffee/auth/challenge` 应返回：

```json
{
  "challengeId": "challenge-...",
  "merchantDid": "did:wba:coffee-merchant.example",
  "nonce": "nonce-...",
  "issuedAtMs": 1710000000000,
  "expiresAtMs": 1710000300000,
  "audience": "http://127.0.0.1:8008/agents/coffee/auth/login"
}
```

`POST /agents/coffee/auth/login` 应：

1. 查找 challenge，校验未过期且一次性消费。
2. 校验 request 中 `sessionId`、`skillId`、`userDid`、`agentDid`、`merchantDid` 与 challenge request 一致。
3. 使用 ANP Python SDK 中间件/底层接口验证 `signedChallenge`：
   - 当前 SDK 文档显示 DID-WBA 默认使用 HTTP Message Signatures（`Signature-Input` / `Signature` / 可选 `Content-Digest`）。
   - Rust 侧 `DockDidChallengeProof` 包含 `type=anp-http-signature/v1`、`method=POST`、`url`、`headers`、`payload`，Python 侧需要用 SDK verify API 对同一 payload 做签名校验。
4. DID document 来源：
   - demo 中从 `examples/identity/did_document.json` 加载 trusted DID document。
   - 实现时可先做静态 resolver，后续再替换成真实 DID resolver。
5. 校验通过后签发 scoped capability token：
   - `coffee:drinks:read`
   - `coffee:order:confirm`
   - `coffee:order:pay`
   - `coffee:order:read`
6. 返回：

```json
{
  "capabilityToken": "<redacted in logs only, actual response carries token>",
  "expiresAtMs": 1710000900000,
  "scopes": ["coffee:drinks:read", "coffee:order:confirm", "coffee:order:pay", "coffee:order:read"]
}
```

#### 4.3.3 Auth middleware / dependency

新增 FastAPI dependency：

```text
require_capability(required_scope)
```

职责：

1. 读取 `Authorization: Bearer <token>`。
2. 验证 token 签名、issuer、audience、merchant DID、user DID、agent DID、skill ID、session ID、expiry。
3. 校验 required scope。
4. 将 claims 放入 request state / route dependency result。
5. 审计记录中只存 redacted summary。

业务路由绑定：

- `GET /api/drinks` → `coffee:drinks:read`
- `POST /api/order/confirm` → `coffee:order:confirm`
- `POST /api/order/pay` → `coffee:order:pay`
- `GET /api/order/{order_id}` → `coffee:order:read`
- `POST /api/login` → 可接受已经登录的 token，做 account binding/status，不直接签发无 DID token。

#### 4.3.4 `/api/login` 兼容策略

短期推荐：

- `/api/login` 要求 Bearer token。
- 如果没有 Bearer token，返回 401 `missing_token`，由容器 `RequestBroker` 负责先完成 DID login 并重试。
- 返回：

```json
{
  "loginStatus": "ok",
  "userDid": "did:wba:...",
  "merchantDid": "did:wba:coffee-merchant.example",
  "tokenReceived": true,
  "capabilityToken": "[REDACTED]"
}
```

长期推荐：

- Coffee Skill 不再显式调用 `/api/login`；`wx.login` 已足够完成 DID auth，业务请求直接走 `/api/drinks`。

### 4.4 渲染与交互改造

建议：

1. 业务组件保持现状：
   - `drink-list`
   - `order-confirm`
   - `payment-result`
2. Mac Chatbot 增加身份链路证据卡：
   - `Intent: coffee_order`
   - `Skill: examples/coffee-skill`
   - `DID Auth: verified`
   - `User DID`
   - `Merchant DID`
   - `Scopes`
   - `Token: [REDACTED]`
   - `Remote: FastAPI + ANP Python SDK`
3. 原生渲染下一步增强：
   - 当前是 summary cards。
   - 后续可将 Render IR node types 映射到 SwiftUI 原生控件，实现真正的 Render IR native renderer。
4. 所有 log/raw JSON 展示前走 redaction：
   - `Authorization`
   - `Signature`
   - `Signature-Input`
   - `Content-Digest`
   - `capabilityToken`
   - `accessToken`
   - `privateKey`
   - `key-1-private.pem`

## 5. 缺失清单与补齐优先级

| 优先级 | 缺失项 | 影响 | 补齐方案 |
| --- | --- | --- | --- |
| P0 | QuickJS `wx.login` 仍是 mock code | 无法满足「小程序触发 DID 登录」 | 注入 `DidAuthSessionManager`，在 `wx.login` 中执行 challenge/sign/login |
| P0 | QuickJS `wx.request` 不自动附加 token | 业务请求无法通过 Python DID auth | 改为 RequestBroker，自动 Bearer token、retry、scope mapping |
| P0 | FastAPI auth/login 只 mock proof | 服务端没有真实身份校验 | 接入 ANP Python SDK verify DID-WBA HTTP signature |
| P0 | FastAPI challenge 字段不完整 | Rust proof payload 无法严格匹配 | 增加 `issuedAtMs`、`audience`，绑定 session/skill/user/agent |
| P0 | `/api/login` 无 DID 绑定 | 任意 code 可登录 | 要求 Bearer token 或 signed login receipt |
| P0 | Token 暴露给 Skill JS | 不符合安全边界 | 改为 container-managed token，JS 不读写 Authorization |
| P1 | Demo UI 缺少 DID 证据 | 不利于演示完整链路 | 增加 DID/Auth evidence card |
| P1 | FastAPI/Rust auth 行为不一致 | Demo 路径分裂 | 对齐 challenge/login/token/scopes/error shape |
| P1 | 缺少跨语言 contract 文档 | Rust/Python 容易偏差 | 在 docs 中定义 challenge proof、token claims、error code |
| P2 | Mac 只渲染 summary cards | 原生渲染不完整 | 后续实现 Render IR → SwiftUI node renderer |
| P2 | 没有并发/缓存策略 | 多 API 调用重复登录 | session-level token cache + expiry + singleflight |

## 6. 建议实施阶段

### Phase 1：定义跨语言认证契约

产出：

- `docs/architecture/did-http-auth-contract.md` 或扩展本方案。
- 明确定义：
  - challenge request/response schema
  - login request/response schema
  - `DockDidChallengeProof` JSON schema
  - capability token claims/scopes
  - error code
  - redaction policy

验收：

- Rust `demo-server` 与 Python FastAPI 都能按照同一 schema 返回/校验。

### Phase 2：容器侧 DID login manager

产出：

- Rust container 中新增/扩展 `DidAuthSessionManager`。
- `wx.login` 触发 DID challenge/sign/login。
- `wx.request` 自动带 Bearer token。

验收：

- 在 Skill JS 不接触 private key 的前提下完成登录。
- JS 不需要手动设置 Authorization。
- 401/expired token 可重登一次。

### Phase 3：Python FastAPI + ANP Python SDK

产出：

- FastAPI auth module。
- DID-WBA proof verification。
- scoped capability token issuer/verifier。
- route dependencies/middleware。

验收：

- 错误 proof 被拒绝。
- DID mismatch 被拒绝。
- 缺少 scope 被拒绝。
- 正确 DID + scope 可以访问 coffee API。

### Phase 4：Demo UI 与渲染证据完善

产出：

- Chatbot auth evidence card。
- Headless summary 扩展认证字段。
- Log redaction 扩展。

验收：

- 用户输入「我要点咖啡」后能看到：intent、DID auth verified、业务组件、支付结果。
- UI/log 中无 raw token/private key/signature。

### Phase 5：测试与回归

建议测试（实现阶段再执行）：

- Rust：
  - `cargo test -p anp-adapter`
  - `cargo test -p js-runtime-quickjs`
  - `cargo test -p dock-cli --test coffee_order_flow`
  - `cargo test --workspace`
- Python：
  - FastAPI auth unit tests：challenge expiry、proof mismatch、scope mismatch、expired token。
  - HTTP integration：`wx.login` + `wx.request` path。
- Mac：
  - `swift build`
  - `ANP_DOCK_DISABLE_OPENAI=1 ANP_DOCK_MAC_HEADLESS=1 swift run`
  - `xcodebuild ... build`

## 7. 推荐最终目录/文件变更

```text
crates/anp-adapter/
  src/challenge.rs              # 复用/稳定 challenge proof sign/verify
  src/token.rs                  # 复用 scoped capability token
  src/login.rs                  # 可新增：client-side challenge login helper

crates/wx-compat/
  src/request.rs                # RequestBroker 扩展 request auth 语义
  src/auth.rs                   # 可新增：wx.login -> DID auth 抽象

crates/js-runtime-quickjs/
  src/api_vm.rs                 # install_host_bridge 捕获 ApiCallContext/auth manager
  src/bridge.rs                 # wx.login/wx.request JS 兼容语义不变

crates/dock-cli/
  src/commands.rs               # run-demo 注入 credential/auth manager；输出 auth evidence

examples/coffee-skill/
  index.js                      # 不再读取 raw token；保留 wx.login + wx.request flow
  apis/*.js                     # 继续请求 localhost service；Authorization 交给 host

examples/coffee-fastapi-server/
  app.py                        # route 组装
  auth.py                       # ANP Python SDK DID verify + token middleware
  requirements.txt              # 添加 anp / token 依赖
  README.md                     # 运行和安全说明

mac-app/AnpMiniappDockMac/
  Sources/.../PipelineSnapshot.swift
  Sources/.../ContentView.swift
  Sources/.../DemoPipelineRunner.swift
```

## 8. 关键安全要求

1. 私钥只在容器 host side 被 Rust SDK/credential provider 读取，不能进入 QuickJS。
2. raw capability token 默认不进入 Skill JS；由 RequestBroker 自动附加。
3. 如果短期需要返回 token 给 JS，必须标注 demo-only，并确保输出 redaction。
4. Python server 不能接受任意 `code` 登录，必须绑定 DID proof/token。
5. challenge 必须一次性使用，并校验 TTL。
6. token 必须有 scope、merchant、user、agent、skill、session、expiry。
7. 所有审计和 UI 输出都必须 redacted。
8. localhost demo 可以使用 HTTP；非 localhost 必须使用 HTTPS 或明确拒绝。

## 9. 外部参考

- ANP Python SDK / AgentConnect GitHub: https://github.com/agent-network-protocol/anp
- PyPI `anp` package: https://pypi.org/project/anp/
- 当前 SDK 文档显示 DID-WBA 默认使用 HTTP Message Signatures（`Signature-Input` / `Signature` / `Content-Digest`），并提供 FastAPI/OpenANP、DID-WBA authentication 示例入口。实现阶段应以实际安装版本的 API 为准。

## 10. 最小可交付验收标准

完成整合后，最小验收流应满足：

1. 启动 FastAPI Python coffee service。
2. 打开 Mac Chatbot Demo。
3. 输入「我要点咖啡」。
4. Chatbot 识别 coffee intent。
5. Coffee Skill 调用 `wx.login`。
6. 容器使用 Example Identity DID 文档和私钥完成 challenge 签名。
7. Python server 使用 ANP Python SDK 校验 DID proof。
8. Python server 签发 scoped token。
9. `wx.request` 自动携带 token 请求 `/api/drinks`。
10. Python server 校验 scope 后返回 drinks。
11. 容器返回 `drink-list` 组件，Mac 原生渲染。
12. 后续 `confirmOrder`、`payOrder` 沿用现有组件 action 流程。
13. UI 展示 DID/auth evidence，所有敏感信息 redacted。
