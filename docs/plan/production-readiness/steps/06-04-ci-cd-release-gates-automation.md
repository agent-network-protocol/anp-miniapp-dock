# Step 06-04：CI/CD Release Gates 自动化

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：06-04
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-14 05:13:33 +0800 |
| Completed | 2026-06-14 05:24:25 +0800 |
| Commit | `afaa5ab` |
| Review evidence | 2026-06-14 05:22:29 +0800 commit 前 Review：修复脚本在 `--report` 目录不存在时解析不稳的问题、修复 full 模式 artifact 输出目录未 export 的问题、修复 quick/full 连续运行时会扫描陈旧 artifacts 的问题；确认 full 模式实际运行 required gates，`skip` 不计 pass，redaction / consent / sandbox / token leakage 均为 hard blocker，runbook 与脚本入口一致。 |
| Verification evidence | 启动前 `git status --short --branch` = `## main...origin/main [ahead 109]`，工作区无未提交变更；已读取主 Plan、Step 06-04 文档、Phase 6 计划、Release Gates runbook、README、API/组件兼容矩阵和 06-03 closure evidence；确认仓库当前无 `scripts/` 或 `.github/`，本 Step 采用 vendor-neutral 本地 gate runner；`bash -n scripts/release-gates.sh` 通过；`./scripts/release-gates.sh --quick --report target/release-gates/quick-report.json` 通过，report `dock.release-gates-report.v1` 为 `status = warning` / `releaseDecision = needs-review`，6 pass / 0 fail / 4 skip，quick skip 不计 pass；`python3 -m json.tool target/release-gates/quick-report.json` 通过；`./scripts/release-gates.sh --report target/release-gates/full-report.json` 通过，report `status = warning` / `releaseDecision = needs-review`，21 pass / 0 fail / 1 skip，唯一 skip 是未提供 release notes path；`python3 -m json.tool target/release-gates/full-report.json` 通过；full report 中 `requiredFailed = 0`、`hardBlockerFailed = 0`；Markdown link checker 检查 751 个本地链接；compatibility matrix checker 检查 130 个 status cells；artifact redaction scan 覆盖 `target/release-gates/artifacts`、`testdata/render-ir`、`testdata/perf`，未命中 `/home/`、`/Users/`、Authorization、Signature、capabilityToken、Bearer、fixture-token、private key、token-secret、latitude 或 longitude；`git diff --check -- scripts docs/runbook docs/plan README.md` 无输出；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 110]`，工作区无未提交变更。 |
| Next action | 进入 Step 06-05 Canary 发布、版本化与回滚策略 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：把 fmt、clippy、unit/integration、sandbox escape、compat fixture、redaction、snapshot、docs link、release notes completeness 等 gates 自动化。
- 用户 / 系统可见行为：任一 required gate 失败不得发布；breaking change 必须有 migration note。
- 非目标：不绑定特定 CI vendor；如未使用 GitHub Actions，也必须提供可本地运行的 gate script。
- 完成标准：release gates 可通过单命令或 CI job 执行，输出 pass/fail/skip 和证据。

## 3. 设计方法

- 设计边界：CI/CD gates 是发布门禁，不替代每个 Step 的 focused verification。
- 核心决策：基础 gates、security gates、compat fixture gates、docs gates、release notes gates 分层；skip 必须有原因和 residual risk。
- 契约 / API / 数据流：gate runner -> commands/checks -> machine-readable report -> release decision。
- 兼容性：沿用 `docs/runbook/release-gates.md` 的命令，逐步自动化 planned gates。
- 风险控制：redaction failure、consent bypass、sandbox escape、token leakage 永远 blocker。

## 4. 实现方法

1. 阅读 Release Gates、Phase 6 CI/CD 计划和已有 tests。
2. 设计 gate runner 脚本或 CI workflow：fmt、clippy、cargo test、coffee E2E、fixtures、sandbox、redaction、snapshot、docs link。
3. 增加 compatibility matrix coverage 和 markdown link check，或记录未自动化原因。
4. 增加 release notes completeness check：版本、compat changes、risk、rollback、migration note。
5. 输出 gate report，包含 pass/fail/skip、原因、命令、commit。
6. 更新 Release Gates runbook、Phase 6 文档和 README badge/CI 说明。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/scripts` | gate runner scripts | 计划新增 |
| `anp/anp-miniapp-dock/.github/workflows` | CI workflow，如项目采用 GitHub Actions | 视仓库策略新增 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | 自动化 gate 命令和报告格式 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-6-observability-release.md` | 同步 CI/CD strategy | 必须 |
| `anp/anp-miniapp-dock/README.md` | 视 CI 使用说明更新 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/06-04-ci-cd-release-gates-automation.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 02-06、Step 03-06、Step 05-03、Step 06-03。
- 外部文档或决策：Release Gates、fixture runner、performance baseline。
- 环境前提：Rust toolchain 1.88.0；CI vendor 可选。

## 7. 验收标准

- [x] Gate runner 或 CI workflow 覆盖基础 Rust gates、security gates、compat fixture、snapshot、redaction、docs link。
- [x] Gate report 记录 pass/fail/skip、命令、commit、原因和 residual risk。
- [x] redaction failure、consent bypass、sandbox escape、token leakage 是 hard blocker。
- [x] Release notes completeness 和 migration note 检查有可执行规则或明确人工 checklist。
- [x] Release Gates runbook 与自动化命令一致。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| Gate runner | `cd anp/anp-miniapp-dock && ./scripts/release-gates.sh` | gate report 通过或明确 skip；若脚本名不同，记录实际命令 |
| 基础命令 | `cd anp/anp-miniapp-dock && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` | 通过 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- scripts .github docs/runbook docs/plan README.md` | 无空白错误 |
| 链接检查 | 自动或手工检查新增 docs/workflow links | 链接目标存在 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：gate runner/workflow、文档同步完成后、commit 前。
- Review 重点：required gates 是否真正运行；skip 是否不能伪装 pass；hard blocker 是否覆盖安全红线；CI 与 runbook 是否一致。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已记录 | `--report` 初版在目录不存在时解析不稳；full 模式 artifact 输出目录未 export；quick/full 连续运行时可能扫描旧 artifacts；release notes completeness 在 06-05 前没有实际文件输入，只能记录为 skip。 |
| 已修复问题 | 已修复 | 相对 `--report` 路径固定到仓库根；export `ARTIFACT_DIR`；每次运行先清空当前 report 的 logs/artifacts，避免陈旧证据；release notes gate 在未提供路径时以 required skip 进入 `needs-review`。 |
| 剩余风险 | 已记录 | 本 Step 未绑定 GitHub Actions 或其他 CI vendor；release notes/canary 文件由 Step 06-05 提供后，必须用 `--release-notes` 或 `RELEASE_NOTES_PATH` 复跑 gate，才能从 `needs-review` 变为 release-ready pass。 |
| 新增或缺失测试 | 已覆盖 | 新增 `scripts/release-gates.sh`，通过 `bash -n`、`--quick`、full mode、JSON parse、artifact redaction scan、Markdown link checker、compatibility matrix checker 和 docs diff check 验证；没有新增 Rust 单元测试，因为本 Step 交付物是 shell gate runner。 |
| 已更新或缺失文档 | 已更新 | 已同步 `README.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/phase-6-observability-release.md`、本 Step 文档和主 Plan 台账；未新增 `.github/workflows`，因为本 Step 非目标是不绑定特定 CI vendor。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 CI/gate runner、direct tests/checks 和相关文档。
- Commit 前状态：记录 `git status --short`。
- 纳入文件：记录本步骤 commit 包含的文件。
- Commit 后证据：记录 commit hash 和 commit 后 `git status --short --branch`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`phase6: automate release gates`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 06-04 小 Plan | 将 CI/CD Release Gates 自动化拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：自动化 gate 若覆盖面不足，会给 release 错误信心。
- 回滚 / 回退：未自动化的 gate 必须留在人工 checklist，不能删除。
- 后续文档：Step 06-05 canary/release 以本 gate report 作为准入条件。
