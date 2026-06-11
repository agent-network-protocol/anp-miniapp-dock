# Phase 5：开发者体验与生态兼容实施计划

## 1. 阶段目标

Phase 5 让外部 Skill 开发者可以自助导入、验证、调试和认证兼容性。完成后，开发者不需要阅读 Rust 源码，也能知道自己的小程序 MCP Skill 在容器中哪些能力可用、哪些会降级、哪些必须改造。

## 2. 输出物

- `dock-cli validate` 增强版兼容报告；
- `dock-cli inspect`；
- `dock-cli test-skill`；
- `dock-cli import-wechat-mcp`；
- `dock-cli doctor`；
- 示例 Skill fixture；
- 迁移指南和 API/组件矩阵文档；
- Host adapter 开发指南。

## 3. CLI 开发顺序

### 3.1 `validate`

输出：

```json
{
  "status": "ok|warning|error",
  "skillId": "...",
  "compatibilityLevel": "p0|p1|partial|unsupported",
  "apis": [],
  "components": [],
  "permissions": [],
  "risks": [],
  "fallbacks": [],
  "releaseBlockers": []
}
```

要求：

- 全 JSON；
- 可被 CI 消费；
- warning 有修复建议；
- demo-only 能力标识清楚。

### 3.2 `inspect`

展示：

- Skill package 文件；
- API 注册与 manifest 对照；
- componentPath；
- 权限需求；
- 风险等级；
- 使用到的 `wx.*` API（可静态扫描 + runtime trace）。

### 3.3 `test-skill`

输入：fixture cases。

执行：

- call API；
- render component；
- dispatch action；
- compare snapshot；
- output audit summary。

### 3.4 `import-wechat-mcp`

目的：复制/导入小程序 MCP Skill 到容器测试目录，不破坏原字段。

动作：

- 检查 `SKILL.md`、`mcp.json`、`index.js`；
- 识别 `app.json agent.skills[]`；
- 输出兼容报告；
- 可生成 ANP `_meta` 建议 patch，但不自动强制改业务逻辑。

### 3.5 `doctor`

检查：

- Rust toolchain；
- DID document/private key；
- private key permissions；
- trusted DID resolver；
- allowlist；
- storage/audit path；
- Host providers；
- sandbox gates；
- remote server health。

## 4. 示例 Skill 体系

保留 coffee，并新增：

| 示例 | 目的 |
|---|---|
| `examples/address-skill` | 表单、地址、手机号、L4 consent |
| `examples/media-skill` | image/file format、media handle、preview fallback |
| `examples/dynamic-status-skill` | dynamic component、request/timer、expire cleanup |
| `examples/location-skill` | location provider、map preview |

每个示例必须有：README、run command、expected JSON、Render IR snapshot、风险说明。

## 5. 文档计划

新增或更新：

- `docs/developer/import-wechat-mcp-skill.md`
- `docs/developer/wx-api-compatibility.md`
- `docs/developer/component-compatibility.md`
- `docs/developer/security-guidelines.md`
- `docs/developer/host-adapter-guide.md`
- `docs/runbook/local-demo.md` 增加多 fixture 调试方式

## 6. 阶段完成检查

- [ ] 开发者能用 CLI 完成 validate/inspect/test。
- [ ] coffee 之外至少 3 个示例可跑。
- [ ] 兼容报告能定位 unsupported API 和 fallback 风险。
- [ ] 迁移指南说明 ANP DID 替代微信身份的方式。
- [ ] 文档和 CLI 使用同一状态枚举。
