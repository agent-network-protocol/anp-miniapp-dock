# Phase 1：接口对齐与 wx Capability Broker 实施计划

## 1. 阶段目标

Phase 1 要把原子接口环境从“coffee demo 可跑”升级为“核心小程序 MCP / 微信 API 语义可被 Skill 稳定调用”。重点是建立统一 `wx Capability Broker`：所有 `wx.modelContext` 与 `wx.*` 调用都通过同一权限、DID、request、storage、consent 和 audit 边界。

本阶段不追求支持所有微信 API，但要做到：

- 已支持 API 语义稳定，callback / Promise / `errMsg` 行为一致；
- 未支持 API 有 deterministic unsupported error；
- 高风险 API 不能绕过 ConsentGate；
- Skill JS 不接触 DID 私钥、raw capability token、Authorization header。

深入设计见：

- [wx API Bridge Contract](phase-1-wx-api-bridge-contract.md)
- [DID Request Session Manager](phase-1-did-request-session-manager.md)

## 2. 涉及模块

| 模块 | 主要职责 |
|---|---|
| `crates/js-runtime-quickjs` | 注入 `wx.*` JS bridge，统一 callback/Promise 包装，sandbox 内错误转换 |
| `crates/wx-compat` | API 描述、capability profile、storage、request broker trait、unsupported shape |
| `crates/anp-adapter` | DID challenge proof、signed request、capability token、token cache、allowlist |
| `crates/dock-core` | Orchestrator、permission、consent、audit、card event routing |
| `crates/consent-audit` | L3/L4 风险 API 的 consent proof 和 redaction |
| `crates/demo-server` | 生产契约候选：challenge/login/scope/business APIs |
| `crates/dock-cli` | validate/call-api/run-demo 输出兼容证据 |
| `examples/coffee-fastapi-server` | 跨语言远端服务契约验证 |

## 3. 开发顺序

### 3.1 先抽象 Broker，再扩展 API

不要继续在 `js-runtime-quickjs` 中为每个 API 写临时 host 函数。建议先定义运行时内部抽象：

```text
WxApiCall
  api_name
  environment: atomic_api | component
  arguments
  callback_mode
  context: userDid / merchantDid / skillId / sessionId / apiName

WxApiOutcome
  ok | fail
  errMsg
  data
  private_meta
  audit_summary
```

然后由 Broker 分发到：

- `ModelContextBroker`：session id、card expiration；
- `AuthBroker`：login/checkSession；
- `RequestBroker`：request/upload/download；
- `StorageBroker`：scoped storage；
- `PrivacyBroker`：phone/address/location/media/file；
- `PaymentBroker`：Payment Intent；
- `UnsupportedBroker`：稳定 unsupported error。

### 3.2 `wx.modelContext` 原子接口能力

实现顺序：

1. `wx.modelContext.getSessionId()`：从 `ApiCallContext.session_id` 返回。
2. `wx.modelContext.expireAllCards(options)`：生成 runtime card event，不在 JS 内直接修改卡片。
3. `wx.modelContext.NotificationType`：与组件 VM 保持同名常量。
4. `wx.modelContext.createSkill(skillPath)`：保留已实现能力，但加强 skillPath 校验和多 Skill 预留。

验收重点：

- 原子接口可调用 `getSessionId` 并用于业务参数；
- `expireAllCards` 进入 audit/card event；
- 非法 componentPath fail closed；
- API VM 与 Component VM 的 NotificationType 不漂移。

### 3.3 登录与会话

实现顺序：

1. 将当前 `HostDidAuthConfig` 的 token cache 迁移/提炼为 `DidAuthSessionManager`。
2. `wx.login()`：返回 code-like receipt，不暴露 token。
3. `wx.checkSession()`：校验本 session 下 token 是否存在且未过期。
4. `wx.logout` 不要求对齐微信，可作为 ANP 扩展能力放到 `_meta.anp` 或 host API。
5. demo-server 与 FastAPI 示例共用 challenge/login JSON contract。

验收重点：

- 多 merchant、多 user、多 skill session token 隔离；
- token 过期后 `checkSession` fail；
- replay challenge fail；
- 登录失败输出脱敏。

### 3.4 网络请求

实现顺序：

1. 把 localhost demo bridge 替换为 `wx-compat::RequestBroker` 的正式 JS 注入路径。
2. 支持常见 `wx.request` options：`url`、`method`、`header`、`data`、`timeout`、`dataType`。
3. JS 传入 Authorization 时必须剥离或拒绝，由 host 自动附加 bearer/signature。
4. 请求前做 allowlist、permission、scope 检查。
5. 401 时允许一次 token clear + challenge retry；非幂等请求需谨慎，默认只对认证握手重试。

验收重点：

- 非 allowlist host 不出站；
- Skill 无法覆盖 Authorization；
- 401 cached token 清理路径有测试；
- response 和 error 均符合 `errMsg` 规范。

### 3.5 Storage

实现顺序：

1. 注入 `getStorage` / `setStorage` / `removeStorage` / `clearStorage`。
2. 注入同步版本 `getStorageSync` / `setStorageSync` / `removeStorageSync` / `clearStorageSync`。
3. 所有 key/value 走 size limit 和 redaction-sensitive-key 检查。
4. Scope 固定：`userDid + merchantDid + skillId`，session 不进入长期 storage scope。

验收重点：

- 不同 DID/merchant/skill 相互不可见；
- 空 key、超限 value fail closed；
- storage 内容不自动进入 model visible result。

### 3.6 隐私、媒体、位置、支付和其它高风险 API

优先级：

1. `wx.getPhoneNumber`、`wx.chooseAddress`：Host privacy provider + consent。
2. `wx.requestPayment`：Payment Intent + ConsentGate + merchant API；不复刻微信收银台。
3. `wx.chooseMedia` / `wx.chooseMessageFile`：返回 opaque file handle，不返回任意本地路径。
4. `wx.getLocation` / `wx.chooseLocation`：Host location provider + L4 audit。
5. `wx.scanCode` / `wx.makePhoneCall`：Host provider，未配置则 fail closed。

验收重点：

- L3/L4 API 未通过 consent 不执行；
- headless mock 需要显式 flag；
- audit 只存参数摘要和 proof，不存隐私原文。

### 3.7 Unsupported API

所有暂不支持 API 必须有稳定 stub：

```json
{
  "errMsg": "wx.cloud.callFunction:fail unsupported",
  "reason": "wx.cloud.* is unsupported by anp-miniapp-dock production runtime",
  "suggestion": "Expose this capability as a merchant Agent API and call it through wx.request"
}
```

不允许出现：

- `undefined is not a function`；
- 静默 no-op 成功；
- demo mock 冒充 production provider。

## 4. 测试计划

| 测试层级 | 内容 |
|---|---|
| unit | 每个 broker 的 success/fail、unsupported shape、redaction |
| VM tests | callback + Promise、sync/async storage、login/checkSession、request error |
| integration | coffee flow、FastAPI flow、token retry、scope mismatch |
| security regression | Authorization 剥离、allowlist deny、private key/token 不进入输出 |
| compatibility report | `dock-cli validate` 能报告每个 API 状态 |

## 5. 阶段完成检查

- [ ] `wx.modelContext.getSessionId`、`expireAllCards` 已注入 Atomic API VM。
- [ ] `wx.login`、`wx.checkSession` 通过正式 `DidAuthSessionManager`。
- [ ] `wx.request` 通过正式 RequestBroker，不再依赖散落 demo bridge。
- [ ] storage API 同步/异步版本可用且 scope 隔离。
- [ ] L3/L4 API 进入 ConsentGate 和 audit。
- [ ] unsupported API 有稳定错误形态。
- [ ] `wx-api-compatibility-matrix.md` 与实现状态一致。
