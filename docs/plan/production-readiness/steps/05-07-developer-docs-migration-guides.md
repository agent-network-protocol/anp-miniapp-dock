# Step 05-07：开发者文档与迁移指南

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：05-07
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-14 03:20:36 +0800 |
| Completed | 2026-06-14 03:29:22 +0800 |
| Commit | `72f00df` |
| Review evidence | 2026-06-14 03:27:50 +0800 commit 前 Review 已记录：修复导入指南使用 `/tmp` 目标路径的可移植性问题，改为 `target/dock-import-coffee`；修复 local demo `runtime-json` 示例包含可复用 capability token 占位的问题，改为不携带 token 的可复制请求；修复 Phase 5 阶段完成检查仍未勾选状态枚举一致性的问题。确认 developer docs 没有把完整微信 Runtime、headless/mock provider、local unencrypted backend 或 demo-only fixture 写成 production-ready。 |
| Verification evidence | 启动前 `git status --short --branch` = `## main...origin/main [ahead 98]`；已读取主 Plan、Step 05-07 文档、Phase 5/6 阶段文档、05-08/06-01 至 06-07 Step 文档、现有 `docs/developer/import-wechat-mcp-skill.md`、README、local demo runbook、API/组件兼容矩阵、release gates 和 Host adapter / Runtime contract 相关文档；确认 05-06 implementation commit `f3d97cc` 与 closure commit `a8df50f`；`git diff --check -- docs/developer docs/runbook docs/plan README.md` 无输出；手工/脚本检查新增 developer docs、README、local demo、release gates 相对链接，未发现缺失目标；`cargo run -q -p dock-cli -- validate --help`、`inspect --help`、`test-skill --help`、`import-wechat-mcp --help`、`doctor --help` 与文档命令一致；`cargo run -q -p dock-cli -- runtime-json examples/coffee-skill ... runtime.callApi` 使用无 token 示例通过，返回 `dock.runtime.v1` / `status = ok`；`rg -n "supported|host-boundary|planned-p1|planned-p2|demo-only|unsupported-by-design" docs/developer docs/architecture` 命中状态枚举且未发现新增未知枚举；`rg -n "token|Authorization|private key|phone|address|file|location|ConsentGate|audit" docs/developer` 命中安全红线说明；敏感占位抽样未发现 `/tmp`、本机绝对路径、`capability-secret-token`、Bearer、PEM、经纬度或可复用 secret 写入新增 developer docs。 |
| Next action | 进入 05-08 Phase 5 final Review gate |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：补齐开发者文档，包括从微信 MCP Skill 迁移、API/组件兼容、权限安全、Host adapter guide 和 CLI 工作流。
- 用户 / 系统可见行为：开发者无需阅读 Rust 源码，也能导入、验证、调试、修复兼容问题并理解 ANP DID 替代微信身份的方式。
- 非目标：不建立完整文档站；不承诺所有微信 API 兼容。
- 完成标准：文档与 CLI report 和兼容矩阵使用同一状态枚举，示例命令可复制，安全红线明确。

## 3. 设计方法

- 设计边界：开发者文档必须讲清 Agentic MiniApp Container 边界，不把产品写成完整微信小程序 runtime。
- 核心决策：以任务流组织文档：import -> inspect -> validate -> test-skill -> doctor -> deploy/Host integration。
- 契约 / API / 数据流：开发者 Skill -> CLI tools -> compatibility report -> migration fixes -> fixtures/tests -> Host adapter。
- 兼容性：API/组件兼容文档直接引用矩阵，避免复制后漂移。
- 风险控制：安全指南明确不要存 token、不要在 content 暴露隐私、如何声明权限、如何处理 fallback。

## 4. 实现方法

1. 创建 `docs/developer/` 文档入口和目录。
2. 编写 `import-wechat-mcp-skill.md`，说明迁移流程、ANP DID 替代微信身份、unsupported API 处理。
3. 编写 `wx-api-compatibility.md` 和 `component-compatibility.md`，引用矩阵并解释状态枚举。
4. 编写 `security-guidelines.md`，覆盖 token、隐私、permissions、ConsentGate、audit、fallback。
5. 编写 `host-adapter-guide.md`，引用 Runtime API、IPC/SDK、Host adapter contract 和 conformance。
6. 更新 README、local demo runbook 和 Phase 5 文档。
7. 回填本 Step 和主 Plan 执行台账；Phase 5 完成后触发阶段 Review。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/docs/developer/import-wechat-mcp-skill.md` | 新增迁移指南 | 计划新增 |
| `anp/anp-miniapp-dock/docs/developer/wx-api-compatibility.md` | 新增 API 兼容说明 | 计划新增 |
| `anp/anp-miniapp-dock/docs/developer/component-compatibility.md` | 新增组件兼容说明 | 计划新增 |
| `anp/anp-miniapp-dock/docs/developer/security-guidelines.md` | 新增安全开发指南 | 计划新增 |
| `anp/anp-miniapp-dock/docs/developer/host-adapter-guide.md` | 新增 Host adapter 开发指南 | 计划新增 |
| `anp/anp-miniapp-dock/docs/runbook/local-demo.md` | 增加多 fixture / CLI 调试方式 | 视实现结果更新 |
| `anp/anp-miniapp-dock/README.md` | 新增 developer docs 入口 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-5-developer-experience.md` | 同步文档完成状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/05-07-developer-docs-migration-guides.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 05-01、Step 05-02、Step 05-03、Step 05-04、Step 05-05、Step 05-06。
- 外部文档或决策：API/组件矩阵、Runtime API、Host adapter contract、Release Gates。
- 环境前提：文档为主；命令示例必须与实际 CLI 对齐。

## 7. 验收标准

- [x] `docs/developer/` 至少包含 import、API compatibility、component compatibility、security guidelines、Host adapter guide。
- [x] 文档使用与 CLI report/矩阵一致的状态枚举。
- [x] CLI 示例命令可复制，或明确标注 planned/环境前提。
- [x] 安全指南明确 token、Authorization、private key、手机号、地址、文件、位置不得进入 model-visible output。
- [x] Host adapter guide 明确 action 必须回 Runtime，不得绕过 consent/audit。
- [x] README 和 runbook 有开发者入口。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- docs/developer docs/runbook docs/plan README.md` | 无空白错误 |
| 链接检查 | 手工检查新增文档相对链接 | 链接目标存在 |
| 命令抽样 | 手工核对文档中的 CLI 命令与 `AGENTS.md` / CLI help 一致 | 命令可复制或标注 planned |
| 术语抽样 | `cd anp/anp-miniapp-dock && rg -n "supported|host-boundary|planned-p1|planned-p2|demo-only|unsupported-by-design" docs/developer docs/architecture` | 状态枚举一致 |
| 安全抽样 | `cd anp/anp-miniapp-dock && rg -n "token|Authorization|private key|phone|address|file|location|ConsentGate|audit" docs/developer` | 命中安全指南和 redaction 规则，未出现真实 secret |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：文档同步完成后、commit 前。
- Review 重点：开发者是否能按文档完成工作流；是否误导完整微信兼容；状态枚举是否漂移；安全红线是否清楚。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已发现并修复 | 1. 既有导入指南使用 `/tmp/dock-import-coffee`，不符合可移植文档路径偏好；2. local demo `runtime-json` 示例携带 `capabilityToken` 占位值，容易被复制为 token 示例；3. Phase 5 完成检查仍未勾选“文档和 CLI 使用同一状态枚举”。 |
| 已修复问题 | 已修复 | 导入指南改为 `target/dock-import-coffee`；`runtime-json` 示例改为不携带 token 且实际运行通过；Phase 5 文档勾选状态枚举一致性；README、local demo 和 release gates 都新增 developer docs 入口。 |
| 剩余风险 | 已记录 | 本 Step 是 docs-only，不新增 Host provider、真实 production renderer、HTTP/gRPC sidecar、加密持久化 backend、CI automation 或 privacy deletion；这些仍由 Phase 6 和后续生产 Host 接入 gate 处理。 |
| 新增或缺失测试 | 文档验证已完成 | 本 Step 未改 Rust 行为，未新增单元测试；已执行文档空白、相对链接、CLI help、`runtime-json` 示例、状态枚举和安全红线抽样。 |
| 已更新或缺失文档 | 已同步 | 新增 `docs/developer/README.md`、API/组件兼容说明、安全指南、Host adapter guide；更新导入指南、README、local demo runbook、release gates、Phase 5 文档、Step 文档和主 Plan 台账；无额外缺失文档。 |

## 10. Commit 要求

- Commit 时机：验证、Review、文档同步完成后。
- Commit 范围：只包含 developer docs、README/runbook 入口和相关计划回填。
- Commit 前状态：`git status --short` 显示 README、`docs/developer/*`、local demo runbook、release gates、Phase 5 文档、Step 05-07 文档和主 Plan 台账变更。
- 纳入文件：`README.md`、`docs/developer/README.md`、`docs/developer/import-wechat-mcp-skill.md`、`docs/developer/wx-api-compatibility.md`、`docs/developer/component-compatibility.md`、`docs/developer/security-guidelines.md`、`docs/developer/host-adapter-guide.md`、`docs/runbook/local-demo.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/phase-5-developer-experience.md`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/steps/05-07-developer-docs-migration-guides.md`。
- Commit 后证据：implementation commit `72f00df docs: add developer migration guides`；commit 后 `git status --short --branch` = `## main...origin/main [ahead 99]`。
- 遗留未提交变更：无。
- 建议消息：`docs: add developer migration guides`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 05-07 小 Plan | 将开发者文档与迁移指南拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：文档若复制矩阵内容，后续容易漂移。
- 回滚 / 回退：文档优先链接矩阵和 CLI report schema，只摘要解释状态。
- 后续文档：Phase 6 release notes 和 runbook 应引用开发者文档作为外部沟通基础。
