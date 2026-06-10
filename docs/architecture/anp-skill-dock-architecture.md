# anp-miniapp-dock 整体系统架构设计

## 1. 架构目标

`anp-miniapp-dock` 是一个独立 Rust 仓库，用于在 ANP 生态中运行小程序 MCP 形态的 Agent Skill。它不依赖 `awiki-deamon`、`im-core`，也不要求修改 aWiki client 仓库；工程只直接使用 ANP Rust SDK 处理 DID、签名、发现、鉴权和网络调用能力。

核心目标：

1. 兼容小程序 MCP 的接口契约，包括 `SKILL.md`、`mcp.json`、原子接口、原子组件元数据、`content / structuredContent / _meta`、`sendFollowUpMessage`、`api/call`、组件过期态和中间件；
2. 使用 Rust 构建独立 Skill Runtime、CLI、服务端 demo 和端到端测试；
3. 使用 QuickJS-NG 执行原子接口 JS，并逐步支持小程序 MCP 原子组件运行时子集；
4. 底层身份和网络替换为 ANP DID 与 ANP Rust SDK 实现；
5. 提供咖啡点单 demo，证明加载 Skill、调用原子接口、渲染组件、用户确认、mock 支付和卡片过期可以端到端跑通。

非目标：

- 不实现完整微信小程序 Runtime；
- 不实现完整 WXML/WXSS/页面路由/TabBar/页面生命周期；
- 不复刻微信账号、openid/unionid、微信支付收银台或微信云开发；
- 不承担 aWiki daemon、IM、消息投递或 aWiki 客户端状态职责。

## 2. 总体架构

```text
Rust CLI / Test Runner / Future Host
  │
  ▼
anp-miniapp-dock
  ├─ Skill Loader
  ├─ MCP Contract Validator
  ├─ Atomic API Runtime
  │   ├─ QuickJS-NG API VM
  │   ├─ CommonJS Loader
  │   └─ Middleware Runner
  ├─ MiniApp MCP Component Runtime
  │   ├─ QuickJS-NG Component VM
  │   ├─ WXML/WXSS Subset Compiler
  │   ├─ Render IR
  │   ├─ Render IR JSON Adapter
  │   ├─ Future Flutter Renderer Adapter
  │   └─ CardSpec Fallback
  ├─ wx Compatibility Layer
  ├─ Consent & Audit Engine
  ├─ Scoped Storage
  └─ ANP SDK Adapter
        │
        │ ANP DID Auth / Signed HTTP / Capability Token
        ▼
Demo Merchant Agent Server
  ├─ Agent Registry
  ├─ Skill Package Service
  ├─ DID Challenge/Login Service
  ├─ Mock Business APIs
  └─ Audit Log
```

整体采用“独立 Rust Runtime + 服务端 Agent demo”的架构。Runtime 负责加载 Skill、执行原子接口、执行或降级渲染原子组件、处理用户动作、执行授权确认和审计；demo server 负责提供商家 Agent 信息、Skill 包、DID challenge 登录、模拟商品/订单/支付接口。

## 3. Rust 工程目录

建议使用 Cargo workspace：

```text
anp-miniapp-dock/
  Cargo.toml
  README.md
  AGENTS.md
  docs/
    architecture/
    runbook/
    weichat-miniapp-mcp-protocol/

  crates/
    mcp-schema/
      src/
        lib.rs
        manifest.rs
        result.rs
        validation.rs
      tests/
        mcp_validation.rs

    dock-core/
      src/
        lib.rs
        orchestrator.rs
        api_registry.rs
        host.rs
        error.rs
      tests/
        api_call_flow.rs

    skill-loader/
      src/
        lib.rs
        package.rs
        resolver.rs
      tests/
        coffee_skill_load.rs

    js-runtime-quickjs/
      src/
        lib.rs
        api_vm.rs
        commonjs.rs
        bridge.rs
        middleware.rs
      tests/
        register_api.rs
        middleware_chain.rs

    wx-compat/
      src/
        lib.rs
        model_context.rs
        request.rs
        storage.rs
        permissions.rs
      tests/
        scoped_storage.rs
        component_permissions.rs

    component-runtime/
      src/
        lib.rs
        loader.rs
        compiler.rs
        component_vm.rs
        wxml.rs
        wxss.rs
        render_ir.rs
        events.rs
      tests/
        component_lifecycle.rs
        wxml_bindings.rs
        set_data.rs

    card-spec/
      src/
        lib.rs
        schema.rs
        fallback.rs
        actions.rs
      tests/
        order_card.rs

    anp-adapter/
      src/
        lib.rs
        did.rs
        challenge.rs
        token.rs
        signed_request.rs
      tests/
        capability_token_scope.rs

    consent-audit/
      src/
        lib.rs
        consent.rs
        audit.rs
      tests/
        payment_requires_consent.rs

    dock-cli/
      src/
        main.rs
        lib.rs
        commands.rs
      tests/
        coffee_order_flow.rs

    demo-server/
      src/
        main.rs
        routes.rs
        auth.rs
        coffee.rs
        audit.rs
      tests/
        demo_api.rs

  examples/
    coffee-skill/
      SKILL.md
      mcp.json
      index.js
      apis/
        searchDrinks.js
        confirmOrder.js
        payOrder.js
      components/
        drink-list/
          index.js
          index.wxml
          index.wxss
          index.json
        order-confirm/
        payment-result/

  tests/
    e2e/
      coffee_order_flow.rs
```

## 4. 小程序 MCP 兼容策略

兼容目标分两层：

1. 接口层尽量兼容小程序 MCP。用户原有 Skill 的 `SKILL.md`、`mcp.json`、`index.js`、`apis/*.js`、`components/*`、`Component({})`、`wx.modelContext`、`wx.*` 调用应尽量保持原样。
2. 实现层替换为 ANP 和 Rust Runtime。微信登录、微信支付、云开发、设备能力等由 `wx Compatibility Layer` 映射为 ANP DID、capability token、mock payment、宿主能力或明确 unsupported/fallback。

`mcp.json` 原始字段必须按小程序 MCP 读取，不为兼容目标强制用户改字段。ANP 扩展能力优先使用兼容扩展位，例如 `_meta.anp` 或 `x_anp`，不得破坏原始小程序 MCP schema。运行时也可以从小程序 MCP 既有字段推导权限，例如 `components[].permissions.scope.dynamic` 打开动态组件能力。

MVP 的详细兼容范围、P0/P1 支持矩阵和验收组件见 [小程序 MCP 兼容方案 MVP](miniapp-mcp-compatibility-mvp.md)。

## 5. Skill Loader

输入：

```text
SKILL.md
mcp.json
index.js
apis/*.js
components/*/{index.js,index.wxml,index.wxss,index.json}
```

职责：

- 解析并校验 `mcp.json.apis[]`、`components[]`、`inputSchema`、`outputSchema` 和 `_meta.ui.componentPath`；
- 读取 `SKILL.md` 作为业务编排说明；
- 加载 `index.js` 并建立 API name 到 JS handler 的映射；
- 建立 `componentPath` 到 Component Runtime / native adapter / CardSpec fallback 的渲染路由；
- 校验 Skill 包路径，禁止跨包 require、路径穿越和远程代码加载；
- 生成 Runtime 可调用的 API registry。

## 6. Atomic API Runtime

Atomic API Runtime 使用 QuickJS-NG 执行原子接口 JS。

MVP 支持：

- CommonJS `require`，仅允许加载 Skill 包内部文件；
- async/await；
- `wx.modelContext.createSkill(skillPath)`；
- `skill.registerAPI(name, handler)`；
- `skill.use(middleware)`；
- 中间件按注册顺序执行，外层到内层再回到外层；
- 每次原子接口调用有独立调用上下文；
- input/output schema 校验；
- console 日志捕获、异常捕获、结构化错误返回；
- 调用超时，默认与小程序 MCP 语义对齐为 300 秒。

调用链：

```text
api/call or model decision
  → Orchestrator
  → inputSchema validation
  → permission check
  → consent gate when risky
  → middleware chain
  → QuickJS API handler
  → wx Compatibility Layer
  → ANP SDK Adapter / demo server
  → result validation
  → component render routing
```

## 7. MiniApp MCP Component Runtime

渲染主线改为小程序 MCP 原子组件运行时子集，而不是纯 CardSpec。详细方案见 [miniapp-mcp-component-runtime.md](miniapp-mcp-component-runtime.md)。

渲染优先级：

1. MiniApp MCP Component Runtime：执行 Component JS，解析 WXML/WXSS 子集，输出 Render IR；
2. 专用 Rust/native component adapter；
3. `structuredContent -> CardSpec` fallback；
4. `content -> text` fallback。

这个设计保证接口层尽量兼容小程序 MCP，同时允许实现层先覆盖常见交易型卡片，再逐步提高组件兼容性。

## 8. wx Compatibility Layer

原子接口环境和原子组件环境必须隔离，不共享 JS 全局变量。

当前 Rust MVP 中，Atomic API VM 直接支持：

- `wx.modelContext.createSkill`；
- `skill.registerAPI`；
- `skill.use` middleware；
- 受限 CommonJS、async handler、超时、错误捕获和 sandbox global 限制。

`wx-compat` 与 `anp-adapter` 已提供下列 host/adapter 边界，供后续把更多 `wx.*` 注入 JS runtime：

- request capability profile 与 `RequestBroker` trait；
- DID-aware signed HTTP adapter、challenge/login contract、allowlist 和 token cache；
- DID + merchant + Skill scoped storage；
- model context、session id、card expiration 和 device/app info helper；
- `wx.login` / `wx.checkSession` / `wx.requestPayment` 的 ANP 映射边界。

Component VM 默认支持：

- `wx.modelContext.getContext(this)`；
- `wx.modelContext.getViewContext(this)`；
- `sendFollowUpMessage`；
- `api/call` 内容块；
- `expirePreviousCards` / `expireAllCards`；
- device/app info 子集；
- tap、image load、image error 事件。

Component VM 默认不开放 `wx.request` 和 timer。只有当组件声明动态能力，例如小程序 MCP 的 `components[].permissions.scope.dynamic`，才开放受限 request/timer 子集。

## 9. ANP SDK Adapter

所有 DID、签名、Agent 发现、challenge 登录、capability token、Signed HTTP 能力优先通过 ANP Rust SDK 实现。`anp-miniapp-dock` 不自建一套独立 DID 协议。

规则：

- 访问商家 Agent 默认携带 DID 签名或 capability token；
- 首次访问商家服务时走 DID challenge 登录；
- 登录成功后缓存短期 token；
- token 按商家 DID、用户 DID、Skill ID 隔离；
- 第三方域名请求必须命中权限 allowlist；
- 高风险接口必须先通过 Consent Engine。

## 10. 服务端 Demo 架构

MVP 在同一 Rust workspace 内提供 `demo-server` crate。

当前 MVP 接口：

```text
GET  /registry/agents
GET  /agents/coffee/manifest
GET  /agents/coffee/SKILL.md
GET  /agents/coffee/mcp.json
GET  /agents/coffee/package.zip   # P0 no-op，返回 package 未生成说明

POST /agents/coffee/auth/challenge
POST /agents/coffee/auth/login

GET  /api/drinks
POST /api/order/confirm
POST /api/order/pay
GET  /api/order/{orderId}
GET  /audit
```

Demo 闭环：

```text
searchDrinks
  → drink-list component
  → bindtap / sendFollowUpMessage / api/call
  → confirmOrder
  → order-confirm component
  → consent
  → payOrder
  → payment-result component
  → expire previous order card
```

## 11. 开发命令

Cargo workspace 创建后的建议命令：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p dock-cli --test coffee_order_flow
cargo run -p demo-server -- --host 127.0.0.1 --port 3000 --skill examples/coffee-skill
cargo run -p dock-cli -- validate examples/coffee-skill
cargo run -p dock-cli -- call-api examples/coffee-skill searchDrinks '{}'
cargo run -p dock-cli -- run-demo --skill examples/coffee-skill --server http://127.0.0.1:3000
```

`dock-cli` 子命令：

```text
validate
call-api
preview-component
preview-card
run-demo
```

## 12. 分阶段落地

P0：第一个 MVP

- 完整支持原子接口契约与执行主线：`Skill Loader`、`mcp.json.apis[]`、`inputSchema` 校验、`index.js / apis/*.js`、`createSkill`、`registerAPI`、`use middleware`、`AtomicApiResult`、错误处理、超时控制和日志捕获；`wx.request`、storage、session/card API 和 ANP DID HTTP 已作为 host/adapter 边界实现，后续可继续注入 JS runtime；
- 支持原子组件运行时最小子集：`componentPath` 解析、`components[]` 元数据、`Component({})`、`data/properties/methods`、`created/attached/detached`、`setData`、`getContext/getViewContext`、`Input/Result/Expire`、`view/text/image/button/scroll-view`、`wx:if/wx:for`、`{{path}}` binding、`bindtap`、基础 WXSS 和 Render IR JSON 输出；Flutter Renderer Adapter 是后续宿主接入工作；
- 保留 `CardSpec / structuredContent / content` fallback；
- 跑通咖啡点单 demo 的 `drink-list`、`order-confirm`、`payment-result` 三张组件卡片。

P1：能力补全

- 原子接口支持 `format:image/file`、`chooseMedia`、`previewMedia`、`scanCode`、`makePhoneCall`、真实 `chooseAddress`、真实 `getPhoneNumber`、真实 Payment Intent 和 WebSocket 子集；
- 原子组件支持 `openDetailPage` fallback、`preloadDetailPage`、`Overflow`、`scope.dynamic`、组件内受限 `wx.request`、timer、map preview、canvas static、input/radio/checkbox/picker。

P2：兼容性扩展

- 支持更多 WXML 表达式、选择器、组件嵌套和更完整 WXSS；
- 引入更多交易型 Skill 兼容测试集；
- 继续保持不做完整微信小程序运行时、完整页面路由、完整半屏小程序页面和微信社交生态 API。

## 13. 关键设计原则

1. Independent Rust Runtime：新仓库独立实现，不依赖 `awiki-deamon`、`im-core` 或 aWiki client。
2. ANP SDK First：DID、签名、认证和网络优先复用 ANP Rust SDK。
3. MCP Interface Compatibility：接口和文件结构尽量兼容小程序 MCP，不要求 Skill 作者重写业务代码。
4. Implementation Substitution：底层能力由 ANP/Rust Runtime 替换微信实现。
5. Component Runtime First：渲染主线是小程序 MCP 原子组件运行时子集，CardSpec 是 fallback。
6. Sandbox First：原子接口和组件运行在隔离 JS 上下文。
7. Consent First：下单、支付、地址、手机号、身份绑定等高风险动作必须用户确认和审计。
