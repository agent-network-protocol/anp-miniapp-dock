# Step 03-07：Phase 3 最终 Review 与整体验证

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：03-07
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 16:04:15 +0800 |
| Completed | 2026-06-13 16:10:22 +0800 |
| Commit | `e888b24`；closure `9f884ac` |
| Review evidence | 2026-06-13 16:10:22 +0800 Phase 3 最终 Review 已记录：修复 roadmap 恢复指针和通用 Codex Goal 提示词仍指向 03-01、Phase 3 子文档 03-07 未关闭的文档漂移；确认 03-01 至 03-06 safety gates、release blockers、demo-only 边界和 redaction 口径一致，未发现阻塞问题。 |
| Verification evidence | `cargo metadata --format-version 1 --no-deps` 通过；`cargo fmt --check` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo test -p dock-cli --test coffee_order_flow` 4 passed；`cargo run -q -p dock-cli -- validate examples/coffee-skill` 输出 `compatibilityLevel: demo-only`、`supplyChain.status = demo-unsigned`、releaseBlockers 含 `supply_chain`；`git diff --check -- docs/plan docs/architecture docs/runbook docs/security README.md AGENTS.md` 无输出；Phase 3 commit hash 均可解析；敏感词抽样仅命中测试假值、redaction 断言、安全文档和 demo-only placeholder，未发现真实 secret/token/proof/private key path、package signature value 或隐私原文输出。 |
| Next action | 本 Goal 停止在 03-07，不进入 Step 04-01；后续若启动新 Goal，应从主 Plan 第一个非 `done` Step 04-01 开始。 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：把 Step 03-01 至 Step 03-06 的安全增强最终 Review 和整体验证变成可追踪、可恢复、可标记 `done` 的执行 gate。
- 用户 / 系统可见行为：Codex Goal 不会在 Phase 3 完成后直接进入 Phase 4；必须先完成阶段级 Review、验证、证据记录和必要修复。
- 非目标：不新增 Phase 4 runtime / Host 能力；不扩大 `04-01` 范围。
- 完成标准：主 Plan 最终全局 Review 记录已追加 Phase 3 证据，执行台账中 03-01 至 03-07 均有 Review/验证/commit 证据，工作区无未提交完成工作。

## 3. 设计方法

- 设计边界：本 Step 是 Phase 3 integration review gate，只有在 03-01 至 03-06 全部 `done` 后启动。
- 核心决策：按主 Plan 的最终全局 Review 与整体验证章节执行；如果 Review 修复需要改文件，本 Step 创建独立 final review commit。
- 契约 / API / 数据流：Step evidence -> ledger audit -> git history audit -> security gate audit -> verification suite -> Review findings/fixes -> final review record。
- 兼容性：确认 Phase 3 threat model、sandbox、permission、DID/token、consent/audit、package supply chain 没有破坏 coffee demo、wx bridge contract、Render IR、DID/request 和 redaction 边界。
- 风险控制：若发现安全、隐私、公开契约、release gate 或文档漂移阻塞问题，不得进入 04-01；必须修复或记录 blocker。

## 4. 实现方法

1. 确认 Step 03-01 至 03-06 在主 Plan 执行台账和各 Step 文档中均为 `done`，且有 commit hash、Review 证据和验证证据。
2. 核对这些 commit 能在 git history 中解析，并且没有未提交完成工作。
3. 执行 Phase 3 全局 Review：Threat Model、Release Gates、API/组件矩阵、安全红线、sandbox gates、permission decisions、DID/token lifecycle、Consent/Audit、package integrity、redaction 和文档漂移。
4. 运行整体验证基线；若命令不能运行，记录原因、影响、替代检查和剩余风险。
5. 修复 Review 发现的必要问题；若修改文件，按本 Step Review/验证/commit gate 创建 focused commit。
6. 在主 Plan `2.3.8 最终全局 Review 与整体验证` 追加 Phase 3 执行记录。
7. 回填本 Step 和主 Plan 执行台账；本 Goal 在本 Step `done` 后停止，不进入 Step 04-01。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 追加 Phase 3 最终 Review 记录，回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/03-07-phase3-final-review-verification.md` | 回填状态、证据、Review、commit | 必须 |
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 审计 Phase 3 安全控制状态 | 视发现更新 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | 审计 security gates 是否覆盖新增能力 | 视发现更新 |
| `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` | 审计 API 风险等级与 permission 状态 | 视发现更新 |
| `anp/anp-miniapp-dock/docs/architecture/component-compatibility-matrix.md` | 审计组件 dynamic / sandbox / action 状态 | 视发现更新 |

## 6. 依赖

- 前置步骤：Step 03-01、Step 03-02、Step 03-03、Step 03-04、Step 03-05、Step 03-06。
- 外部文档或决策：主 Plan `2.3.8 最终全局 Review 与整体验证`、Threat Model、Release Gates、API/组件兼容矩阵。
- 环境前提：Rust toolchain 1.88.0；若完整 workspace 验证耗时或环境缺失，必须记录 focused 替代和残余风险。

## 7. 验收标准

- [x] 主 Plan 执行台账中 Step 03-01 至 03-06 全部为 `done`，且 commit hash、Review 证据、验证证据完整。
- [x] git history 能解析 Step 03-01 至 03-06 的 commit hash。
- [x] 执行或明确记录主 Plan 整体验证基线：metadata、fmt、clippy、workspace tests、coffee E2E、docs diff check。
- [x] Review 覆盖 Threat Model、Release Gates、sandbox、permission、DID/token、Consent/Audit、Skill package integrity、redaction、安全边界和文档漂移。
- [x] 必要 Review 发现已修复；无法修复的问题已记录为 blocker 或剩余风险。
- [x] 主 Plan `2.3.8` 已追加 Phase 3 最终 Review 记录。
- [ ] 本步骤在进入 Step 04-01 之前已经创建 focused commit，并回填主 Plan 执行台账；本 Goal 到此结束。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 工作区状态 | `cd anp/anp-miniapp-dock && git status --short --branch` | 无未提交完成工作；若有用户改动，记录并保护 |
| Metadata | `cd anp/anp-miniapp-dock && cargo metadata --format-version 1 --no-deps` | 通过 |
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Clippy | `cd anp/anp-miniapp-dock && cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| Workspace tests | `cd anp/anp-miniapp-dock && cargo test --workspace` | 通过 |
| Coffee E2E | `cd anp/anp-miniapp-dock && cargo test -p dock-cli --test coffee_order_flow` | 通过 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- docs/plan docs/architecture docs/runbook docs/security README.md AGENTS.md` | 无空白错误 |
| 敏感信息抽样 | 手工或 `rg` 检查 Phase 3 相关输出、fixtures 和 docs | 不含 raw token、Authorization、signature、private key material、手机号、地址、文件内容或真实隐私原文 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：整体验证完成后、commit 前。
- Review 重点：Step 证据是否完整；Phase 3 security gates 是否默认 fail closed；是否存在跨 Step 契约漂移；是否仍有未记录的 production release blocker。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已记录 | 文档漂移：roadmap 恢复指针和通用 Codex Goal 提示词仍指向 03-01；Phase 3 子文档仍把 03-07 标为未完成。未发现需要修改 Phase 3 代码的阻塞问题。 |
| 已修复问题 | 已修复 | 恢复指针、通用 Codex Goal 提示词、Phase 3 完成状态和主 Plan `2.3.8` final Review 记录已更新。 |
| 剩余风险 | 已记录 | CI 自动化、生产 Host/registry/cache、生产签名 verifier、secret store、持久化 token cache/revocation restore、生产 Host UI/conformance、真实 registry zip extraction 和运维发布自动化仍待 Phase 4/6；coffee demo 仍为 `demo-only`。 |
| 新增或缺失测试 | 已覆盖 | 本 Step 是 final Review gate，未新增代码测试；已重新运行 metadata、fmt、clippy、workspace tests、coffee E2E、validate coffee 和 docs diff check。 |
| 已更新或缺失文档 | 已更新 | 已更新 roadmap、03-07 Step 文档和 Phase 3 子文档；Threat Model / Release Gates 未发现需要改动的状态漂移。 |

## 10. Commit 要求

- Commit 时机：最终 Review、整体验证、必要修复和主 Plan 记录完成后。
- Commit 范围：只包含 Phase 3 final review 记录、必要文档修复和直接关联证据更新。
- Commit 前状态：记录 `git status --short`。
- 纳入文件：记录本步骤 commit 包含的文件。
- Commit 后证据：记录 commit hash 和 commit 后 `git status --short --branch`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`docs: record phase3 final review`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 无 | 无 | 无 | 当前步骤 / 整体计划 | 无 blocker，03-07 可关闭 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-13 | 创建 Step 03-07 小 Plan | 将 Phase 3 最终 Review 与整体验证变成可追踪 gate，避免 Codex Goal 直接进入 Phase 4 | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：如果只在 03-06 的下一步文字中提到 Phase 3 Review，长跑 Goal 恢复时可能跳过 Phase 4 前置安全审计。
- 回滚 / 回退：若本 Step 发现阻塞问题，保持 03-07 为 `blocked`，不得启动 04-01。
- 后续文档：Step 04-01 必须以本 Step 的最终 Review 记录作为 Phase 4 启动前证据。
