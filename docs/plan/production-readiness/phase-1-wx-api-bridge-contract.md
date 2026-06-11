# Phase 1 子文档：wx API Bridge Contract

## 1. 目标

本文定义 Atomic API VM 中 `wx.*` bridge 的统一契约。实现前先冻结此契约，避免每个 API 分别实现 callback、Promise、错误和权限逻辑，造成行为不一致。

## 2. JS 暴露原则

1. 所有 API 同时支持 callback 与 Promise，除同步 API 外。
2. 返回对象必须包含微信风格 `errMsg`。
3. `success`、`fail`、`complete` callback 的调用顺序固定。
4. Host 私有数据不进入 JS 返回值，除非该 API 本身就是返回业务数据。
5. `Authorization`、DID proof、capability token、private key path 永不进入 JS。
6. 未实现 API 也必须存在函数，返回 deterministic unsupported failure。

## 3. 统一执行模型

```text
Skill JS calls wx.someApi(options)
  -> JS wrapper normalizes options/callbacks
  -> __dock.wxApi(apiName, environment, optionsJson)
  -> Rust bridge parses WxApiCall
  -> Capability Broker checks permission
  -> Specialized broker executes or returns unsupported
  -> Rust returns WxApiOutcome JSON
  -> JS wrapper invokes success/fail/complete and resolves/rejects Promise
```

## 4. `WxApiCall` 建议字段

```ts
interface WxApiCall {
  apiName: string
  environment: 'atomic_api' | 'component'
  options: Record<string, unknown>
  context: {
    userDid?: string
    agentDid?: string
    merchantDid?: string
    skillId: string
    sessionId: string
    apiName: string
  }
}
```

## 5. `WxApiOutcome` 建议字段

```ts
interface WxApiOutcome {
  ok: boolean
  errMsg: string
  data?: unknown
  statusCode?: number
  header?: Record<string, string>
  audit?: {
    riskLevel?: 'L0' | 'L1' | 'L2' | 'L3' | 'L4'
    consentProofId?: string
    redacted: true
  }
}
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

## 6. Callback / Promise 规则

| 场景 | callback | Promise |
|---|---|---|
| 成功 | `success(result)` then `complete(result)` | resolve(result) |
| 业务失败 / unsupported | `fail(result)` then `complete(result)` | reject(result) 或 resolve? |
| 微信兼容风险 | 优先对齐微信；如果难以确认，文档固定本容器行为 | 文档固定 |

实现时需要在兼容矩阵中为每个 API 标注 Promise 失败是 reject 还是 resolve。为了减少 Skill 迁移风险，建议大多数 `fail` 场景 reject，但 `wx.request` 的 HTTP 非 2xx 是否 fail 需按微信语义在实现前确认并写入矩阵。

## 7. API 分组

### 7.1 ModelContext API

- `wx.modelContext.createSkill`
- `skill.registerAPI`
- `skill.use`
- `wx.modelContext.getSessionId`
- `wx.modelContext.expireAllCards`
- `wx.modelContext.NotificationType`

### 7.2 Auth API

- `wx.login`
- `wx.checkSession`

### 7.3 Network API

- `wx.request`
- `wx.uploadFile`
- `wx.downloadFile`
- WebSocket 子集（Phase 2+ 或 Phase 4+）

### 7.4 Storage API

- `wx.getStorage` / `wx.setStorage` / `wx.removeStorage` / `wx.clearStorage`
- 同步版本
- batch 版本可后置

### 7.5 Privacy / Payment / Device API

- phone、address、location、media、file、payment、scan、phone call。
- 默认都需要 host provider。
- L3/L4 默认需要 consent。

### 7.6 Unsupported API

- `wx.cloud.*`
- 微信社交、广告、公众号/视频号/客服、跳转其它小程序
- WiFi、蓝牙、TCP、UDP、mDNS、复杂传感器
- 人脸核身、完整地图交互

## 8. 错误码与脱敏

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

脱敏规则：

- 任何 key 包含 token、authorization、signature、secret、private、credential、phone、address、fileContent 等都必须 redacted。
- 错误字符串中出现 JWT / Signature header / private key path 时整体替换为 `[REDACTED]`。

## 9. 实现验收

- [ ] 所有 API 都从同一 JS wrapper 入口进入 Rust。
- [ ] callback 与 Promise 有测试。
- [ ] unsupported API 不抛 JS TypeError，而是稳定 fail。
- [ ] 错误输出通过 redaction test。
- [ ] 兼容矩阵记录每个 API 的 callback/Promise 行为。
