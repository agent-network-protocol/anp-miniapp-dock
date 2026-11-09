# wx API 兼容矩阵

> 状态：Phase 1 API 矩阵，已记录 Step 01-05 deterministic unsupported stub 覆盖
> 日期：2026-06-12
> 范围：基于小程序 MCP 本地参考和当前能力基线，标注 `wx.modelContext` 与关键 `wx.*` API 的当前状态、目标阶段、运行时映射、安全边界和验证证据。
> 上游计划：[`../plan/production-readiness-roadmap.md`](../plan/production-readiness-roadmap.md) Step 00-02。

## 1. 使用口径

本矩阵中的“协议支持”来自 [`weichat-miniapp-mcp.txt`](../weichat-miniapp-mcp-protocol/weichat-miniapp-mcp.txt) 的 API 支持列表，只表示微信小程序 AI 开发模式中的可调用能力，不表示 `anp-miniapp-dock` 当前已支持。

本项目状态只使用以下枚举：

| 状态 | 含义 |
|---|---|
| `supported` | 当前 runtime 已直接支持该 API 或等价的核心语义，并有源码/测试证据。 |
| `host-boundary` | 已有 Rust trait、数据结构或 action 边界，但需要生产 Host provider、renderer、card manager 或持久化实现。 |
| `planned-p1` | Phase 1 需要实现或冻结契约。 |
| `planned-p2` | Phase 2+ 或后续阶段实现；当前不阻塞核心交易型 Skill。 |
| `demo-only` | 仅 coffee demo、localhost、mock provider 或 CLI harness 可用，不能作为生产能力。 |
| `unsupported-by-design` | 与 Agentic MiniApp Container 边界冲突，或应由 merchant Agent / Host 原生能力替代，默认 fail closed。 |

风险等级沿用 `consent-audit`：`L0` 公共读、`L1` 登录/账号、`L2` 普通写、`L3` 交易/支付、`L4` 隐私/设备/文件/位置。所有 `L3` / `L4` API 默认必须进入 ConsentGate、audit 和 redaction；未配置 provider 时必须 fail closed。

## 2. Callback / Promise 决策

| API 类别 | 当前决策 | 后续要求 |
|---|---|---|
| 已实现 Skill API 注册 | `createSkill`、`registerAPI`、`skill.use` 使用当前 QuickJS bridge 语义；`skillPath` 已做包内相对路径校验。 | 不支持跨 Skill 远端注册；后续多 Skill registry 必须继续通过 Host/registry 边界。 |
| Step 01-04 `wx.login` / `wx.checkSession` / `wx.request` | 已返回 Promise，并触发 `success` / `fail` / `complete` callback；失败会 reject 同一个脱敏 result。 | Host DID 配置下复用 `DidAuthSessionManager`；未配置 Host DID 的 `wx.login` 仍返回 `dock-login-code-localhost` demo fallback，不能作为生产契约。 |
| 计划中的异步 `wx.*` | 同时支持 callback 与 Promise，返回对象包含 `errMsg`。 | 统一走 Phase 1 `WxApiOutcome` wrapper：成功 `success` + `complete` + Promise resolve；失败 `fail` + `complete` + Promise reject。 |
| `wx.request` HTTP response | RequestBroker 收到 HTTP response 后，即使是 4xx/5xx，也返回 `request:ok`、调用 `success`、Promise resolve，并暴露 `statusCode`。 | allowlist deny、auth/header violation、network transport、timeout、invalid options 等 broker/local failure 才进入 `request:fail`、`fail` callback 和 Promise reject。 |
| 同步 storage / account API | 不使用 callback/Promise。 | 成功直接返回值；失败抛出带脱敏 `errMsg` / `code` 的 `Error`。 |
| unsupported API | 必须存在 deterministic stub。 | 返回 `errMsg: "<api>:fail unsupported"`，并包含 safe `reason` / `suggestion`；调用 `fail`、Promise reject；不得出现 `undefined is not a function` 或静默成功。 |

## 3. `wx.modelContext` 新增 API

| category | api | environment | status | target_phase | runtime_mapping | risk_level | owner_crate | callback_promise | tests | notes |
|---|---|---|---|---|---|---|---|---|---|---|
| Skill | `wx.modelContext.createSkill(skillPath)` | 原子接口 | `supported` | P0 | QuickJS bridge 创建 Skill handle，`skillPath` 只允许包内相对路径并规范化，拒绝绝对路径、URL、反斜杠、NUL 和逃逸 `..`。 | L0 | `js-runtime-quickjs`、`skill-loader` | 同步返回 handle | [`register_api.rs`](../../crates/js-runtime-quickjs/tests/register_api.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) | 当前不支持跨 Skill 远端注册。 |
| Skill | `skill.registerAPI(name, handler)` | 原子接口 | `supported` | P0 | Runtime registry；manifest 与注册名一致性由 VM/loader 校验。 | L0 | `js-runtime-quickjs`、`dock-core` | 同步注册 | [`register_api.rs`](../../crates/js-runtime-quickjs/tests/register_api.rs) | 重名或未声明 API fail closed。 |
| Skill | `skill.use(middleware)` | 原子接口 | `supported` | P0 | middleware onion chain，与 handler 共用 timeout。 | L0 | `js-runtime-quickjs` | async middleware Promise | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) | 支持多 middleware 和 `next()` 重入保护。 |
| Session | `wx.modelContext.getSessionId()` | 原子接口 | `supported` | Step 01-03 | 从 `ApiCallContext.session_id` 返回，不暴露 token、challenge id、credential path 或 session secret。 | L1 | `js-runtime-quickjs`、`wx-compat`、`dock-core` | 同步返回 session id | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) | 仅暴露 runtime session id；token 生命周期由后续 DID session manager 收敛。 |
| Card | `wx.modelContext.expireAllCards({ componentPaths, match })` | 原子接口 | `supported` | Step 01-03 | 生成 `_meta.modelContext.cardEvents` runtime event；`componentPaths` canonicalize，只允许 `components[]` 中声明 `expirable: true` 的包内组件；不直接修改 JS card state。 | L2 | `js-runtime-quickjs`、`wx-compat`、`component-runtime` | callback + Promise；失败 reject | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs) | CLI/Host 可在非模型可见 `_meta` 中审计 card event；生产 card manager、持久化 audit 和 Host 展示策略仍待 Phase 2/4。 |
| Component context | `wx.modelContext.getContext(this)` | 原子组件 / 半屏页面 | `supported` | P0 | Component VM 注入 model context，支持 notification handler 和 `sendFollowUpMessage`。 | L0/L2 | `component-runtime`、`dock-core` | 同步返回 context | [`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs) | 半屏页面真实 Host 环境仍是 host-boundary。 |
| Component view | `wx.modelContext.getViewContext(this)` | 原子组件 | `supported` | P0 | Component VM 注入 view context，暴露尺寸、card action 和页面关联 action。 | L0/L2 | `component-runtime` | 同步返回 context | [`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs) | 当前尺寸为 runtime 默认值；真实 Host 尺寸需要 Host adapter。 |
| Notification | `wx.modelContext.NotificationType.Input` | 原子接口 / 原子组件 | `supported` | P0 | Atomic API VM 与 Component VM 共用 `wx-compat` 常量源；Component mount 时发送 Input notification。 | L0 | `wx-compat`、`js-runtime-quickjs`、`component-runtime` | 常量 | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs) | Step 01-03 增加防漂移测试。 |
| Notification | `wx.modelContext.NotificationType.Result` | 原子接口 / 原子组件 | `supported` | P0 | Atomic API VM 与 Component VM 共用 `wx-compat` 常量源；Component mount 时发送 Result notification。 | L0 | `wx-compat`、`js-runtime-quickjs`、`component-runtime` | 常量 | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs) | 已被 coffee components 使用。 |
| Notification | `wx.modelContext.NotificationType.Expire` | 原子接口 / 原子组件 | `supported` | P0 | Atomic API VM 与 Component VM 共用 `wx-compat` 常量源；Component expire 时发送 Expire notification。 | L2 | `wx-compat`、`js-runtime-quickjs`、`component-runtime` | 常量 | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs) | 与 card manager 的生产策略仍需 Phase 2/4 收敛。 |
| Notification | `wx.modelContext.NotificationType.Overflow` | 原子接口 / 原子组件 | `host-boundary` | Phase 2 | 常量已在 Atomic API VM 与 Component VM 暴露；真实 overflow event 依赖 Host renderer 尺寸测量。 | L0 | `wx-compat`、`js-runtime-quickjs`、`component-runtime`、Host renderer | 常量 + event callback | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)；真实 overflow fixture 待 Phase 2 | 当前不能保证内容高度事件。 |
| Follow-up | `wx.modelContext.getContext().sendFollowUpMessage()` | 原子组件 / 半屏页面 | `supported` | P0 | 转换为 `sendFollowUpMessage` action；内含 `api/call` 时可触发下一步 API。 | L2 | `component-runtime`、`dock-core`、`dock-cli` | 同步返回 `errMsg` | [`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) | 半屏页面真实关闭和上行由 Host adapter 实现。 |
| Related page | `wx.modelContext.getViewContext(this).setRelatedPage({ path, query })` | 原子组件 | `host-boundary` | Phase 2/4 | Component VM 捕获 action；`components[].relatedPage` manifest 已可解析并在 `dock-cli validate` 中报告；生产 Host 负责关联页面入口和 query。 | L2 | `component-runtime`、`mcp-schema`、`dock-cli`、Host adapter | 同步返回 `errMsg` | 源码 action 覆盖；[`mcp_validation.rs`](../../crates/mcp-schema/tests/mcp_validation.rs) 覆盖 manifest warning；缺少 Host test | Runtime metadata 流向和 Host 展示仍待 Phase 2/4。 |
| Detail page | `wx.modelContext.getViewContext(this).openDetailPage({ url })` | 原子组件 | `host-boundary` | Phase 2/4 | Component VM 捕获 action；Host 以 BottomSheet/WebView/native fallback 打开。 | L3/L4 by URL/content | `component-runtime`、Host adapter | 同步返回 `errMsg` | action 源码；缺少 Host E2E | URL 必须 canonicalize，禁止跳出安全边界。 |
| Detail page | `wx.modelContext.getViewContext(this).preloadDetailPage({ url })` | 原子组件 | `planned-p2` | Phase 2/4 | 可实现为 Host 预加载或 safe no-op。 | L2 | Host adapter | Phase 2 冻结 | 待新增 | 当前未实现。 |
| Expire card | `wx.modelContext.getViewContext(this).expirePreviousCards({ componentPaths, match })` | 原子组件 | `supported` | P0 + Phase 2 | Component VM 捕获 action，CLI demo 可观察；生产 card manager 需 Host/runtime 收敛。 | L2 | `component-runtime`、`dock-cli`、`wx-compat` | 同步返回 `errMsg` | [`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) | 已支持 action 捕获；只应影响声明可过期的组件，生产 card manager 仍是后续边界。 |

## 4. 核心 `wx.*` API 矩阵

| category | api | protocol_environment | status | target_phase | runtime_mapping | risk_level | owner_crate | callback_promise | tests | notes |
|---|---|---|---|---|---|---|---|---|---|---|
| 登录 | `wx.login` | 原子接口；动态组件需 `scope.dynamic` | `host-boundary` | Step 01-04 | Host DID 配置下经 `DidAuthSessionManager.ensure_session` 执行 ANP DID challenge/login，返回 code-like receipt；token 仅 Host/runtime 持有。 | L1 | `js-runtime-quickjs`、`anp-adapter`、`demo-server` | callback + Promise；失败 reject | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) | 无 Host DID 配置时仍返回 `dock-login-code-localhost` demo fallback；生产 Host 必须提供 credential/session 配置。 |
| 登录 | `wx.checkSession` | 原子接口；动态组件需 `scope.dynamic` | `supported` | Step 01-04 | 查询 `DidAuthSessionManager` token/session 状态；缺失、过期、clear/revoke 后 fail closed。 | L1 | `js-runtime-quickjs`、`anp-adapter` | callback + Promise；失败 reject | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`session.rs`](../../crates/anp-adapter/src/session.rs) | 不泄露 token 或 proof；当前 revoke 语义以 `clear_session` 移除缓存表达，持久化 revocation list 留到 Phase 3/4。 |
| 网络 | `wx.request` | 原子接口；动态组件需 `scope.dynamic` | `demo-only` | Step 01-04 + Step 03-03 | QuickJS 已收敛到 `wx-compat::RequestBroker` trait 的本地 DID broker，执行 loopback allowlist、session bearer 注入和 response header redaction；`anp-adapter::SignedRequestPolicy` 已支持 scheme、host、port、path prefix、method、scope allowlist，默认 deny。 | L1-L4 by URL/data | `js-runtime-quickjs`、`wx-compat`、`anp-adapter` | HTTP response resolve；broker/local failure reject | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`capability_token_scope.rs`](../../crates/anp-adapter/tests/capability_token_scope.rs)、[`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) | JS 传入 `Authorization` / `Signature` / `Signature-Input` / `Cookie` 会 `request:fail permission_denied` 且不出站；非 loopback URL `network_denied`。统一 permission decision / allowlist 本地 gate 已有 focused tests；生产 Host transport、registry 配置来源和 persistent request audit 仍待 Phase 4/03-05。 |
| 网络状态 | `wx.getNetworkType` | 原子接口 | `planned-p2` | Phase 1.5/2 | Host snapshot provider，headless 可返回 deterministic fallback。 | L1 | `wx-compat`、Host adapter | Phase 1 冻结 | 待新增 | 不应暴露过多设备隐私。 |
| 网络状态 | `wx.onNetworkStatusChange` / `wx.offNetworkStatusChange` | 原子接口 | `planned-p2` | Phase 2/4 | Host listener；无 Host 时 deterministic unsupported。 | L1 | Host adapter | callback listener | 待新增 | headless 环境可 no-op fail closed。 |
| 网络状态 | `wx.onNetworkWeakChange` / `wx.offNetworkWeakChange` | 原子接口 | `planned-p2` | Phase 2/4 | Host listener；仅作为提示，不影响业务正确性。 | L1 | Host adapter | callback listener | 待新增 | 可后置。 |
| 网络状态 | `wx.getLocalIPAddress` | 原子接口 | `unsupported-by-design` | 无 | 本地 IP 属于设备/网络隐私，Agentic MiniApp Container 默认不暴露；Step 01-05 已注入 deterministic unsupported stub。 | L4 | `wx-compat` unsupported stub | fail callback + rejected Promise | [`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) | 如业务需要，应由 Host provider 返回最小化网络状态，不返回 IP。 |
| Storage | `wx.getStorage` / `wx.setStorage` / `wx.removeStorage` / `wx.clearStorage` | 原子接口；组件环境待 Phase 2/4 | `supported` | Step 01-06 + Step 04-06 | Atomic API VM 当前注入 scoped in-memory storage；`wx-compat` 在 Step 04-06 已冻结持久化 backend trait、`userDid + merchantDid + skillId + namespace` scope、quota、restart restore、remove/clear/delete scope 和 redacted restore report。 | L2 | `wx-compat`、`js-runtime-quickjs` | async callback + Promise | [`scoped_storage.rs`](../../crates/wx-compat/tests/scoped_storage.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) | `LocalFileScopedStorageBackend` 是未加密 local JSON dev/test 证据后端，`productionReady = false`；生产必须接 Host encrypted store 或 encrypted SQLite，并处理 migration/access control/privacy deletion。storage 内容不会自动进入 model-visible result。 |
| Storage sync | `wx.getStorageSync` / `wx.setStorageSync` / `wx.removeStorageSync` / `wx.clearStorageSync` | 原子接口；组件环境待 Phase 2/4 | `supported` | Step 01-06 + Step 04-06 | 同一 scoped storage contract；成功直接返回 data 或 `undefined`，失败抛出脱敏 Error，`errMsg` 使用同步 API 名称；后端不可用、scope 无效或 corrupt snapshot 返回稳定 provider-unavailable shape。 | L2 | `wx-compat`、`js-runtime-quickjs` | sync throw | [`scoped_storage.rs`](../../crates/wx-compat/tests/scoped_storage.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) | 协议参考列出 `getStorageSync` / `setStorageSync`，remove/clear sync 按微信常见语义纳入；同步失败和 restore report 不泄露 key/value 原文。 |
| Storage info/batch | `wx.getStorageInfo` / `wx.batchGetStorage` / `wx.batchSetStorage` | 原子接口、组件 | `planned-p2` | Phase 1.5/2 | Storage broker 批量查询；必须做 key/value size limit。 | L2 | `wx-compat` | callback/Promise | 待新增 | 不进入 P1 最小闭环。 |
| 系统 | `wx.getDeviceInfo` | 原子组件 | `supported` | P0 + Step 01-07 | Component VM 使用 `wx-compat` shared default snapshot，返回最小 host/runtime 信息。 | L1 | `component-runtime`、`wx-compat` | sync | [`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs)、[`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs) | 只返回最小化字段，不暴露真实设备指纹。 |
| 系统 | `wx.getDeviceInfo` | 原子接口 | `supported` | Step 01-07 | Atomic API VM 注入同一 `wx-compat` default snapshot；返回冻结对象，不接受 callback/Promise。 | L1 | `js-runtime-quickjs`、`wx-compat` | sync | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs) | 不返回 local IP、MAC、device id、广告标识、Host 账号、credential path 或私钥路径；真实 Host 覆盖留到 Phase 4。 |
| 系统 | `wx.getAppBaseInfo` | 原子组件 | `supported` | P0 + Step 01-07 | Component VM 使用 `wx-compat` shared default snapshot，返回 runtime version。 | L1 | `component-runtime`、`wx-compat` | sync | [`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs)、[`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs) | Host 可在 Phase 4 contract 中覆盖 app/runtime info。 |
| 系统 | `wx.getAppBaseInfo` | 原子接口 | `supported` | Step 01-07 | Atomic API VM 注入同一 `wx-compat` default snapshot；返回冻结对象，不复刻微信账号或客户端完整环境。 | L1 | `js-runtime-quickjs`、`wx-compat` | sync | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs) | 只返回 `SDKVersion` 和 `version`；真实 Host 覆盖留到 Phase 4。 |
| 账号信息 | `wx.getAccountInfoSync` | 原子接口 | `planned-p2` | Phase 4/5 | Host/registry 返回 agent/container/account summary，不返回微信账号标识。 | L1 | Host adapter、Skill registry | sync | 待新增 | 不复刻微信 appid/openid。 |
| 隐私 | `wx.getPhoneNumber` | 原子接口 | `host-boundary` | Step 01-08 + Phase 3/4 | Atomic API 已接入高风险 Host provider boundary；未配置 provider 时 `provider_unavailable`，`__dockConsentRequired` 测试路径返回 `consent_required`；真实手机号 provider 仍需 Host consent/audit。 | L4 | `wx-compat`、`js-runtime-quickjs`、`consent-audit`、`dock-core`、Host adapter | callback + Promise；失败 reject | [`high_risk_provider.rs`](../../crates/wx-compat/tests/high_risk_provider.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`payment_requires_consent.rs`](../../crates/consent-audit/tests/payment_requires_consent.rs)、[`api_call_flow.rs`](../../crates/dock-core/tests/api_call_flow.rs) | 不返回手机号原文；真实 provider/conformance 留到 Phase 3/4。 |
| 隐私 | `wx.getRealtimePhoneNumber` | 原子接口 | `planned-p2` | Phase 3/4 | 同手机号 provider，但实时凭证要求更高审计和过期策略。 | L4 | Host adapter、`consent-audit` | callback/Promise | 待新增 | 可后置。 |
| 地址 | `wx.chooseAddress` | 原子接口 | `host-boundary` | Step 01-08 + Step 02-06 + Phase 3/4 | Atomic API 已接入高风险 Host provider boundary；默认 fail closed，dev-only mock 只能返回 opaque token 和 `devOnly/mock` 标识；address-form fixture 只使用 opaque handle。 | L4 | `wx-compat`、`js-runtime-quickjs`、Host adapter、`consent-audit`、`dock-core` | callback + Promise；失败 reject | [`high_risk_provider.rs`](../../crates/wx-compat/tests/high_risk_provider.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`address-form.prepareAddressForm.json`](../../testdata/render-ir/address-form.prepareAddressForm.json) | 不允许 Skill 直接读本机地址簿或地址原文；真实 provider/conformance 留到 Phase 3/4。 |
| 授权/设置 | `wx.authorize` / `wx.getSetting` / `wx.openSetting` | 原子接口 | `planned-p2` | Phase 3/4 | Host permission UI / policy query。 | L2-L4 | Host adapter、`consent-audit` | callback/Promise | 待新增 | 无 Host 时 unsupported。 |
| 位置 | `wx.getLocation` / `wx.getFuzzyLocation` | 原子接口 | `host-boundary` | Step 01-08 + Step 02-06 + Phase 3/4 | Atomic API 已接入高风险 Host provider boundary；默认 fail closed，dev-only mock 只返回 opaque location token / fuzzy 标识；location-map-preview fixture 只渲染 opaque token 与 static map preview。 | L4 | `wx-compat`、`js-runtime-quickjs`、Host adapter、`consent-audit` | callback + Promise；失败 reject | [`high_risk_provider.rs`](../../crates/wx-compat/tests/high_risk_provider.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`location-map-preview.prepareLocationMap.json`](../../testdata/render-ir/location-map-preview.prepareLocationMap.json) | 不返回精确经纬度原文；真实 provider/conformance 留到 Phase 3/4。 |
| 位置 | `wx.chooseLocation` | 原子接口 | `host-boundary` | Step 01-08 + Phase 3/4 | Atomic API 已接入高风险 Host provider boundary；默认 fail closed，真实 picker 必须由 Host UI 执行。 | L4 | `wx-compat`、`js-runtime-quickjs`、Host adapter、`consent-audit` | callback + Promise；失败 reject | [`high_risk_provider.rs`](../../crates/wx-compat/tests/high_risk_provider.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) | headless 默认 fail closed。 |
| 位置 | `wx.openLocation` | 原子接口、组件 | `host-boundary` | Phase 2/4 | Host map/deeplink provider；组件只产生 host action。 | L4 | Host adapter、`component-runtime` | callback/Promise | 待新增 | 不实现完整地图 runtime。 |
| 媒体/文件 | `wx.chooseMedia` | 原子接口 | `host-boundary` | Step 01-08 + Step 02-06 + Phase 3/4 | Atomic API 已接入高风险 Host provider boundary；默认 fail closed；dev-only mock 只返回 opaque `fileHandle`，本地路径输入被拒绝且不回显；media-review fixture 只使用 opaque image/file handles。 | L4 | `wx-compat`、`js-runtime-quickjs`、`mcp-schema`、Host file/media provider、`consent-audit` | callback + Promise；失败 reject | [`high_risk_provider.rs`](../../crates/wx-compat/tests/high_risk_provider.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`mcp_validation.rs`](../../crates/mcp-schema/tests/mcp_validation.rs)、[`media-review.reviewMedia.json`](../../testdata/render-ir/media-review.reviewMedia.json) | 与 `format: "image"` 输入字段联动；真实 chooser 仍待 provider/conformance。 |
| 媒体/文件 | `wx.chooseMessageFile` | 原子接口 | `host-boundary` | Step 01-08 + Step 02-06 + Phase 3/4 | Atomic API 已接入高风险 Host provider boundary；默认 fail closed；dev-only mock 只返回 opaque `fileHandle`；media-review fixture 覆盖 file format/opaque handle。 | L4 | `wx-compat`、`js-runtime-quickjs`、`mcp-schema`、Host file provider、`consent-audit` | callback + Promise；失败 reject | [`high_risk_provider.rs`](../../crates/wx-compat/tests/high_risk_provider.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`mcp_validation.rs`](../../crates/mcp-schema/tests/mcp_validation.rs)、[`media-review.reviewMedia.json`](../../testdata/render-ir/media-review.reviewMedia.json) | 禁止任意路径读；真实 chooser 仍待 provider/conformance。 |
| 媒体 | `wx.previewMedia` | 原子组件 | `host-boundary` | Phase 2/4 | Host preview provider；无 Host 时 CardSpec/Render IR fallback。 | L2/L4 | Host renderer | callback/Promise | 待新增 | 不在 headless runtime 展示真实 UI。 |
| 上传/下载 | `wx.uploadFile` / `wx.downloadFile` | 原子接口；`downloadFile` 组件协议支持 | `planned-p2` | Phase 2/4 | File broker + RequestBroker + opaque file handle；不暴露路径。 | L4 | `wx-compat`、`anp-adapter`、Host file provider | callback/Promise | 待新增 | 比 `wx.request` 晚一阶段实现。 |
| 文件 | `wx.openDocument` | 原子接口、组件 | `host-boundary` | Phase 2/4 | Host document viewer，输入必须是 opaque handle 或 trusted URL。 | L4 | Host adapter | callback/Promise | 待新增 | 禁止本地任意文件路径。 |
| 图片 | `wx.getImageInfo` | 原子接口 | `planned-p2` | Phase 2/4 | Host/media broker 读取 image handle 元数据。 | L4 | Host media provider | callback/Promise | 待新增 | 不读取任意 URL/路径。 |
| 图片 | `wx.saveImageToPhotosAlbum` | 原子接口 | `unsupported-by-design` | 无 | 写入用户相册属于强宿主 UI 能力，默认不由容器执行；Step 01-05 已注入 deterministic unsupported stub。 | L4 | unsupported stub | fail callback + rejected Promise | [`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs) registry 覆盖 | 生产 Host 可另行提供显式用户操作。 |
| 支付 | `wx.requestPayment` | 原子接口 | `host-boundary` | Step 01-08 + Phase 3/4 | Atomic API 已接入高风险 Host provider boundary；默认 `provider_unavailable`，未通过 consent 时 `consent_required`；不复刻微信收银台、不采集支付密码，dev-only mock 只返回 Payment Intent 状态。 | L3 | `wx-compat`、`js-runtime-quickjs`、`consent-audit`、`dock-core`、merchant adapter | callback + Promise；失败 reject | [`high_risk_provider.rs`](../../crates/wx-compat/tests/high_risk_provider.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`api_call_flow.rs`](../../crates/dock-core/tests/api_call_flow.rs) | coffee `payOrder` 是 mock business API，不是 `wx.requestPayment`；真实 payment provider/conformance 留到 Phase 3/4。 |
| 支付 | `wx.requestVirtualPayment` / `wx.requestJointPayment` | 原子接口 | `host-boundary` | Step 01-08 + Phase 3/4 | Atomic API 已接入同一高风险 Host provider boundary；真实 Payment Intent 子类型留到 provider contract。 | L3 | `wx-compat`、`js-runtime-quickjs`、payment provider、`consent-audit` | callback + Promise；失败 reject | [`high_risk_provider.rs`](../../crates/wx-compat/tests/high_risk_provider.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) | 不采集支付密码。 |
| 支付 | `wx.verifyPaymentPassword` | 原子接口 | `unsupported-by-design` | 无 | 不接触用户支付密码；由真实支付 provider/Host 处理；Step 01-05 已注入 deterministic unsupported stub。 | L4 | unsupported stub | fail callback + rejected Promise | [`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs) registry 覆盖 | 不允许容器采集密码。 |
| 支付业务视图 | `wx.openPublicServicePayment` / `wx.openBusinessView` payment variants | 原子接口 | `unsupported-by-design` | 无 | 微信特定业务视图不复刻；应映射为 merchant Agent API 或 Host native flow。 | L3/L4 | unsupported stub | fail callback + rejected Promise | 待 unsupported test | 包括 public service payment、trafficInvestList、wxpayPapayIndex、wxpayScore。 |
| 订阅消息 | `wx.requestSubscribeMessage` | 原子接口 | `planned-p2` | Phase 4 | Host notification permission provider + consent/audit。 | L4 | Host adapter、`consent-audit` | callback/Promise | 待新增 | 不阻塞交易闭环。 |
| 分享 | `wx.shareAppMessage` | 原子接口、组件 tap 回调 | `host-boundary` | Phase 4/5 | Host share sheet provider；组件必须来自用户点击。 | L3 | Host adapter | callback/Promise | 待新增 | headless 默认 unsupported。 |
| 设备 | `wx.makePhoneCall` | 原子接口、组件 | `host-boundary` | Step 01-08 + Phase 4 | Atomic API 已接入高风险 Host provider boundary；默认 fail closed，真实拨号必须由 Host UI 和 explicit consent 执行。 | L4 | `wx-compat`、`js-runtime-quickjs`、Host adapter、`consent-audit` | callback + Promise；失败 reject | [`high_risk_provider.rs`](../../crates/wx-compat/tests/high_risk_provider.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) | 不能由 Skill 静默拨号。 |
| 设备 | `wx.scanCode` | 原子接口 | `host-boundary` | Step 01-08 + Phase 4 | Atomic API 已接入高风险 Host provider boundary；默认 fail closed，真实扫码必须由 Host UI 执行。 | L4 | `wx-compat`、`js-runtime-quickjs`、Host adapter、`consent-audit` | callback + Promise；失败 reject | [`high_risk_provider.rs`](../../crates/wx-compat/tests/high_risk_provider.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) | headless fail closed。 |
| 界面 | `wx.showToast` / `wx.hideToast` | 原子组件 | `host-boundary` | Phase 2/4 | Host renderer UI affordance；headless no-op 或 unsupported。 | L0 | Host renderer | callback/Promise | 待新增 | 不应影响业务状态。 |
| 加密 | `wx.getUserCryptoManager` | 原子接口 | `planned-p2` | Phase 3/4 | 可映射为 ANP/Host crypto provider，但不暴露私钥。 | L4 | `anp-adapter`、Host crypto provider | sync/object | 待新增 | 需 threat model 后再实现。 |
| 人脸核身 | `wx.startFacialRecognitionVerify` / `wx.startFacialRecognitionVerifyAndUploadVideo` | 原子接口 | `unsupported-by-design` | 无 | 生物识别与视频上传超出容器边界；只能由合规 Host/provider 单独实现；Step 01-05 已注入 deterministic unsupported stub。 | L4 | unsupported stub | fail callback + rejected Promise | [`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs) registry 覆盖 | 不在默认产品范围。 |
| 城市服务/发票/微信运动 | `wx.openBusinessView businessType=wxCityWxpayAuth` / `wx.chooseInvoiceTitle` / `wx.chooseInvoice` / `wx.getWeRunData` | 原子接口 | `unsupported-by-design` | 无 | 微信生态专属能力，不复刻；Step 01-05 已为列入 registry 的发票/运动 API 注入 deterministic unsupported stub。 | L4 | unsupported stub | fail callback + rejected Promise | [`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs) registry 覆盖 | 业务应改为 merchant Agent API 或 Host capability。 |

## 5. 大类 unsupported / deferred 覆盖

协议参考中的下列长尾 API 不进入 P1，必须提供 deterministic unsupported stub 或清晰 Host boundary。Step 01-05 已把 P1 deferred、unsupported-by-design 和代表性长尾 API 纳入 `wx-compat` registry，并由 Atomic API VM 注入 async/sync stub；真实 provider 状态仍按上表保持 `planned-*`、`host-boundary` 或 `unsupported-by-design`。后续若要支持，必须先更新本矩阵、threat model 和 Step 文档。

| category | protocol APIs | status | reason / suggestion |
|---|---|---|---|
| 云开发 | `wx.cloud.init`、`wx.cloud.callFunction`、`wx.cloud.database` | `unsupported-by-design` | 不复刻微信云开发；业务应暴露 merchant Agent API，再通过 `wx.request` / ANP DID 访问。 |
| WiFi | `wx.startWifi`、`wx.stopWifi`、`wx.connectWifi`、`wx.getWifiList`、`wx.getConnectedWifi`、WiFi listeners | `unsupported-by-design` | 设备网络控制超出容器边界，且存在隐私/安全风险。 |
| 蓝牙 / BLE / PeripheralServer | `wx.openBluetoothAdapter`、discovery、pairing、BLE connection/characteristic、PeripheralServer 全族 | `unsupported-by-design` | 低层设备控制不属于 Agentic MiniApp Container；如有需要由 Host 原生 provider 单独授权。 |
| Socket / WebSocket | `wx.connectSocket`、`SocketTask.*`、`wx.createTCPSocket`、`TCPSocket.*`、`wx.createUDPSocket`、`UDPSocket.*` | `unsupported-by-design` | 任意 socket 出站会绕过 RequestBroker、allowlist 和 audit；业务网络统一走 `wx.request`/broker。 |
| mDNS / local service discovery | `wx.startLocalServiceDiscovery`、`wx.stopLocalServiceDiscovery`、local service listeners | `unsupported-by-design` | 局域网探测不符合默认安全边界。 |
| 传感器 | accelerometer、compass、device motion、gyroscope 全族 | `unsupported-by-design` | 传感器流数据不是交易型 Skill 必需能力，且隐私风险高。 |
| MapContext | `MapContext.*` 交互 API | `unsupported-by-design` | Phase 2 可考虑单独新增静态 map preview 能力；完整地图交互不复刻。 |
| 小程序跳转/路由 | `wx.restartMiniProgram`、official account、embedded mini program、`navigateToMiniProgram`、`switchTab`、`navigateTo`、`router.*` 等 | `unsupported-by-design` | 容器不实现完整小程序页面路由；半屏详情走 Host adapter。 |
| 聊天工具/群能力 | `wx.shareVideoToGroup`、`wx.selectGroupMembers`、`wx.openChatTool` 等 | `unsupported-by-design` | 微信社交/群能力不复刻。 |
| 视频号/客服/表情/广告 | channels、customer service、sticker、rewarded/interstitial/splash ad APIs | `unsupported-by-design` | 微信生态专属或广告能力不属于当前产品边界。 |
| 半屏页面受限 API | 半屏页面中的跳转、社交、广告、MapContext.openMapApp 等 | `unsupported-by-design` | 半屏页面只能作为 Host 受控详情面板，不允许跳出容器或打开广告/社交链路。 |

统一 unsupported 返回建议：

```json
{
  "errMsg": "wx.cloud.callFunction:fail unsupported",
  "code": "unsupported",
  "reason": "wx.cloud.* is unsupported by anp-miniapp-dock production runtime",
  "suggestion": "Expose this capability as a merchant Agent API and call it through wx.request"
}
```

## 6. Owner 与证据索引

| owner | 证据 |
|---|---|
| `js-runtime-quickjs` | [`bridge.rs`](../../crates/js-runtime-quickjs/src/bridge.rs)、[`api_vm.rs`](../../crates/js-runtime-quickjs/src/api_vm.rs)、[`register_api.rs`](../../crates/js-runtime-quickjs/tests/register_api.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) |
| `wx-compat` | [`permissions.rs`](../../crates/wx-compat/src/permissions.rs)、[`request.rs`](../../crates/wx-compat/src/request.rs)、[`unsupported.rs`](../../crates/wx-compat/src/unsupported.rs)、[`storage.rs`](../../crates/wx-compat/src/storage.rs)、[`model_context.rs`](../../crates/wx-compat/src/model_context.rs)、[`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs)、[`scoped_storage.rs`](../../crates/wx-compat/tests/scoped_storage.rs) |
| `anp-adapter` | [`challenge.rs`](../../crates/anp-adapter/src/challenge.rs)、[`token.rs`](../../crates/anp-adapter/src/token.rs)、[`signed_request.rs`](../../crates/anp-adapter/src/signed_request.rs)、[`capability_token_scope.rs`](../../crates/anp-adapter/tests/capability_token_scope.rs) |
| `dock-core` / `consent-audit` | [`orchestrator.rs`](../../crates/dock-core/src/orchestrator.rs)、[`api_call_flow.rs`](../../crates/dock-core/tests/api_call_flow.rs)、[`consent.rs`](../../crates/consent-audit/src/consent.rs)、[`payment_requires_consent.rs`](../../crates/consent-audit/tests/payment_requires_consent.rs) |
| `component-runtime` | [`component_vm.rs`](../../crates/component-runtime/src/component_vm.rs)、[`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs)、[`wxml_bindings.rs`](../../crates/component-runtime/tests/wxml_bindings.rs) |
| `dock-cli` / demo | [`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs)、[`local-demo.md`](../runbook/local-demo.md)、[`current-capability-baseline.md`](current-capability-baseline.md) |

## 7. Phase 1 决策点

Step 01-01 已基于本矩阵在 [`phase-1-wx-api-bridge-contract.md`](../plan/production-readiness/phase-1-wx-api-bridge-contract.md) 冻结以下契约；后续实现 Step 不得再隐式改变这些行为：

- `WxApiOutcome` 统一结构、`errMsg` code、unsupported shape 和 redaction。
- callback 与 Promise 在 fail、unsupported、timeout、permission denied、consent required、HTTP 非 2xx 场景下的行为：通用失败 reject；`wx.request` HTTP response resolve。
- `wx.request` 拒绝 JS-provided `Authorization` / `Signature` / `Signature-Input` / `Cookie`，不出站，不静默剥离后继续。
- storage sync API 的异常/返回语义，以及 key/value size limit。
- `wx.modelContext.NotificationType` 在 Atomic API VM 与 Component VM 中的单源或防漂移测试。
- L3/L4 API 的 Host provider contract、mock 标识、consent proof 与 audit 字段。

## 8. 安全红线

- `Authorization`、DID proof、capability token、private key path、HTTP Signature、手机号、地址、文件内容不得进入模型可见输出、CLI JSON、日志、audit export 或 Render IR。
- 所有未实现 API 必须 fail closed，不能静默 no-op 成功。
- 所有网络出站必须通过 allowlist 和 RequestBroker，不能继续扩展 ad hoc localhost bridge。
- 文件/媒体 API 只能返回 opaque handle，不返回真实本地路径。
- 支付不复刻微信收银台；默认走 Payment Intent + ConsentGate + merchant API。
