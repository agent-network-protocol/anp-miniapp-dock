# MiniApp MCP Component Runtime 渲染方案

## 1. 目标

`MiniApp MCP Component Runtime` 用于在 `anp-miniapp-dock` 中运行小程序 MCP 原子组件子集。目标不是完整复刻微信小程序组件运行时，而是在接口层尽量兼容小程序 MCP 的原子组件写法，在实现层用 Rust、QuickJS-NG、WXML/WXSS 子集编译、Render IR 和 Flutter Renderer Adapter 完成渲染。

核心目标：

- 支持 `mcp.json` 中 `_meta.ui.componentPath` 和 `components[]`；
- 支持组件目录中的 `index.js`、`index.wxml`、`index.wxss`、`index.json`；
- 支持 `Component({})`、基础生命周期、`data`、`properties`、`methods`、`setData`；
- 支持原子组件接收原子接口 `Result`，并响应 tap、image load/error、Expire 等事件；
- 在不兼容时降级到 native adapter、CardSpec 或纯文本。

## 2. 总体架构

```text
mcp.json
  ↓ componentPath
Atomic Component Directory
  ├─ index.js
  ├─ index.wxml
  ├─ index.wxss
  └─ index.json
        ↓
Component Loader
        ↓
QuickJS-NG Component VM
        ↓
WXML/WXSS Subset Compiler
        ↓
Render IR / Component Tree
        ↓
Flutter Renderer Adapter
        ↓
Conversation Card
```

关键原则：

- WXML AST 不直接绑定 Flutter；
- 中间层必须是平台无关 `Render IR`；
- Flutter Renderer Adapter 只是 Render IR 的一个后端；
- CardSpec fallback 保留，避免单个组件失败导致整个 Skill 失败。

## 3. 渲染优先级

```text
1. MiniApp MCP Component Runtime
   Component JS + WXML/WXSS subset -> Render IR

2. 专用 Rust/native component adapter
   针对高价值业务卡片提供手写适配器

3. structuredContent -> CardSpec fallback
   使用原子接口结构化数据生成通用卡片

4. content -> text fallback
   只展示 TextContent
```

Runtime 必须记录 fallback 原因，例如组件加载失败、JS 执行失败、WXML 解析失败、WXSS 不支持、事件处理异常或渲染后端不可用。

## 4. Component JS 子集

MVP 支持：

```text
Component({})
data
properties
methods
lifetimes.created
lifetimes.attached
lifetimes.detached
this.setData()
this.triggerEvent()，可选
bindtap
image load/error
wx.modelContext.getContext(this)
wx.modelContext.getViewContext(this)
NotificationType.Input
NotificationType.Result
NotificationType.Expire
```

MVP 暂不支持：

```text
behaviors
relations
externalClasses
observers
复杂 computed
自定义组件嵌套
slots
page lifecycle
完整小程序页面能力
```

Component VM 与 Atomic API VM 使用不同 QuickJS 上下文，不共享全局变量。组件默认不能调用 `wx.request` 或 timer；只有声明动态组件权限后才开放受限子集。

## 5. WXML 子集

MVP 支持：

```text
view
text
image
button
scroll-view，先只支持横向
wx:if
wx:for
{{ path }} 简单数据绑定
bindtap
class
style
```

MVP 暂不支持：

```text
复杂表达式
template/include/import
slot
复杂自定义组件递归
复杂事件冒泡模型
完整 selector query
```

数据绑定只支持路径读取，例如 `{{ item.name }}`、`{{ total }}`。复杂 JS 表达式应在组件 JS 中计算后写入 `data`。

## 6. WXSS 子集

MVP 支持：

```text
class 选择器
id 选择器，可选
标签选择器，可选
flex 布局
width / height
margin / padding
color / background
font-size / font-weight / line-height
border / border-radius
text-align
```

MVP 暂不支持：

```text
复杂选择器
动画
transition
复杂 transform
复杂 media query
自定义字体
高级 filter / mask
```

不支持的样式应被忽略并记录 warning，不应导致整个组件渲染失败，除非该样式影响安全或布局约束。

## 7. Render IR

Render IR 是平台无关组件树，用于隔离小程序模板语义和具体渲染后端。

建议结构：

```text
RenderNode
  id
  kind: View | Text | Image | Button | ScrollView
  text
  props
  style
  children
  events
  accessibility
```

事件结构：

```text
RenderEventBinding
  event: tap | image_load | image_error
  method: string
  dataset: object
```

动作结果：

```text
ComponentAction
  SendFollowUpMessage
  ApiCall
  ExpireCards
  OpenDetailPageFallback
  Noop
```

## 8. wx Compatibility Layer

原子组件环境默认支持：

- `wx.modelContext.getContext(this)`；
- `wx.modelContext.getViewContext(this)`；
- `ctx.sendFollowUpMessage({ content })`；
- `viewCtx.expirePreviousCards({ componentPaths, match })`；
- `wx.modelContext.expireAllCards({ componentPaths, match })`；
- `viewCtx.setRelatedPage({ path, query })`，MVP 可记录但不打开完整页面；
- storage 子集；
- `wx.getDeviceInfo`、`wx.getAppBaseInfo`。

动态组件权限打开后，才支持：

- 受 allowlist 限制的 `wx.request`；
- 受生命周期管理的 `setTimeout` / `setInterval` 子集；
- 组件 detach 时自动清理 timer。

`components[].permissions.scope.dynamic` 是小程序 MCP 原始动态组件声明，应优先识别。ANP 扩展权限可以放在 `_meta.anp` 或 `x_anp` 中，但不能要求原 Skill 必须改写。

## 9. 生命周期与数据流

原子接口返回结果后：

```text
Atomic API Result
  → componentPath lookup
  → Component VM created
  → lifetimes.created
  → NotificationType.Result(result)
  → initial data binding
  → lifetimes.attached
  → WXML/WXSS compile
  → Render IR
  → Renderer
```

用户点击后：

```text
tap event
  → RenderEventBinding
  → Component VM method
  → setData / sendFollowUpMessage / api/call / expire
  → Render IR patch or new API call
```

组件过期后：

```text
expire request
  → mark card expired
  → NotificationType.Expire
  → lifetimes.detached when removed
  → disable actions and show expired overlay/text
```

## 10. 分阶段实现

P0：原子组件运行时最小子集

- 加载组件目录；
- 执行 `Component({ data, properties, lifetimes, methods })` 子集；
- 支持 `created`、`attached`、`detached`；
- 支持 `setData`；
- 支持 `NotificationType.Input`、`NotificationType.Result`、`NotificationType.Expire`；
- 解析 `view`、`text`、`image`、`button`、横向 `scroll-view`；
- 支持 `wx:if`、`wx:for`、`{{ path }}`、`bindtap` 和基础 WXSS；
- 输出 Render IR；
- Flutter Renderer Adapter 展示基础组件；
- 任一阶段失败时 fallback 到 CardSpec / structuredContent / content。

P1：动态组件与半屏 fallback

- 支持 `scope.dynamic` 下的 request/timer 子集；
- 支持 `openDetailPage` / `preloadDetailPage` 的 BottomSheet 或 WebView fallback；
- 支持 `Overflow`；
- 支持 map preview、canvas static、input/radio/checkbox/picker。

P2：更完整兼容

- 支持更多 WXML 表达式、选择器、组件嵌套和 WXSS 能力；
- 扩展交易型 Skill 兼容测试集。

详细 P0/P1 支持矩阵见 [小程序 MCP 兼容方案 MVP](miniapp-mcp-compatibility-mvp.md)。

## 11. 咖啡点单 demo 验证

demo 至少包含三张原子组件卡片：

```text
drink-list
  - 展示饮品列表
  - bindtap 选择商品
  - 发起 api/call: confirmOrder

order-confirm
  - 展示订单摘要和价格
  - 点击确认支付
  - 进入 consent
  - 发起 api/call: payOrder

payment-result
  - 展示支付结果
  - 触发旧 order-confirm 卡片过期
```

验收标准：

- 组件目录按 `componentPath` 加载；
- Component VM 能接收原子接口结果；
- WXML/WXSS 子集能生成 Render IR；
- 点击事件能触发 methods；
- `api/call` 回到统一 Orchestrator 调用链；
- 任一组件失败时能降级到 CardSpec；
- 失败原因进入日志和测试断言。
