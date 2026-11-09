# Step 04-06：Scoped Storage 持久化与 quota

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：04-06
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 20:17:07 +0800 |
| Completed | 2026-06-13 20:35:46 +0800 |
| Commit | `67237ba` |
| Review evidence | 2026-06-13 20:33:19 +0800 commit 前 Review 已完成：修复 local file backend 对单条非法 persisted record 直接让整个 restore 失败、无法按脱敏 rejection 清理 snapshot 的问题；补充 `StoragePersistenceSnapshot` public re-export；确认 scope 覆盖 user DID、merchant DID、Skill id、namespace，persistent set/remove/clear/delete scope 先写 backend snapshot 再更新内存，quota fail closed，restore report/Debug 只输出 scope summary、key/value bytes、reason 和 redaction metadata，`localFileUnencrypted` 明确 dev/test/local evidence 且非 production-ready |
| Verification evidence | 启动前 `git status --short --branch` = `## main...origin/main [ahead 72]`，工作区无未提交变更；已读取主 Plan、Phase 4 章节、Step 04-06 文档、执行台账、Codex Goal 执行协议、Review/提交门禁、Blocked 处理、Plan 变更记录和 04-05 closure evidence。实现后验证：`cargo fmt --check` 通过；`cargo test -p wx-compat storage` 14 passed；`cargo test -p js-runtime-quickjs storage` 6 passed；`cargo test -p wx-compat` 16 component permission + 5 high-risk + 14 storage + doctests passed；`cargo test -p js-runtime-quickjs` 5 unit + 40 middleware + 3 register + doctests passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/wx-compat crates/js-runtime-quickjs crates/dock-core docs/architecture docs/security docs/runbook docs/plan` 无输出；敏感词抽样仅命中文档红线、测试假值、redaction 断言和 dev/local backend 状态，未发现真实 storage 隐私 value、token、Authorization、signature、private key material 或生产凭据；实现 commit 后 `git status --short --branch` = `## main...origin/main [ahead 73]` |
| Next action | 进入 04-07 Persistent Audit Sink retention/export |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：为 `wx.*` storage 建立按 user DID / merchant DID / Skill id / storage namespace 隔离的持久化 backend、quota 和 scope cleanup。
- 用户 / 系统可见行为：重启后 storage 按 scope 恢复；跨用户、跨商家、跨 Skill 不能读取；超 quota 有稳定 fail shape。
- 非目标：不实现 token cache、audit sink 或 Skill package cache；不保存真实隐私样例。
- 完成标准：storage backend 有 scope isolation、quota、restart restore、delete scope 和 redaction tests。

## 3. 设计方法

- 设计边界：storage 是 Skill 可见状态，必须按 DID/merchant/Skill scope 隔离，不能成为跨 session 隐私通道。
- 核心决策：保留 in-memory dev backend；当前提供未加密 local file JSON 作为 dev/test/local evidence backend；production profile 只允许 Host encrypted store 或 encrypted SQLite，并通过 Step 04-04 config reference 注入。
- 契约 / API / 数据流：wx storage API -> scoped storage provider -> quota check -> persistent backend -> scope cleanup。
- 兼容性：保持 Step 01-06 JS bridge 语义；sync/async storage fail shape 不漂移。
- 风险控制：storage key/value diagnostics 默认 redacted 或 size-only；privacy deletion 必须能按 scope 清理。

## 4. 实现方法

1. 阅读 Step 01-06 Storage JS Bridge、Step 04-04 config boundary 和 `wx-compat` scoped storage 现状。
2. 定义 storage backend trait、scope key、quota policy、error shape 和 in-memory/dev vs production profile。
3. 实现或规划 persistent backend wiring、restart restore、quota enforcement 和 scope cleanup API。
4. 增加 tests：same scope restore、cross scope deny、quota exceeded、remove/clear persistence、delete scope、redacted diagnostics。
5. 更新 API 矩阵、Threat Model、Release Gates、local runbook 和 Phase 4 文档。
6. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/wx-compat` | scoped storage backend、quota、scope cleanup、tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/js-runtime-quickjs` | storage bridge persistence regression | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/dock-core` | Runtime storage provider wiring | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` | 同步 storage production profile | 必须 |
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 同步 storage privacy 风险 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | storage persistence/quota gate | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-4-runtime-host-integration.md` | 同步 storage persistence 状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/04-06-scoped-storage-persistence.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 01-06、Step 04-04。
- 外部文档或决策：Storage JS Bridge contract、Threat Model、Runtime config。
- 环境前提：Rust toolchain 1.88.0；production backend 可先以 feature/trait 边界落地。

## 7. 验收标准

- [x] scoped storage 有持久化 backend boundary、in-memory dev profile 和 production profile gate。
- [x] storage scope 至少包含 user DID、merchant DID、Skill id 和 namespace；跨 scope 读取/清理有 tests。
- [x] quota exceeded 返回稳定 fail shape，不泄露 value 原文。
- [x] restart restore、remove、clear、delete scope 行为有 tests。
- [x] API 矩阵、Threat Model、Release Gates 和 Phase 4 文档与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Storage tests | `cd anp/anp-miniapp-dock && cargo test -p wx-compat storage` | scope/quota/restore/delete tests 通过 |
| JS bridge regression | `cd anp/anp-miniapp-dock && cargo test -p js-runtime-quickjs storage` | storage JS bridge 不回归；若 filter 不匹配，记录实际命令 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/wx-compat crates/js-runtime-quickjs crates/dock-core docs/architecture docs/security docs/runbook docs/plan` | 无空白错误 |
| 敏感信息扫描 | 手工或 `rg` 检查 storage diagnostics/test output | 不含真实隐私 value、token、Authorization 或 private key path |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：scope isolation 是否完整；quota 是否稳定；delete scope 是否精确；diagnostics 是否不泄露 storage value。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已处理 | local file backend 初版对单条 invalid persisted record 直接让整个 `restore()` 返回 `BackendCorrupt`/entry validation error，不能按 Step 要求记录 redacted rejection 并清理坏 entry；新增 `StoragePersistenceSnapshot` 是 public trait 返回类型但初版未从 crate root re-export。 |
| 已修复问题 | 已修复 | 为 persistence backend 增加 `load_restore_snapshot()`，local file backend 先读取原始 JSON record，再把 invalid entry 转为 `StorageRestoreRejectionReason::InvalidEntry` 并由 restore 重写 snapshot；从 `wx-compat` crate root 导出 `StoragePersistenceSnapshot`；补充 namespace 隔离、invalid entry cleanup 和 report 不泄露原文测试。 |
| 剩余风险 | 已记录 | 当前只提供 trait、profile gate、quota/restore/delete-scope 策略和未加密 `LocalFileScopedStorageBackend` dev/test/local evidence；真实 Host encrypted store 或 encrypted SQLite、migration、access control、backup/repair 和 privacy deletion 仍是后续 Phase 4/6 production release blocker。 |
| 新增或缺失测试 | 已补齐 | 新增/扩展 storage persistence tests 覆盖 same-scope restore、user/namespace cross-scope isolation、remove/clear/delete scope 持久化、quota restore rejection、invalid entry cleanup、report/Debug key/value redaction、profile production-ready gate；`cargo test -p wx-compat storage` 实际命中 14 tests。 |
| 已更新或缺失文档 | 已更新 | 已同步 wx API 兼容矩阵、Threat Model、Release Gates、local demo runbook、Phase 4 文档、本 Step 和主 Plan 台账；未修改 `dock-core` runtime wiring，因为本 Step 只冻结 provider/backend contract，不声明生产 Host storage provider 已接入。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 scoped storage persistence/quota、direct tests 和相关文档。
- Commit 前状态：记录 `git status --short`。
- 纳入文件：记录本步骤 commit 包含的文件。
- Commit 后证据：记录 commit hash 和 commit 后 `git status --short --branch`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`phase4: add scoped storage persistence`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 04-06 小 Plan | 按 Review 发现将原 04-04 的 scoped storage persistence 拆成 focused Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：scope 设计错误会造成跨用户或跨商家的长期数据泄露。
- 回滚 / 回退：production backend 不满足 isolation/quota 时 fail closed；保留 in-memory dev backend 供 tests/demo。
- 后续文档：Step 06-06 privacy deletion 必须引用本 Step 的 delete scope 能力。
