# Phase 1 子文档：wx API Bridge Contract

> 状态：Step 01-01 冻结契约
> 冻结日期：2026-06-12
> 关联矩阵：[`../../architecture/wx-api-compatibility-matrix.md`](../../architecture/wx-api-compatibility-matrix.md)

## 1. 目标

本文定义 Atomic API VM 中 `wx.*` bridge 的统一契约。实现前先冻结此契约，避免每个 API 分别实现 callback、Promise、错误和权限逻辑，造成行为不一致。

本文的冻结决策适用于 Step 01-02 之后新增或迁移的 `wx.*` / `wx.modelContext` JS bridge。Step 01-04 已将 `wx.login` / `wx.checkSession` / `wx.request` 接入本契约约束：Host DID 配置下复用 `DidAuthSessionManager`，`wx.request` 经 `wx-compat::RequestBroker` trait 的本地 DID broker 执行；无 Host DID 配置和 loopback transport 仍是 demo-only / host-boundary，不是 production Host RequestBroker。

## 2. 冻结决策摘要

| 决策项 | 冻结行为 |
|---|---|
| 异步 API 返回 | 除同步 API 外，所有 `wx.*` API 均返回 Promise，并同时支持 `success` / `fail` / `complete` callback。 |
| 成功结果 | `success(result)` -> `complete(result)` -> Promise resolve，三者使用同一个脱敏后的 JS result。 |
| 失败结果 | `fail(result)` -> `complete(result)` -> Promise reject，reject reason 是同一个脱敏后的 JS result。 |
| `wx.request` HTTP status | 只要 RequestBroker 收到 HTTP response，包含 4xx/5xx，都视为 transport success：`errMsg: "request:ok"`、调用 `success`、Promise resolve，并返回 `statusCode` / `header` / `data`。业务用 `statusCode` 判断应用层失败。 |
| `wx.request` broker failure | allowlist deny、JS-provided `Authorization`、auth material 生成失败、网络传输失败、timeout、invalid options、provider unavailable 均为 `request:fail <code>`，调用 `fail`、Promise reject。 |
| unsupported API | API 必须存在函数；返回 deterministic failure：`<api>:fail unsupported`，包含 safe `reason` / `suggestion`，调用 `fail`、Promise reject。 |
| 同步 API | 不接受 callback，不返回 Promise；成功直接返回值，失败抛出带脱敏 `errMsg` / `code` 的 `Error`。 |
| callback exception | callback 抛错不改变 broker outcome。wrapper 必须尽力继续调用 `complete`，并把 callback exception 作为 redacted diagnostic 记录到 Host 日志或 audit summary；Promise 仍按原始 `WxApiOutcome` resolve/reject。 |
| Host 私有数据 | DID proof、capability token、`Authorization`、HTTP Signature、private key path/material、raw consent proof、Host credential path 永不进入 JS result。 |

## 3. JS 暴露原则

1. 所有 API 同时支持 callback 与 Promise，除同步 API 外。
2. 返回对象必须包含微信风格 `errMsg`。
3. `success`、`fail`、`complete` callback 的调用顺序固定。
4. Host 私有数据不进入 JS 返回值，除非该 API 本身就是返回业务数据。
5. `Authorization`、DID proof、capability token、HTTP Signature、private key path/material 永不进入 JS。
6. 未实现 API 也必须存在函数，返回 deterministic unsupported failure。
7. JS 传入的 callback 不得被序列化到 Rust；JS wrapper 只向 Rust 传递 JSON options 和 wrapper 生成的 metadata。
8. JS 传入 `Authorization`、`Signature`、`Signature-Input`、`Cookie` 或其它 Host-owned auth header 时，`wx.request` 必须在出站前 fail closed，不能剥离后静默继续。

## 4. 统一执行模型

```text
Skill JS calls wx.someApi(options)
  -> JS wrapper separates callbacks and normalizes JSON options
  -> __dock.wxApi(callJson)
  -> Rust bridge parses WxApiCall
  -> Capability Broker checks permission
  -> Specialized broker executes or returns unsupported
  -> Rust returns WxApiOutcome JSON
  -> JS wrapper invokes success/fail/complete and resolves/rejects Promise
```

执行要求：

- `success` / `fail` 和 `complete` 在同一个 Promise job 中按顺序调用。
- `complete` 必须在 Promise settle 前调用。
- JS wrapper 不能让 API-specific code 直接访问 DID credential、token cache、RequestBroker transport、Host provider 或 audit sink。
- Runtime 可以在 Rust 内部保留 `privateMeta`，但返回给 JS 前必须剥离。

## 5. `WxApiCall` 冻结字段

```ts
interface WxApiCall {
  apiName: string
  environment: 'atomic_api' | 'component'
  requestId: string
  options: Record<string, unknown>
  callbackMode: 'promise' | 'callback' | 'both'
  context: {
    userDid?: string
    agentDid?: string
    merchantDid?: string
    skillId: string
    sessionId: string
    apiName: string
    componentPath?: string
    source: 'api_vm' | 'component_vm' | 'host_adapter'
  }
}
```

字段规则：

- `apiName` 必须是 manifest 或 unsupported registry 中的 canonical name。
- `environment` 只能由 runtime 填充，Skill JS 不能覆盖。
- `requestId` 用于 callback、audit、log 关联，不得包含 DID、token 或业务隐私。
- `options` 必须是 JSON-safe 值；函数、Symbol、BigInt、循环引用和 callback 字段在 JS wrapper 层剥离或转成 `invalid_options`。
- `callbackMode` 只用于观测和兼容测试，不改变 Promise/callback 语义。

## 6. `WxApiOutcome` 冻结字段

```ts
interface WxApiOutcome {
  ok: boolean
  apiName: string
  errMsg: string
  code?: WxApiErrorCode
  data?: unknown
  statusCode?: number
  header?: Record<string, string>
  reason?: string
  suggestion?: string
  audit?: {
    riskLevel?: 'L0' | 'L1' | 'L2' | 'L3' | 'L4'
    consentProofId?: string
    summary?: Record<string, unknown>
    redacted: true
  }
  privateMeta?: never
}

type WxApiErrorCode =
  | 'unsupported'
  | 'permission_denied'
  | 'consent_required'
  | 'auth_failed'
  | 'network_denied'
  | 'timeout'
  | 'invalid_options'
  | 'provider_unavailable'
  | 'transport_failed'
```

JS wrapper 返回时应把 `data` 展开为微信 API 期望结构。例如 `wx.request` 返回：

```json
{
  "errMsg": "request:ok",
  "statusCode": 200,
  "header": {},
  "data": {}
}
```

失败结果必须使用稳定 shape：

```json
{
  "errMsg": "wx.chooseAddress:fail consent_required",
  "code": "consent_required",
  "reason": "user consent is required before address access",
  "suggestion": "Ask the Host for address consent or provide a manual address form fallback",
  "audit": {
    "riskLevel": "L4",
    "redacted": true
  }
}
```

Rust 内部可以携带 `privateMeta`、raw provider payload、token cache status、auth retry reason 或 raw consent proof，但这些字段只能停留在 Rust/Host 边界，不得序列化进 JS result、model-visible output、Render IR、CLI JSON 或 audit export。

## 7. Callback / Promise 规则

| 场景 | callback | Promise | `errMsg` / 字段 |
|---|---|---|---|
| 通用成功 | `success(result)` then `complete(result)` | resolve(result) | `<api>:ok` |
| unsupported | `fail(result)` then `complete(result)` | reject(result) | `<api>:fail unsupported` + safe `reason` / `suggestion` |
| permission denied | `fail(result)` then `complete(result)` | reject(result) | `<api>:fail permission_denied` |
| consent required / denied | `fail(result)` then `complete(result)` | reject(result) | `<api>:fail consent_required`；audit summary 可记录 `riskLevel`，不得含隐私原文 |
| invalid options | `fail(result)` then `complete(result)` | reject(result) | `<api>:fail invalid_options`；指出字段名，不回显敏感值 |
| timeout | `fail(result)` then `complete(result)` | reject(result) | `<api>:fail timeout` |
| provider unavailable | `fail(result)` then `complete(result)` | reject(result) | `<api>:fail provider_unavailable` |
| sync API 成功 | 不调用 callback | 不是 Promise | 直接返回值 |
| sync API 失败 | 不调用 callback | 不是 Promise | 抛出 `Error`，其 `message` 为 `<api>:fail <code>`，可附带脱敏 `code` / `reason` |

callback 入参和 Promise settlement value 必须是同一个脱敏 result object。实现可以复制对象防止 Skill 在 callback 中修改后影响 Promise consumer，但字段和值必须一致。

## 8. API 分组冻结语义

### 8.1 ModelContext API

- `wx.modelContext.createSkill`
- `skill.registerAPI`
- `skill.use`
- `wx.modelContext.getSessionId`
- `wx.modelContext.expireAllCards`
- `wx.modelContext.NotificationType`

规则：

- `createSkill`、`registerAPI`、`skill.use` 保持当前同步/Promise handler 语义。
- `getSessionId()` 为同步 API，返回当前 runtime session id；不得返回 token、challenge id、credential path 或 Host secret。
- `expireAllCards(options)` 为异步 API，成功 resolve `{ errMsg: "modelContext.expireAllCards:ok", expiredCount }`；非法 component path、未声明 `expirable` 或权限不足时 reject。
- `NotificationType` 必须由单一常量源或防漂移测试保证 Atomic API VM 与 Component VM 一致。

### 8.2 Auth API

- `wx.login`
- `wx.checkSession`

规则：

- `wx.login(options)` 为异步 API；成功返回 code-like receipt，例如 `{ errMsg: "login:ok", code, expiresAtMs? }`，不得返回 raw capability token、DID proof、Authorization 或 private key path。
- `wx.checkSession(options)` 为异步 API；有效 session resolve `{ errMsg: "checkSession:ok" }`；缺失、过期、撤销或 scope mismatch reject `{ errMsg: "checkSession:fail auth_failed", code: "auth_failed" }`。
- login/checkSession 的 auth failure message 必须脱敏，不能回显 challenge payload 中的签名、token 或 credential path。

### 8.3 Network API

- `wx.request`
- `wx.uploadFile`
- `wx.downloadFile`
- WebSocket 子集（Phase 2+ 或 Phase 4+）

`wx.request(options)` 冻结规则：

- 必填 `url`；`method` 默认 `GET`；`header` 默认 `{}`；`data` 可为 JSON-safe value；`timeout` 由 runtime clamp 到安全范围。
- `header` 中出现 `Authorization`、`Signature`、`Signature-Input`、`Cookie` 或大小写变体时，必须 reject `request:fail permission_denied`，不出站。
- 非 allowlist URL 必须 reject `request:fail network_denied`，不出站。
- RequestBroker 收到 HTTP response 后，无论 `statusCode` 是 2xx、3xx、4xx 还是 5xx，均调用 `success` 并 resolve，`errMsg` 为 `request:ok`。
- DNS、TCP、TLS、timeout、body serialization、auth material generation、challenge proof、provider unavailable 等 broker/local failure 调用 `fail` 并 reject。
- 401 challenge retry 是 broker 内部行为；若重试后仍收到 HTTP response，按 HTTP response success 规则返回 `statusCode: 401`。若 challenge proof 生成或验证流程在本地失败，返回 `request:fail auth_failed`。
- response `header` 返回给 JS 前必须剥离或脱敏 `Authorization`、`Set-Cookie`、`Signature`、`Signature-Input`、token-like header 和其它 Host-owned auth metadata。

`wx.uploadFile` / `wx.downloadFile` 后续必须复用 RequestBroker 和 opaque file handle，不得返回真实本地路径。WebSocket、TCP、UDP、mDNS 默认 unsupported-by-design，不得绕过 RequestBroker。

### 8.4 Storage API

- `wx.getStorage` / `wx.setStorage` / `wx.removeStorage` / `wx.clearStorage`
- 同步版本
- batch 版本可后置

规则：

- 异步 storage API 遵守通用 callback/Promise 规则。
- 同步 storage API 成功直接返回或 `undefined`；失败抛出脱敏 `Error`。
- `key` 为空、超过大小限制、包含 NUL 或超出 runtime quota 时 fail closed。
- storage scope 固定为 `userDid + merchantDid + skillId`，不得把 `sessionId` 作为长期 storage 隔离维度。
- storage value 不自动进入 model-visible result；audit 只记录 key hash、size 和 redacted summary。
- Step 01-06 已在 Atomic API VM 中实现 in-memory scoped storage bridge；生产持久化、加密、migration、backend quota 和 cleanup 仍是 Phase 4 边界。

### 8.5 Privacy / Payment / Device API

- phone、address、location、media、file、payment、scan、phone call。
- 默认都需要 host provider。
- L3/L4 默认需要 consent。

规则：

- 未配置 Host provider 时返回 `provider_unavailable` 或 `unsupported`，不能使用 mock 冒充 production。
- 未通过 ConsentGate 时返回 `consent_required`，executor/provider 不得执行。
- headless/CLI mock provider 必须在结果或 audit summary 中带 `mock: true` / `devOnly: true` 这类脱敏标识，且 release gates 不允许其进入 production profile。
- phone、address、location、file/media result 只返回业务最小字段或 opaque handle；不得返回完整地址簿、本地文件路径、原始文件内容或设备指纹。
- payment 只返回 Payment Intent / merchant API 的脱敏状态，不复刻微信支付密码或收银台。

### 8.6 Unsupported API

- `wx.cloud.*`
- 微信社交、广告、公众号/视频号/客服、跳转其它小程序
- WiFi、蓝牙、TCP、UDP、mDNS、复杂传感器
- 人脸核身、完整地图交互

统一 unsupported 返回：

```json
{
  "errMsg": "wx.cloud.callFunction:fail unsupported",
  "code": "unsupported",
  "reason": "wx.cloud.* is unsupported by anp-miniapp-dock production runtime",
  "suggestion": "Expose this capability as a merchant Agent API and call it through wx.request"
}
```

## 9. 错误码与脱敏

错误消息建议格式：

```text
<api>:fail <code>: <safe message>
```

常见 code：

| code | 含义 |
|---|---|
| `unsupported` | API 不支持 |
| `permission_denied` | 权限或 manifest 声明不足 |
| `consent_required` | 需要用户确认但未批准 |
| `auth_failed` | DID login/checkSession 失败 |
| `network_denied` | allowlist 不允许 |
| `timeout` | 超时 |
| `invalid_options` | 参数不合法 |
| `provider_unavailable` | Host provider 未配置 |
| `transport_failed` | 网络传输失败或响应不可解析 |

脱敏规则：

- 任何 key 包含 token、authorization、signature、secret、private、credential、phone、address、fileContent 等都必须 redacted，匹配大小写不敏感。
- `Authorization`、`Signature`、`Signature-Input`、`Cookie`、DID proof、capability token、JWT、private key path/material、merchant secret、phone、address、precise location、file path、file content、raw consent proof 一律不得进入 JS result。
- 错误字符串中出现 JWT、Bearer value、Signature header、private key path、PEM block、手机号、地址、文件内容时整体替换为 `[REDACTED]`。
- audit summary 只能包含 risk level、provider kind、mock/devOnly 标识、字段名、大小、hash/digest 或 proof id；不得保存 raw secret 或隐私原文。
- CLI JSON、日志、Render IR、model-visible API result 和 audit export 复用同一脱敏口径。

## 10. 实现验收

- [x] Step 01-04 覆盖的 `wx.login` / `wx.checkSession` / `wx.request` / `modelContext.expireAllCards` 已从统一 async wrapper 入口进入 Rust；后续新增 API 仍必须沿用本契约。
- [x] Step 01-04 覆盖的 async API callback 与 Promise 有测试。
- [x] Step 01-05 覆盖的 unsupported API 不抛 JS TypeError，而是稳定 fail；包括 async callback/Promise reject、sync throw、nested `wx.cloud.*` 和 unknown root fallback。
- [x] Step 01-04 覆盖的 login/request/session 错误输出通过 redaction test；Step 01-05 覆盖的 unsupported stub 不回显 options、token-like 字段或 Host 私有数据；真实 L3/L4 provider 结果 redaction 仍随后续 Step 补齐。
- [x] 兼容矩阵记录 Step 01-03 / 01-04 覆盖 API 的 callback/Promise 行为。
- [x] `wx.request` HTTP response resolve，broker/local failure reject；非 2xx/401 重试的生产 Host transport 语义仍需 Phase 4 扩展验证。
- [x] JS-provided `Authorization` / `Signature` / `Signature-Input` / `Cookie` header 被拒绝且不出站。
- [ ] L3/L4 API 未配置 provider 或未通过 consent 时 fail closed；Step 01-05 已为未配置 provider 的 P1/P2 高风险 `wx.*` 调用提供 deterministic unsupported stub，真实 provider + consent/audit 仍按 Step 01-08 和 Phase 3 推进。
