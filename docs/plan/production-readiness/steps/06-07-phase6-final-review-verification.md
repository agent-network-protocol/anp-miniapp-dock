# Step 06-07：Phase 6 最终 Review 与整体验证

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：06-07
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-14 05:48:24 +0800 |
| Completed | 2026-06-14 05:56:25 +0800 |
| Commit | `8257147` |
| Review evidence | 2026-06-14 05:53:04 +0800 Phase 6 final Review 已记录：确认 06-01 至 06-06 台账和 Step 文档均为 `done`，commit hash、Review 证据和验证证据齐全；修复 `scripts/release-gates.sh` 的 artifact redaction gate report command 字段写入本机绝对路径的问题；修复 `docs/runbook/release-gates.md` 顶部状态仍写 Phase 6 observability gates 进行中的文档漂移；确认 structured events、metrics/tracing、perf baseline、release gates、canary/rollback、operations/privacy deletion 文档没有把 local/headless/mock/backend、Stage 0 local canary 或本地 perf 数字误写成 production-ready。 |
| Verification evidence | 启动前 `git status --short --branch` = `## main...origin/main [ahead 115]`，工作区无未提交变更；已读取主 Plan、Step 06-07 文档、Phase 6 文档和执行台账；Step 06-01 至 06-06 在主台账与 Step 文档中均为 `done`；implementation / closure commit `3fb65f0`、`2e899b0`、`7fa8aee`、`b26d04c`、`67a869e`、`b8ccfed`、`afaa5ab`、`96f5572`、`3b26ba2`、`8866855`、`e6c05bd`、`4ea90a3` 均可解析；`cargo metadata --format-version 1 --no-deps`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo test -p dock-cli --test coffee_order_flow` 均通过，其中 coffee E2E 13 passed；`bash -n scripts/release-gates.sh` 通过；`./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md --report target/release-gates/06-07-release-notes-report.json` 通过，report `dock.release-gates-report.v1` 为 `status = ok`、`releaseDecision = pass`、22 pass / 0 fail / 0 skip、`requiredFailed = 0`、`hardBlockerFailed = 0`、`skipCountsAsPass = false`；`python3 -m json.tool target/release-gates/06-07-release-notes-report.json >/tmp/06-07-release-notes-report.json` 通过；`git diff --check -- docs/plan docs/architecture docs/runbook docs/developer docs/security README.md AGENTS.md` 与 `git diff --check -- scripts docs/runbook docs/plan README.md` 均无输出；`rg -n "\[ \]" docs/plan/production-readiness/steps/06-0{1,2,3,4,5,6}-*.md docs/plan/production-readiness/phase-6-observability-release.md` 无未完成验收项；严格 artifact/report scan 仅命中 release report 的 hard blocker 名称 `Authorization or Signature leakage`，未发现 raw token、Authorization/Signature value、capabilityToken、private key material、本机绝对路径、手机号、真实地址、文件内容或精确位置进入 artifacts、Render IR snapshots 或 perf baseline；final review commit `8257147` 后 `git status --short --branch` = `## main...origin/main [ahead 116]`，工作区无未提交变更。 |
| Next action | 当前 Phase 5/6 Goal 已关闭；后续 production Host/deploy/provider 接入需通过新 Plan 或新增 Step 执行 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：把 Step 06-01 至 Step 06-06 的观测、性能、发布运营工作做一次阶段级 Review 和整体验证，形成当前 Phase 5/6 Goal 的 closure gate。
- 用户 / 系统可见行为：Codex Goal 不会在 06-06 完成后直接宣称完成；必须先确认 observability、metrics/tracing、performance、CI gates、canary/rollback、operations/privacy deletion 文档和 release blockers 一致。
- 非目标：不新增 Phase 7 或真实 production Host 接入；不把未完成的生产部署、CI、Host、secret store 或加密 backend 误标为 production-ready。
- 完成标准：主 Plan 最终全局 Review 记录已追加 Phase 6 证据，执行台账中 06-01 至 06-07 均有 Review/验证/commit 证据，工作区无未提交完成工作。

## 3. 设计方法

- 设计边界：本 Step 是 Phase 6 integration review gate，只有在 06-01 至 06-06 全部 `done` 后启动。
- 核心决策：按主 Plan 的最终全局 Review 与整体验证章节执行；如果 Review 修复需要改文件，本 Step 创建独立 final review commit。
- 契约 / API / 数据流：Step evidence -> ledger audit -> git history audit -> observability/release contract audit -> verification suite -> Review findings/fixes -> final review record。
- 兼容性：确认结构化事件、metrics/tracing、performance baseline、CI gate、canary/rollback、ops/privacy runbook 没有破坏 coffee demo、Runtime API、CLI developer tools、wx bridge contract、Render IR、DID/request、ConsentGate、audit、redaction、sandbox 和 supply-chain gate。
- 风险控制：若发现敏感观测字段泄漏、release gate 漏报、runbook 误导 production-ready、性能基线不可复现或 CI/ops 文档漂移，不得关闭当前 Goal；必须修复或记录 blocker。

## 4. 实现方法

1. 确认 Step 06-01 至 06-06 在主 Plan 执行台账和各 Step 文档中均为 `done`，且有 commit hash、Review 证据和验证证据。
2. 核对这些 commit 能在 git history 中解析，并且没有未提交完成工作。
3. 执行 Phase 6 全局 Review：structured events/log redaction、metrics/tracing labels、performance/stress baseline、CI/CD release gates、canary/rollback strategy、operations/privacy deletion runbooks、release blockers 和文档漂移。
4. 运行整体验证基线；若命令不能运行，记录原因、影响、替代检查和剩余风险。
5. 修复 Review 发现的必要问题；若修改文件，按本 Step Review/验证/commit gate 创建 focused commit。
6. 在主 Plan `2.3.8 最终全局 Review 与整体验证` 追加 Phase 6 执行记录。
7. 回填本 Step 和主 Plan 执行台账；本 Step `done` 后结束当前 Phase 5/6 Goal。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 追加 Phase 6 最终 Review 记录，回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/06-07-phase6-final-review-verification.md` | 回填状态、证据、Review、commit | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-6-observability-release.md` | 审计 Phase 6 完成状态 | 视发现更新 |
| `anp/anp-miniapp-dock/docs/runbook` | 审计运维、发布、隐私删除 runbook | 视发现更新 |
| `anp/anp-miniapp-dock/docs/security` | 审计敏感信息、审计、隐私边界 | 视发现更新 |
| `anp/anp-miniapp-dock/README.md` | 审计发布/开发者入口说明 | 视发现更新 |

## 6. 依赖

- 前置步骤：Step 06-01、Step 06-02、Step 06-03、Step 06-04、Step 06-05、Step 06-06。
- 外部文档或决策：主 Plan `2.3.8 最终全局 Review 与整体验证`、Phase 6 章节、Release Gates、Threat Model、开发者文档和运维 runbook。
- 环境前提：Rust toolchain 1.88.0；若完整 workspace 验证耗时或环境缺失，必须记录 focused 替代和残余风险。

## 7. 验收标准

- [x] 主 Plan 执行台账中 Step 06-01 至 06-06 全部为 `done`，且 commit hash、Review 证据、验证证据完整。
- [x] git history 能解析 Step 06-01 至 06-06 的 commit hash。
- [x] 执行或明确记录主 Plan 整体验证基线：metadata、fmt、clippy、workspace tests、coffee E2E、docs diff check。
- [x] Review 覆盖 observability、metrics/tracing、performance/stress、CI/CD gates、canary/rollback、operations/privacy deletion、release blockers、redaction 和文档漂移。
- [x] 必要 Review 发现已修复；无法修复的问题已记录为 blocker 或剩余风险。
- [x] 主 Plan `2.3.8` 已追加 Phase 6 最终 Review 记录。
- [x] 本步骤已经创建 focused commit、回填主 Plan 执行台账，并记录当前 Goal closure 状态。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 工作区状态 | `cd anp/anp-miniapp-dock && git status --short --branch` | 无未提交完成工作；若有用户改动，记录并保护 |
| Metadata | `cd anp/anp-miniapp-dock && cargo metadata --format-version 1 --no-deps` | 通过 |
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Clippy | `cd anp/anp-miniapp-dock && cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| Workspace tests | `cd anp/anp-miniapp-dock && cargo test --workspace` | 通过 |
| Coffee E2E | `cd anp/anp-miniapp-dock && cargo test -p dock-cli --test coffee_order_flow` | 通过 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- docs/plan docs/architecture docs/runbook docs/developer docs/security README.md AGENTS.md` | 无空白错误；若目录不存在，记录实际命令 |
| 敏感信息抽样 | 手工或 `rg` 检查 Phase 6 events、metrics、reports、fixtures 和 docs | 不含 raw token、Authorization、signature、private key material、手机号、地址、文件内容、本机绝对路径或真实隐私原文 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：整体验证完成后、commit 前。
- Review 重点：Step 证据是否完整；observability/release contracts 是否稳定；敏感字段默认脱敏；runbook 是否区分本地/demo/headless 与 production requirements。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已记录 | `release-gates.sh` 的 artifact redaction gate 在 report `command` 字段中写入 `$ARTIFACT_DIR` 本机绝对路径；`release-gates.md` 顶部状态仍写 Phase 6 observability gates 进行中。 |
| 已修复问题 | 已修复 | `check_no_match` 的 report command 改为仓库相对 target 列表，保留实际扫描路径不变；release gates runbook 顶部状态改为 Phase 6 本地 release gates 已完成并通过 final Review，同时保留真实 production Host blocker。 |
| 剩余风险 | 已记录 | Phase 6 完成本地 observability、metrics/tracing、performance smoke、release gate runner、Stage 0 local canary、operations/troubleshooting/privacy deletion runbooks；真实 production Host secure store、encrypted storage/audit backend、deploy platform、traffic router、provider conformance、production privacy deletion job/approval workflow、vendor exporter 和生产 SLO 仍是后续生产接入 blocker。 |
| 新增或缺失测试 | 已覆盖 | 未新增 Rust 测试；已重跑 metadata、fmt、clippy、workspace tests、coffee E2E、full release gate、release gate JSON parse、script syntax、docs diff check 和 artifact/report redaction scan。 |
| 已更新或缺失文档 | 已更新 | 主 Plan `2.3.8` 追加 Phase 6 final Review 记录；同步 Step 06-07 执行状态、Review/验证证据；修复 release gates runbook 状态漂移。 |

## 10. Commit 要求

- Commit 时机：最终 Review、整体验证、必要修复和主 Plan 记录完成后。
- Commit 范围：只包含 Phase 6 final review 记录、必要文档修复和直接关联证据更新。
- Commit 前状态：`git status --short --branch` = `## main...origin/main [ahead 115]`，未提交变更仅包含 06-07 final Review 记录、release gates runbook 状态修复和 release gate report path redaction 修复。
- 纳入文件：`scripts/release-gates.sh`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/steps/06-07-phase6-final-review-verification.md`。
- Commit 后证据：final review commit `8257147 docs: record phase6 final review`；commit 后 `git status --short --branch` = `## main...origin/main [ahead 116]`，工作区无未提交变更。
- 遗留未提交变更：无。
- 建议消息：`docs: record phase6 final review`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 无 | 无 | 无 | 当前步骤 / 整体计划 | 无 blocker，06-07 可创建 final review commit，然后创建 closure commit 结束当前 Phase 5/6 Goal |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-13 | 创建 Step 06-07 小 Plan | 将 Phase 6 最终 Review 与整体验证变成可追踪 gate，避免 Codex Goal 在 06-06 后跳过当前目标 closure | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：如果只在 06-06 的下一步文字中提到 Phase 6 Review，长跑 Goal 恢复时可能跳过最终集成审计并误报当前 Phase 5/6 Goal 完成。
- 回滚 / 回退：若本 Step 发现阻塞问题，保持 06-07 为 `blocked`，不得关闭当前 Goal。
- 后续文档：当前 Goal 完成后，若继续生产 Host 或部署级能力，应通过新的 Plan 变更或后续阶段 Step 执行。
