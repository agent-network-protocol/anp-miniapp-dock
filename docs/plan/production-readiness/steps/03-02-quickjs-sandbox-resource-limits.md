# Step 03-02：QuickJS 沙箱逃逸回归与资源限制

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：03-02
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 13:52:49 +0800 |
| Completed | 2026-06-13 14:11:33 +0800 |
| Commit | `1c4e784` |
| Review evidence | 2026-06-13 14:06:07 +0800 commit 前 Review：修复 API VM console trace 因 `Rc::try_unwrap` 失败而丢失的问题；修复 `InvalidResult` 仍可能通过 serde 错误文本回显敏感 payload 的问题；确认 Atomic API VM WebSocket/timer globals deny、Promise job drain、console/result size、Component VM snapshot size、dynamic timer cleanup 与文档 gate 一致。 |
| Verification evidence | `cargo fmt --check` 通过；`cargo test -p js-runtime-quickjs sandbox` 通过；`cargo test -p js-runtime-quickjs limit` 2 passed；`cargo test -p js-runtime-quickjs console` 1 passed；`cargo test -p js-runtime-quickjs invalid_atomic` 1 passed；`cargo test -p js-runtime-quickjs pending_job` 1 passed；`cargo test -p component-runtime sandbox` 2 passed；`cargo test -p component-runtime dynamic` 5 passed + snapshot dynamic 2 passed；`cargo test -p component-runtime snapshot_size` 1 passed；`cargo test -p js-runtime-quickjs` 47 passed；`cargo test -p component-runtime` 53 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/js-runtime-quickjs crates/component-runtime docs/security docs/runbook docs/plan` 无输出；敏感词抽样仅命中文档红线、测试假值和 redaction 断言。 |
| Next action | 进入 Step 03-03 权限策略引擎与 allowlist decision |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：在 Step 02-05 已完成 dynamic component 最小安全 gate 的基础上，把 Atomic API VM 和 Component VM 的完整 sandbox escape regression 与资源限制升级为 Phase 3 CI/release gate。
- 用户 / 系统可见行为：恶意 Skill 无法通过 constructor/eval/Function/process/fetch/WebSocket/timer/require 逃逸，超限行为返回稳定失败并可审计。
- 非目标：不开放新的 dynamic 能力；不替换 QuickJS-NG 引擎。
- 完成标准：沙箱策略、逃逸回归测试、memory/stack/CPU/Promise/log/result size 限制和 release gate 证据齐备。

## 3. 设计方法

- 设计边界：Skill JS 默认不可信，任何 API 或组件能力都只能由 broker/profile 显式开放。
- 核心决策：Atomic API VM 与 Component VM 共用安全红线，但可有不同能力 profile；dynamic timer/request 的最小开放安全 gate 已由 Step 02-05 承担，本 Step 补齐全量 VM gate、release gate 和残余风险收敛。
- 契约 / API / 数据流：Skill JS -> QuickJS context -> sandbox policy -> broker/profile -> outcome/audit；limit hit -> stable error + redacted diagnostic。
- 兼容性：保持 coffee demo 和已支持 API 行为；只把逃逸入口和超限路径稳定化。
- 风险控制：console/result/debug 输出必须截断并脱敏；timeout 后不得继续执行高风险 callback/action。

## 4. 实现方法

1. 阅读 `js-runtime-quickjs`、`component-runtime` 当前 sandbox 初始化、timeout 和 tests。
2. 增加或收敛 sandbox policy 配置：禁用 `eval`、`Function`、async/generator constructor、prototype constructor、`process`、`fetch`、`WebSocket`、未授权 timer、remote require；复核 Step 02-05 dynamic gate 是否已覆盖开放前的 Component VM 最小安全集。
3. 增加资源限制：memory、stack、CPU/interrupt timeout、Promise job drain、console size、result size。
4. 为 Atomic API VM 和 Component VM 增加 focused escape tests 与 limit tests。
5. 将 sandbox gate 写入 `docs/runbook/release-gates.md` 和 threat model 控制矩阵。
6. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/js-runtime-quickjs` | API VM sandbox policy、resource limit、tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/component-runtime` | Component VM sandbox policy、dynamic cleanup regression、tests | 代码实现 |
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 同步 sandbox 控制与残余风险 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | sandbox escape regression gate 升级 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-3-security-hardening.md` | 同步 Phase 3 sandbox 状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/03-02-quickjs-sandbox-resource-limits.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 02-05、Step 03-01。
- 外部文档或决策：Threat Model、安全红线、Release Gates。
- 环境前提：Rust toolchain 1.88.0；无需外部 Host provider。

## 7. 验收标准

- [x] Atomic API VM 和 Component VM 都有 constructor/eval/Function/process/fetch/WebSocket/require escape regression tests；Atomic API 的 `require` 是受控包内 CommonJS 能力，包外/remote/path escape 由 `require_parent_escape_is_rejected` 和 CommonJS resolver tests 覆盖。
- [x] memory、stack、CPU timeout、Promise job drain、console size、result size 至少有 focused test 或明确 skip 原因：memory/stack 由 QuickJS runtime config gate 保持默认限制；CPU timeout、Promise job drain、console size、Atomic API result size 和 Component snapshot size 均有 focused tests。
- [x] limit hit 返回稳定脱敏错误，不泄露 JS 源码中的敏感值或 Host private data。
- [x] 复核 Step 02-05 的 dynamic component gate 仍通过，且 Component expire/detach 后不能继续触发事件、timer 或高风险 action。
- [x] Release Gates 将 sandbox escape regression 列为 required。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| API VM sandbox tests | `cd anp/anp-miniapp-dock && cargo test -p js-runtime-quickjs sandbox` | escape 和资源限制 tests 通过；若 filter 不匹配，记录实际命令 |
| Component sandbox tests | `cd anp/anp-miniapp-dock && cargo test -p component-runtime sandbox` | component escape、timer/default deny、detach tests 通过 |
| Workspace 回归 | `cd anp/anp-miniapp-dock && cargo test --workspace` | 通过；如耗时受限，记录 focused 替代和风险 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/js-runtime-quickjs crates/component-runtime docs/security docs/runbook docs/plan` | 无空白错误 |
| 脱敏抽样 | 手工检查 limit error、console、debug 输出 | 不含 token、Authorization、signature、private key path 或隐私原文 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：逃逸入口是否完整；限制是否可配置且默认安全；timeout 后是否停止后续 action；dynamic 例外是否只通过 capability profile 开放。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已发现并修复 | API VM console trace 初版使用 `Rc::try_unwrap(console).unwrap_or_default()`，Host bridge closure 仍持有 `Rc` 时会丢弃全部 console 记录，导致 console size gate 不可审计；`InvalidResult` 初版仍包含 serde error 文本，可能回显 invalid payload 中的敏感字段；主 Plan Step 拆分表仍把 03-01/03-02 标为 pending，与执行台账不一致。 |
| 已修复问题 | 已修复 | API VM trace 改为 clone console buffer；invalid schema error 改为稳定 `schema validation failed`，不带原始 payload；Atomic API VM 增加 WebSocket/timer globals deny、Promise job drain、console/result size 限制；Component VM 增加 snapshot output size 限制；主 Plan 状态将在本 Step 收尾中同步。 |
| 剩余风险 | 已记录 | 本 Step 只完成本地 required release gate，不声明 CI 自动化已完成；真实 Host transport/background scheduler、persistent request/audit、permission allowlist、token lifecycle、Skill 包签名和 resource metrics 仍由 03-03 至 03-06、Phase 4 和 Phase 6 承接。 |
| 新增或缺失测试 | 已补齐本 Step 范围 | 新增/扩展 `crates/js-runtime-quickjs/tests/middleware_chain.rs` 覆盖 WebSocket/timer globals、prototype/async/generator constructor、console truncation、result size、invalid result redaction、pending job drain；新增/扩展 `crates/component-runtime/tests/component_lifecycle.rs` 覆盖 require/eval/process/clear timer/constructor escape 和 snapshot size limit。memory/stack 依赖 QuickJS runtime config，未新增 OOM/stack overflow pressure test，避免环境敏感和不稳定。 |
| 已更新或缺失文档 | 已更新 | 已同步 `docs/security/threat-model.md`、`docs/runbook/release-gates.md`、Phase 3 安全文档、Phase 3 threat-model 摘要、主 Plan 和本 Step 文档；明确 CI 自动化仍待 Phase 6，不把本地 gate 误写成已自动化。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 sandbox/resource limit、直接 tests 和相关文档。
- Commit 前状态：记录 `git status --short`。
- 纳入文件：记录本步骤 commit 包含的文件。
- Commit 后证据：记录 commit hash 和 commit 后 `git status --short --branch`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`phase3: harden quickjs sandbox limits`

执行记录：

- Commit 前状态：`git status --short --branch` 显示仅 Step 03-02 范围内的 `crates/js-runtime-quickjs`、`crates/component-runtime`、`docs/security/threat-model.md`、`docs/runbook/release-gates.md`、Phase 3 文档、主 Plan 和本 Step 文档变更。
- 纳入文件：`crates/js-runtime-quickjs/src/api_vm.rs`、`crates/js-runtime-quickjs/src/bridge.rs`、`crates/js-runtime-quickjs/tests/middleware_chain.rs`、`crates/component-runtime/src/component_vm.rs`、`crates/component-runtime/tests/component_lifecycle.rs`、`docs/security/threat-model.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/phase-3-security-hardening.md`、`docs/plan/production-readiness/phase-3-threat-model-and-controls.md`、主 Plan 和本 Step 文档。
- Commit 后证据：implementation commit `1c4e784 phase3: harden quickjs sandbox limits`；post-commit `git status --short --branch` = `## main...origin/main [ahead 49]`，工作区无未提交实现变更。
- 遗留未提交变更：仅本 Step 文档和主 Plan 的 commit hash / done 状态回填，准备单独创建 docs closure commit。

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 03-02 小 Plan | 将 QuickJS sandbox 加固拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |
| 2026-06-12 | 调整 dynamic sandbox sequencing | 按 Review 发现，dynamic component 的最小 escape/resource-limit gate 前置到 Step 02-05；本 Step 保留 Phase 3 全量 release gate | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：资源限制过严会破坏合法 Skill，过松会留下 DoS 面。
- 回滚 / 回退：限制默认值可配置但 production profile 必须保守；任何放宽都要更新 threat model 和 release gates。
- 后续文档：Phase 6 性能基线需要记录 sandbox limit hit 和 VM resource metrics。
