# Phase 2：组件运行时对齐实施计划

## 1. 阶段目标

Phase 2 要把当前 coffee 三卡片可用的 Component Runtime 扩展为稳定的 MiniApp MCP 原子组件核心子集。目标不是完整 UI 展现，而是让组件 JS、WXML/WXSS 子集、事件、动态能力和 Render IR contract 足够稳定，便于不同 Host 渲染。

深入设计见：[Render IR 与 Fixture 体系](phase-2-render-ir-and-fixtures.md)。

## 2. 涉及模块

| 模块 | 职责 |
|---|---|
| `crates/component-runtime` | Component VM、WXML/WXSS parser/compiler、Render IR、events |
| `crates/wx-compat` | component capability profile、model/view context、card events、dynamic permission |
| `crates/dock-core` | render routing、component action 回流、fallback |
| `crates/card-spec` | fallback card schema |
| `crates/dock-cli` | `preview-component`、snapshot fixture runner |
| `examples/*-skill` | 组件兼容 fixture |

## 3. 开发顺序

### 3.1 组件 manifest 元数据

实现内容：

- 读取并保留 `components[].relatedPage`；
- 支持 `components[].permissions.scope.dynamic`；
- 支持 `components[].expirable` 和 `expiredText`；
- 保留 `_meta` 和未知字段；
- `api._meta.ui.componentPath` 与 `components[].path` 统一 canonicalize。

开发建议：

1. 先在 `mcp-schema` 扩展结构；
2. 再在 `skill-loader` 校验路径；
3. 然后在 `component-runtime` 的 `ComponentInput` 或 component metadata 中传入；
4. 最后让 `dock-cli validate` 输出组件能力报告。

Step 02-02 已完成当前 metadata flow：`relatedPage`、`permissions.scope.dynamic`、`expirable`、`expiredText` 由 manifest 进入 Component Runtime `ComponentMetadata` 和 `dock-cli` render / validate envelope；unsafe `relatedPage.path` 不进入 runtime metadata，query 与 scopeDynamic 中的 token-like 字段会脱敏。该 flow 只表达权限与 Host metadata，不开放 dynamic request/timer。

### 3.2 Component JS 增强

P1 支持：

- `properties` 默认值和基础类型；
- `this.triggerEvent()` 转 Host action；
- `viewCtx.setRelatedPage({ path, query })` 的规范化记录；
- `NotificationType.Overflow`；
- lifecycle trace 细化。

当前 01-05 至 02-07 执行批次只覆盖 Phase 2 起步能力：manifest metadata runtime flow、WXML/WXSS P1 子集、表单/静态媒体节点、dynamic request/timer gate、fixtures 和 Render IR snapshots。`this.triggerEvent()` 与 `preloadDetailPage()` 仍保留为组件矩阵中的 `planned-p1`，后续必须单独拆 Step 或通过 Plan 变更降级后，才能把 Phase 2 全量 P1 关闭。

P2 规划：

- `observers`；
- `options` 部分字段；
- 更复杂 property observer；
- 自定义组件嵌套。

不支持：

- behaviors、relations、slots、externalClasses、完整 pageLifetimes。

### 3.3 WXML 子集增强

当前 P0 基础上增加：

- `wx:elif` / `wx:else`；
- `catchtap`；
- disabled button 事件抑制；
- 简单表达式：`!flag`、`a === b`、literal、简单 boolean；
- 表单组件：`input`、`textarea`、`radio`、`checkbox`、`picker`；
- `map` preview 和 `canvas` static 的 Render IR node。

Step 02-03 已完成其中的 WXML P1 语法子集：`wx:if` / `wx:elif` / `wx:else` 条件链、`catchtap` Render IR event、disabled button 事件抑制，以及 `!flag`、`===`、`!==`、literal、boolean、null 和 path 的受限 expression evaluator。function call、mutation、复杂运算和 arbitrary JS 仍 fail closed 并产生 warning。

Step 02-04 已完成表单与静态媒体节点的 Render IR 子集：`input`、`textarea`、`radio`、`checkbox`、`picker` 输出安全 props、`input` / `change` 受控事件和 disabled 状态；`map` / `canvas` 分别编译为 `map-preview` / `canvas-static` 静态节点。真实 Host 输入控件、表单提交路径、位置 provider、media provider、MapContext 和 canvas script 仍不在本 Step 开放。

仍不支持：

- template/import/include；
- slot；
- 完整自定义组件嵌套；
- complex expression / function call / arbitrary JS expression。

### 3.4 WXSS 子集增强

P1 增加：

- id selector；
- tag selector；
- simple descendant selector；
- `gap`、`justify-content`、`align-items`；
- `min-width` / `max-width` / `min-height` / `max-height`；
- `box-shadow`；
- `overflow-x`。

Step 02-03 已完成 id selector、tag selector、一层 simple descendant selector，以及 `gap`、`justify-content`、`align-items`、min/max、`box-shadow`、`overflow-x` 的 `RenderStyle` 输出。复杂 selector、media query、animation、transition、filter/mask 和 custom font 仍保持 warning / unsupported。

仍降级或 warning：

- animation / transition；
- complex transform；
- media query；
- filter / mask；
- custom font。

### 3.5 动态组件

仅当组件声明 `permissions.scope.dynamic` 时开放：

- 受限 `wx.request`；
- `setTimeout` / `setInterval` / clear；
- 可选 polling helper。

安全限制：

- request 仍走 allowlist 与 token boundary；
- timer 数量和频率限制；
- expire/detach 自动清理；
- Host background 自动暂停；
- 动态请求进入 audit summary。

Step 02-02 只让 `scope.dynamic` 在 runtime metadata 中可观测；真正开放 request/timer 必须等 Step 02-05 的 sandbox escape、resource-limit、cleanup gate 通过。

Step 02-05 已完成最小 dynamic component gate：Component VM 默认仍 deny `wx.request` 和 timer；只有 runtime metadata 中 `scope.dynamic` 生效时才注入受限 `wx.request` / `setTimeout` / `setInterval` / clear；request 通过 injected `RequestBroker`，默认 `UnsupportedRequestBroker` fail closed，JS 提供 `Authorization` / `Signature` / `Signature-Input` / `Cookie` 会被拒绝，响应 auth/token-like headers 会脱敏；timer 有数量限制、clear 和 expire/detach cleanup。当前 headless runtime 只在 mount/event flush 中执行 delay 0 timer，不提供生产 Host background scheduler；Host background pause、真实 polling helper、生产网络 transport 和 request audit persistence 仍属于 Phase 4 / Host 边界。

### 3.6 Component Action 回流

组件内动作不直接执行高风险操作，只能返回 action 给 Host/Orchestrator：

- `sendFollowUpMessage`；
- `api/call`；
- `expirePreviousCards`；
- `expireAllCards`；
- `openDetailPage` fallback；
- `setRelatedPage` metadata。

`api/call` 必须回到 `dock-core::Orchestrator.call_api`，重新经过 input validation、permission、consent、audit。

## 4. Fixture 计划

至少新增以下 fixture：

| Fixture | 覆盖能力 |
|---|---|
| coffee | 交易基础、列表、确认、支付、过期 |
| address-form | input/textarea/picker、chooseAddress Host boundary、opaque address handle、L4 audit summary |
| media-review | `format:image/file`、image preview、opaque file/image handle、canvas-static poster |
| dynamic-status | `scope.dynamic`、RequestBroker request、timer flush、expire cleanup |
| location-map-preview | location provider fail closed、opaque location token、map preview、fallback |

每个 fixture 包含：

- Skill package；
- API input cases；
- expected Render IR snapshot；
- expected actions；
- expected warnings / metadata / state；
- expected audit summary。

Step 02-06 已将这些 fixture 放入 `examples/fixtures/`，golden snapshots 放入 `testdata/render-ir/`，并由 `cargo test -p component-runtime snapshot` 与 `cargo test -p dock-cli fixture` 验证。

## 5. 测试计划

| 层级 | 内容 |
|---|---|
| parser tests | WXML/WXSS 新语法 |
| compiler tests | Render IR node/style/event output |
| VM tests | lifecycle、properties、setData、triggerEvent、dynamic permission |
| snapshot tests | fixture Render IR 稳定性 |
| integration tests | action -> Orchestrator -> API -> render loop |
| fallback tests | WXML parse failed、unsupported node、host unsupported |

## 6. 阶段完成检查

- [x] 组件 manifest 元数据进入 runtime。
- [x] 当前批次覆盖的 P1 WXML/WXSS、表单/静态媒体、dynamic 和 fixture 能力有测试。
- [ ] `this.triggerEvent()` 与 `preloadDetailPage()` 仍为 `planned-p1`，需要后续 Step 或 Plan 变更处理。
- [x] 动态组件默认关闭，声明后受限开放。
- [x] Render IR schema version 已定义。
- [x] 至少 3 个 coffee 之外 fixture 有 snapshot。
- [x] `component-compatibility-matrix.md` 与实现一致。
