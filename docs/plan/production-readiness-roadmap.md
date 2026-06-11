# anp-miniapp-dock 产品化迭代总体计划

> 状态：计划文档  
> 日期：2026-06-11  
> 范围：仅补齐后续开发计划文档，不在本步骤开发代码。  
> 依据：`docs/architecture/`、`docs/weichat-miniapp-mcp-protocol/weichat-miniapp-mcp.txt`、`docs/runbook/`、`docs/plan/did-wx-python-integration-plan.md`、当前 Cargo workspace 与测试现状。

## 1. 目标与边界

本计划的目标是把当前 coffee Demo 原型逐步演进为可线上使用的稳定 **Agentic MiniApp Container**。核心原则保持不变：

1. **智能体原生**：容器服务于 Agent 对话场景中的 Skill 调用，而不是复刻完整微信小程序运行时。
2. **MCP 接口兼容优先**：尽量对齐小程序 MCP 的 `SKILL.md`、`mcp.json`、原子接口、原子组件、`wx.modelContext` 与关键 `wx.*` 能力。
3. **ANP DID 替换底层身份与网络**：登录、鉴权、签名、请求、token、商家 Agent 访问使用 ANP DID / ANP Rust SDK 能力承载。
4. **核心能力补齐，不做完整 UI**：组件运行时要对齐原子组件契约与 Render IR 输出，但不进入完整页面路由、TabBar、半屏页面、小程序宿主 UI 复刻。
5. **安全默认开启**：沙箱隔离、权限声明、allowlist、capability token、human authorization、审计与脱敏都必须成为默认路径。

### 1.1 当前已具备的基线能力

当前代码已经不是空 Demo，已具备 P0/P0.5 基线：

- `mcp-schema`：`mcp.json.apis[]`、`components[]`、`inputSchema`、`outputSchema`、`AtomicApiResult`、模型可见字段与 `_meta` 隔离的基础模型和校验。
- `skill-loader`：加载 `SKILL.md`、`mcp.json`、`index.js`、`apis/*.js` 与组件包，阻断路径穿越和跨包 require。
- `js-runtime-quickjs`：QuickJS 原子接口 VM、受限 CommonJS、`createSkill`、`registerAPI`、`skill.use`、middleware、超时、日志、禁用 `fetch` / `process` / `eval` / `Function` 等全局逃逸入口；当前已注入 demo 级 `wx.login` / `wx.request` localhost DID 登录桥。
- `component-runtime`：`Component({})` 子集、`data` / `properties` / `methods`、`created` / `attached` / `detached`、`setData`、`Input` / `Result` / `Expire`、`sendFollowUpMessage`、`api/call`、`expirePreviousCards` / `expireAllCards`、tap / image load / image error、WXML/WXSS 子集与 Render IR JSON。
- `wx-compat`：capability profile、request broker trait、scoped storage、model context、card expiration、device/app info helper。
- `anp-adapter`：DID credential provider、HTTP signature helper、challenge proof、scoped capability token、token cache、allowlist request broker。
- `consent-audit`：风险分级、mock consent provider、consent proof、审计记录与敏感字段脱敏。
- `card-spec`：结构化 fallback card。
- `demo-server` / `dock-cli`：coffee merchant Agent demo、真实 DID challenge proof、scoped capability token、coffee order E2E、组件 action 驱动、卡片过期验证。
- `examples/coffee-fastapi-server` 与 `mac-app/`：用于演示 Python 远端服务和 Mac Chatbot host 的辅助链路。

### 1.2 仍需补齐的产品化缺口

为了达到线上稳定产品，后续不能只扩展 demo 业务，而要补齐以下系统能力：

- **接口对齐缺口**：`wx.modelContext` 与微信标准 `wx.*` API 还没有完整的能力分层、JS 注入、错误语义、callback / Promise 兼容、权限声明和测试矩阵。
- **组件对齐缺口**：当前组件运行时覆盖交易型卡片 P0 子集，尚未系统支持小程序 MCP 的组件支持列表、`relatedPage`、`expirable`、`openDetailPage` fallback、`Overflow`、动态组件、表单类组件、map/canvas 静态能力和更完整 WXML/WXSS。
- **安全缺口**：需要正式 threat model、生产级沙箱资源限制、包完整性/签名、权限策略引擎、token 轮换/撤销、真实审计落盘、敏感输出审计和供应链治理。
- **运行时产品缺口**：缺少稳定公共 API、持久化 session/token/storage/audit、Skill 获取与版本管理、Host 接入协议、生产部署形态、观测指标和发布门禁。
- **开发者生态缺口**：需要导入/校验/迁移工具、兼容性报告、golden fixtures、示例 Skill 集、文档和 release certification。

## 2. 三层计划总览

本计划按三个层级组织：

1. **总体计划**：从 Demo 原型到线上容器的阶段路线图。
2. **每个阶段的整体计划**：说明阶段目标、主要工作包、产出物和验收门槛。
3. **每个阶段中的具体细分小阶段及实施方案**：拆成可执行的开发迭代单元。

### 2.1 总体路线图

| 阶段 | 名称 | 核心目标 | 主要产出 | 完成标志 |
|---|---|---|---|---|
| Phase 0 | 基线冻结与产品化门槛 | 把当前 P0.5 能力变成可追踪基线，建立兼容矩阵和 release gates | 兼容缺口台账、测试基线、里程碑拆分 | 所有后续开发都有明确 scope、DoD 和验收命令 |
| Phase 1 | 接口对齐与 wx Capability Broker | 补齐原子接口环境中关键 `wx.modelContext` / `wx.*` 能力，对齐小程序 MCP 接口语义 | wx API 兼容层、权限映射、JS bridge、测试矩阵 | 核心交易型 Skill 可不改或少改运行 |
| Phase 2 | 组件运行时对齐 | 补齐小程序 MCP 原子组件核心能力，保持 Render IR 主线 | 组件兼容矩阵、更多 WXML/WXSS/内置组件、动态组件受控能力 | 多个真实交易型组件 fixture 通过快照和交互测试 |
| Phase 3 | 安全增强与可信执行 | 让容器默认满足线上安全边界 | threat model、沙箱加固、权限策略、token 生命周期、审计落盘、包签名 | 安全审计 checklist 全部通过，高风险动作无法绕过 |
| Phase 4 | 生产运行时与 Host 接入 | 从 CLI demo 变成可集成、可部署、可升级的容器 | Runtime API、进程/SDK 形态、持久化、Skill registry/cache、Host adapter contract | 至少一个真实 Host 能通过稳定协议接入 |
| Phase 5 | 开发者体验与生态兼容 | 让 Skill 开发者能迁移、调试、认证兼容性 | CLI/SDK、导入工具、示例 Skill、兼容报告、文档站 | 外部 Skill 可自助完成本地验证 |
| Phase 6 | 观测、性能与发布运营 | 达到线上可运维、可回滚、可持续发布 | metrics/logs/traces、性能基线、CI/CD gates、runbook | 可灰度发布并定位线上问题 |

> 推荐执行顺序：Phase 0 必须先做；Phase 1、Phase 2 可并行小步推进，但 Phase 3 的安全设计应在 Phase 1/2 开始前冻结关键原则；Phase 4 依赖 Phase 1/2/3 的稳定接口；Phase 5/6 与各阶段同步补齐。

## 3. Phase 0：基线冻结与产品化门槛

### 3.1 阶段整体计划

Phase 0 的目标不是新增功能，而是把当前 Demo 能力、缺口和上线门槛固化下来，避免后续开发变成零散 patch。

主要工作包：

- 冻结当前 P0.5 能力清单与验证命令；
- 建立 `wx.*` API 和组件兼容矩阵；
- 定义线上产品的 release gate、测试 gate、安全 gate；
- 将后续 Phase 拆成可创建 issue / milestone 的 backlog。

阶段产出物：

- `docs/architecture/wx-api-compatibility-matrix.md`：接口/API 支持等级、映射策略、错误语义、优先级。
- `docs/architecture/component-compatibility-matrix.md`：组件、事件、WXML、WXSS、动态组件支持等级。
- `docs/security/threat-model.md`：容器威胁模型与安全基线。
- `docs/runbook/release-gates.md`：每次 release 前必须通过的命令、fixture、审计项。

### 3.2 细分小阶段与实施方案

#### 0.1 当前能力盘点与基线固化

实施方案：

1. 将当前 workspace crate、CLI 命令、demo-server endpoint、coffee Skill 能力整理成表格。
2. 为每项能力标注证据：对应 crate、测试文件、runbook 命令或 demo 输出。
3. 将 `cargo metadata --format-version 1 --no-deps`、`cargo test --workspace`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings` 设为基础 gate。
4. 在文档中明确当前 demo-only 能力，例如 localhost `wx.request` bridge、mock payment、mock consent provider、非生产 FastAPI 示例。

验收：

- 文档能回答“当前已经做到什么、由哪个 crate 负责、哪些测试证明”。
- 任何新功能都能挂到已有能力图或明确新增模块。

#### 0.2 建立接口兼容矩阵

实施方案：

1. 从 `docs/weichat-miniapp-mcp-protocol/weichat-miniapp-mcp.txt` 抽取 API 列表，按原子接口环境、原子组件环境、半屏页面环境分类。
2. 为每个 API 标注：`supported`、`host-boundary`、`planned-p1`、`planned-p2`、`unsupported-by-design`、`demo-only`。
3. 对每个 planned API 定义映射策略：
   - 原样 JS 语义；
   - Host capability；
   - ANP DID 替代；
   - mock / fallback；
   - deterministic unsupported error。
4. 定义 callback、Promise、`errMsg`、错误码、敏感字段返回规则。

验收：

- 每个小程序 MCP 列表中的 API 都有明确状态，不允许出现“未知”。
- 不支持的 API 有原因，不因兼容压力进入完整微信 Runtime 复刻。

#### 0.3 建立组件兼容矩阵

实施方案：

1. 将组件支持列表拆成内置组件、Component JS、WXML、WXSS、事件、`wx.modelContext`、动态组件。
2. 将当前 P0 支持能力和目标 P1/P2 能力分离。
3. 明确哪些能力只输出 Render IR，不要求当前提供完整 UI 展现。
4. 为每个能力绑定 fixture 或未来 golden snapshot。

验收：

- 交易型卡片必须能力和可延期能力区分清晰。
- Host renderer 不再阻塞 component runtime 的契约演进。

#### 0.4 定义 release gates 与 milestone backlog

实施方案：

1. 为每个 Phase 建立 Definition of Done。
2. 将每个小阶段拆成 issue 粒度：输入、输出、影响 crate、测试、文档。
3. 规定每次开发必须同步更新兼容矩阵和 runbook。
4. 建立失败回滚规则：任何新 API 默认 fail closed，不能静默 mock 成成功。

验收：

- 后续代码开发可以直接按 issue/milestone 执行。
- 文档、测试、runbook 与实现同步成为 release 条件。

## 4. Phase 1：接口对齐与 wx Capability Broker

### 4.1 阶段整体计划

Phase 1 的目标是补齐原子接口环境的核心能力，使容器底层对 Skill 暴露的接口尽量对齐小程序 MCP 和微信标准 API 的调用方式。重点不是一次性支持所有微信 API，而是建立统一的 `wx Capability Broker`，让已支持、后续支持和明确不支持的 API 都有一致语义。

主要工作包：

- 完整化 Skill package / manifest 校验；
- 扩展原子接口 JS bridge 的 `wx.modelContext`；
- 把 `wx.login` / `wx.checkSession` / `wx.request` / storage / payment / privacy APIs 纳入统一 capability broker；
- 对齐 callback / Promise / `errMsg` 语义；
- 引入接口级权限声明、allowlist 和风险分级。

阶段完成标志：

- coffee Skill 不依赖 demo-only 特殊路径也能通过统一 broker 运行；
- 一个新的交易型 Skill 可以使用登录、请求、存储、下单、支付确认、地址/手机号授权等核心能力；
- 不支持 API 返回稳定、可测试、可脱敏的错误，不会访问宿主资源。

### 4.2 细分小阶段与实施方案

#### 1.1 Skill package 与 manifest 对齐

实施方案：

1. 扩展 validator 支持小程序 MCP 关键约束：
   - `SKILL.md` 单文件和长度限制；
   - `mcp.json` 长度统计规则；
   - `apis[].name` 与注册名一致；
   - `inputSchema` 必须为对象；
   - `outputSchema` mismatch 作为 warning；
   - `format: "image" | "file"` 输入字段识别；
   - `_meta.ui.componentPath` 和 `components[].path` 关系；
   - `components[].relatedPage`、`permissions.scope.dynamic`、`expirable`、`expiredText`。
2. 增加可选 `app.json` / `AGENTS.md` 读取规划：
   - 不要求 P1 完整支持小程序分包；
   - 可识别 `agent.skills[]` 作为多 Skill registry 输入；
   - 保留单 Skill 目录作为当前默认路径。
3. 校验策略分层：
   - spec error：不能加载；
   - compatibility warning：可加载但报告降级；
   - production warning：demo 可用但上线不允许。

涉及模块：`mcp-schema`、`skill-loader`、`dock-cli validate`。

验收：

- `dock-cli validate` 输出机器可读兼容性报告。
- 每个 manifest warning 都有修复建议或明确降级行为。

#### 1.2 `wx.modelContext` 原子接口 API 对齐

实施方案：

1. 在 Atomic API VM 注入：
   - `wx.modelContext.getSessionId()`；
   - `wx.modelContext.expireAllCards({ componentPaths, match })`；
   - `wx.modelContext.NotificationType` 常量；
   - 未来多 Skill 场景下的 `createSkill(skillPath)` 路径校验。
2. 将卡片过期从组件 VM 行为扩展为 runtime-level card event，不直接耦合 CLI demo。
3. 规定 `expireAllCards` 的策略：
   - 只影响声明 `expirable: true` 的组件；
   - `componentPaths` 必须 canonicalize；
   - `match: latest` 与 all 行为明确；
   - 操作进入 audit。

涉及模块：`js-runtime-quickjs`、`wx-compat`、`dock-core`、`component-runtime`。

验收：

- 原子接口 JS 可直接调用上述 API。
- CLI 和测试能观察到 card expiration event。

#### 1.3 `wx.login` / `wx.checkSession` 生产化

实施方案：

1. 把当前 `HostDidAuthConfig` 提炼为正式 `DidAuthSessionManager`：
   - key：`merchantDid + userDid + agentDid + skillId + sessionId + serverBaseUrl`；
   - token 只由 host 持有，默认不暴露给 Skill；
   - 支持 token refresh、过期清理、强制登出、会话隔离。
2. 对齐微信语义：
   - `wx.login()` 返回 code-like receipt 与 `errMsg`；
   - `wx.checkSession()` 校验 session/token 状态；
   - callback 和 Promise 同时支持；
   - DID proof、token、Authorization 不进入模型可见输出。
3. 服务端契约固定：
   - challenge 字段包含 `challengeId`、`nonce`、`merchantDid`、`issuedAtMs`、`expiresAtMs`、`audience`；
   - login 请求携带 signed challenge；
   - 返回 scoped capability token 和 expiry；
   - replay、audience mismatch、scope mismatch 必须失败。

涉及模块：`anp-adapter`、`js-runtime-quickjs`、`demo-server`、`examples/coffee-fastapi-server`。

验收：

- 同一 session 重复 `wx.login` 使用缓存或刷新策略；
- token 不出现在 JS result、CLI output、日志和 audit 中；
- Rust demo-server 与 FastAPI 示例使用同一契约。

#### 1.4 `wx.request` 与网络能力对齐

实施方案：

1. 将当前 demo-only localhost bridge 替换/下沉到统一 `RequestBroker`：
   - 支持 method、headers、data、timeout、responseType、statusCode、header、data、errMsg；
   - 禁止 JS 传入 `Authorization` 覆盖 host token；
   - 默认 allowlist fail closed；
   - 自动附加 DID signature 或 cached bearer。
2. 区分请求类型：
   - business API：走 capability token；
   - login/challenge：走 DID signature；
   - public GET：可按策略匿名或签名；
   - upload/download：走文件 broker，不直接开放任意文件路径。
3. 对齐错误与重试：
   - 401 清 token 后可一次 challenge retry；
   - 网络错误返回 `request:fail ...`；
   - 非 2xx 仍走 success callback 还是 fail callback需按微信语义固定；
   - 所有错误输出脱敏。

涉及模块：`wx-compat`、`anp-adapter`、`js-runtime-quickjs`、`dock-core`。

验收：

- 非 allowlist 域名无网络出站；
- Skill 不能读取或覆盖 Authorization；
- 请求行为通过 fake transport 和 integration tests 覆盖。

#### 1.5 storage、文件、媒体与设备核心 API

实施方案：

1. 注入 storage API：
   - `wx.getStorage` / `setStorage` / `removeStorage` / `clearStorage`；
   - 同步版本 `getStorageSync` / `setStorageSync`；
   - batch API 可 P1.5；
   - scope 固定为 DID + merchant + Skill。
2. 文件/媒体 API 优先实现 host-boundary 形态：
   - `format:image/file` 入参只接收 host 提供的 opaque handle；
   - `wx.chooseMedia`、`wx.chooseMessageFile` 返回 host file handle，不暴露任意本地路径；
   - `wx.previewMedia` 在 Host 能力不足时返回 fallback。
3. 设备和系统 API：
   - `wx.getDeviceInfo` / `wx.getAppBaseInfo` 保持最小真实信息；
   - `wx.getNetworkType`、`wx.onNetworkStatusChange` 可先以 snapshot/no-op listener 支持；
   - 复杂传感器、WiFi、蓝牙、TCP/UDP 默认 unsupported-by-design。

验收：

- storage 隔离测试覆盖不同 DID、merchant、Skill。
- 文件/media API 不泄漏真实路径和隐私内容。

#### 1.6 隐私、地址、手机号、支付与高风险 API

实施方案：

1. 将高风险 API 统一接入 `ConsentGate`：
   - `wx.getPhoneNumber`；
   - `wx.chooseAddress`；
   - `wx.requestPayment` / `wx.requestVirtualPayment` / `wx.requestJointPayment` 的 ANP Payment Intent 映射；
   - `wx.openLocation` / `wx.chooseLocation`；
   - `wx.makePhoneCall` / `wx.scanCode`。
2. 默认实现分层：
   - production host：调用真实 host UI/系统能力；
   - headless/CLI：显式 mock provider，输出 mock 标识；
   - 未配置 provider：fail closed。
3. payment 不复刻微信支付收银台：
   - 以 Payment Intent + user consent + merchant API 为主线；
   - demo 可 mock pay，但必须保留风险等级、proof 和 audit。

验收：

- 未配置 consent/provider 时无法执行 L3/L4 API。
- 审计记录只包含脱敏摘要、proof id 和 digest。

#### 1.7 明确 unsupported API 策略

实施方案：

1. 对 `wx.cloud.*`、微信社交、广告、公众号/视频号/客服、WiFi、蓝牙、TCP、UDP、mDNS、传感器、人脸核身、完整地图交互等 API 建立 deterministic unsupported stub。
2. stub 返回：
   - `errMsg: "<api>:fail unsupported"`；
   - `reason`；
   - `suggestion`；
   - 不访问任何宿主资源。
3. 兼容矩阵中标注 unsupported-by-design 或 P2+。

验收：

- 任何未实现 API 都不会变成 `undefined is not a function` 这类不稳定错误。
- 业务开发者能从错误中知道如何 fallback。

## 5. Phase 2：组件运行时对齐

### 5.1 阶段整体计划

Phase 2 的目标是让组件运行时从 coffee P0 卡片子集扩展到小程序 MCP 原子组件的稳定核心子集。仍然不做完整 UI 展现，重点是 **Component VM + WXML/WXSS 子集 + Render IR contract** 足够稳定，Host 可以用 Flutter、SwiftUI、Web 或 native card adapter 渲染。

主要工作包：

- 组件 manifest 元数据对齐；
- Component JS 语义增强；
- WXML/WXSS 与内置组件支持扩展；
- 动态组件能力受控开放；
- Render IR 版本化和 snapshot tests；
- 多 fixture 兼容性套件。

阶段完成标志：

- `drink-list`、`order-confirm`、`payment-result` 之外，至少再加入 3 类真实交易/表单 Skill fixture；
- Render IR 有稳定 schema version；
- 动态组件 request/timer 只在声明权限后可用；
- Host renderer 不支持时可稳定 fallback 到 CardSpec。

### 5.2 细分小阶段与实施方案

#### 2.1 组件声明与生命周期元数据

实施方案：

1. 完整读取 `components[]` 字段：
   - `path`；
   - `relatedPage`；
   - `permissions.scope.dynamic`；
   - `expirable`；
   - `expiredText`；
   - `_meta` 扩展。
2. 读取 `index.json` 基础配置，至少保留 unknown fields。
3. 组件路径 canonicalize，保证 `api._meta.ui.componentPath` 与 `components[].path` 一致。
4. 组件实例增加元数据：component id、render id、created at、expiry state、related page state。

验收：

- 组件 manifest 信息能进入 RenderOutcome 或 card event。
- `expirable: false` 的组件不会被误过期。

#### 2.2 Component JS 语义增强

实施方案：

1. 增强 `properties` 类型处理：String、Number、Boolean、Object、Array、optional/default。
2. 增加 `this.triggerEvent()` P1 支持，将事件转换为 Render IR / Host event，不直接执行宿主动作。
3. 增加 `observers` / simple watchers P2 规划，P1 可先 warning。
4. 补齐 `NotificationType.Overflow` 和 view dimension 事件。
5. 统一 lifecycle trace：created、attached、result/input notification、event、setData、expire、detached。

验收：

- Component state update 和 Render IR refresh 有 snapshot。
- 不支持的 Component 选项有 warning，不静默忽略高风险行为。

#### 2.3 WXML 子集增强

实施方案：

1. 保持 P0 表达式简单，P1 增加常见表达式：
   - `wx:elif` / `wx:else`；
   - boolean not / equality；
   - string/number literal；
   - 简单三元表达式可 P2。
2. 增加事件和 dataset 语义：
   - `catchtap`；
   - `data-*` camelCase / original key 双表示；
   - disabled button 不触发 tap。
3. 增加内置组件：
   - 表单：`input`、`textarea`、`radio`、`checkbox`、`picker`；
   - 展示：`map` preview、`canvas` static；
   - 仍不支持 `video`、`web-view`、`navigator`、广告和社交 open-type。

验收：

- 每个新增 WXML 语法都有 parser test、compiler test 和 fixture snapshot。
- 表单组件只产生 host action / component state，不绕过 consent。

#### 2.4 WXSS 子集增强

实施方案：

1. P1 增加选择器：id、标签、简单后代选择器。
2. P1 增加属性：min/max width/height、box-shadow、gap、justify-content、align-items、overflow-x。
3. 维持禁止或降级：animation、transition、复杂 transform、filter、mask、自定义字体。
4. rpx 与 host logical pixels 规则文档化，避免不同 host 渲染不一致。

验收：

- unsupported WXSS 输出 warning，不影响安全渲染。
- Render IR style 字段保持跨 Host 中立。

#### 2.5 动态组件能力

实施方案：

1. 只有声明 `components[].permissions.scope.dynamic` 的组件可使用：
   - 受限 `wx.request`；
   - `setTimeout` / `setInterval` / clear；
   - 可选 polling helper。
2. 动态组件必须满足：
   - request allowlist；
   - timer 最大数量与频率限制；
   - expire/detach 后清理；
   - host background 后暂停；
   - 审计动态请求摘要。
3. 默认组件仍禁用网络、timer、WebSocket。

验收：

- 未声明 dynamic 的组件调用 request/timer 必须失败。
- expire 后 timer 不再触发。

#### 2.6 Render IR 版本化与 Host adapter contract

实施方案：

1. 为 Render IR 增加 schema version、node kind registry、action registry。
2. 规定 Host adapter 必须处理：
   - unknown node kind fallback；
   - unknown style warning；
   - action confirmation boundary；
   - accessibility fields。
3. 增加 golden snapshot tests：相同 input 生成稳定 Render IR。
4. 增加 CardSpec fallback contract：组件加载失败、WXML 解析失败、host 不支持 node、API error。

验收：

- Render IR 变更必须更新 schema version 或 migration notes。
- Host adapter 可以独立于 Component VM 开发和测试。

## 6. Phase 3：安全增强与可信执行

### 6.1 阶段整体计划

Phase 3 的目标是把安全能力从“Demo 中证明可行”升级为“线上默认安全”。后续新增任何 API 或组件能力都必须先通过本阶段定义的安全边界。

主要工作包：

- threat model 与安全基线；
- QuickJS 沙箱和资源限制加固；
- 权限策略引擎；
- DID / token 生命周期生产化；
- consent / audit 真实化；
- Skill 包供应链安全。

阶段完成标志：

- 高风险动作无法绕过 consent；
- 非 allowlist 网络、跨包 require、远程代码、私钥/令牌泄露都有自动化测试；
- audit 可落盘、可检索、可脱敏导出；
- Skill 包加载前可校验来源和完整性。

### 6.2 细分小阶段与实施方案

#### 3.1 Threat Model 与安全分级

实施方案：

1. 建立攻击面清单：Skill package、JS runtime、component runtime、request broker、storage、DID key、token cache、Host adapter、demo/server、logs。
2. 定义攻击者模型：恶意 Skill、被篡改 Skill 包、恶意商家 Agent、恶意 Host plugin、网络中间人、日志读取者。
3. 为每条威胁定义控制措施、测试和残余风险。
4. 将风险等级 L0-L4 与 API、manifest、consent、audit 绑定。

验收：

- 每个高风险 capability 都能在 threat model 中找到对应控制措施。

#### 3.2 QuickJS 沙箱加固

实施方案：

1. 审计 `eval` / `Function` / prototype constructor / async function constructor / generator constructor escape。
2. 增加资源限制：
   - memory hard limit；
   - stack limit；
   - CPU/interrupt timeout；
   - Promise job drain 上限；
   - console/log size 上限；
   - result size 上限。
3. 将 API VM 和 Component VM 的 sandbox policy 文档化并测试。
4. 禁止远程代码、任意文件系统、socket、WebSocket，除非 capability broker 明确开放。

验收：

- sandbox escape tests 作为 CI gate。
- 超限行为返回稳定错误并记录 audit。

#### 3.3 权限策略与 allowlist

实施方案：

1. 从 `mcp.json`、`_meta.anp`、`x_anp`、`components[].permissions` 推导权限。
2. 支持宿主级 policy override：allow、deny、mock、prompt。
3. 网络 allowlist 支持：scheme、host、port、path prefix、method、scope。
4. storage、file、media、location、phone、address、payment 全部通过 capability broker。
5. 未声明权限但调用敏感能力时 fail closed。

验收：

- 任何 capability 都有 permission decision 记录。
- CLI/headless mock 必须显式开启，不能默默通过。

#### 3.4 DID、token 与会话安全

实施方案：

1. token claims 固定：issuer、audience、merchantDid、userDid、agentDid、skillId、sessionId、scopes、iat/nbf/exp、jti、version。
2. 支持 token refresh、revoke、logout、cache eviction。
3. challenge 防 replay：nonce 一次性、TTL、audience、method/url、DID document binding。
4. DID document resolver 生产化：cache、TTL、trust anchors、network failure policy。
5. 私钥只在 host/credential provider 边界使用，永不进入 JS 或日志。

验收：

- replay、wrong audience、wrong scope、expired token、wrong DID document 全部有测试。
- token 轮换不破坏 running Skill session。

#### 3.5 Consent 与审计生产化

实施方案：

1. 把 mock consent provider 抽象为 host consent adapter：CLI、Mac、Flutter、server-side headless policy 可分别实现。
2. ConsentProof 增加不可抵赖所需字段：policy version、UI prompt digest、decision actor、timestamp、parameter digest。
3. Audit sink 支持持久化：SQLite / file append / remote audit service，至少一个生产候选实现。
4. 审计查询必须默认脱敏，原始敏感字段不落盘或加密保存。
5. 建立 redaction regression tests：Authorization、Signature、token、secret、private key、phone、address、file content。

验收：

- 支付/下单/地址/手机号 API 没有 consent proof 不会执行。
- `GET /audit` 或导出接口永不泄露 token/signature。

#### 3.6 Skill 包完整性与供应链

实施方案：

1. 支持 Skill package digest、签名、版本、publisher DID。
2. 下载/缓存 Skill 时校验 digest 和 path boundary。
3. 禁止 symlink escape、absolute path、remote require。
4. 记录 package source、version、digest 到 audit。
5. 为第三方 Skill 建立 quarantine / review / allowlist 流程。

验收：

- 篡改包、路径穿越、签名不匹配、未知 publisher 默认无法加载。

## 7. Phase 4：生产运行时与 Host 接入

### 7.1 阶段整体计划

Phase 4 的目标是把 CLI/demo-server 形态升级为可被真实宿主集成和线上部署的容器。这里仍不要求做完整 UI，只要求稳定的 runtime API、进程边界、持久化和 Host adapter contract。

主要工作包：

- Runtime 公共 API / SDK；
- Skill 发现、下载、缓存、版本管理；
- session/token/storage/audit 持久化；
- 本地进程或嵌入式 SDK 形态；
- Host renderer / action protocol；
- 并发、取消、重试和幂等。

阶段完成标志：

- 一个真实 Host 可以通过稳定协议调用容器加载 Skill、执行 API、渲染 Render IR、处理 action；
- 容器重启后 session/storage/audit 能按策略恢复；
- 多用户、多商家、多 Skill session 隔离通过测试。

### 7.2 细分小阶段与实施方案

#### 4.1 Runtime API 稳定化

实施方案：

1. 定义 public Rust API：load skill、validate、call api、render component、dispatch action、expire cards、query audit。
2. 定义可选 IPC API：HTTP/gRPC/JSON-RPC，以便非 Rust Host 接入。
3. 保持 API 输入输出模型稳定并版本化。
4. 将 CLI 改为调用同一 Runtime API，避免 CLI 逻辑成为第二套 runtime。

验收：

- CLI、Mac host、未来 Flutter host 共用一套 runtime contract。

#### 4.2 Skill 发现、获取与版本管理

实施方案：

1. 对接 ANP Agent registry / merchant manifest：发现 merchant DID、Skill manifest URL、package digest、auth endpoints。
2. 支持本地 cache：按 publisher DID + skill id + version + digest 存储。
3. 支持版本选择策略：latest、pinned、allow prerelease、rollback。
4. package.zip 从 no-op 变成真实服务路径，但加载仍先解包到安全隔离目录。

验收：

- 相同 digest 可复用缓存；digest mismatch 会拒绝加载。
- 可回滚到上一个已验证 Skill version。

#### 4.3 持久化与配置

实施方案：

1. session/token cache：生产可用 secure store 或加密 SQLite。
2. scoped storage：持久化 backend，按 DID/merchant/Skill 隔离，支持 quota。
3. audit：append-only 或 SQLite backend，支持 retention policy。
4. 配置项：identity、trusted DID、allowlist、token issuer、storage path、log level、mock providers。
5. secrets：env/secret store 注入，不写入 config 文件或日志。

验收：

- 重启后可恢复非过期 token 和 storage。
- 删除用户/Skill 数据能按 scope 清理。

#### 4.4 Host renderer 与 action protocol

实施方案：

1. 定义 Host 需要实现的最小协议：
   - render Render IR；
   - render CardSpec fallback；
   - request consent；
   - handle phone/address/media/file/payment/location providers；
   - dispatch user events back to container。
2. action 必须回到 `dock-core`，不允许组件直接调用高风险 host 操作。
3. 定义 headless 模式：只输出 JSON，不做 UI；用于 CI 和 server-side agent。
4. Mac/Flutter/Web adapter 可作为参考实现，但不是容器核心。

验收：

- Render IR snapshot 可被至少一个 adapter 渲染或安全 fallback。
- 用户点击后 action flow 保持 audit/consent 边界。

#### 4.5 并发、取消、重试与幂等

实施方案：

1. 每个 session 支持多个并发 API 调用，但同一高风险交易可按 policy 串行化。
2. API call 支持 cancellation token 和 timeout。
3. request broker 支持 retry policy，但支付/下单等非幂等 API 默认不自动重试。
4. order/payment API 建议引入 idempotency key。
5. 组件过期和 session 结束会取消动态请求/timer。

验收：

- 并发场景不会串 session/token/storage。
- 取消后不会继续执行高风险 action。

## 8. Phase 5：开发者体验与生态兼容

### 8.1 阶段整体计划

Phase 5 的目标是让外部 Skill 开发者能理解容器能力、导入 Skill、自助调试并获得兼容性报告。

主要工作包：

- CLI / SDK 工具链；
- 兼容性报告；
- 示例 Skill 和迁移指南；
- 本地调试器；
- 文档与模板。

阶段完成标志：

- 开发者可用一个命令验证 Skill 是否可在容器内上线；
- 兼容性报告能指出哪些 API/组件会降级或失败；
- 至少有 coffee 之外的多个示例覆盖不同能力。

### 8.2 细分小阶段与实施方案

#### 5.1 CLI 命令扩展

实施方案：

1. `dock-cli validate` 输出 compatibility level、warnings、unsupported API、component fallback risk。
2. `dock-cli inspect` 展示 Skill package、权限、风险 API、组件树、依赖。
3. `dock-cli test-skill` 执行 fixture inputs、snapshot Render IR、审计输出。
4. `dock-cli import-wechat-mcp` 将小程序 MCP Skill 目录转换/复制为容器可验证结构，但不强制修改原字段。
5. `dock-cli doctor` 检查 DID identity、host providers、allowlist、storage、sandbox。

验收：

- CLI 输出全 JSON，可用于 CI。
- 所有敏感值默认 redacted。

#### 5.2 示例 Skill 与兼容测试集

实施方案：

1. 保留 coffee Skill 作为交易流程基线。
2. 新增至少三类 fixture：
   - 表单/地址/手机号授权；
   - 图片/文件输入处理；
   - 动态组件/状态刷新；
   - 可选地图/位置预览。
3. 每个 fixture 包含 `SKILL.md`、`mcp.json`、API JS、组件、expected result、Render IR snapshot。

验收：

- 每类核心能力都有可复制的示例。
- 新增兼容能力必须先新增或更新 fixture。

#### 5.3 文档与迁移指南

实施方案：

1. 编写“从小程序 MCP Skill 迁移到 ANP MiniApp Dock”的指南。
2. 编写 API 对齐表和组件对齐表，标明 ANP 替代实现。
3. 编写安全开发指南：不要存 token、不要在 content 暴露隐私、如何声明权限、如何处理 fallback。
4. 编写 Host adapter 开发指南。

验收：

- 开发者无需阅读 Rust 源码也能完成 Skill 验证。
- 文档和 CLI 报告术语一致。

## 9. Phase 6：观测、性能与发布运营

### 9.1 阶段整体计划

Phase 6 的目标是让容器具备线上运行所需的可观测性、性能边界、发布门禁和故障处理流程。

主要工作包：

- 结构化日志、metrics、traces；
- 性能和资源基准；
- CI/CD gates；
- release/canary/rollback；
- 线上 runbook。

阶段完成标志：

- 线上问题能通过 session id、skill id、merchant DID、api name 定位；
- 任意 release 都可回滚；
- 性能退化和敏感信息泄露能在 CI 阶段发现。

### 9.2 细分小阶段与实施方案

#### 6.1 可观测性

实施方案：

1. 统一结构化事件：skill_load、api_call_start/end、request_start/end、consent_prompt/decision、render_start/end、component_event、audit_record。
2. metrics：API latency、VM time、render time、request status、fallback rate、consent required/approved/denied、sandbox timeout、memory limit hit。
3. traces：同一用户请求贯穿 model decision、API call、request、render、action。
4. 所有 logs 默认 redacted。

验收：

- 可以在不看敏感 payload 的情况下定位失败阶段。

#### 6.2 性能与容量

实施方案：

1. 建立基准：Skill load time、API call latency、component render latency、memory per VM、token cache lookup。
2. 加入 stress tests：并发 session、多 Skill、多组件渲染、动态组件 timer。
3. 定义资源限制默认值和可配置范围。
4. 对生产 host 建议 warm cache，但保持每次调用安全上下文隔离。

验收：

- 性能基准写入 release notes。
- 超过资源限制时 fail closed，不影响其他 session。

#### 6.3 CI/CD 与 release gates

实施方案：

1. CI 必跑：fmt、clippy、unit、integration、sandbox escape、compat fixture、redaction、snapshot。
2. release 前必跑：完整 demo、多个 fixture、security checklist、docs link check。
3. 版本策略：runtime API、Render IR、capability token、Skill package contract 分别版本化。
4. 建立 canary：先在 headless/CLI 和内部 Host 跑，再开放外部 Skill。

验收：

- 任一 gate 失败不得发布。
- breaking change 必须有 migration note。

#### 6.4 运维 runbook

实施方案：

1. 编写部署、配置、identity、secret、storage、audit、升级、回滚文档。
2. 编写常见故障处理：DID 验签失败、token scope mismatch、allowlist deny、component render failed、consent required、sandbox timeout。
3. 定义数据清理与用户隐私删除流程。

验收：

- 运维人员能独立判断是 Skill 问题、商家 Agent 问题、Host provider 问题还是容器问题。

## 10. 能力优先级建议

后续实际开发应按以下优先级推进。

### 10.1 必须优先补齐

1. `wx.login` / `wx.checkSession` / `wx.request` / storage 的正式 JS bridge 与统一 capability broker。
2. `wx.modelContext.getSessionId`、`expireAllCards`、card event 的 runtime 化。
3. `components[].relatedPage`、`expirable`、`expiredText`、`permissions.scope.dynamic` 的完整读取与行为。
4. 高风险 API 的 consent/audit 强制路径。
5. API/组件兼容矩阵与 CLI 兼容报告。
6. sandbox escape、allowlist、redaction、token replay/scope 的 CI gate。

### 10.2 第二优先级

1. `format:image/file`、`chooseMedia`、`previewMedia`、`uploadFile` / `downloadFile` / `openDocument` 的 host-boundary 形态。
2. `chooseAddress`、`getPhoneNumber`、`requestPayment` 的 production provider 接口。
3. 表单组件、`openDetailPage` fallback、`Overflow`、动态组件 request/timer。
4. Skill package digest/signature/cache。
5. persistent storage/token/audit。

### 10.3 可后置或明确不做

1. 完整微信页面路由、TabBar、多页面生命周期。
2. 完整半屏小程序页面能力。
3. 微信云开发、微信支付收银台、微信社交生态 API。
4. 蓝牙、WiFi、TCP、UDP、mDNS、复杂传感器、完整地图交互。
5. 完整 WXML/WXSS 和完整自定义组件系统。

## 11. 阶段验收总表

| 能力方向 | Demo 原型可接受 | 线上产品必须达到 |
|---|---|---|
| Skill 加载 | 单 coffee Skill，本地目录 | 多 Skill、版本、digest、签名、缓存、兼容报告 |
| 原子接口 | P0 createSkill/registerAPI/use | 核心 `wx.modelContext` 与 `wx.*` JS bridge 对齐，unsupported API 稳定失败 |
| 网络 | localhost demo bridge | allowlist + DID signature + scoped bearer + retry/refresh + redaction |
| 身份 | 示例 DID 文件 | host credential provider、DID resolver、token lifecycle、secret store |
| 组件 | coffee 三卡片 | 组件矩阵 P1、Render IR version、动态组件受控、snapshot fixtures |
| 安全 | 单元测试覆盖关键点 | threat model、sandbox gates、package signing、audit persistence、CI fail closed |
| Host | CLI/Mac demo | 稳定 Runtime API / IPC、Host adapter contract、headless mode |
| 运维 | 本地 runbook | metrics/logs/traces、release gates、rollback、privacy deletion |

## 12. 立即下一步建议

如果下一轮开始进入代码开发，建议按以下顺序开工：

1. 先完成 Phase 0 的三份基础文档：`wx-api-compatibility-matrix.md`、`component-compatibility-matrix.md`、`threat-model.md`。
2. 从 Phase 1.1 与 Phase 1.2 开始：增强 validator 与 `wx.modelContext` JS bridge，因为它们影响面清晰、风险较低、会为后续 API 对齐提供骨架。
3. 同步把当前 demo-only `wx.login` / `wx.request` 代码收敛为正式 `DidAuthSessionManager` 和 `RequestBroker` 注入路径。
4. 每补一个 API 或组件能力，必须同时补：兼容矩阵、unit test、fixture 或 snapshot、runbook/CLI 输出。
5. 在 Phase 1/2 期间就启动 Phase 3 threat model，避免后续因为安全边界不清晰返工。
