# Step 04-08：Skill Cache cleanup 与版本清理

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：04-08
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 21:01:51 +0800 |
| Completed | 2026-06-13 21:22:02 +0800 |
| Commit | `01a0cec` |
| Review evidence | 2026-06-13 21:20:37 +0800 commit 前 Review 已记录：确认 sidecar metadata 写在 cache root 下而非 Skill 包目录内，不影响包 digest；dry-run/report 不输出 cache root、本机绝对路径或 package URL secret/query；delete scope 只删除匹配 package dir 与对应 sidecar；rollback pin 与 active retain 会保留；quarantined sidecar 会让后续 reload fail closed；legacy cache 无 sidecar 时只被全量 cleanup 匹配；本 Step 未新增 CLI 命令，CLI/ops cleanup surface 留给 Phase 5/6。 |
| Verification evidence | 启动前 `git status --short --branch` = `## main...origin/main [ahead 76]`，工作区无未提交变更；已读取主 Plan、Step 04-08 文档、Phase 4 章节、执行台账、Codex Goal 执行协议、Review/提交门禁、Blocked 处理、Plan 变更记录和 04-07 closure evidence。`cargo fmt --check` 通过；`cargo test -p skill-loader cache` 7 passed；`cargo test -p skill-loader` 14 package/path tests + 11 registry/cache tests + doctests passed；`cargo test -p dock-cli cache` 通过但 filter 命中 0 tests，本 Step 未触及 CLI surface；`cargo clippy -p skill-loader --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/skill-loader crates/dock-core crates/dock-cli docs/security docs/runbook docs/plan` 无输出；敏感词扫描仅命中文档红线、测试假值、redaction 断言和既有计划文本，未发现真实 token、Authorization、signature、private key material、本机绝对路径或生产凭据进入 cleanup report。 |
| Next action | 进入 04-09 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：为 Skill registry/cache 增加 digest-keyed cache cleanup、eviction、version pin 清理、quarantine 清理和 privacy/delete scope hooks。
- 用户 / 系统可见行为：Skill 包缓存可以按 digest、publisher DID、merchant DID、Skill id、version 和 rollback policy 清理；被 quarantine 的包不会再次加载。
- 非目标：不重新设计 registry/version resolver；不实现 token/storage/audit 持久化。
- 完成标准：cache cleanup 有 dry-run/report、eviction、quarantine、rollback protection 和 redacted diagnostics tests。

## 3. 设计方法

- 设计边界：Skill package cache 是代码供应链状态，不存用户隐私 payload；清理策略不能破坏 rollback pin 或加载未验证包。
- 核心决策：cache key 以 digest 为主，metadata 记录 publisher DID、Skill id、version、verified status、quarantine reason 和 last-used；cleanup 先 dry-run。
- 契约 / API / 数据流：registry ref -> verified package -> digest cache -> version pin / rollback -> cleanup report -> cache purge。
- 兼容性：保留本地 examples/dev Skill 路径；production package cache 必须经过 Step 03-06 integrity gate。
- 风险控制：cache path diagnostics redacted；quarantine package 不可被 fallback loader 绕过。

## 4. 实现方法

1. 阅读 Step 03-06 Skill 包完整性、Step 04-03 registry/cache/versioning 和 Step 04-04 config boundary。
2. 定义 cache metadata、cleanup policy、dry-run report、eviction rule、quarantine rule 和 rollback pin protection。
3. 实现或规划 cache cleanup API/CLI/internal command，并确保清理不会删除 active pinned version。
4. 增加 tests：digest cache cleanup、quarantine deny, rollback pin retained, dry-run report redacted, delete scope hook。
5. 更新 Release Gates、Threat Model、Phase 4 文档和后续 runbook 计划。
6. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/skill-loader` | cache metadata、cleanup/quarantine、tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/dock-core` | Runtime cache cleanup wiring | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/dock-cli` | cache cleanup/inspect dry-run 命令 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 同步 supply-chain/cache cleanup 风险 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | package cache cleanup/quarantine gate | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-4-runtime-host-integration.md` | 同步 Skill cache cleanup 状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/04-08-skill-cache-cleanup.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 04-03、Step 04-04。
- 外部文档或决策：Skill package integrity gate、registry/cache versioning、Release Gates。
- 环境前提：Rust toolchain 1.88.0；package cache directory 使用 mock/dev paths。

## 7. 验收标准

- [x] cache cleanup 支持 dry-run/report，且 report 不泄露本机绝对路径、secret 或隐私数据。
- [x] digest-keyed cache eviction 不删除 active pinned/rollback-required version。
- [x] quarantine package 不能被 loader 或 fallback path 绕过。
- [x] cleanup 可按 publisher DID、merchant DID、Skill id、version/digest scope 执行。
- [x] Release Gates、Threat Model 和 Phase 4 文档与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Skill cache tests | `cd anp/anp-miniapp-dock && cargo test -p skill-loader cache` | cleanup/quarantine/pin tests 通过 |
| CLI cleanup tests | `cd anp/anp-miniapp-dock && cargo test -p dock-cli cache` | dry-run/report tests 通过；若未触及 CLI，记录原因 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/skill-loader crates/dock-core crates/dock-cli docs/security docs/runbook docs/plan` | 无空白错误 |
| 路径/敏感信息扫描 | 手工或 `rg` 检查 cache cleanup report | 不含本机绝对路径、raw token、Authorization、signature 或 private key path |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：cleanup 是否会破坏 rollback；quarantine 是否不可绕过；dry-run 是否准确；report 是否 redacted。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已记录并处理 | 初版需要确认 sidecar 是否会改变包 digest、quarantine 是否能被同 digest reload 绕过、cleanup 是否可能误删 rollback 版本、报告是否泄露 cache root 或 package URL secret、legacy cache 无 metadata 时如何 scoped cleanup。 |
| 已修复问题 | 已修复 | sidecar 放在 cache root 下；`load_or_insert()` 先检查 quarantined metadata；cleanup 保留 active retain 和 rollback pin；report 只输出 sanitized cache ref、structured key、redaction metadata 和 action/reason；legacy cache 无 sidecar 时只匹配全量 cleanup，不做不可靠路径反解。 |
| 剩余风险 | 已记录，非本 Step 阻塞 | 当前是 `skill-loader` Rust API contract，未新增 CLI/ops cleanup 命令；真实远端 registry quarantine feed、部署级 cache 防篡改、签名吊销同步、privacy deletion runbook 和 CI release report 留给 Phase 5/6。 |
| 新增或缺失测试 | 已新增 focused tests | 新增 dry-run report 脱敏、delete scope、rollback pin/active retain、quarantine fail-closed/purge 测试；`cargo test -p dock-cli cache` 命中 0 tests，因为本 Step 未触及 CLI surface。 |
| 已更新或缺失文档 | 已更新 | 已同步 Phase 4 文档、Threat Model、Release Gates、主 Plan 和本 Step 文档；CLI/ops cleanup surface 在文档中明确留给后续 Phase 5/6。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 Skill cache cleanup/quarantine、direct tests 和相关文档。
- Commit 前状态：记录 `git status --short`。
- 纳入文件：记录本步骤 commit 包含的文件。
- Commit 后证据：记录 commit hash 和 commit 后 `git status --short --branch`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`phase4: add skill cache cleanup`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 04-08 小 Plan | 按 Review 发现将原 04-04 的 Skill cache cleanup 拆成 focused Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：错误清理会删除 rollback 所需包，或让 quarantine 包重新可加载。
- 回滚 / 回退：cleanup 先 dry-run；不确定时保留 cache 并记录 release blocker。
- 后续文档：Step 06-05 rollback 和 Step 06-06 operations runbook 必须引用 cache purge/quarantine 行为。
