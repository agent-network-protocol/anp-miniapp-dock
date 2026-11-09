# 当前能力基线

> 状态：Phase 0 基线冻结
> 日期：2026-06-12
> 范围：记录当前 P0/P0.5 能力、证据、限制和 demo-only 边界；不声明新增运行时能力。
> 上游计划：[`../plan/production-readiness-roadmap.md`](../plan/production-readiness-roadmap.md) Step 00-01。

> 注意：本文是 Step 00-01 的时点基线，不是实时能力矩阵。Step 01-03 / 01-04 已完成的 `wx.modelContext`、`wx.login`、`wx.checkSession` 和 `wx.request` 收敛状态，以 [`wx-api-compatibility-matrix.md`](wx-api-compatibility-matrix.md)、[`../runbook/release-gates.md`](../runbook/release-gates.md) 和对应 Step 文档为准。

## 1. 状态枚举

本基线只使用下列状态词，后续 API 矩阵、组件矩阵和 release gates 应沿用这些语义：

| 状态 | 含义 |
|---|---|
| `implemented` | 当前代码已实现，且有源码、测试或 runbook 证据支撑。 |
| `host-boundary` | 已有 trait、数据结构或 Host 边界，但生产 provider、真实宿主 UI 或稳定接入协议尚未完成。 |
| `demo-only` | 仅用于 coffee demo、localhost、mock provider、测试 fixture 或非生产示例。 |
| `planned` | roadmap 已规划，但当前代码不能作为已实现能力使用。 |
| `unsupported-by-design` | 当前产品边界明确不做完整微信 Runtime 或特定高风险/宿主专属能力。 |

## 2. 验证与事实来源

| 证据 | 结果 | 用途 |
|---|---|---|
| `cargo metadata --format-version 1 --no-deps` | 成功；workspace 包含 11 个 crate。 | 确认 Rust workspace 成员和 crate 边界。 |
| `find crates -maxdepth 3 -type f \( -name '*test*.rs' -o -path '*/tests/*' \) -print` | 找到 15 个 crate 级测试文件。 | 确认每个核心 crate 的测试证据入口。 |
| [`README.md`](../../README.md) | 已描述 MVP 能力、CLI、demo、Mac host 和安全边界。 | 校准用户可见能力与非目标。 |
| [`local-demo.md`](../runbook/local-demo.md) | 记录 Rust demo-server、FastAPI localhost 和 CLI coffee flow。 | 校准 demo-only 运行方式。 |
| [`security.md`](../runbook/security.md) | 记录本地安全运行、凭据和脱敏要求。 | 校准安全边界。 |
| [`production-readiness-roadmap.md`](../plan/production-readiness-roadmap.md) | 本文件冻结 Step 00-01 时点；后续 Step 状态以后续台账、矩阵和 Step 文档为准。 | 避免把时点基线误读为实时现状。 |

## 3. Workspace 基线

| crate / 入口 | 当前职责 | 状态 | 证据 | 限制 / 后续阶段 |
|---|---|---|---|---|
| `mcp-schema` | `mcp.json` manifest、API/component declaration、component metadata、`format:image/file` 输入字段识别、`AtomicApiResult`、模型可见结果、manifest/input/output 校验和 warning 分层。 | `implemented` | [`manifest.rs`](../../crates/mcp-schema/src/manifest.rs)、[`validation.rs`](../../crates/mcp-schema/src/validation.rs)、[`mcp_validation.rs`](../../crates/mcp-schema/tests/mcp_validation.rs) | 仍未覆盖小程序 MCP 全量 package 约束、`SKILL.md`/`mcp.json` 长度限制、签名和远端 package 约束；runtime 行为仍需后续 Phase 对齐。 |
| `skill-loader` | 加载 `SKILL.md`、`mcp.json`、`index.js`、`apis/*.js` 和组件包；发现组件路径；阻断绝对路径、父目录穿越和跨包路径。 | `implemented` | [`package.rs`](../../crates/skill-loader/src/package.rs)、[`resolver.rs`](../../crates/skill-loader/src/resolver.rs)、[`coffee_skill_load.rs`](../../crates/skill-loader/tests/coffee_skill_load.rs) | 仅支持本地单 Skill 目录；Skill registry、远端 package 获取、签名和版本管理仍为 `planned`。 |
| `js-runtime-quickjs` | QuickJS 原子接口 VM、受限 CommonJS、`createSkill`、`registerAPI`、`skill.use` middleware、超时/内存/栈限制、日志收集、禁用 `fetch` / `process` / `eval` / `Function`。 | `implemented` | [`api_vm.rs`](../../crates/js-runtime-quickjs/src/api_vm.rs)、[`bridge.rs`](../../crates/js-runtime-quickjs/src/bridge.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`register_api.rs`](../../crates/js-runtime-quickjs/tests/register_api.rs) | `wx.login` / `wx.request` 仍是 localhost DID demo bridge；完整 callback/Promise 兼容、统一 `wx Capability Broker`、`wx.checkSession` 和全量 unsupported stub 仍为 Phase 1。 |
| `component-runtime` | Component VM、`Component({})` 子集、`data` / `properties` / `methods`、生命周期、`setData`、Result/Input/Expire notification、`sendFollowUpMessage`、`api/call`、card expiration action、tap/image 事件、WXML/WXSS 子集到 Render IR。 | `implemented` | [`component_vm.rs`](../../crates/component-runtime/src/component_vm.rs)、[`compiler.rs`](../../crates/component-runtime/src/compiler.rs)、[`render_ir.rs`](../../crates/component-runtime/src/render_ir.rs)、[`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs)、[`wxml_bindings.rs`](../../crates/component-runtime/tests/wxml_bindings.rs) | 只覆盖交易型卡片 P0 子集；表单类组件、更完整 WXML/WXSS、动态组件权限、Render IR 版本化和 golden fixture 仍为 Phase 2。 |
| `wx-compat` | Capability profile、`RequestBroker` trait、scoped storage、`ModelContext`、card event sink、device/app info helper。 | `host-boundary` | [`permissions.rs`](../../crates/wx-compat/src/permissions.rs)、[`request.rs`](../../crates/wx-compat/src/request.rs)、[`storage.rs`](../../crates/wx-compat/src/storage.rs)、[`model_context.rs`](../../crates/wx-compat/src/model_context.rs)、[`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs)、[`scoped_storage.rs`](../../crates/wx-compat/tests/scoped_storage.rs) | `RequestBroker` 默认实现是 unsupported；storage 为内存实现；model context 主要是 Rust helper，原子接口 JS 注入未完整对齐。 |
| `anp-adapter` | DID credential provider、HTTP signature helper、challenge proof、scoped capability token、token cache、allowlist request broker 和脱敏辅助。 | `implemented` | [`did.rs`](../../crates/anp-adapter/src/did.rs)、[`challenge.rs`](../../crates/anp-adapter/src/challenge.rs)、[`token.rs`](../../crates/anp-adapter/src/token.rs)、[`signed_request.rs`](../../crates/anp-adapter/src/signed_request.rs)、[`capability_token_scope.rs`](../../crates/anp-adapter/tests/capability_token_scope.rs) | 当前主要服务 demo-server/FastAPI 与未来 RequestBroker 收敛；生产级 token rotation/revocation、持久化和 registry 仍为 planned。 |
| `consent-audit` | 风险分级、ConsentRequest/ConsentProof、mock decision provider、审计记录、敏感字段脱敏。 | `implemented` | [`consent.rs`](../../crates/consent-audit/src/consent.rs)、[`audit.rs`](../../crates/consent-audit/src/audit.rs)、[`payment_requires_consent.rs`](../../crates/consent-audit/tests/payment_requires_consent.rs) | 当前 provider 为 mock / in-memory；真实 Host consent UI、审计落盘、合规导出和策略引擎仍为 Phase 3/4。 |
| `card-spec` | `card-spec/v0` fallback card schema、structured/text fallback 和 action 类型。 | `implemented` | [`schema.rs`](../../crates/card-spec/src/schema.rs)、[`fallback.rs`](../../crates/card-spec/src/fallback.rs)、[`order_card.rs`](../../crates/card-spec/tests/order_card.rs) | 作为 fallback contract；不替代 Host renderer，也不承诺完整 UI 展现。 |
| `dock-core` | Orchestrator、API registry、input/result validation、permission gate、ConsentGate、ApiExecutor、RenderRouter、AuditSink 和组件 action 路由边界。 | `implemented` | [`orchestrator.rs`](../../crates/dock-core/src/orchestrator.rs)、[`host.rs`](../../crates/dock-core/src/host.rs)、[`api_call_flow.rs`](../../crates/dock-core/tests/api_call_flow.rs) | Host、consent、executor、renderer、audit 都是 trait 边界；生产 Host API、持久化和 sidecar/SDK 形态仍为 Phase 4。 |
| `demo-server` | Rust coffee merchant Agent demo server、DID challenge/login、capability token 签发/校验、coffee APIs、audit redaction。 | `demo-only` | [`routes.rs`](../../crates/demo-server/src/routes.rs)、[`auth.rs`](../../crates/demo-server/src/auth.rs)、[`coffee.rs`](../../crates/demo-server/src/coffee.rs)、[`demo_api.rs`](../../crates/demo-server/tests/demo_api.rs) | 使用 mock coffee 数据和本地 secret；不能视为生产 merchant server。 |
| `dock-cli` | `validate`、`call-api`、`preview-component`、`preview-card`、`run-demo`；输出 JSON；coffee E2E harness；CLI 层脱敏。 | `implemented` / `demo-only` | [`commands.rs`](../../crates/dock-cli/src/commands.rs)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs)、[`local-demo.md`](../runbook/local-demo.md) | CLI 是开发/demo 入口；不是稳定 Host 接入协议。`run-demo` 内的 consent approval 和本地服务流程为 demo-only。 |

## 4. 能力基线表

| 能力 | Owner | 状态 | 证据 | 限制 / 下一阶段 |
|---|---|---|---|---|
| Skill package 加载 | `skill-loader` | `implemented` | [`coffee_skill_load.rs`](../../crates/skill-loader/tests/coffee_skill_load.rs) 断言 coffee Skill 有 3 个 API、3 个组件且 validation valid。 | 本地目录加载；远端 registry/cache/package zip 为 planned。 |
| Manifest 与输入校验 | `mcp-schema`、`dock-core`、`dock-cli` | `implemented` | [`mcp_validation.rs`](../../crates/mcp-schema/tests/mcp_validation.rs)、[`api_call_flow.rs`](../../crates/dock-core/tests/api_call_flow.rs)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) | outputSchema mismatch 仍是 warning；`dock-cli validate` 已输出兼容报告，但真实 Host provider/runtime 行为仍需后续 Phase 对齐。 |
| 模型可见输出隔离 | `mcp-schema`、`dock-core`、`dock-cli` | `implemented` | [`result.rs`](../../crates/mcp-schema/src/result.rs)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) 断言 `modelVisible` 不含 `_meta`。 | 仍需扩展到更完整敏感字段/Render IR/audit export gate。 |
| Atomic API JS 执行 | `js-runtime-quickjs` | `implemented` | [`register_api.rs`](../../crates/js-runtime-quickjs/tests/register_api.rs) 覆盖 API 注册、调用和 missing API；[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) 覆盖 middleware 和 timeout。 | 未实现完整微信 JS Runtime；unsupported API 策略待 Phase 1。 |
| JS 沙箱基础限制 | `js-runtime-quickjs`、`skill-loader` | `implemented` | [`bridge.rs`](../../crates/js-runtime-quickjs/src/bridge.rs) 禁用 `fetch`、`process`、`eval`、`Function`；[`resolver.rs`](../../crates/skill-loader/src/resolver.rs) 阻断路径逃逸。 | 生产级 CPU/内存/IO 隔离、包签名和供应链治理仍为 Phase 3。 |
| Demo `wx.login` | `js-runtime-quickjs`、`anp-adapter`、`demo-server` | `demo-only` | [`api_vm.rs`](../../crates/js-runtime-quickjs/src/api_vm.rs)、[`auth.rs`](../../crates/demo-server/src/auth.rs)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) | 返回 `dock-login-code-localhost`；正式 `DidAuthSessionManager`、`wx.checkSession` 和微信语义对齐为 Phase 1。 |
| Demo `wx.request` localhost bridge | `js-runtime-quickjs`、`anp-adapter` | `demo-only` | [`api_vm.rs`](../../crates/js-runtime-quickjs/src/api_vm.rs) 只允许 loopback；[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) 断言 token/Auth/Signature 不出现在 CLI 输出。 | 非统一 RequestBroker；非 allowlist 的生产网络出站仍需 Phase 1 收敛。 |
| ANP DID challenge proof | `anp-adapter`、`demo-server`、FastAPI 示例 | `implemented` / `demo-only` | [`challenge.rs`](../../crates/anp-adapter/src/challenge.rs)、[`auth.rs`](../../crates/demo-server/src/auth.rs)、[`examples/coffee-fastapi-server/README.md`](../../examples/coffee-fastapi-server/README.md) | 证明格式已稳定用于 demo；生产 resolver、rotation、revocation 和部署配置仍待后续阶段。 |
| Scoped capability token | `anp-adapter`、`demo-server` | `implemented` | [`token.rs`](../../crates/anp-adapter/src/token.rs)、[`capability_token_scope.rs`](../../crates/anp-adapter/tests/capability_token_scope.rs)、[`demo_api.rs`](../../crates/demo-server/tests/demo_api.rs) | 使用 in-memory cache 和 demo issuer secret；生产持久化、吊销和多商户策略待 Phase 3/4。 |
| Allowlist HTTP request broker | `anp-adapter`、`wx-compat` | `host-boundary` | [`signed_request.rs`](../../crates/anp-adapter/src/signed_request.rs)、[`capability_token_scope.rs`](../../crates/anp-adapter/tests/capability_token_scope.rs) | `wx-compat::UnsupportedRequestBroker` 仍是默认 unsupported；Phase 1 需统一注入到 JS bridge。 |
| Component VM 与生命周期 | `component-runtime` | `implemented` | [`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs)、[`set_data.rs`](../../crates/component-runtime/tests/set_data.rs) | Component options 只覆盖 P0 子集；observers、复杂 properties、动态能力仍 planned。 |
| WXML/WXSS 到 Render IR | `component-runtime` | `implemented` | [`wxml_bindings.rs`](../../crates/component-runtime/tests/wxml_bindings.rs)、[`render_ir.rs`](../../crates/component-runtime/src/render_ir.rs) | 支持 `view`、`text`、`image`、`button`、`scroll-view` 与有限样式；更完整组件和样式矩阵在 Step 00-03 / Phase 2。 |
| 组件动作到 API 流程 | `component-runtime`、`dock-core`、`dock-cli` | `implemented` | [`component_vm.rs`](../../crates/component-runtime/src/component_vm.rs)、[`api_call_flow.rs`](../../crates/dock-core/tests/api_call_flow.rs)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) | `openDetailPage` 当前只作为 action/fallback 边界；真实 Host 页面打开能力为 host-boundary。 |
| Card expiration | `component-runtime`、`wx-compat`、`dock-cli` | `implemented` / `host-boundary` | [`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs)、[`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) | Runtime-level card event 与 audit 策略仍需 Phase 1/2 收敛。 |
| Consent 与 audit | `dock-core`、`consent-audit` | `implemented` / `host-boundary` | [`payment_requires_consent.rs`](../../crates/consent-audit/tests/payment_requires_consent.rs)、[`api_call_flow.rs`](../../crates/dock-core/tests/api_call_flow.rs) | 当前 consent provider 和 audit sink 为 mock/in-memory；真实 UI、落盘和导出为 planned。 |
| CardSpec fallback | `card-spec`、`dock-cli`、`dock-core` | `implemented` | [`order_card.rs`](../../crates/card-spec/tests/order_card.rs)、[`api_call_flow.rs`](../../crates/dock-core/tests/api_call_flow.rs)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) | fallback schema 是 v0；Host renderer contract 仍需版本化。 |
| Coffee Skill fixture | `examples/coffee-skill` | `demo-only` | [`mcp.json`](../../examples/coffee-skill/mcp.json)、[`SKILL.md`](../../examples/coffee-skill/SKILL.md)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) | 三个 API 和三个组件只代表 coffee 交易 demo；mock payment 不是生产支付。 |
| FastAPI localhost merchant | `examples/coffee-fastapi-server` | `demo-only` | [`README.md`](../../examples/coffee-fastapi-server/README.md)、[`app.py`](../../examples/coffee-fastapi-server/app.py) | 模拟远端 HTTP merchant；package.zip 是 no-op marker；默认 secret 禁止生产使用。 |
| Mac Chatbot host | `mac-app/AnpMiniappDockMac` | `demo-only` / `host-boundary` | [`README.md`](../../mac-app/AnpMiniappDockMac/README.md)、[`ContentView.swift`](../../mac-app/AnpMiniappDockMac/Sources/AnpMiniappDockMac/ContentView.swift) | 通过 `dock-cli` process boundary 调用本地容器；不是稳定 Swift/Rust FFI 或 sidecar API。 |

## 5. Demo-only 与 Host-boundary 清单

以下能力不能在后续文档中写成 production-ready：

| 能力 | 当前标注 | 原因 | 后续落点 |
|---|---|---|---|
| localhost `wx.request` bridge | `demo-only` | 只允许 loopback，并在 JS VM 内直接用 TCP 请求本地 demo 服务。 | Phase 1 `RequestBroker` 收敛。 |
| `wx.login` 返回 `dock-login-code-localhost` | `demo-only` | 用于触发 DID challenge/login demo，不是微信 login 语义。 | Step 01-04。 |
| Mock coffee payment | `demo-only` | `payOrder` 只更新 demo 订单状态，没有真实支付 provider。 | Phase 1/3 高风险 API 与 Payment Intent。 |
| `DecisionConsentProvider` / CLI `ApproveConsent` | `demo-only` | 自动批准仅用于测试和 E2E harness。 | Phase 3/4 Host consent UI。 |
| In-memory token/storage/audit | `host-boundary` | 适合测试和单进程 demo，不满足生产持久化与审计要求。 | Phase 3/4。 |
| FastAPI server 默认 secret | `demo-only` | README 明确禁止生产使用默认 token issuer secret。 | 生产部署配置与 secret 管理。 |
| Mac app `dock-cli` process boundary | `demo-only` / `host-boundary` | 证明 Host 可调用容器，但不是稳定 SDK/sidecar。 | Phase 4 Host adapter contract。 |
| 完整微信小程序 Runtime、半屏页面、TabBar、社交/广告/云开发 | `unsupported-by-design` | 产品边界是 Agentic MiniApp Container，不复刻完整微信 Runtime。 | 只保留必要 fallback 或 Host boundary。 |

## 6. 测试证据映射

| 测试文件 | 覆盖重点 |
|---|---|
| [`mcp_validation.rs`](../../crates/mcp-schema/tests/mcp_validation.rs) | manifest 字段保留、重复 API、inputSchema、componentPath、outputSchema warning、模型可见输出隔离。 |
| [`coffee_skill_load.rs`](../../crates/skill-loader/tests/coffee_skill_load.rs) | coffee Skill 加载、组件发现、manifest validation、缺失文件、路径逃逸。 |
| [`register_api.rs`](../../crates/js-runtime-quickjs/tests/register_api.rs) | API 注册、AtomicApiResult、`_meta`、missing API。 |
| [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) | middleware、async handler、`wx.login` demo bridge、timeout、unsafe require。 |
| [`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs) | Component mount、Result/Input/Expire notification、事件、actions、image load/error、timeout。 |
| [`set_data.rs`](../../crates/component-runtime/tests/set_data.rs) | `setData` 更新嵌套状态并刷新 Render IR。 |
| [`wxml_bindings.rs`](../../crates/component-runtime/tests/wxml_bindings.rs) | WXML binding、`wx:for`、`wx:if`、事件 dataset、WXSS 子集。 |
| [`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs) | component profile 默认禁止 request/timer/payment，dynamic request opt-in，card event。 |
| [`scoped_storage.rs`](../../crates/wx-compat/tests/scoped_storage.rs) | DID + merchant + Skill storage scope 隔离，session id helper。 |
| [`capability_token_scope.rs`](../../crates/anp-adapter/tests/capability_token_scope.rs) | allowlist、cached bearer、HTTP signature fallback、401 retry、token cache scope、脱敏。 |
| [`payment_requires_consent.rs`](../../crates/consent-audit/tests/payment_requires_consent.rs) | 风险分级、consent request/proof、audit redaction。 |
| [`order_card.rs`](../../crates/card-spec/tests/order_card.rs) | fallback card 状态、structured content、错误结果和 private state 隔离。 |
| [`api_call_flow.rs`](../../crates/dock-core/tests/api_call_flow.rs) | Orchestrator input validation、permission/consent/audit、model-visible filtering、render fallback、component `api/call`。 |
| [`demo_api.rs`](../../crates/demo-server/tests/demo_api.rs) | demo-server coffee APIs、DID challenge/login、token scope、audit redaction。 |
| [`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) | CLI validate/call/preview/run-demo、DID auth、component actions、card expiration、输出脱敏。 |

## 7. 后续引用规则

- Step 00-02 建立 `wx API` 兼容矩阵时，应把本文件中 `wx.login`、`wx.request`、storage、payment、privacy、unsupported API 的状态作为初始事实。
- Step 00-03 建立组件兼容矩阵时，应把本文件中 Component VM、WXML/WXSS、Render IR、card action、card expiration 的状态作为初始事实。
- Step 00-04 建立 threat model 和 release gates 时，应把本文件的 demo-only / host-boundary 清单作为上线红线。
- 任何新能力在进入 `implemented` 前，必须同时补源码/测试/runbook 证据，不能只在 roadmap 中写 planned。
