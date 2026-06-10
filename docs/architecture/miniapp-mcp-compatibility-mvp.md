# 小程序 MCP 兼容方案 MVP

## 1. 兼容边界

MVP 边界定义为：

> 原子接口尽量做到小程序 MCP 契约级兼容；原子组件做到“小程序 MCP 原子组件运行时子集”兼容，不做完整小程序组件/页面运行时。

`anp-miniapp-dock` 完整继承小程序 MCP 的核心抽象：`SKILL.md`、`mcp.json`、原子接口、原子组件、`content`、`structuredContent`、`_meta`、`sendFollowUpMessage` 和 `api/call`。底层运行时由独立 Rust Runtime、QuickJS-NG、ANP Rust SDK、Render IR 和 Flutter Renderer Adapter 实现，不复刻微信账号、微信支付、云开发、完整页面路由或完整 WXML/WXSS。

## 2. 总体运行时

MVP 拆成两个隔离 VM：

```text
Atomic API VM
  执行 index.js / apis/*.js
  负责原子接口注册、中间件、参数校验、wx.request 和标准返回

Atomic Component VM
  执行 components/*/index.js
  驱动 WXML/WXSS 子集编译、Render IR 和卡片交互
```

两者不共享 JS 全局变量。原子接口、原子组件、动态组件分别使用不同上下文，符合小程序 MCP 的运行模型。

```text
mcp.json
  ├─ apis[]       → Atomic API VM
  └─ components[] → Atomic Component VM

AtomicApiResult
  ├─ content           → Agent / LLM 上下文
  ├─ structuredContent → Agent / LLM 上下文 + 组件渲染数据
  └─ _meta             → 仅组件私有数据，不进入模型上下文
```

## 3. Skill 加载

P0 必须支持：

```text
SKILL.md
mcp.json
index.js
apis/*.js
components/*/{index.js,index.wxml,index.wxss,index.json}
CommonJS require
```

典型目录：

```text
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
    order-confirm/
    payment-result/
```

| 能力 | MVP 策略 |
|---|---|
| `SKILL.md` | P0 读取，作为业务说明和 Agent instruction |
| `mcp.json.apis[]` | P0 支持 |
| `mcp.json.components[]` | P0 读取元数据和动态能力声明 |
| `index.js` | P0 执行 |
| `apis/*.js` | P0 通过受限 CommonJS 加载 |
| `components/*` | P0 支持运行时子集 |
| 多 Skill | P0 可先单 Skill，P1 支持多 Skill |
| 微信分包语义 | 不支持，只按目录加载 |

加载限制：

- `require` 只能加载 Skill 包内文件；
- 禁止路径穿越；
- 禁止远程代码加载；
- 运行时不得修改 `mcp.json`；
- `mcp.json` 原始字段必须保留，ANP 扩展字段使用 `_meta.anp` 或 `x_anp`。

## 4. 原子接口兼容

### 4.1 P0 目标

原子接口是 MVP 优先级最高的兼容面。P0 需要做到：

```text
加载 Skill
注册原子接口
执行原子接口
运行 middleware
校验 inputSchema
返回标准 AtomicApiResult
通过 wx.request 调用商家服务
通过 ANP DID 完成身份和网络请求
```

### 4.2 `mcp.json.apis[]`

P0 必须支持：

```json
{
  "name": "searchDrinks",
  "description": "搜索饮品",
  "_meta": {
    "ui": {
      "componentPath": "components/drink-list/index"
    }
  },
  "inputSchema": {},
  "outputSchema": {}
}
```

| 字段 | 支持策略 |
|---|---|
| `name` | P0 必须支持，唯一，并与 `registerAPI` 名称一致 |
| `description` | P0 必须支持，给 Agent 选择工具使用 |
| `inputSchema` | P0 必须支持，用于参数校验 |
| `outputSchema` | P0 读取，弱校验；失败输出 warning |
| `_meta.ui.componentPath` | P0 必须支持，用于组件绑定 |
| 其他 `_meta` | P0 保留，只传给组件，不暴露给模型 |

P0 校验规则：

1. `name` 必须唯一；
2. `name` 必须能在 `index.js` 中注册；
3. `inputSchema` 必须是 JSON object；
4. `componentPath` 如果存在，必须能在 `components[]` 或文件目录中找到；
5. `outputSchema` 校验失败不阻塞执行，但必须记录 warning。

### 4.3 注册与中间件

P0 必须兼容：

```js
const skill = wx.modelContext.createSkill('/path/to/skill')

skill.registerAPI('searchDrinks', searchDrinks)

skill.use(async (ctx, next) => {
  await next()
})
```

| API / 语义 | P0 |
|---|---|
| `wx.modelContext.createSkill(skillPath)` | 支持 |
| `skill.registerAPI(name, handler)` | 支持 |
| `skill.use(middleware)` | 支持 |
| 多 middleware | 支持 |
| middleware 顺序 | 注册顺序，洋葱模型 |
| `ctx.name` | 支持 |
| `ctx.skillPath` | 支持 |
| `ctx.arguments` | 支持 |
| `next()` | 支持 |
| middleware 捕获异常 | 支持 |
| middleware + handler 共享超时 | 支持，默认 300 秒 |

不支持：

```text
跨 Skill 注册 API
动态注册远程 API
运行时修改 mcp.json
```

### 4.4 入参与 schema

P0 支持：

```text
string
number
boolean
object
array
enum
required
default
description
```

P1 支持：

```text
format: image
format: file
```

P0 暂时只支持文本和结构化参数。图片/文件格式可以先在 schema 中保留，但运行时返回 `unsupported_format` 或进入 P1。

### 4.5 返回结构

P0 必须严格支持：

```ts
interface AtomicApiResult {
  isError?: boolean
  content: TextContent[]
  structuredContent?: Record<string, unknown>
  _meta?: Record<string, unknown>
}

interface TextContent {
  type: "text"
  text: string
}
```

执行规则：

| 返回字段 | 行为 |
|---|---|
| `isError: true` | 不渲染组件，只把 `content` 返回给 Agent |
| `content` | 必须存在，进入 Agent / LLM 上下文 |
| `structuredContent` | 进入 Agent 上下文，也传给组件 |
| `_meta` | 仅传给组件，不进入模型上下文 |
| 绑定组件且非错误 | 尝试渲染组件 |
| 组件渲染失败 | fallback 到 CardSpec |
| 无组件绑定 | 使用 `structuredContent` 生成通用卡片或纯文本 |

### 4.6 原子接口环境 `wx.*`

P0 必须支持：

| 分类 | API | 实现方式 |
|---|---|---|
| Skill | `wx.modelContext.createSkill` | QuickJS Host Bridge |
| Skill | `skill.registerAPI` | Runtime registry |
| Skill | `skill.use` | Middleware chain |
| Session | `wx.modelContext.getSessionId` | Runtime session id |
| Card | `wx.modelContext.expireAllCards` | 通知渲染层过期卡片 |
| 网络 | `wx.request` | ANP DID HTTP Client |
| 存储 | `wx.getStorage / setStorage` | DID + Skill scope storage |
| 存储 | Sync storage API | 本地同步 KV |
| 文件 | `wx.uploadFile` | P0 mock 或简单实现 |
| 文件 | `wx.downloadFile` | P0 简单实现 |
| 文件 | `wx.openDocument` | 调宿主能力或 fallback |
| 系统 | `wx.getDeviceInfo` | 返回 Runtime 环境信息 |
| 系统 | `wx.getAppBaseInfo` | 返回 `anp-miniapp-dock` 信息 |

P0 替代实现：

| 微信 API | ANP Runtime 实现 |
|---|---|
| `wx.login` | ANP DID 登录 |
| `wx.checkSession` | capability token 校验 |
| `wx.requestPayment` | Payment Intent mock + 用户确认 |
| `wx.getPhoneNumber` | MVP mock，后续手机号凭证 |
| `wx.chooseAddress` | MVP mock，后续宿主地址选择器 |
| `wx.getLocation` | 可选授权，P0 可 mock |
| `wx.getFuzzyLocation` | 可选授权，P0 可 mock |

P1 可支持：

| 分类 | API |
|---|---|
| 网络 | WebSocket 子集 |
| 位置 | `wx.chooseLocation` |
| 媒体 | `wx.chooseMedia` |
| 媒体 | `wx.previewMedia` |
| 通知 | `wx.requestSubscribeMessage` 映射到宿主通知授权 |
| 电话 | `wx.makePhoneCall` |
| 扫码 | `wx.scanCode` |

不支持 / 不建议支持：

```text
wx.cloud.*
微信原生支付族 API
微信支付分
微信运动
发票
广告
公众号
视频号
客服
跳转其他小程序
WiFi
蓝牙
TCP
UDP
mDNS
传感器
完整地图能力
人脸核身
```

### 4.7 安全上下文

每个原子接口调用都必须携带运行上下文：

```ts
interface ApiCallContext {
  userDid: string
  agentDid: string
  merchantDid: string
  skillId: string
  sessionId: string
  apiName: string
  arguments: Record<string, unknown>
  capabilityToken?: string
}
```

执行限制：

1. 每次调用独立 QuickJS context；
2. 每个 Skill 独立 storage；
3. `wx.request` 必须走 allowlist；
4. 默认禁止访问文件系统；
5. 默认禁止 `eval`；
6. 默认禁止远程代码加载；
7. 默认超时 300 秒；
8. 高风险 API 必须先走 human authorization。

风险分级：

| 等级 | 例子 | 策略 |
|---|---|---|
| L0 查询 | 搜索商品、查询天气 | 可自动执行 |
| L1 个性化查询 | 查订单、查资产 | 需要登录 |
| L2 写操作 | 加购物车、改规格 | 可执行，但需记录 |
| L3 交易动作 | 下单、支付、退款 | 必须用户确认 |
| L4 隐私动作 | 手机号、地址、证件 | 必须用户确认 + 审计 |

## 5. 原子组件兼容

### 5.1 P0 目标

原子组件目标是：

```text
支持小程序 MCP 原子组件运行时子集
支持组件 JS 生命周期
支持 WXML/WXSS 子集
支持结构化数据渲染成 Flutter 卡片
支持用户点击后继续驱动 Agent 流程
```

推荐描述为：MVP 支持 `MiniApp MCP Atomic Component Runtime Subset`。不要承诺完整兼容微信原子组件、完整 WXML/WXSS 或完整微信小程序组件。

### 5.2 组件加载

组件目录：

```text
components/order-confirm/
  index.js
  index.wxml
  index.wxss
  index.json
```

| 文件 / 能力 | P0 策略 |
|---|---|
| `index.js` | QuickJS Component VM 执行 |
| `index.wxml` | WXML 子集解析 |
| `index.wxss` | WXSS 子集解析 |
| `index.json` | 读取基础配置，弱支持 |
| 自定义组件嵌套 | 不支持 |
| `behaviors` | 不支持 |
| `relations` | 不支持 |
| `slots` | 不支持 |

### 5.3 Component JS

P0 支持：

```js
Component({
  properties: {
    title: String
  },
  data: {
    selected: null
  },
  lifetimes: {
    created() {},
    attached() {},
    detached() {}
  },
  methods: {
    onTap(e) {
      this.setData({ selected: e.currentTarget.dataset.id })
    }
  }
})
```

| 能力 | P0 |
|---|---|
| `Component({})` | 支持 |
| `data` | 支持 |
| `properties` | 支持基础类型 |
| `methods` | 支持 |
| `lifetimes.created` | 支持 |
| `lifetimes.attached` | 支持 |
| `lifetimes.detached` | 支持 |
| `this.setData()` | 支持 |
| `this.data` | 支持 |
| `this.properties` | 支持 |
| `this.triggerEvent()` | P1 |
| `observers` | P2 |
| `behaviors` | 不支持 |
| `relations` | 不支持 |
| `externalClasses` | 不支持 |
| `pageLifetimes` | 不支持 |
| `options` | P2 |
| `slots` | 不支持 |

P0 生命周期：

```text
created
  → 注入 modelContext/viewContext
  → 绑定 input/result/expire 事件

attached
  → 首次生成 Render IR
  → Flutter Renderer 渲染卡片

setData
  → 更新 Component State
  → 重新计算 WXML binding
  → diff Render IR
  → Flutter 局部刷新或整体刷新

detached
  → 清理事件监听
```

### 5.4 WXML 子集

P0 支持示例：

```xml
<view class="card">
  <text>{{title}}</text>
  <image src="{{cover}}" bindload="onImageLoad" binderror="onImageError" />
  <scroll-view scroll-x>
    <view wx:for="{{items}}" wx:key="id" data-id="{{item.id}}" bindtap="onSelect">
      <text>{{item.name}}</text>
    </view>
  </scroll-view>
  <button bindtap="onConfirm">确认</button>
</view>
```

| 能力 | P0 |
|---|---|
| `view` | 支持 |
| `text` | 支持 |
| `image` | 支持 |
| `button` | 支持 |
| `scroll-view` | 支持横向 |
| `wx:if` | 支持 |
| `wx:elif / wx:else` | P1 |
| `wx:for` | 支持 |
| `wx:key` | 支持基础 |
| `{{path}}` 数据绑定 | 支持 |
| 简单表达式 | P1 |
| `bindtap` | 支持 |
| `catchtap` | P1 |
| `data-*` | 支持 |
| `class` | 支持 |
| `style` | 支持 |
| `template/import/include` | 不支持 |
| `slot` | 不支持 |
| 自定义组件嵌套 | P2 |

P0 表达式只支持：

```text
{{foo}}
{{user.name}}
{{items.length}}
{{index}}
{{item.name}}
```

复杂表达式需要开发者在 JS 中通过 `setData` 计算好字段。P0 不支持三元表达式、算术表达式、逻辑默认值或函数调用。

### 5.5 WXSS 子集

P0 支持示例：

```css
.card {
  display: flex;
  flex-direction: column;
  padding: 12px;
  background-color: #ffffff;
  border-radius: 12px;
}

.title {
  font-size: 16px;
  font-weight: bold;
  color: #111111;
}
```

| 能力 | P0 |
|---|---|
| class 选择器 | 支持 |
| id 选择器 | P1 |
| 标签选择器 | P1 |
| 后代选择器 | P1 |
| flex 布局 | 支持 |
| width / height | 支持 |
| min / max width / height | 支持 |
| margin / padding | 支持 |
| color | 支持 |
| background-color | 支持 |
| font-size | 支持 |
| font-weight | 支持 |
| line-height | 支持 |
| text-align | 支持 |
| border | 支持 |
| border-radius | 支持 |
| opacity | 支持 |
| display none/block/flex | 支持 |
| rpx | 支持，映射到 Flutter logical pixels |
| vw | P1 |
| media query | P2 |
| animation / transition | 不支持 |
| transform | P2 |
| box-shadow | P1 |
| 自定义字体 | 不支持 |
| filter / mask | 不支持 |

P0 只追求交易型卡片视觉足够用，不追求 CSS 完整性。

### 5.6 内置组件

P0 必须支持：

| 组件 | 支持范围 |
|---|---|
| `view` | 基础容器、flex、点击区域 |
| `text` | 文本展示，不支持 user-select |
| `image` | 网络图片、png/jpg/webp 可选 |
| `button` | 普通按钮，不支持 open-type |
| `scroll-view` | 仅横向滚动 |

P1 支持：

| 组件 | 支持范围 |
|---|---|
| `map` | MapPreview，不支持拖拽缩放 |
| `canvas` | 静态绘制或降级图片 |
| `input` | 表单场景需要时加入 |
| `textarea` | 表单场景需要时加入 |
| `radio` | 可用 Flutter 组件替代 |
| `checkbox` | 可用 Flutter 组件替代 |
| `picker` | 规格选择可用 BottomSheet 替代 |

不支持：

```text
video
swiper
navigator
web-view 作为组件
ad
ad-custom
functional-page-navigator
所有微信社交 open-type
```

### 5.7 组件环境 `wx.*`

P0 支持：

| API | 策略 |
|---|---|
| `wx.modelContext.getContext(this)` | 支持 |
| `wx.modelContext.getViewContext(this)` | 支持 |
| `modelCtx.on(NotificationType.Input)` | 支持 |
| `modelCtx.on(NotificationType.Result)` | 支持 |
| `viewCtx.on(NotificationType.Expire)` | 支持 |
| `viewCtx.getDimensions()` | 支持 |
| `viewCtx.setRelatedPage({ path, query })` | 记录元数据，MVP fallback |
| `viewCtx.expirePreviousCards()` | 支持 |
| `wx.modelContext.expireAllCards()` | 支持 |
| `sendFollowUpMessage()` | 支持 |
| `wx.getStorage / setStorage` | P0 可支持 |
| `wx.getDeviceInfo` | 支持 |
| `wx.getAppBaseInfo` | 支持 |

P1 支持：

| API | 策略 |
|---|---|
| `viewCtx.openDetailPage()` | BottomSheet / WebView fallback |
| `viewCtx.preloadDetailPage()` | no-op 或预加载 WebView |
| `viewCtx.on(NotificationType.Overflow)` | P1 |
| `wx.previewMedia` | P1 |
| `wx.makePhoneCall` | P1 |

默认不支持，除非声明 dynamic：

```text
wx.request
setTimeout
setInterval
WebSocket
```

如果 `mcp.json.components[].permissions.scope.dynamic` 存在，P1 可开放：

```text
wx.request
setTimeout
setInterval
clearTimeout
clearInterval
```

动态组件限制：

1. 请求域名必须命中 allowlist；
2. 限制最长运行时间；
3. 卡片销毁时清理 timer；
4. 卡片过期时停止动态请求；
5. 宿主后台时暂停轮询。

### 5.8 组件事件

P0 支持：

| 事件 | P0 |
|---|---|
| `tap` | 支持 |
| `image load` | 支持 |
| `image error` | 支持 |
| `Expire` | 支持 |
| `Input` | 支持 |
| `Result` | 支持 |

P1 支持：

```text
longpress
touchstart / touchend，谨慎
Overflow
```

不支持：

```text
复杂手势
动画事件
滚动纵向事件
输入法复杂事件
页面级事件
```

## 6. 数据流

标准数据流：

```text
用户消息
  ↓
Agent 选择 API
  ↓
Atomic API VM 执行 handler
  ↓
返回 AtomicApiResult
  ↓
如果 isError=true:
    content → Agent 回复
  ↓
如果 isError=false 且有 componentPath:
    structuredContent + _meta → Atomic Component VM
  ↓
Component VM 生成 Render IR
  ↓
Flutter Renderer 渲染卡片
  ↓
用户点击组件
  ↓
sendFollowUpMessage 或 api/call
  ↓
Agent 继续下一步
```

接口到组件输入：

```ts
interface ComponentRenderInput {
  apiName: string
  arguments: Record<string, unknown>
  result: {
    content: TextContent[]
    structuredContent?: Record<string, unknown>
    _meta?: Record<string, unknown>
  }
}
```

组件动作输出：

```ts
type ComponentAction =
  | {
      type: "sendFollowUpMessage"
      content: ContentBlock[]
    }
  | {
      type: "api/call"
      name: string
      arguments: Record<string, unknown>
    }
  | {
      type: "openDetailPage"
      url: string
    }
  | {
      type: "expirePreviousCards"
      componentPaths?: string[]
      match?: "all" | "latest"
    }
```

所有 `api/call` 必须回到统一 Orchestrator 调用链，不能由组件直接绕过 inputSchema、权限、consent、middleware 或审计。

## 7. Render IR

WXML AST 不直接驱动 Flutter。P0 渲染链路是：

```text
WXML + WXSS + Component State
  ↓
Render IR
  ↓
Flutter Renderer Adapter
```

P0 Render IR：

```ts
interface RenderNode {
  id: string
  type: "view" | "text" | "image" | "button" | "scroll-view"
  props: Record<string, unknown>
  style: StyleObject
  events?: Record<string, EventBinding>
  children?: RenderNode[]
}
```

Render IR 的作用：

1. 让 Flutter Renderer 更简单；
2. 未来可支持 Web Renderer；
3. 可做快照测试；
4. 可做组件调试器；
5. 可在渲染失败时 fallback 到 CardSpec。

## 8. MVP 优先级

P0 原子接口：

```text
Skill Loader
mcp.json.apis[]
inputSchema 校验
index.js / apis require
createSkill
registerAPI
use middleware
AtomicApiResult
wx.request + ANP DID
storage
getSessionId
expireAllCards
错误处理
超时控制
日志捕获
```

P0 原子组件：

```text
componentPath 解析
components[] 元数据
Component({})
data/properties/methods
created/attached/detached
setData
getContext/getViewContext
Input/Result/Expire
view/text/image/button/scroll-view
wx:if/wx:for
{{path}} binding
bindtap
基础 WXSS
Render IR
Flutter Renderer Adapter
sendFollowUpMessage
api/call
expirePreviousCards
CardSpec fallback
```

P1 原子接口：

```text
format:image/file
chooseMedia
previewMedia
scanCode
makePhoneCall
chooseAddress 真实现
getPhoneNumber 真实现
Payment Intent 真实现
WebSocket
```

P1 原子组件：

```text
openDetailPage fallback
preloadDetailPage
Overflow
scope.dynamic
wx.request in component
timer
map preview
canvas static
input/radio/checkbox/picker
```

不做：

```text
完整微信小程序运行时
完整页面路由
完整半屏小程序页面
完整 WXML/WXSS
完整自定义组件系统
微信云开发
微信原生支付
微信社交生态 API
设备底层复杂 API
```

## 9. MVP 验收组件

使用咖啡点单 demo 验收三张组件。

### 9.1 `drink-list`

对应接口：`searchDrinks`

验证能力：

```text
wx:for
image
text
scroll-view 横向
bindtap
sendFollowUpMessage
api/call
```

### 9.2 `order-confirm`

对应接口：`confirmOrder`

验证能力：

```text
订单结构化展示
价格展示
button
humanAuthorization
expirePreviousCards
```

### 9.3 `payment-result`

对应接口：`payOrder`

验证能力：

```text
状态展示
错误/成功分支
卡片过期
纯文本 fallback
```

## 10. 最终 P0 边界

```text
P0：完整支持原子接口契约与执行；
P0：支持原子组件运行时最小子集；
P0：不支持完整微信小程序组件和页面；
P0：渲染链路采用 Component VM → WXML/WXSS 子集 → Render IR → Flutter Renderer Adapter；
P0：失败时 fallback 到 CardSpec / structuredContent / content。
```

这个边界比纯 CardSpec 更有小程序 MCP 生态兼容价值，同时避免进入完整微信小程序 Runtime 的实现范围。
