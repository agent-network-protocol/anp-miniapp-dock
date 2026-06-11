# anp-miniapp-dock 15 分钟分享文字稿

> 适用场景：15 分钟技术分享。每页文字稿按自然口语设计，可根据现场节奏删减。

## 1. 开源的智能体原生小程序 MCP 容器

大家好，今天想分享我们正在做的一个开源原型：`anp-miniapp-dock`，也可以叫“智能体原生小程序 MCP 容器”。

它不是要复刻一个完整的微信小程序运行时，而是想解决 Agent 调用真实商家服务时的一个关键问题：我们已经看到了小程序 MCP 这种“原子接口 + 原子组件 + Skill”的形态很适合智能体，但如果身份入口仍然强绑定在某一个大平台里，那么开放智能体之间的互联互通会被卡住。所以这个项目的核心，是在兼容小程序 MCP 契约的同时，把底层身份和网络能力替换成开放的 ANP DID。

## 2. 为什么需要新的容器？

先看现状。小程序 MCP 的价值很明确：商家把业务能力抽象成原子接口，把交互抽象成原子组件，再用 `SKILL.md` 和 `mcp.json` 告诉模型什么时候调用、怎么调用、返回什么结构。

但当前小程序 MCP 的登录身份通常基于微信登录。大商家可以分别为微信、支付宝、自己的 App 或会员系统做适配。但问题是，大厂平台通常不会为每一个中小公司的 Agent 单独做身份支持。这样一来，一个用户 Agent 想调用很多商家的服务，就会遇到身份不开放、对接成本高、服务边界封闭的问题。

所以我们要问的问题是：有没有一种方式，让商家不必为每一个 Agent 单独适配账号体系，也能确认“是谁在请求服务”？

## 3. 我们的答案：开放 DID 身份层

我们的答案不是推翻已有平台身份，而是在它们之上叠加一个开放的三方身份层，也就是 ANP DID。

有了 DID 之后，用户可以拿着自己的 DID 去请求服务。服务方不需要认识每一个 Agent 平台，也不需要提前为每个平台建账号通道；它只需要解析 DID Document，验证请求签名，再根据本地策略决定是否授权。换句话说，身份从“某个平台的登录态”扩展成了“可验证、跨平台、服务方可独立校验的开放身份”。

这也是我们开发这个容器的根本原因：让每个商家的智能体，都有机会服务所有符合开放身份协议的智能体。

## 4. 产品定位与边界

所以这个容器的定位很清楚：它是面向 Agent 的 MiniApp MCP Skill 运行容器。它兼容小程序 MCP 的接口契约，使用 ANP DID 做登录、鉴权和调用方身份识别，并用 Rust 独立实现运行时。

同时我们也刻意收窄边界。它不是完整微信小程序 Runtime，不支持复杂页面路由、TabBar、完整 WXML/WXSS，也不替代微信账号、微信支付或云开发。我们只做 Agent 对话场景里最必要的能力：原子接口、原子组件、结构化返回、卡片交互、用户确认和审计。

这个边界非常重要，因为只有保持足够小，才能把重点放在 Agent 调用服务的主链路上。

## 5. 总体架构：独立 Rust Runtime

从架构上看，`anp-miniapp-dock` 是一个独立 Rust Runtime。上层可以是 CLI、测试 Runner，未来也可以接入宿主 App。

Runtime 里有几块核心模块：Skill Loader 负责加载 `SKILL.md`、`mcp.json`、`apis` 和 `components`；MCP Contract Validator 做契约校验；Atomic API Runtime 用 QuickJS-NG 执行接口 JS；MiniApp MCP Component Runtime 用 Component VM 和 WXML/WXSS 子集生成 Render IR；`wx Compatibility Layer` 把小程序能力映射到宿主能力边界；最底层是 ANP SDK Adapter，负责 DID、签名、发现、鉴权、Signed HTTP 和 capability token。

右侧 demo server 模拟商家 Agent，提供 Skill 包、DID challenge 登录、商品、订单、支付和审计接口。这样我们可以端到端验证从 Skill 加载到支付卡片过期的完整流程。

## 6. 兼容小程序 MCP 契约

兼容策略有两层。第一层是接口层尽量兼容小程序 MCP：原有 Skill 的 `SKILL.md`、`mcp.json`、`index.js`、`apis/*.js`、`components/*`、`Component({})`、`wx.modelContext` 等写法尽量保持不变。

第二层是实现层替换。凡是涉及微信登录、微信支付、云开发、设备能力的地方，我们用 `wx Compatibility Layer` 映射到 ANP DID、capability token、mock payment、宿主能力或 fallback。

特别要注意几个字段：`structuredContent` 会进入 Agent/LLM 上下文，也会传给组件渲染；`_meta` 对模型不可见，主要传递组件私有数据；`_meta.ui.componentPath` 用来绑定原子组件；组件里的 `sendFollowUpMessage` 和 `api/call` 都必须回到统一 Orchestrator，不能绕过权限、schema、consent 和审计。

## 7. 组件运行时：从 WXML/WXSS 到 Render IR

组件这块我们没有选择直接把所有东西降级成 CardSpec，而是设计了 MiniApp MCP Component Runtime。原因是，如果只支持通用卡片，迁移小程序 MCP Skill 的价值会低很多；但如果完整复刻微信小程序 Runtime，成本又太高。

所以 P0 的折中方案是：支持 `Component({})` 子集、基础生命周期、`data`、`properties`、`methods`、`setData`、`bindtap`，支持 `view`、`text`、`image`、`button`、横向 `scroll-view`、`wx:if`、`wx:for` 和简单绑定。运行后输出平台无关的 Render IR JSON。未来宿主可以用 Flutter Renderer 或 Web Renderer 来消费它。

如果组件加载失败、JS 执行失败、WXML/WXSS 不支持，Runtime 会按顺序 fallback 到 native adapter、CardSpec、最后纯文本。这保证单个组件不兼容时，整个 Skill 不会失败。

## 8. 登录流程：咖啡点单中的 DID 认证

接下来用咖啡点单看登录流程。用户说“帮我点一杯少糖拿铁”，Runtime 先发现咖啡商家 Agent，读取它的 Skill 和 `mcp.json`。

第一次访问商家服务时，Runtime 会向服务端请求 challenge，或者直接对登录请求做签名。用户 DID 通过 ANP Rust SDK 生成签名，请求里会带上 `Signature-Input`、`Signature`，有 body 时还会带 `Content-Digest`。服务端拿到请求后，解析 DID Document，检查 `keyid` 指向的验证方法是否存在并被 `authentication` 授权，然后重建签名基字符串验签，验证时间窗口和 nonce，最后创建或查找 DID 绑定账户。

验证通过后，商家返回一个短期 capability token。后续 `searchDrinks`、`confirmOrder`、`payOrder` 等原子接口调用就携带这个 token。token 会按商家 DID、用户 DID 和 Skill ID 隔离。

## 9. 咖啡点单闭环

在 demo 里，端到端流程是这样的：先调用 `searchDrinks`，返回饮品列表，绑定 `drink-list` 组件。用户点击某个饮品后，组件可以发送 follow-up message，也可以触发 `api/call` 调 `confirmOrder`。

`confirmOrder` 返回订单确认卡片，用户点击确认支付时，这一步是高风险动作，所以必须进入 consent，也就是用户确认和审计边界。确认后 Runtime 才调用 `payOrder`，返回 `payment-result` 组件，并让旧的订单确认卡片过期。

这个 demo 验证的不是咖啡业务本身，而是整个容器最关键的链路：加载 Skill、执行原子接口、渲染组件、组件事件回到 Orchestrator、用户授权、支付模拟、卡片过期和审计。

## 10. 安全边界与风控

安全上我们有几条原则。第一，JS sandbox 默认不访问宿主全局对象，不允许任意文件系统、任意网络或远程代码加载。`require` 只能加载 Skill 包内部文件。

第二，网络、存储、会话、设备、支付都走 host capability boundary。比如 `wx.request` 要经过 allowlist 和 DID-aware signed HTTP adapter；storage 按 DID、merchant 和 Skill 隔离；capability token 也按用户、商家和 Skill 隔离。

第三，高风险动作必须 consent first。下单、支付、地址、手机号、身份绑定等动作不能由 Skill 直接执行，必须有用户确认和审计记录。DID 证明的是“谁在请求”，高风险授权仍然要由上层策略和人类确认来完成。

## 11. 当前原型与后续路线

当前项目还是原型/MVP。P0 的重点是跑通主链路：Skill Loader、原子接口、QuickJS API VM、Component VM、Render IR JSON、CardSpec fallback、DID 登录、capability token、coffee demo 的三张卡片。

后续 P1 会补更多接口和组件能力，比如图片/文件、扫码、电话、地址、手机号、真实 Payment Intent、WebSocket，以及 `openDetailPage` fallback、动态组件 request/timer、更多 input 类组件。P2 会继续做更完整的 WXML/WXSS 兼容、交易型 Skill 测试集、性能和宿主渲染器。

但我们会坚持一个原则：不做完整微信小程序运行时，而是做 Agentic 场景里够用、可审计、可跨平台的 MCP 容器。

## 12. 邀请共建

最后总结一下：我们做这个开源容器，是为了在各大厂和商家已有身份体系之上，再创建一个开放的 DID 身份入口，让每个商家的智能体能够服务所有智能体。

欢迎大家一起贡献：更多 MiniApp MCP 兼容测试、更多交易型组件、更多 host capability adapter、Flutter 或 Web Renderer、安全审计、以及真实商家 Skill 示例。这个项目现在还很早，但它指向的是一个更开放的 Agent 服务网络。
