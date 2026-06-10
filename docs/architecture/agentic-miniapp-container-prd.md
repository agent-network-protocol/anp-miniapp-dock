# 智能体原生小程序容器 MVP 需求说明

## 1. 背景与目标

本产品面向 aWiki 及 ANP 生态，开发一个“智能体原生小程序容器”。容器的核心目标不是复刻微信小程序运行时，而是在智能体对话场景中承载小程序 MCP（详细讲 (微信小程序mcp)[docs/weichat-miniapp-mcp-protocol/weichat-miniapp-mcp.txt]  ） 形态的 Skill：通过自然语言触发原子接口，返回结构化数据，并以轻量组件卡片完成选择、确认、支付、状态展示等关键交互。

微信小程序 AI 开发模式已经证明了“原子接口 + 原子组件 + SKILL + mcp.json”的产品形态：开发者把小程序功能抽象为原子接口和原子组件，并封装成 SKILL，供小程序 AI 通过小程序 MCP 选择和调用。我们的容器将复用这套契约机制，但身份层改为 ANP DID，运行环境由 aWiki/ANP 容器提供。

微信小程序AI开发指南：https://developers.weixin.qq.com/miniprogram/dev/ai/guide.html

## 2. 产品定位

产品名称暂定为 **Agentic MiniApp Container**，中文可称为“智能体原生小程序容器”。

产品定位：

- 面向 Agent 的小程序 Skill 运行容器；
- 兼容小程序 MCP 契约；
- 使用 ANP DID 做登录、鉴权和调用方身份识别；
- 支持 JS Sandbox 执行原子接口；
- 使用轻量卡片组件承载交互，不实现完整传统小程序 UI；
- MVP 只支持必要组件和必要接口，不完整兼容微信全部 API和组件。

本产品不是：

- 不是完整微信小程序运行时；
- 不是 WXML/WXSS/页面路由完整兼容层；
- 不是微信账号、微信支付、微信云开发能力的替代实现；
- 不是传统 App 页面容器。

## 3. 核心用户与场景

### 3.1 用户角色

1. **终端用户**：在 aWiki 中通过自然语言调用商家或服务方智能体。
2. **Skill 开发者**：按照小程序 MCP 结构提供 `SKILL.md`、`mcp.json`、原子接口和组件。
3. **商家/服务方 Agent**：通过 ANP 暴露服务能力，接收 DID 登录和调用。
4. **aWiki 容器**：负责发现、鉴权、加载、执行、渲染和授权确认。

### 3.2 典型场景

用户在 aWiki 中输入“帮我点一杯少糖拿铁”，aWiki 发现咖啡商家 Agent，使用用户 DID 完成登录，加载该 Agent 的 Skill，调用搜索、选品、确认订单等原子接口，展示订单确认卡片。用户点击确认后，容器触发 human authorization，再调用支付或模拟支付接口，最后展示订单状态卡片。

## 4. MVP 产品边界

### 4.1 MVP 必须支持

1. ANP DID 登录验证；
2. 加载并校验小程序 MCP 格式的 `mcp.json`；
3. 加载 `SKILL.md` 作为业务流程说明；
4. 支持 `wx.modelContext.createSkill`、`skill.registerAPI`、`skill.use` 语义；
5. 支持 JS Sandbox 执行原子接口 JS；
6. 支持原子接口返回 `isError`、`content`、`structuredContent`、`_meta`；
7. 支持 `_meta.ui.componentPath` 绑定组件；
8. 支持通用结构化卡片渲染；
9. 支持 `sendFollowUpMessage` 和 `api/call`；
10. 支持卡片过期态；
11. 支持最小 `wx` shim：网络、存储、会话、文件、位置可选；
12. 高风险动作支持用户确认和审计。
13. 接口和组件与小程序mcp保持一致

### 4.2 MVP 暂不支持

1. 完整 WXML/WXSS 渲染；
2. 完整页面路由、TabBar、多页面生命周期；
3. 微信登录、微信 openid/unionid；
4. 微信原生支付收银台；
5. 微信云开发原生能力；
6. 广告、公众号、视频号、客服、跳转其他小程序；
7. 蓝牙、WiFi、TCP、UDP、传感器等复杂设备能力；
8. 完整地图交互；
9. 完整半屏小程序页面。

## 5. 兼容策略

“完全兼容小程序 MCP”在 MVP 中定义为**契约兼容**，不是完整运行时兼容。

必须兼容的契约包括：

- `SKILL.md`：业务说明、流程编排、跨接口规则；
- `mcp.json.apis[]`：原子接口声明；
- `name`：接口名，与注册函数一致；
- `description`：模型选择工具的依据；
- `inputSchema`：入参 JSON Schema；
- `outputSchema`：结构化返回 Schema；
- `_meta.ui.componentPath`：组件绑定；
- `components[]`：组件路径、过期态、关联页面元数据；
- 原子接口返回结构：`isError`、`content`、`structuredContent`、`_meta`；
- 中间件：统一鉴权、日志、错误处理。

对于原子组件，采用两级策略：

1. 如果存在 aWiki 原生组件适配器，MVP版本 执行完整小程序组件代码，则渲染专用卡片；
2. 否则使用 `structuredContent` 渲染通用结构化卡片。

## 6. 身份与鉴权需求

容器使用 ANP DID 作为核心登录身份。

登录流程：

1. aWiki 发现商家 Agent，一般是在我们服务上有注册；
2. 读取 skill。
3. 用户 DID 对登录请求或 challenge 签名；
4. 商家 Agent 验证 DID Document 和签名；
5. 商家创建或查找 DID 绑定账户；
6. 商家返回短期 capability token；
7. 后续 Skill 原子接口调用携带该 token。

容器需支持：

- 用户 DID 管理；
- Agent DID 校验；
- DID 签名登录；
- Token 缓存与过期处理；
- 多商家会话隔离；
- 高风险动作的 human authorization；
- 未来可扩展 EID → DID 绑定凭证。

## 7. JS Sandbox 需求

MVP 必须提供 JS Sandbox，用于执行小程序 MCP 原子接口代码。

### 7.1 Sandbox 能力

- 支持加载 Skill 目录中的 `index.js` 和 `apis/*.js`；
- 支持 CommonJS 风格 `require`，限制只能加载 Skill 包内文件；
- 支持 async/await；
- 支持 `skill.registerAPI(name, handler)`；
- 支持 `skill.use(middleware)`；
- 支持每次原子接口调用独立上下文；
- 支持超时控制，默认不超过 300 秒；
- 支持日志捕获、错误捕获、调用链追踪。

### 7.2 安全限制

- 禁止访问宿主全局对象；
- 禁止任意文件系统访问；
- 禁止任意网络访问，必须走 `wx.request` allowlist；
- 禁止动态加载远程代码；
- 禁止 eval 或可配置禁用 eval；
- 限制内存、CPU、执行时间；
- 每个 Skill 独立存储命名空间；
- 每个商家 Agent 独立权限域。

## 8. wx Shim / Capability Broker

容器提供有限 `wx` 兼容层。

### 8.1 MVP 支持

- `wx.request`：受域名白名单限制；
- `wx.getStorage`、`wx.setStorage`、Sync 版本：DID + Skill 作用域隔离；
- `wx.getStorageInfo`、`wx.removeStorage`、`wx.clearStorage`；
- `wx.downloadFile`、`wx.uploadFile`、`wx.openDocument`；
- `wx.getLocation` / `wx.getFuzzyLocation`：需用户授权；
- `wx.getDeviceInfo`、`wx.getAppBaseInfo`：返回容器环境信息；
- `wx.modelContext.getSessionId`；
- `wx.modelContext.expireAllCards`；
- `sendFollowUpMessage`；
- `api/call`。

### 8.2 替代实现

- `wx.login` → ANP DID 登录；
- `wx.checkSession` → capability token 校验；
- `wx.getPhoneNumber` → 用户授权手机号凭证，MVP 可 mock；
- `wx.chooseAddress` → aWiki 地址选择器，MVP 可 mock；
- `wx.requestPayment` → Payment Intent + 用户确认，MVP 可 mock；
- `openDetailPage` → aWiki 半屏卡片或 WebView fallback。

## 9. 组件与交互需求

MVP 不追求完整 UI，只支持组件卡片。

必须支持的卡片能力：

- 文本说明；
- 图片展示；
- 列表展示；
- 按钮动作；
- 单选/多选；
- 表单输入；
- 价格/订单确认；
- 状态展示；
- 错误提示；
- 卡片过期态；
- 点击后触发 `sendFollowUpMessage`；
- 点击后触发 `api/call`；
- 高风险按钮触发用户确认。

组件渲染优先级：

1. 专用 aWiki 原生卡片；
2. 通用 JSON Card；
3. 纯文本 fallback。

## 10. 安全与风控

容器必须内置权限声明和运行时校验。

Skill 需要声明：

- 网络域名；
- 是否需要位置；
- 是否需要手机号；
- 是否需要地址；
- 是否需要支付；
- 是否需要文件/图片；
- 是否需要动态组件能力。

高风险动作必须用户确认，包括：

- 下单；
- 支付；
- 提交地址；
- 提交手机号；
- 绑定身份；
- 上传隐私文件；
- 打开外部链接。

每次高风险动作应记录：

- 用户 DID；
- 商家 Agent DID；
- Skill 名称；
- API 名称；
- 关键参数摘要；
- 用户确认时间；
- human authorization proof；
- 调用结果。

## 11. 开发者体验需求

MVP 需要提供基础 CLI 或 SDK：

- 初始化 Skill 项目；
- 导入小程序 MCP Skill；
- 校验 `mcp.json`；
- 校验 `inputSchema` / `outputSchema`；
- 本地运行 JS Sandbox；
- 模拟原子接口调用；
- 预览结构化卡片；
- 输出 ANP Agent 接入示例。

建议命令：

```bash
agent-mini init
agent-mini validate
agent-mini run-skill
agent-mini call-api
agent-mini preview-card
```

## 12. 验收标准

MVP 完成后，应至少跑通一个咖啡点单 demo：

1. aWiki 通过 ANP DID 登录商家 Agent；
2. 容器加载 `SKILL.md` 和 `mcp.json`；
3. JS Sandbox 注册并执行原子接口；
4. 用户自然语言触发搜索商品接口；
5. 返回 `structuredContent` 并渲染商品卡片；
6. 用户点击卡片触发 `api/call`；
7. 调用确认订单接口；
8. 展示订单确认卡；
9. 用户确认支付；
10. 触发 human authorization；
11. 调用支付 mock 接口；
12. 展示支付结果和订单状态；
13. 旧订单确认卡被置为过期态。

## 13. 后续扩展

MVP 后可逐步支持：

- WXML/WXSS 子集到 Card Schema 转换；
- 原子组件 JS 的受限执行；
- 半屏页面 WebView fallback；
- 更完整的地址、手机号、支付凭证；
- EID-DID 绑定凭证；
- Skill 市场；
- 自动评测；
- 微信 Skill 双向导入/导出；
- 商家 Agent SDK。

## 14. 关键原则

1. Skill-first，不做 Page-first。
2. Contract-first，不做完整小程序 Runtime-first。
3. DID-first，用 ANP 身份替代微信登录。
4. Card-first，不做复杂 UI。
5. Sandbox-first，执行能力必须安全隔离。
6. Consent-first，高风险动作必须用户确认。
7. Compatibility-by-contract，优先兼容小程序 MCP 契约，而不是微信全部 API。
