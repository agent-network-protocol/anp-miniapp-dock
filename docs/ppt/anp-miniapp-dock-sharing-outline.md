# anp-miniapp-dock 15 分钟分享 PPT 大纲

主题：**开源的智能体原生小程序 MCP 容器：用 ANP DID 打开跨平台服务入口**

分享目标：用 15 分钟说明为什么要做 `anp-miniapp-dock`、它如何兼容小程序 MCP 契约并替换底层身份为 ANP DID、当前原型架构与咖啡点单登录流程，以及后续需要社区一起补齐的方向。

## 时间分配

| 页码 | 标题 | 核心信息 | 建议时间 | 图片/视觉 |
|---:|---|---|---:|---|
| 1 | 开源的智能体原生小程序 MCP 容器 | 主题开场：不是复刻微信小程序，而是让 Agent 能以开放身份调用商家 Skill | 0:45 | `assets/open-did-bridge.png` |
| 2 | 为什么需要新的容器？ | 小程序 MCP 形态有价值，但登录身份通常绑定大厂平台；中小 Agent 很难被每个商家适配 | 1:20 | 平台身份孤岛示意图（PPT 形状） |
| 3 | 我们的答案：开放 DID 身份层 | 在既有平台身份之上叠加 ANP DID；用户 Agent 拿 DID 请求服务，服务方只校验 DID | 1:20 | 平台层 + ANP DID 层 + 商家 Agent 网络（PPT 形状） |
| 4 | 产品定位与边界 | Agentic MiniApp Container：兼容 MCP 契约、ANP DID-first、只做 Agent 交互必要子集 | 1:10 | 定位卡片 + 非目标清单（PPT 形状） |
| 5 | 总体架构：独立 Rust Runtime | Rust Runtime 负责加载、执行、渲染、授权、审计；底层身份网络交给 ANP Rust SDK | 1:40 | `assets/runtime-container.png` + 分层架构图 |
| 6 | 兼容小程序 MCP 契约 | 保持 `SKILL.md`、`mcp.json`、`apis`、`components`、`structuredContent`、`_meta`、`api/call` 等契约 | 1:20 | Skill 包目录 + 数据结果结构（PPT 形状） |
| 7 | 组件运行时：从 WXML/WXSS 到 Render IR | Component VM 执行子集，输出平台无关 Render IR；失败时 fallback 到 CardSpec/content | 1:30 | 渲染管线 + fallback 梯子（PPT 形状） |
| 8 | 登录流程：咖啡点单中的 DID 认证 | 发现商家、读取 Skill、challenge、DID 签名、服务端验签、返回 capability token | 1:50 | `assets/coffee-did-flow.png` + 时序步骤 |
| 9 | 咖啡点单闭环 | `searchDrinks → drink-list → confirmOrder → consent → payOrder → payment-result` | 1:20 | 交易卡片流（PPT 形状） |
| 10 | 安全边界与风控 | JS sandbox、host capability boundary、allowlist、scoped storage、consent/audit | 1:20 | 安全控制面板（PPT 形状） |
| 11 | 当前原型与后续路线 | P0 已验证主链路；P1/P2 增强更多 API、组件、兼容性、性能与宿主渲染 | 1:20 | P0/P1/P2 Roadmap（PPT 形状） |
| 12 | 邀请共建 | 开源目标：让每个商家智能体可以服务所有智能体；邀请贡献接口、组件、测试与安全审计 | 0:45 | 开放网络收束图/hero 复用 |

## 叙事主线

1. **先讲原因**：小程序 MCP 已经证明“原子接口 + 原子组件 + SKILL + `mcp.json`”适合 Agent 调用，但它的身份入口天然更偏平台内闭环。
2. **再讲身份方案**：不否定微信、阿里或商家自有登录，而是在这些体系之上加入开放的 ANP DID 身份层，让用户 Agent 可以带着 DID 请求服务。
3. **再讲容器设计**：容器必须兼容小程序 MCP 契约，同时把底层身份、鉴权、网络、存储和支付边界替换成 ANP/Rust Runtime。
4. **再讲技术实现**：Rust workspace、QuickJS-NG API VM、Component VM、Render IR、wx Compatibility Layer、ANP Rust SDK Adapter。
5. **用咖啡点单讲登录和端到端闭环**：把 DID 签名登录、capability token、`api/call`、consent/audit 放到一个能理解的交易场景里。
6. **最后明确原型状态和号召**：当前是 MVP/原型，还需要更多接口、组件、兼容测试、性能和安全优化，欢迎共建。
