# anp-skill-dock 整体系统架构设计

## 1. 架构目标

`anp-skill-dock` 是一个面向 ANP 生态的智能体原生小程序容器。它的目标不是完整复刻微信小程序运行时，而是兼容小程序 MCP 的核心契约：`SKILL.md`、`mcp.json`、原子接口、原子组件元数据、`content / structuredContent / _meta` 返回结构、`sendFollowUpMessage`、`api/call`、卡片过期态和中间件机制。

MVP 的核心目标：

1. 使用 ANP DID 做登录、鉴权、调用方身份识别；
2. 使用 QuickJS-NG 作为 JS Sandbox，执行小程序 MCP 原子接口；
3. 兼容小程序 MCP 契约，不完整兼容微信全部 API 和组件；
4. 使用轻量卡片渲染，不做完整 WXML/WXSS/页面路由；
5. `wx.request` 等底层网络能力直接集成 ANP DID 身份；
6. 提供一个服务端 demo，供 aWiki 客户端集成验证。

## 2. 总体架构

```text
aWiki Client
  ├─ ANP DID Wallet
  ├─ Skill Loader
  ├─ QuickJS-NG Sandbox
  ├─ wx Shim / Capability Broker
  ├─ Card Renderer
  └─ Consent & Audit Engine
        │
        │ ANP DID Auth / Signed HTTP / Capability Token
        ▼
Demo Merchant Agent Server
  ├─ Agent Registry
  ├─ Skill Package Service
  ├─ DID Login Service
  ├─ Skill RPC Service
  ├─ Mock Business APIs
  └─ Audit Log
```

整体采用“端侧容器 + 服务端 Agent demo”的架构。aWiki 负责加载 Skill、执行原子接口、渲染卡片和用户授权；服务端负责提供商家 Agent 注册信息、Skill 包、DID 登录校验、模拟商品/订单/支付接口。

## 3. 客户端模块设计

### 3.1 Skill Loader

负责加载和校验 Skill 包。

输入：

```text
SKILL.md
mcp.json
index.js
apis/*.js
components metadata
```

职责：

- 解析 `mcp.json.apis[]`；
- 校验 `name / description / inputSchema / outputSchema`；
- 校验 `_meta.ui.componentPath`；
- 加载 `SKILL.md` 作为模型/编排说明；
- 建立 API name 到 JS handler 的映射；
- 建立 componentPath 到 Card Renderer 的映射；
- 生成可供 aWiki Agent 调用的 tool registry。

### 3.2 QuickJS-NG Sandbox

JS 引擎使用 QuickJS-NG。每个 Skill 独立创建 Sandbox 实例，每次原子接口调用创建隔离上下文。

MVP 支持：

- CommonJS `require`，仅允许加载 Skill 包内部文件；
- async/await；
- `wx.modelContext.createSkill(skillPath)`；
- `skill.registerAPI(name, handler)`；
- `skill.use(middleware)`；
- 中间件链式执行；
- 调用超时，默认 300 秒；
- console 日志捕获；
- 异常捕获与结构化错误返回；
- 每个 Skill 独立 storage namespace。

安全限制：

- 禁止任意文件系统访问；
- 禁止动态加载远程代码；
- 禁止直接访问宿主能力；
- 禁止任意网络访问，必须走 `wx.request`；
- 网络请求必须通过 domain allowlist；
- 限制执行时间、内存和调用深度；
- Sandbox 只能通过 Host Bridge 调用容器能力。

### 3.3 wx Shim / Capability Broker

容器提供有限 `wx` 兼容层。JS 代码调用 `wx.*`，实际由 Host Bridge 转发到 aWiki 原生能力或 ANP 网络能力。

MVP 支持 API 边界：

```text
wx.modelContext.createSkill
skill.registerAPI
skill.use
wx.modelContext.getSessionId
wx.modelContext.expireAllCards

wx.request
wx.getStorage / wx.setStorage
wx.getStorageSync / wx.setStorageSync
wx.getStorageInfo
wx.removeStorage
wx.clearStorage

wx.downloadFile
wx.uploadFile
wx.openDocument

wx.getDeviceInfo
wx.getAppBaseInfo
wx.getLocation / wx.getFuzzyLocation，可选授权
```

替代实现：

```text
wx.login              → ANP DID 登录
wx.checkSession       → capability token 校验
wx.getPhoneNumber     → MVP mock，后续接手机号凭证
wx.chooseAddress      → MVP mock，后续接地址选择器
wx.requestPayment     → MVP mock Payment Intent + 用户确认
openDetailPage        → MVP WebView / 半屏卡片 fallback
```

暂不支持：

```text
微信云开发
微信原生支付
公众号/视频号/客服
广告
跳转其他小程序
蓝牙/WiFi/TCP/UDP
完整地图交互
完整页面路由和生命周期
```

### 3.4 ANP DID 网络层

所有由 Skill 发出的网络请求统一经过 `AnpHttpClient`。

调用链：

```text
JS wx.request
  → Host Bridge
  → Permission Check
  → AnpHttpClient
  → DID Signature / Capability Token
  → HTTP Request
```

规则：

- 访问商家 Agent 默认携带 DID 签名或 capability token；
- 首次访问商家服务时走 DID challenge 登录；
- 登录成功后缓存短期 token；
- token 按商家 DID、用户 DID、Skill ID 隔离；
- 第三方域名请求必须在 Skill permission manifest 中声明；
- 高风险接口必须由 Consent Engine 先完成用户确认。

## 4. 渲染引擎方案

### 4.1 推荐方案：CardSpec Renderer

MVP 不实现 WXML/WXSS 渲染，而实现一个 `CardSpec Renderer`。这是最适合一两天用 Codex 落地的方案。

核心思想：

```text
structuredContent + _meta + componentPath
  → Component Adapter
  → CardSpec JSON
  → Flutter Native Card
```

渲染优先级：

1. `componentPath` 命中特定适配器，渲染专用卡片；
2. 未命中时，根据 `structuredContent` 自动生成通用 JSON Card；
3. 如果结构无法识别，降级为 `content` 文本消息。

### 4.2 CardSpec MVP 组件

MVP 只支持以下组件：

```text
Text
Image
List
Button
ActionBar
RadioGroup
CheckboxGroup
FormInput
PriceBlock
OrderSummary
StatusBlock
ErrorBlock
ExpireOverlay
```

事件能力：

```text
sendFollowUpMessage
api/call
expireCard
openDetailPage fallback
humanAuthorization
```

示例：

```json
{
  "type": "order_summary",
  "title": "确认订单",
  "items": [
    { "name": "拿铁", "spec": "中杯 / 少糖", "quantity": 1, "price": 18 }
  ],
  "payable": 18,
  "actions": [
    {
      "type": "api/call",
      "label": "确认支付",
      "api": "payOrder",
      "arguments": { "orderId": "o_001" },
      "risk": "payment"
    }
  ],
  "expirable": true
}
```

### 4.3 后续渲染演进

MVP 后可增加：

- WXML/WXSS 子集到 CardSpec 的转换；
- 原子组件 JS 的受限执行；
- WebView 半屏页面 fallback；
- 更复杂的动态组件；
- Flutter 原生组件市场。

## 5. 原子接口调用流程

```text
1. 用户在 aWiki 中发起自然语言请求
2. aWiki Agent 根据 SKILL.md + mcp.json 选择原子接口
3. Skill Runtime 校验 inputSchema
4. QuickJS-NG 执行 API handler
5. handler 内部通过 wx.request 调商家服务
6. AnpHttpClient 自动附加 DID 身份
7. handler 返回 isError/content/structuredContent/_meta
8. Card Renderer 根据 componentPath 渲染卡片
9. 用户点击卡片按钮
10. 触发 sendFollowUpMessage 或 api/call
11. 高风险动作进入 Consent Engine
12. 用户确认后继续调用后续原子接口
```

## 6. 服务端 Demo 架构

MVP 需要创建一个服务端示例项目：`anp-skill-dock-demo-server`。

服务端职责：

- 提供商家 Agent 注册信息；
- 提供 Skill 包下载；
- 提供 DID challenge 登录；
- 验证 DID 签名；
- 签发 capability token；
- 提供咖啡点单模拟 API；
- 提供订单和支付 mock；
- 输出审计日志。

建议接口：

```text
GET  /registry/agents
GET  /agents/coffee/manifest
GET  /agents/coffee/SKILL.md
GET  /agents/coffee/mcp.json
GET  /agents/coffee/package.zip

POST /agents/coffee/auth/challenge
POST /agents/coffee/auth/login

GET  /api/drinks
POST /api/order/confirm
POST /api/order/pay
GET  /api/order/{orderId}
```

Skill 包示例结构：

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

Demo 业务闭环：

```text
searchDrinks → 渲染饮品列表
confirmOrder → 渲染订单确认卡
payOrder → 用户确认 → mock 支付 → 渲染支付结果
expireAllCards → 旧确认卡过期
```

## 7. 权限与风控

Skill 必须声明权限：

```json
{
  "permissions": {
    "network": ["https://demo.example.com"],
    "storage": true,
    "location": false,
    "phoneNumber": false,
    "address": false,
    "payment": true
  }
}
```

运行时规则：

- 未声明权限不可调用对应能力；
- 网络请求必须命中 allowlist；
- 支付、下单、地址、手机号等动作必须用户确认；
- 所有高风险动作生成审计记录；
- 审计记录包含用户 DID、商家 DID、Skill ID、API name、参数摘要、确认时间、执行结果。

## 8. 一两天落地路径

第一天：

1. 创建 `anp-skill-dock` 客户端模块骨架；
2. 集成 QuickJS-NG；
3. 实现 `createSkill/registerAPI/use`；
4. 实现 Skill Loader；
5. 实现 `wx.request`、storage、console bridge；
6. 实现 demo server 的 mcp.json、SKILL.md 和 mock API。

第二天：

1. 实现 CardSpec Renderer；
2. 实现 `sendFollowUpMessage` 和 `api/call`；
3. 实现 DID login mock 或接入现有 ANP DID；
4. 实现 `AnpHttpClient` 自动带身份；
5. 跑通咖啡点单闭环；
6. 输出 demo 运行说明和测试用例。

## 9. 关键设计原则

1. Contract-first：优先兼容小程序 MCP 契约。
2. DID-first：身份、网络、会话都围绕 ANP DID。
3. Sandbox-first：所有 Skill JS 必须隔离执行。
4. Card-first：不做完整页面，只做卡片交互。
5. Permission-first：所有宿主能力必须声明和校验。
6. Consent-first：高风险动作必须用户确认。
7. Demo-first：先跑通端到端闭环，再扩展组件/API。
