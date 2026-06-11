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

### 3.2 Component JS 增强

P1 支持：

- `properties` 默认值和基础类型；
- `this.triggerEvent()` 转 Host action；
- `viewCtx.setRelatedPage({ path, query })` 的规范化记录；
- `NotificationType.Overflow`；
- lifecycle trace 细化。

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
| address-form | input/textarea/picker、chooseAddress、L4 consent |
| media-review | `format:image/file`、image preview、file handle |
| dynamic-status | `scope.dynamic`、request/timer、expire cleanup |
| location-map-preview | location provider、map preview、fallback |

每个 fixture 包含：

- Skill package；
- API input cases；
- expected AtomicApiResult；
- expected Render IR snapshot；
- expected actions；
- expected audit summary。

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

- [ ] 组件 manifest 元数据进入 runtime。
- [ ] P1 Component JS/WXML/WXSS 能力有测试。
- [ ] 动态组件默认关闭，声明后受限开放。
- [ ] Render IR schema version 已定义。
- [ ] 至少 3 个 coffee 之外 fixture 有 snapshot。
- [ ] `component-compatibility-matrix.md` 与实现一致。
