# 开发者文档

本目录面向 MiniApp MCP Skill 开发者和 Host adapter 集成方。它只描述 `anp-miniapp-dock` 当前已经冻结的开发者工作流和容器边界，不承诺完整复刻微信小程序 Runtime。

## 推荐工作流

1. 导入或准备 Skill 包。

   ```bash
   cargo run -p dock-cli -- import-wechat-mcp examples/coffee-skill --dry-run
   ```

2. 静态检查 package、manifest、API、组件、权限和 release blockers。

   ```bash
   cargo run -p dock-cli -- inspect examples/coffee-skill
   cargo run -p dock-cli -- validate examples/coffee-skill
   ```

3. 用 headless fixture runner 跑 API、组件 action、Render IR snapshot 和 audit summary。

   ```bash
   cargo run -p dock-cli -- test-skill examples/coffee-skill
   cargo run -p dock-cli -- test-skill examples/fixtures/address-form
   cargo run -p dock-cli -- test-skill examples/fixtures/media-review
   cargo run -p dock-cli -- test-skill examples/fixtures/dynamic-status
   cargo run -p dock-cli -- test-skill examples/fixtures/location-map-preview
   ```

4. 检查本地环境、Runtime config、DID identity、resolver、allowlist、storage/audit backend、Host provider 和 sandbox gate surface。

   ```bash
   cargo run -p dock-cli -- doctor
   ```

5. 按兼容报告修复 Skill，再回到第 2 步。准备接入真实 Host 时，先阅读 Host adapter guide 和 release gates。

## 文档入口

| 文档 | 用途 |
|---|---|
| [导入 WeChat MiniApp MCP Skill](import-wechat-mcp-skill.md) | 说明 `import-wechat-mcp`、迁移流程、ANP DID 替代微信身份、safe copy 和后续验证命令。 |
| [wx API 兼容说明](wx-api-compatibility.md) | 解释 API 状态枚举、`wx.modelContext`、`wx.request`、storage、高风险 Host boundary 和 unsupported API 处理。 |
| [组件兼容说明](component-compatibility.md) | 解释 Render IR、WXML/WXSS 子集、内置组件、dynamic component、fixtures 和 Host renderer 边界。 |
| [安全开发指南](security-guidelines.md) | 说明 token、Authorization、private key、phone、address、file、location、ConsentGate、audit 和 redaction 红线。 |
| [Host adapter guide](host-adapter-guide.md) | 说明 `dock.runtime.v1`、`dock.host-adapter.v1`、action routing、provider boundary、conformance 和 release blockers。 |

权威矩阵仍以 [`../architecture/wx-api-compatibility-matrix.md`](../architecture/wx-api-compatibility-matrix.md) 和 [`../architecture/component-compatibility-matrix.md`](../architecture/component-compatibility-matrix.md) 为准。本目录只解释开发者如何使用这些状态。

## 状态枚举

开发者文档、CLI report 和兼容矩阵统一使用以下状态枚举：

| 状态 | 含义 |
|---|---|
| `supported` | 当前 runtime 已支持核心语义，并有源码、测试或 fixture 证据。 |
| `host-boundary` | Runtime 已有 trait、action、Render IR、provider 或 fallback 边界，但真实生产 Host provider/renderer 仍需接入。 |
| `planned-p1` | 已进入近期计划或当前阶段计划，但当前不能当作可用能力。 |
| `planned-p2` | 后续阶段能力，不阻塞当前交易型 Skill 主线。 |
| `demo-only` | 仅本地 demo、headless、mock、localhost 或 dev fixture 可用，不能作为生产能力。 |
| `unsupported-by-design` | 与 Agentic MiniApp Container 边界冲突，默认 fail closed 或 fallback。 |

CLI report 顶层 `status` / `reportStatus` 表示报告或 release-readiness 状态，`commandStatus` 表示命令是否成功输出 JSON。`warning` 不等于 production-ready。

## 示例 Skill

| 示例 | 覆盖能力 |
|---|---|
| `examples/coffee-skill` | coffee 交易主线、Atomic API、组件 action、mock payment、card expiration。 |
| `examples/fixtures/address-form` | 表单、地址 Host boundary、L4 consent/audit。 |
| `examples/fixtures/media-review` | image/file format、opaque media/file handle、preview fallback。 |
| `examples/fixtures/dynamic-status` | dynamic component、受控 request/timer、expire cleanup。 |
| `examples/fixtures/location-map-preview` | location provider fail-closed、static map preview。 |

这些示例都是 mock/dev evidence。它们能帮助迁移和回归测试，但不能证明真实 Host provider、生产 renderer、加密持久化 backend、远端 registry、CI release gate 或隐私删除流程已经完成。
