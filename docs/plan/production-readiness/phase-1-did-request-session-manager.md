# Phase 1 子文档：DID Request Session Manager 开发方案

## 1. 目标

把当前 demo 中分散的 DID 登录、token cache 和 `wx.request` Authorization 注入收敛成正式的 `DidAuthSessionManager`。Skill JS 只看到小程序兼容的登录和请求结果，DID 私钥、challenge proof、capability token 全部留在 host/runtime 边界。

## 2. 核心对象

### 2.1 Session Key

```text
serverBaseUrl
merchantDid
userDid
agentDid?
skillId
sessionId
```

所有 token、challenge、request auth 状态都按这个 key 隔离。

### 2.2 Session State

```text
Unknown
  -> ChallengeIssued
  -> ProofSigned
  -> TokenActive
  -> TokenExpired
  -> Revoked
  -> Failed
```

实现可先不暴露完整状态机，但内部日志和测试应能区分：未登录、登录中、token 有效、token 过期、已撤销、失败。

### 2.3 Token Claims

必须包含：

- `iss`
- `aud`
- `sub`
- `merchantDid`
- `userDid`
- `agentDid?`
- `skillId`
- `sessionId`
- `scopes`
- `iat` / `nbf` / `exp`
- `jti`
- `version`

## 3. 登录流程

```text
wx.login()
  -> DidAuthSessionManager.ensure_session(baseUrl, context)
  -> if cached active token exists: return receipt
  -> POST /agents/{id}/auth/challenge
  -> build ChallengeProofPayload
  -> sign proof with ANP Rust SDK credential provider
  -> POST /agents/{id}/auth/login
  -> verify response shape and scopes
  -> cache token internally
  -> return { code, errMsg: 'login:ok', didAuth: redacted evidence }
```

JS 可见字段建议：

```json
{
  "code": "dock-login-receipt-...",
  "errMsg": "login:ok",
  "didAuth": {
    "status": "ok",
    "tokenReceived": true,
    "tokenVisibleToSkill": false,
    "userDid": "did:wba:...",
    "merchantDid": "did:wba:...",
    "scopes": ["coffee:drinks:read"]
  }
}
```

`didAuth` 可按生产策略关闭或只用于 debug。无论如何不得包含 raw token/proof。

## 4. `wx.checkSession`

```text
wx.checkSession()
  -> lookup session key
  -> verify token exists and exp > now + skew
  -> optionally verify revocation state
  -> success or fail
```

失败场景：

- 未登录；
- token 过期；
- token revoked；
- scope 不满足；
- storage/cache 不可用。

## 5. `wx.request` Auth 注入

```text
wx.request(options)
  -> normalize URL and method
  -> request allowlist check
  -> remove/deny JS-provided Authorization
  -> ensure_session when endpoint requires auth
  -> attach Authorization: Bearer <cached token>
  -> send through HttpTransport
  -> if 401 and safe to retry: clear token, re-login, retry once
  -> redact response headers before JS/log/audit when needed
```

## 6. Server Contract

### 6.1 Challenge response

```json
{
  "challengeId": "...",
  "merchantDid": "did:wba:merchant.example",
  "nonce": "...",
  "issuedAtMs": 1781190000000,
  "expiresAtMs": 1781190300000,
  "audience": "http://127.0.0.1:3000"
}
```

### 6.2 Login request

```json
{
  "sessionId": "session-cli",
  "skillId": "coffee",
  "userDid": "did:wba:user.example",
  "agentDid": "did:wba:agent.example",
  "merchantDid": "did:wba:coffee-merchant.example",
  "challengeId": "...",
  "signedChallenge": { "type": "anp-http-signature/v1" }
}
```

### 6.3 Login response

```json
{
  "capabilityToken": "<redacted in all logs>",
  "expiresAtMs": 1781190300000
}
```

## 7. Scope Strategy

初始 scope 可沿用 coffee demo：

- `coffee:drinks:read`
- `coffee:order:confirm`
- `coffee:order:pay`
- `coffee:order:read`

产品化后需要从 Skill manifest / merchant manifest / policy 推导 scope。推导必须可审计，不能由 Skill JS 自行声明后无条件信任。

## 8. 测试矩阵

| 场景 | 期望 |
|---|---|
| 首次 login | challenge + proof + token cache |
| 重复 login | 命中缓存或按策略 refresh |
| expired token | checkSession fail，request 触发 refresh |
| wrong DID document | login fail |
| replay challenge | login fail |
| wrong audience | login fail |
| missing scope | business API 403/401 |
| JS Authorization header | 被剥离或拒绝 |
| log/audit/CLI output | token/proof/signature 全部 redacted |

## 9. 完成标准

- [ ] `DidAuthSessionManager` 有清晰 public API 或 crate 内接口。
- [ ] `wx.login`、`wx.checkSession`、`wx.request` 均复用该 manager。
- [ ] demo-server 与 FastAPI 示例使用同一 JSON contract。
- [ ] 所有失败路径 fail closed 并脱敏。
