# Step 03-06：Skill 包完整性与供应链 Gate

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：03-06
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 15:34:39 +0800 |
| Completed | 2026-06-13 16:02:50 +0800 |
| Commit | `b9c767b` |
| Review evidence | 2026-06-13 15:54:50 +0800 commit 前 Review：发现并修复 validate report 可能输出 package signature value 的测试缺口；发现 digest contract 文档要求 lowercase hex 但实现接受大写 hex，已收紧为 64 位小写 hex；确认 supply-chain gate 在 `load_skill_with_integrity_policy` 中早于 entry/API/component 加载，dev/local unsigned 明确 `demo-unsigned`，production policy 对 unsigned、digest mismatch、signature mismatch、unknown publisher quarantine/fail closed；真实 registry/cache 和生产签名 verifier 留给 Phase 4/6，不在本 Step 冒充完成。 |
| Verification evidence | `cargo fmt --check` 通过；`cargo test -p skill-loader package` 通过，filter 实际命中 `symlink_outside_package_fails_closed` 1 test；`cargo test -p skill-loader` 14 passed，覆盖 digest mismatch、signature mismatch、unknown publisher、trusted publisher allowlist、unsigned production quarantine、outside symlink、absolute path、`..`、zip slip；`cargo test -p mcp-schema -p dock-cli validate` 通过，mcp-schema 2 filtered tests + dock-cli validate 4 unit / 1 integration passed；`cargo test -p js-runtime-quickjs remote_require_is_rejected` 1 passed；`cargo run -q -p dock-cli -- validate examples/coffee-skill` 输出 `compatibilityLevel: demo-only`，`compatibilityReport.supplyChain.status = demo-unsigned`，releaseBlockers 含 `supply_chain`；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/skill-loader crates/mcp-schema crates/dock-cli crates/js-runtime-quickjs docs/security docs/runbook docs/plan Cargo.toml Cargo.lock` 无输出；敏感词抽样仅命中测试假值、redaction 断言、安全文档、runbook 和既有 demo-only placeholder，未发现真实 secret/token/proof/private key path 或 package signature value 输出。 |
| Next action | 进入 03-07 Phase 3 最终 Review 与整体验证 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：为 Skill package 增加 digest、signature、publisher DID、trusted publisher allowlist、cache quarantine 和 path/symlink 逃逸 gate。
- 用户 / 系统可见行为：篡改包、签名不匹配、未知 publisher、路径穿越、remote require 默认无法加载。
- 非目标：不实现完整远端 registry 服务；Phase 4 会接入下载、缓存和版本管理。
- 完成标准：本地包和未来 package.zip 都有完整性 contract；供应链失败进入稳定错误和 release blocker。

## 3. 设计方法

- 设计边界：Skill 包加载前必须先证明来源、完整性和路径边界；运行时 sandbox 不能替代供应链校验。
- 核心决策：digest-keyed cache 只读；publisher DID 和 signature 作为 production profile 必需项，dev/local 可显式降级并记录 warning。
- 契约 / API / 数据流：package source -> digest/signature verify -> publisher DID allowlist -> quarantine/cache -> skill-loader path validation -> audit summary。
- 兼容性：保留本地 coffee demo 可加载，但 validate/report 必须把未签名/本地 dev 标为 demo/dev-only。
- 风险控制：symlink、absolute path、`..`、remote require、package.zip slip 均 fail closed；错误不泄露本地绝对路径或 secret。

## 4. 实现方法

1. 阅读 `skill-loader` resolver/package loading、mcp-schema manifest validation 和 release gates。
2. 定义 Skill package digest/signature/publisher DID metadata 结构和 production/dev profile 差异。
3. 实现 digest verification、signature verification trait 或初版实现、trusted publisher allowlist 和 quarantine decision。
4. 加固 path boundary：symlink outside package、absolute path、zip slip、remote require、package cache read-only。
5. 增加 tests：digest mismatch、signature mismatch、unknown publisher、symlink escape、absolute path、remote require、audit redaction。
6. 更新 Threat Model、Release Gates、Phase 3 文档和开发者 validate 输出计划。
7. 回填本 Step 和主 Plan 执行台账；完成后进入 Step 03-07 Phase 3 最终 Review 与整体验证，不得直接进入 Step 04-01。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/skill-loader` | digest/signature/publisher DID、cache quarantine、path/symlink tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/mcp-schema` | manifest metadata / validation warning | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/dock-cli` | validate 供应链报告字段 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 同步 package supply chain 控制 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | package integrity release gate | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-3-security-hardening.md` | 同步供应链完成状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账并指向 Step 03-07 final Review gate | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/03-06-skill-package-integrity-supply-chain.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 01-02、Step 03-01、Step 03-03。
- 外部文档或决策：Threat Model、Release Gates、Phase 4 registry/cache 计划。
- 环境前提：Rust toolchain 1.88.0；真实远端 registry 可后置到 Phase 4。

## 7. 验收标准

- [x] Skill package digest 和 verification result 可记录、可审计。
- [x] package signature / publisher DID contract 清晰；production profile 对未知 publisher fail closed 或 release blocker。
- [x] trusted publisher allowlist 和 quarantine decision 有 tests。
- [x] symlink outside package、absolute path、`..`、zip slip、remote require 均 fail closed。
- [x] 本地 coffee demo 的未签名状态被标为 dev/demo-only，不误标 production-ready。
- [x] Threat Model、Release Gates、CLI validate 计划与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Loader tests | `cd anp/anp-miniapp-dock && cargo test -p skill-loader package` | digest/signature/path tests 通过；若 filter 不匹配，记录实际命令 |
| Schema/CLI tests | `cd anp/anp-miniapp-dock && cargo test -p mcp-schema -p dock-cli validate` | manifest/report tests 通过 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/skill-loader crates/mcp-schema crates/dock-cli docs/security docs/runbook docs/plan` | 无空白错误 |
| 安全抽样 | 手工检查 loader errors、audit、CLI JSON | 不含本地绝对 secret path、token、private key material 或真实 publisher secret |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：供应链 gate 是否在加载前；dev/local 降级是否显式；path/symlink/zip slip 是否完整；错误和 audit 是否脱敏。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 2 项需修复问题 | 1. `dock-cli validate` 已避免输出 signature value，但缺少带敏感 signature fixture 的回归断言；2. digest 文档建议为 lowercase hex，但实现初版接受大写 hex，可能造成同一 digest 的多文本表示。 |
| 已修复问题 | 已修复 | 新增 `validate_redacts_package_signature_value`；`mcp-schema` 和 `skill-loader` 均收紧为 64 位小写 hex。 |
| 剩余风险 | 可接受，且已文档化 | 真实远端 registry/cache、生产签名 verifier、publisher allowlist 配置来源、CI gate 自动化和 cache cleanup 仍属于 Phase 4/6；当前 Step 只冻结并验证本地 supply-chain contract 与 fail-closed gate。 |
| 新增或缺失测试 | 已补 focused tests | 新增 digest mismatch、signature mismatch、unknown publisher、unsigned production quarantine、trusted publisher allowlist、outside symlink、zip slip、remote require、manifest supply-chain warning、CLI supply-chain report 和 signature redaction 测试；未新增真实 zip extraction 测试，因为 Phase 4 才实现 registry/package.zip extraction。 |
| 已更新或缺失文档 | 已更新 | 已同步 `docs/security/threat-model.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/phase-3-security-hardening.md`、`docs/plan/production-readiness/phase-3-threat-model-and-controls.md`；主 Plan 台账待 commit 后回填 hash。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 Skill package integrity/supply chain gate、直接 tests 和相关文档。
- Commit 前状态：`git status --short` 只包含 03-06 代码、测试和文档变更。
- 纳入文件：`Cargo.toml`、`Cargo.lock`、`crates/skill-loader/Cargo.toml`、`crates/skill-loader/src/integrity.rs`、`crates/skill-loader/src/lib.rs`、`crates/skill-loader/src/package.rs`、`crates/skill-loader/src/resolver.rs`、`crates/skill-loader/tests/coffee_skill_load.rs`、`crates/mcp-schema/src/manifest.rs`、`crates/mcp-schema/src/validation.rs`、`crates/mcp-schema/tests/mcp_validation.rs`、`crates/dock-cli/src/commands.rs`、`crates/js-runtime-quickjs/tests/middleware_chain.rs`、`docs/security/threat-model.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/phase-3-security-hardening.md`、`docs/plan/production-readiness/phase-3-threat-model-and-controls.md`、`docs/plan/production-readiness-roadmap.md`、本 Step 文档。
- Commit 后证据：`b9c767b`；post-commit `git status --short --branch` = `## main...origin/main [ahead 57]`。
- 遗留未提交变更：无；本 closure 文档回填作为单独提交记录。
- 建议消息：`phase3: add skill package integrity gates`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 03-06 小 Plan | 将 Skill 包完整性与供应链 Gate 拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |
| 2026-06-13 | 接入 Step 03-07 final Review gate | 按当前 Goal 要求，Phase 3 最终 Review 必须是可追踪 Step，不能只作为 free-form 下一步文字 | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：过早强制签名会破坏本地开发，过晚强制会留下供应链风险。
- 回滚 / 回退：dev/local profile 可显式 warning，production profile 必须 fail closed 或 release blocker。
- 后续文档：本 Step 完成后进入 Step 03-07，执行 Phase 3 最终全局 Review 和整体验证；Phase 4 registry/cache 和 Phase 5 import/validate 必须复用本供应链 contract。
