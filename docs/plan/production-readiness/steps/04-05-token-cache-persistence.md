# Step 04-05：Token Cache 持久化与恢复

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：04-05
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 19:51:13 +0800 |
| Completed | 2026-06-13 20:14:30 +0800 |
| Commit | `f742304` |
| Review evidence | 2026-06-13 20:12:35 +0800 commit 前 Review 已完成：修复 raw token entry 可被 JSON diagnostics 误序列化、fallible `try_put`/`try_clear` 先改内存后落盘导致失败后状态污染、entry metadata 未显式绑定 issuer/audience/jti、clippy bool assert warning；确认 restore policy fail closed，rejected entry 清出 backend snapshot，report 只含 scope summary/reason/redaction metadata，in-memory profile 明确 dev-only |
| Verification evidence | 启动前 `git status --short --branch` = `## main...origin/main [ahead 70]`，工作区无未提交变更；已读取主 Plan、Phase 4 章节、Step 04-05 文档、执行台账、Codex Goal 执行协议、Review/提交门禁、Blocked 处理、Plan 变更记录和 04-04 closure evidence。实现后验证：`cargo fmt --check` 通过；`cargo test -p anp-adapter token_cache` 9 unit + 1 integration under filter passed；`cargo test -p anp-adapter session` 10 passed；`cargo test -p anp-adapter token` 26 unit + 4 integration under filter passed；`cargo test -p anp-adapter` 53 unit + 11 integration + doctests passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/anp-adapter crates/dock-core docs/security docs/runbook docs/plan` 无输出；敏感词抽样仅命中文档红线、测试假值、redaction 代码和既有测试断言，未发现真实 token、Authorization、signature、private key material 或生产凭据；实现 commit 后 `git status --short --branch` = `## main...origin/main [ahead 71]` |
| Next action | 进入 04-06 Scoped Storage 持久化与 quota |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：为 DID session / capability token cache 建立生产候选持久化 backend、恢复策略和 secure store 边界。
- 用户 / 系统可见行为：容器重启后只恢复未过期、未撤销、scope 匹配的 token cache entry；过期、撤销或 replay 风险 entry 默认丢弃。
- 非目标：不实现 scoped storage、audit sink 或 Skill cache 持久化；不把 token 明文写入普通 config。
- 完成标准：token cache backend 有 TTL/revocation/replay restore policy、secure storage boundary、redaction 和 failure policy tests。

## 3. 设计方法

- 设计边界：token cache 是敏感 runtime state，只允许 secure store 或加密 backend；普通文件和 config 只能保存 backend reference。
- 核心决策：entry key 绑定 user DID、merchant DID、Skill id、scope、issuer、audience、jti 和 expiry；restore 时重新校验 trust anchor / revocation / expiry。
- 契约 / API / 数据流：DID session manager -> token cache provider -> secure persistence -> restart restore -> scope validation -> RequestBroker。
- 兼容性：保留 in-memory backend 用于 tests/dev；production profile 缺少 secure token backend 时 release blocked。
- 风险控制：raw token、Authorization、signature、private key path 不进入 logs、CLI JSON、audit export 或 error。

## 4. 实现方法

1. 阅读 Step 03-04 DID / Token 生命周期、Step 04-04 runtime config secret boundary 和 `anp-adapter` token cache 现状。
2. 定义 token cache backend trait、entry schema、restore filter、secure backend reference 和 in-memory dev backend。
3. 实现或规划 token cache persistence wiring，确保 raw token material 只在 secure boundary 内处理。
4. 增加 tests：restart restore、expired not restored、revoked not restored、scope mismatch deny、jti replay deny、redacted diagnostics。
5. 更新 Threat Model、Release Gates、local runbook 和 Phase 4 文档。
6. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/anp-adapter` | token cache backend、restore policy、tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/dock-core` | Runtime config provider wiring | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 同步 token persistence 风险 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | token cache production gate | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/local-demo.md` | 同步 dev/in-memory 与 production backend 区分 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-4-runtime-host-integration.md` | 同步 token persistence 状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/04-05-token-cache-persistence.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 03-04、Step 04-04。
- 外部文档或决策：Threat Model、Release Gates、Runtime config secret boundary。
- 环境前提：Rust toolchain 1.88.0；生产 secure store 可先以 trait + explicit unsupported backend gate 落地。

## 7. 验收标准

- [x] token cache backend 有明确 secure store / encrypted backend boundary 和 in-memory dev profile。
- [x] restart restore 只恢复未过期、未撤销、scope 匹配、trust anchor 有效的 token entry。
- [x] expired/revoked/replayed/scope mismatch token 不恢复且有脱敏错误或 audit summary。
- [x] raw token、Authorization、signature、private key path 不出现在 logs、CLI JSON、audit export 或 tests output。
- [x] Release Gates 标出 in-memory token backend 不能 production-ready。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Token persistence tests | `cd anp/anp-miniapp-dock && cargo test -p anp-adapter token_cache` | restore/expiry/revocation/scope tests 通过；若 filter 不匹配，记录实际命令 |
| Session regression | `cd anp/anp-miniapp-dock && cargo test -p anp-adapter session` | DID session tests 不回归 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/anp-adapter crates/dock-core docs/security docs/runbook docs/plan` | 无空白错误 |
| 敏感信息扫描 | 手工或 `rg` 检查 token cache diagnostics/test output | 不含 raw token、Authorization、signature、private key material |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：restore policy 是否 fail closed；token 是否只在 secure boundary 内；revocation/replay/scope mismatch 是否阻断；redaction 是否覆盖所有输出。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已处理 | raw token entry 初版派生 Serialize/Deserialize，可能被误作 JSON diagnostics 输出；fallible persistence API 初版先改内存再写 backend，backend 失败时会留下状态污染；persisted entry 初版只含 scope/expiry/token，未显式绑定 issuer/audience/jti metadata；clippy 发现 bool assert warning。 |
| 已修复问题 | 已修复 | 移除 entry 的公开 serde 派生，只保留 redacted Debug 和 secure-boundary `token()`；`try_put()`/`try_clear()` 先 replace backend snapshot，成功后才改内存；restore 校验 issuer/audience/jti metadata 与 claims 一致；测试断言改为 `assert!(!...)`。 |
| 剩余风险 | 已记录 | 当前只提供 trait、restore policy 和 `inMemoryDev` dev/test backend；真实 Host secure store 或 encrypted backend、跨进程 replay/revocation store、真实 secret resolve 和 DID rotation 仍是后续 Phase 4/6 production release blocker。 |
| 新增或缺失测试 | 已补齐 | 新增 `token_cache_persistence_*` 单元测试覆盖 valid restore/snapshot、expired、revoked、replayed、scope mismatch、metadata trust mismatch、redacted report/Debug、in-memory dev-only profile、backend failure 不污染内存；`cargo test -p anp-adapter token_cache` 实际命中 9 unit + 1 integration under filter。 |
| 已更新或缺失文档 | 已更新 | 已同步 Phase 4 文档、Release Gates、Threat Model、local demo runbook、本 Step 和主 Plan 台账；未更新兼容矩阵，因为本 Step 不改变 wx API/组件兼容状态。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 token cache persistence、direct tests 和相关文档。
- Commit 前状态：记录 `git status --short`。
- 纳入文件：记录本步骤 commit 包含的文件。
- Commit 后证据：记录 commit hash 和 commit 后 `git status --short --branch`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`phase4: add token cache persistence`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 04-05 小 Plan | 按 Review 发现将原 04-04 的 token persistence 拆成 focused Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：错误恢复 token 会造成越权、replay 或跨 scope 访问。
- 回滚 / 回退：production secure backend 不可用时 fail closed 或 release blocked；in-memory 仅限 dev/test。
- 后续文档：Step 05-05 doctor 和 Phase 6 runbook 需要覆盖 token backend 和 restore failure。
