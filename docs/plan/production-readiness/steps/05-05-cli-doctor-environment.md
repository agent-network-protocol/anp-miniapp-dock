# Step 05-05：CLI doctor 环境诊断

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：05-05
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-14 02:43:16 +0800 |
| Completed | 2026-06-14 02:58:25 +0800 |
| Commit | `9d19744` |
| Review evidence | 2026-06-14 02:57:06 +0800 commit 前 Review 已记录：修复 `doctor` 在仓库子目录运行时 toolchain/sandbox gate 使用相对路径可能误报的问题；确认 `dock.doctor-report.v1` 覆盖 toolchain/workspace/runtime config/Skill/DID/signing credential permission/resolver/allowlist/storage/audit/Host provider/sandbox/server health，默认 warning/skip 不被误标 production-ready，`--ci` 只在 fail 时返回非零且先输出 JSON。 |
| Verification evidence | 启动前 `git status --short --branch` = `## main...origin/main [ahead 94]`；已读取主 Plan、Step 05-05 文档、Phase 5 文档、Release Gates、local demo runbook、README、执行台账、Codex Goal 执行协议、Review/提交门禁、Blocked 处理和 Plan 变更记录；`cargo fmt --check` 通过；`cargo test -p dock-cli doctor` 4 passed；`cargo run -p dock-cli -- doctor` 输出 `dock.doctor-report.v1`、`status = warning`、`commandStatus = ok`、summary 为 5 pass / 7 warn / 1 skip / 0 fail；`python3 -m json.tool /tmp/dock-doctor.json` 通过；`cargo test -p dock-cli --test coffee_order_flow` 11 passed；`cargo clippy -p dock-cli --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/dock-cli crates/anp-adapter crates/dock-core docs/runbook docs/plan README.md` 无输出；doctor JSON 敏感串扫描未命中 `/home/`、Authorization、Signature、capabilityToken、Bearer、raw token、private key material、PEM header 或 secret；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 95]`。 |
| Next action | 进入 05-06 示例 Skill 与兼容测试集 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：新增或增强 `dock-cli doctor`，检查 Rust toolchain、DID identity、private key permissions、trusted resolver、allowlist、storage/audit path、Host providers、sandbox gates 和 remote server health。
- 用户 / 系统可见行为：开发者可以快速定位本地环境、凭据、Host provider 或远端服务配置问题。
- 非目标：不读取或输出私钥内容；不自动修复生产 secret。
- 完成标准：doctor 输出 JSON + human summary，区分 pass/warn/fail/skip，并提供修复建议。

## 3. 设计方法

- 设计边界：doctor 是诊断工具，不应执行高风险业务 API 或泄露 secret。
- 核心决策：所有检查都要有 severity、evidence、suggestion、redaction；无法检查的项目标记 skip 而不是 pass。
- 契约 / API / 数据流：Local config/env -> checks -> redacted diagnostic report -> optional CI failure code。
- 兼容性：复用 release gates 和 runbook 的命令/术语。
- 风险控制：private key path 可 redacted，private key material 永不读取输出；Authorization/token 不输出。

## 4. 实现方法

1. 阅读 Release Gates、local demo runbook、Runtime config plan、token cache persistence、scoped storage persistence 和 persistent audit sink plan。
2. 定义 doctor check registry：toolchain、workspace、DID document、private key permissions、resolver、allowlist、storage/audit path、Host providers、sandbox tests、server health。
3. 实现 redacted diagnostic report 和 exit code 策略。
4. 增加 tests：missing toolchain/config、bad key permissions、missing allowlist、unavailable provider、skip reason、redaction。
5. 更新 README、runbook 和 Phase 5 文档。
6. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/dock-cli` | `doctor` command、check registry、tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/anp-adapter` | DID resolver/credential diagnostic helper | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/dock-core` | runtime config diagnostic | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/runbook/local-demo.md` | 同步 doctor 用法 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | doctor 作为预检 gate | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-5-developer-experience.md` | 同步 doctor contract | 必须 |
| `anp/anp-miniapp-dock/README.md` | 视 CLI 使用说明更新 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/05-05-cli-doctor-environment.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 03-04、Step 04-04、Step 04-05、Step 04-06、Step 04-07、Step 05-01。
- 外部文档或决策：Release Gates、local demo runbook、Runtime config。
- 环境前提：Rust toolchain 1.88.0；外部 server health checks 可 skip 并记录原因。

## 7. 验收标准

- [ ] `dock-cli doctor` 覆盖 toolchain、DID identity、private key permission、resolver、allowlist、storage/audit path、Host providers、sandbox gates、server health。
- [ ] 输出 pass/warn/fail/skip、evidence 和 suggestion。
- [ ] skip 不被计为 pass；fail 可用于 CI exit code。
- [ ] private key material、token、Authorization、signature 和 secret 不出现在 report。
- [ ] README/runbook/Phase 5 文档与实现状态同步。
- [ ] Review 发现已经修复或明确记录。
- [ ] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Doctor tests | `cd anp/anp-miniapp-dock && cargo test -p dock-cli doctor` | doctor checks/report tests 通过 |
| Manual doctor | `cd anp/anp-miniapp-dock && cargo run -p dock-cli -- doctor` | 输出 redacted diagnostic；环境相关 skip/fail 需记录 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/dock-cli crates/anp-adapter crates/dock-core docs/runbook docs/plan README.md` | 无空白错误 |
| 脱敏抽样 | 手工检查 doctor report | 不含 private key material、raw token、Authorization、signature 或真实 secret |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：doctor 是否读取/输出 secret；skip/fail 语义是否清晰；建议是否可执行；是否与 release gates 术语一致。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已发现并修复 | 初版 toolchain 和 sandbox gate 文件存在性检查使用当前工作目录相对路径；从仓库子目录执行 doctor 时可能误报。 |
| 已修复问题 | 已修复 | toolchain 和 sandbox gate 文件检查改为优先基于 `default_project_root()` 定位项目根目录；`--ci` failure 路径仍先输出 JSON 再返回非零。 |
| 剩余风险 | 已记录 | doctor 只记录 sandbox gate surface，不执行重型 sandbox tests；默认 development config、unsigned/demo Skill、in-memory storage/audit、缺 Host provider、缺 allowlist 和无 `--server` 只能作为 warning/skip evidence，不是 production-ready evidence。 |
| 新增或缺失测试 | 已补充 | 新增 doctor 参数解析、完整 report/check 覆盖、production runtime config backend 通过、CI fail 输出 JSON 和脱敏断言；缺真实 production Host/server health E2E，因本 Step 只提供 CLI 诊断面。 |
| 已更新或缺失文档 | 已同步 | 更新 README、local demo runbook、release gates 和 Phase 5 文档；无额外缺失文档。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 doctor command、direct tests 和相关文档。
- Commit 前状态：`git status --short --branch` = `## main...origin/main [ahead 94]`，纳入文件只包含 05-05 doctor 代码、直接测试和相关文档。
- 纳入文件：`crates/dock-cli/src/commands.rs`、`README.md`、`docs/runbook/local-demo.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/phase-5-developer-experience.md`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/steps/05-05-cli-doctor-environment.md`。
- Commit 后证据：implementation commit `9d19744 phase5: add doctor diagnostics`；commit 后 `git status --short --branch` = `## main...origin/main [ahead 95]`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`phase5: add doctor diagnostics`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 05-05 小 Plan | 将 CLI doctor 环境诊断拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |
| 2026-06-12 | 更新 Phase 4 持久化依赖 | 按 Review 发现，原 04-04 已拆为 config/token/storage/audit focused Steps，doctor 需依赖对应诊断面 | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：doctor 为了“帮忙”读取 secret 会造成泄露。
- 回滚 / 回退：只检查存在性、权限和 redacted path；不读取 private key material。
- 后续文档：运维 runbook 应引用 doctor 作为故障定位第一步。
