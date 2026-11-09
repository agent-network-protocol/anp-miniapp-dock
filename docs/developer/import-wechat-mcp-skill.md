# 导入 WeChat MiniApp MCP Skill

`dock-cli import-wechat-mcp` 是迁移辅助命令，用于把现有 MiniApp MCP Skill 复制到受控测试目录，并生成兼容报告与 ANP `_meta` 建议 patch。它不会修改源目录，也不会把导入结果标记为 production-ready。

## 基本流程

先执行 dry-run：

```bash
cargo run -p dock-cli -- import-wechat-mcp examples/coffee-skill --dry-run
```

报告 schema 为 `dock.import-wechat-mcp-report.v1`，主要字段包括：

- `structure`：检查 `SKILL.md`、`mcp.json`、`index.js`、`apis/*.js`、`components/*/index.*` 和 symlink。
- `appJson`：识别可选的 `app.json agent.skills[]`，并对敏感字段脱敏。
- `compatibilityReport`：复用 validate 的 API、组件、权限、风险、fallback、supply-chain 与 release blocker 口径。
- `migrationPatch`：给出 ANP `_meta`、Host provider、ConsentGate 和 dynamic component 的建议 patch；该 patch 仅供人工 review，不会自动写入。
- `copyPlan`：列出 safe copy 计划，目标路径冲突时默认 blocked。
- `nextCommands`：给出后续 `validate`、`test-skill` 或 safe-copy 命令。

确认 dry-run 无 blocker 后，再显式写入受控测试目录：

```bash
cargo run -p dock-cli -- import-wechat-mcp examples/coffee-skill --dest target/dock-import-coffee --write
cargo run -p dock-cli -- validate target/dock-import-coffee
cargo run -p dock-cli -- test-skill target/dock-import-coffee
```

目标目录已存在同名文件时，命令默认 fail closed。只有确认要替换时才使用 `--overwrite`。

## 安全边界

- 默认 dry-run；`--write` 才会复制文件。
- 只复制真实文件和目录，symlink 会被拒绝。
- 目标目录不能位于源目录内部，也不能包含源目录。
- 命令保留原始 `mcp.json` 字段，不自动改写业务逻辑。
- 输出不包含源码内容，只包含相对路径、状态、建议和脱敏后的 metadata。
- 本地绝对路径、`Authorization`、signature、capability token、private key path、secret、真实手机号、真实地址和经纬度不得进入报告。

## 迁移关注点

WeChat 身份、登录态、网络和高风险能力需要映射到容器边界：

- 身份与会话：使用 ANP DID runtime session，不把 WeChat 登录凭据或 token 写入 Skill 包。
- 网络：`wx.request` 必须经过 RequestBroker、allowlist、capability token 和 redaction。
- 高风险 API：phone、address、location、media、payment、scan 和 phone call 必须经过 Host provider、ConsentGate 和 audit。
- 动态组件：只有声明 `scope.dynamic` 且通过 sandbox/resource gate 后，才能使用受控 request/timer。
- 供应链：本地未签名包仍是 dev/demo-only；生产发布需要 publisher DID、digest、signature 和 trusted publisher policy。

导入成功只说明包可以进入本地兼容验证链路。上线前仍必须通过 `validate`、`inspect`、`test-skill`、`doctor`、release gates、Host provider conformance 和安全 review。

## 后续阅读

- [开发者文档入口](README.md)
- [wx API 兼容说明](wx-api-compatibility.md)
- [组件兼容说明](component-compatibility.md)
- [安全开发指南](security-guidelines.md)
- [Host adapter guide](host-adapter-guide.md)
