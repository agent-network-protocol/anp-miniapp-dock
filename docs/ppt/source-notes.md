# 分享内容来源映射

本 PPT 主要依据 `docs/architecture/` 中的架构文档，并参考 `../AgentNetworkProtocol` 中 DID-WBA 身份认证规范的关键流程。

## 本仓库架构文档

- `architecture/agentic-miniapp-container-prd.md`
  - 背景与目标：容器不是复刻微信小程序运行时，而是在 Agent 对话场景承载小程序 MCP 形态的 Skill。
  - 产品定位：兼容小程序 MCP 契约，使用 ANP DID 做登录、鉴权和调用方身份识别。
  - MVP 边界：支持 `SKILL.md`、`mcp.json`、原子接口、`structuredContent`、`_meta.ui.componentPath`、组件运行时子集、`sendFollowUpMessage`、`api/call`、consent/audit。
  - 身份流程：发现商家 Agent、读取 Skill、DID 签名、服务端验证 DID Document 和签名、返回 capability token。

- `architecture/anp-skill-dock-architecture.md`
  - 总体架构：独立 Rust Runtime，包含 Skill Loader、MCP Contract Validator、Atomic API Runtime、MiniApp MCP Component Runtime、wx Compatibility Layer、Consent & Audit、Scoped Storage、ANP SDK Adapter。
  - 兼容策略：接口层保持小程序 MCP 契约，实现层替换为 ANP/Rust Runtime。
  - 技术选择：Rust、QuickJS-NG、Render IR、CardSpec fallback、ANP Rust SDK。
  - demo 闭环：`searchDrinks → drink-list → confirmOrder → order-confirm → consent → payOrder → payment-result`。

- `architecture/miniapp-mcp-compatibility-mvp.md`
  - 兼容边界：原子接口契约级兼容；原子组件做到运行时子集兼容，不做完整页面运行时。
  - 数据流：`content`、`structuredContent` 和 `_meta` 的可见性与组件传递语义。
  - 安全上下文：每次调用独立 QuickJS context，`wx.request` 走 allowlist，storage 按 DID/merchant/Skill 隔离，高风险 API 先走 human authorization。
  - P0/P1/P2 优先级与不支持范围。

- `architecture/miniapp-mcp-component-runtime.md`
  - 组件运行时：`Component({})` 子集、WXML/WXSS 子集、Render IR JSON、Flutter Renderer Adapter 作为后续宿主后端。
  - 渲染优先级：Component Runtime → native adapter → CardSpec → text fallback。
  - 生命周期与事件：Result 输入、tap 事件、`setData`、`sendFollowUpMessage`、`api/call`、expire。

## ANP DID-WBA 认证规范参考

参考文件：`../../AgentNetworkProtocol/chinese/03-did-wba方法规范.md`。

PPT 第 8 页口播中的登录/验签流程对应 DID-WBA 规范中跨平台身份认证流程：

- 客户端使用 `Signature-Input` 和 `Signature` 对 HTTP 请求签名。
- 请求带 body 时用 `Content-Digest` 绑定消息体完整性。
- 服务端解析 DID Document，检查 `keyid` 指向的验证方法是否存在，并被 `authentication` 授权。
- 服务端重建 signature base 验签，检查时间窗口和 nonce/replay 风险。
- 验证通过后，服务端可以返回短期 access token / capability token，后续请求携带 token。
