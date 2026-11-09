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
  "schemaVersion": "dock.validate-report.v1",
  "status": "ok|warning|error",
  "commandStatus": "ok",
  "reportStatus": "ok|warning|error",
  "skillId": "...",
  "skillRef": {
    "kind": "local-directory",
    "path": "examples/coffee-skill|[REDACTED]",
    "redacted": false
  },
  "compatibilityLevel": "supported|compatible-with-warnings|demo-only|invalid",
  "apis": [],
  "apiNames": [],
  "components": [],
  "componentPaths": [],
  "permissions": [],
  "risks": [],
  "fallbacks": [],
  "releaseBlockers": [],
  "repairSuggestions": [],
  "releaseReadiness": {}
}
```

要求：

- 全 JSON；
- 可被 CI 消费；
- 顶层字段和 `compatibilityReport` 嵌套对象都包含 APIs、components、permissions、risks、fallbacks、releaseBlockers 和 repair suggestions；
- `status` / `reportStatus` 表示报告状态，`commandStatus` 表示 CLI 命令执行成功；
- warning/blocker 有修复建议；
- demo-only/mock/in-memory/local 未加密/unsigned package 能力标识清楚；
- 绝对路径、private key path、token、Authorization、signature 和隐私原文必须脱敏。

### 3.2 `inspect`

输出 `dock.inspect-report.v1` JSON：

```json
{
  "schemaVersion": "dock.inspect-report.v1",
  "status": "ok|warning",
  "commandStatus": "ok",
  "skillId": "...",
  "skillRef": {},
  "package": {},
  "files": [],
  "apis": [],
  "registeredApis": [],
  "registeredApisSource": "api-vm-registration-trace|static-register-api-scan|unknown-with-reason",
  "components": [],
  "permissions": {},
  "risks": [],
  "wxApiUsage": {
    "status": "scanned|unknown-with-reason",
    "items": []
  },
  "warnings": []
}
```

要求：

- Skill package 文件只展示相对路径、类型和大小，不输出源码内容；
- API 注册与 manifest 对照必须标出 `declared-and-registered`、`declared-only` 或 `registered-static-with-vm-error`；
- componentPath、权限需求、风险等级复用 validate 状态枚举；
- `wx.*` 使用痕迹来自静态字符串扫描，必须说明 dynamic property access 需要 `test-skill` 继续验证；
- 绝对路径、包外路径、token、Authorization、signature、private key path 和隐私原文必须脱敏或 fail closed。

### 3.3 `test-skill`

输出 `dock.test-skill-report.v1` JSON：

```json
{
  "schemaVersion": "dock.test-skill-report.v1",
  "status": "ok|failed",
  "commandStatus": "ok",
  "skillId": "...",
  "skillRef": {},
  "fixtureSet": "coffee|address-form|media-review|dynamic-status|location-map-preview|...",
  "mockProvider": {
    "status": "dev-only",
    "productionReady": false
  },
  "summary": {
    "total": 0,
    "passed": 0,
    "failed": 0
  },
  "cases": []
}
```

执行：

- call API；
- render component；
- dispatch action；
- compare Render IR snapshot；
- output audit summary。

要求：

- 默认复用 `RuntimeService` / Component Runtime，不绕过生产 Runtime API facade；
- coffee fixture 覆盖 search / confirm / pay 三个 API、组件 action 和 payment expire；
- `examples/fixtures/address-form`、`media-review`、`dynamic-status`、`location-map-preview` 对比 `testdata/render-ir/*.json` golden snapshot；
- dynamic fixture 使用受控 headless `RequestBroker`，report 必须显示 mock/dev-only 且 `productionReady = false`；
- failure diff 必须指出 API/result/render/action/audit/snapshot 中的失败层和稳定 JSON path；
- report 和 snapshot compare 不得包含 token、Authorization、signature、private key path、本机路径、真实手机号、真实地址或经纬度。

### 3.4 `import-wechat-mcp`

目的：复制/导入小程序 MCP Skill 到容器测试目录，不破坏原字段。

输出 `dock.import-wechat-mcp-report.v1` JSON：

```json
{
  "schemaVersion": "dock.import-wechat-mcp-report.v1",
  "status": "dry-run|copied|blocked",
  "commandStatus": "ok",
  "skillId": "...",
  "source": {},
  "destination": {},
  "mode": {
    "dryRun": true,
    "write": false,
    "overwrite": false
  },
  "structure": {},
  "appJson": {},
  "compatibilityReport": {},
  "migrationPatch": {},
  "copyPlan": [],
  "blockers": [],
  "nextCommands": []
}
```

动作：

- 检查 `SKILL.md`、`mcp.json`、`index.js`；
- 检查 `apis/*.js`、`components/*/index.*` 和 symlink；
- 识别 `app.json agent.skills[]`；
- 输出兼容报告和 safe-copy plan；
- 可生成 ANP `_meta` 建议 patch，但不自动强制改业务逻辑。

要求：

- 默认 dry-run，只有显式 `--write --dest <dir>` 才复制；
- safe copy 拒绝 symlink、source/dest 包含关系和未授权 overwrite；
- copy 保留原始 `mcp.json` 字段，不自动把不安全能力标成 production-ready；
- patch 只作为人工 review 建议，重点提示 ANP DID session、Host provider、ConsentGate、audit、dynamic component 和 supply-chain 改造；
- report 不得包含源码内容、token、Authorization、signature、private key path、本机路径或真实隐私原文。

### 3.5 `doctor`

输出 `dock.doctor-report.v1` JSON：

```json
{
  "schemaVersion": "dock.doctor-report.v1",
  "status": "ok|warning|error",
  "commandStatus": "ok|failed",
  "reportStatus": "ok|warning|error",
  "ci": false,
  "runtimeConfig": {},
  "summary": {
    "total": 0,
    "pass": 0,
    "warn": 0,
    "fail": 0,
    "skip": 0,
    "skipCountsAsPass": false
  },
  "humanSummary": [],
  "checks": [],
  "redaction": {}
}
```

检查：

- Rust toolchain；
- workspace layout；
- runtime config contract；
- Skill package；
- DID document / signing credential；
- signing credential permissions；
- trusted DID resolver；
- allowlist；
- storage/audit path；
- Host providers；
- sandbox gates；
- remote server health。

要求：

- 默认 `dock-cli doctor` 不访问外部 server；未提供 `--server` 时 remote server health 为 `skip`，且 `skipCountsAsPass = false`。
- 可传 `--runtime-config <path>`、DID identity flags、`--skill <path>` 和 `--server <url>` 做目标环境诊断。
- `--ci` 只在存在 `fail` 时让 `commandStatus = "failed"` 并返回非零；warning/skip 仍需要人工或 CI policy 判断。
- doctor 不执行高风险业务 API，不读取或输出 signing credential material，不输出 raw token、Authorization、signature、secret、本机绝对路径或隐私原文。
- 默认 development config、unsigned/demo Skill、in-memory storage/audit、缺 Host provider 和缺 allowlist 只能报告为 warning/skip，不能写成 production-ready。

## 4. 示例 Skill 体系

保留 coffee，并复用 Step 02-06 已建立的 `examples/fixtures/*` 作为 coffee 之外的开发者示例：

| 示例 | 目的 |
|---|---|
| `examples/fixtures/address-form` | 表单、地址 Host boundary、L4 consent/audit |
| `examples/fixtures/media-review` | image/file format、opaque media/file handle、preview fallback |
| `examples/fixtures/dynamic-status` | dynamic component、request/timer、expire cleanup |
| `examples/fixtures/location-map-preview` | location provider fail-closed、static map preview |

每个示例必须有：README、run command、expected JSON、Render IR snapshot、风险说明。

## 5. 文档计划

新增或更新：

- `docs/developer/README.md`
- `docs/developer/import-wechat-mcp-skill.md`
- `docs/developer/wx-api-compatibility.md`
- `docs/developer/component-compatibility.md`
- `docs/developer/security-guidelines.md`
- `docs/developer/host-adapter-guide.md`
- `docs/runbook/local-demo.md` 增加多 fixture 调试方式

## 6. 阶段完成检查

- [x] 开发者能用 CLI 完成 validate/inspect/test。
- [x] coffee 之外至少 3 个示例可跑。
- [x] 兼容报告能定位 unsupported API 和 fallback 风险。
- [x] 迁移指南说明 ANP DID 替代微信身份的方式。
- [x] 开发者文档覆盖 import、API compatibility、component compatibility、security guidelines 和 Host adapter guide。
- [x] 文档和 CLI 使用同一状态枚举。
