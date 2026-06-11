# PPT 图片资产与生成说明

本目录下的图片用于 `anp-miniapp-dock-sharing.pptx`。其中需要 AI 生成的分享主视觉已通过 `$imagegen` 生成，并复制到 `docs/ppt/assets/`。

| 文件 | 使用页面 | 用途 | 生成方式 |
|---|---|---|---|
| `assets/open-did-bridge.png` | 1、12 | 开放 DID 身份层连接平台身份孤岛和商家 Agent 网络 | `$imagegen` |
| `assets/runtime-container.png` | 5 | Rust Runtime 容器将 MiniApp MCP Skill 转成 Agent 卡片交互 | `$imagegen` |
| `assets/coffee-did-flow.png` | 8 | 咖啡点单中的 DID 签名、验签和 token 返回 | `$imagegen` |
| PPT 内形状图 | 2、3、4、6、7、9、10、11 | 平台身份孤岛、分层架构、Skill 包、Render IR、交易闭环、安全边界、路线图 | 直接用 PPT 可编辑形状绘制，避免把简单流程图固化成位图 |

## `$imagegen` 生成提示词摘要

### `open-did-bridge.png`

现代企业技术插画：开放去中心化身份层连接多个平台身份孤岛、独立 AI Agent 和商家服务；深色背景，蓝绿紫光效，留出标题空间；无 logo、无文字。

### `runtime-container.png`

现代技术 3D/isometric 插画：轻量 Rust Runtime 容器将 MiniApp MCP Skill 文件输入，内部有 JS 引擎与组件渲染层，输出结构化卡片给对话 Agent；深色背景，蓝绿和琥珀高光；无 logo、无可读文字。

### `coffee-did-flow.png`

咖啡点单 AI Agent 流程视觉：用户 Agent、咖啡订单卡、数字签名钥匙、安全通道、商家 Agent 验证身份并返回 token pass；深色背景，咖啡琥珀与安全蓝绿光效；无 logo、无可读文字。
