# Step 01-05：Unsupported API Registry 与统一 fail shape

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：01-05
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 20:02:26 +0800 |
| Completed | 2026-06-12 20:21:27 +0800 |
| Commit | `8e475dd` |
| Review evidence | 2026-06-12 20:20:16 +0800 Review：修复 focused `unsupported` filter 未覆盖 unknown root fallback 的测试命名；确认 registry 覆盖 P1 deferred/unsupported-by-design 代表 API，已支持 `wx.login` / `wx.checkSession` / `wx.request` 未被覆盖，Proxy intrinsic 仅供内部 fallback 且 `globalThis.Proxy` 被隐藏；真实 storage/device/payment provider 状态未误标为 supported。 |
| Verification evidence | `cargo fmt --check` 通过；`cargo test -p js-runtime-quickjs unsupported` 5 passed；`cargo test -p wx-compat unsupported` 4 passed；`cargo test -p js-runtime-quickjs wx_` 13 passed；`cargo test -p js-runtime-quickjs` 33 passed；`cargo test -p wx-compat` 11 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/wx-compat crates/js-runtime-quickjs docs/architecture docs/runbook docs/plan` 无输出；敏感词抽样仅命中安全文档说明、测试夹具假值和 redaction 断言，unsupported result 不回显 options、token-like 字段或 Host 私有数据。 |
| Next action | 进入 Step 01-06 Storage JS Bridge |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：为所有暂不支持的 `wx.*` / `wx.modelContext` 能力建立 deterministic unsupported registry 和统一 fail shape。
- 用户 / 系统可见行为：Skill 调用未支持 API 时得到稳定的 `<api>:fail unsupported`，不会出现 `undefined is not a function`、静默 no-op 或 demo mock 冒充 production provider。
- 非目标：不实现这些 API 的真实 provider；不把 `unsupported-by-design` 能力升级为 supported。
- 完成标准：API VM 中 registry 覆盖矩阵里的 P1 长尾和 unsupported-by-design 大类，callback / Promise / `errMsg` 行为符合 Step 01-01 契约。

## 3. 设计方法

- 设计边界：unsupported registry 是兼容性和安全边界，不是功能实现入口；它只能返回 safe reason / suggestion。
- 核心决策：registry 以 canonical API name 为 key，按 `unsupported`、`provider_unavailable`、`network_denied` 等错误码区分；未知 API 默认走 deterministic unsupported。
- 契约 / API / 数据流：Skill JS 调用 `wx.someUnsupportedApi(options)` -> JS wrapper -> `WxApiCall` -> UnsupportedBroker -> `WxApiOutcome` -> `fail` callback、`complete` callback、Promise reject。
- 兼容性：保留已支持的 `wx.login`、`wx.checkSession`、`wx.request`、`wx.modelContext.*` 行为；只补齐缺失函数和稳定失败。
- 风险控制：返回内容不得包含 token、Authorization、DID proof、private key path、手机号、地址、文件内容或用户原始参数。

## 4. 实现方法

1. 阅读 `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` 的 `planned-p1`、`planned-p2`、`unsupported-by-design` 和大类 unsupported 覆盖。
2. 阅读 `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md` 中 `WxApiOutcome`、callback / Promise 和 unsupported shape 冻结决策。
3. 在 `anp/anp-miniapp-dock/crates/wx-compat` 定义或补齐 unsupported registry、reason / suggestion 数据和错误码枚举，避免在 JS wrapper 中散落硬编码。
4. 在 `anp/anp-miniapp-dock/crates/js-runtime-quickjs` 注入 registry 中的 API 函数，保持已支持 API 由专用 broker 处理。
5. 增加 VM tests：unsupported API 存在、Promise reject、`fail` / `complete` 顺序、结果 shape 一致、callback exception 不改变原始 outcome、未知 API deterministic failure。
6. 更新 `wx-api-compatibility-matrix.md`、release gates 或 bridge contract 中与 unsupported 覆盖状态相关的记录。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/wx-compat` | unsupported registry、错误码、reason / suggestion helper | 代码实现 |
| `anp/anp-miniapp-dock/crates/js-runtime-quickjs` | `wx.*` unsupported stub 注入和 wrapper 测试 | 代码实现 |
| `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` | 同步 unsupported 覆盖状态和证据 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | 将 unsupported stub 覆盖从 planned gap 调整为本 Step 证据 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/01-05-unsupported-api-registry-fail-shape.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 01-01、Step 01-04。
- 外部文档或决策：wx API Bridge Contract、wx API 兼容矩阵、Threat Model、Release Gates。
- 环境前提：Rust toolchain 1.88.0；无需外部 Host provider。

## 7. 验收标准

- [x] 矩阵中明确 unsupported / deferred 的 API 在 Atomic API VM 中都有 deterministic stub 或明确 registry fallback。
- [x] 未支持 API 调用不会出现 `undefined is not a function`、静默成功或 no-op 成功。
- [x] async unsupported API 调用 `fail` -> `complete`，Promise reject，callback 与 Promise value 使用同一脱敏 result shape。
- [x] sync unsupported API 抛出带脱敏 `errMsg` / `code` 的 `Error`，不接受 callback。
- [x] `reason` / `suggestion` 为 safe 文案，不回显敏感参数或 Host 私有数据。
- [x] 已支持 API 的行为和 tests 不回归。
- [x] `wx-api-compatibility-matrix.md` 和 release gate 记录与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Focused VM tests | `cd anp/anp-miniapp-dock && cargo test -p js-runtime-quickjs unsupported` | unsupported stub、callback / Promise、sync failure 测试通过；若 filter 不匹配，记录实际命令 |
| Compat tests | `cd anp/anp-miniapp-dock && cargo test -p wx-compat unsupported` | registry / shape 测试通过 |
| 回归 | `cd anp/anp-miniapp-dock && cargo test -p js-runtime-quickjs wx_` | 已支持 `wx.*` 相关回归通过；若 filter 不匹配，记录实际命令 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/wx-compat crates/js-runtime-quickjs docs/architecture docs/runbook docs/plan` | 无空白错误 |
| 安全抽样 | 手工检查 unsupported result、test output 和 CLI JSON | 不含 token、Authorization、signature、private key path、手机号、地址、文件内容 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：registry 覆盖是否来自矩阵；unsupported shape 是否与 Step 01-01 一致；未知 API 是否 fail closed；已支持 API 是否被误覆盖；错误文案是否泄露参数。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已发现并修复一个测试覆盖表达问题 | `cargo test -p js-runtime-quickjs unsupported` 起初未命中 unknown root fallback 测试，因为测试名不含 `unsupported`。 |
| 已修复问题 | 已修复 | 将测试改名为 `unsupported_unknown_root_wx_api_fails_deterministically`，重新运行 focused 命令后 5 passed；同时确认 `Proxy` intrinsic 只供内部 `wx` fallback 使用，`globalThis.Proxy` 已隐藏并有 sandbox globals 测试。 |
| 剩余风险 | 已记录 | registry 覆盖代表性 P1 deferred、planned-p2 和 unsupported-by-design API；真实 storage/device/payment/privacy/location/media provider 仍按 Step 01-06、01-07、01-08 和后续 Phase 推进，不在本 Step 误标 supported。 |
| 新增或缺失测试 | 已新增 | 新增 `wx-compat` registry / fail shape / unknown safe fallback tests；新增 Atomic API VM tests 覆盖 async unsupported callback/Promise、sync throw、nested `wx.cloud.*`、unknown root fallback、已支持 API 不被覆盖和 Proxy 隐藏。 |
| 已更新或缺失文档 | 已更新 | 已同步 `wx-api-compatibility-matrix.md`、`release-gates.md`、`phase-1-wx-api-bridge-contract.md`、`phase-1-wx-capability-broker.md`；未新增开发者文档，原因是本 Step 是 runtime fail-closed 能力，外部迁移文档留到 Phase 5。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 unsupported registry、stub 注入、直接 tests 和相关文档。
- Commit 前状态：记录 `git status --short`。
- 纳入文件：记录本步骤 commit 包含的文件。
- Commit 后证据：记录 commit hash 和 commit 后 `git status --short --branch`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`phase1: add unsupported api registry`

执行记录：

- Commit 前状态：`git status --short --branch` 显示仅本 Step 代码、测试、矩阵、release gates、Phase 1 文档、Step 文档和 roadmap 变更。
- 纳入文件：`crates/wx-compat/src/unsupported.rs`、`crates/wx-compat/src/lib.rs`、`crates/wx-compat/src/request.rs`、`crates/wx-compat/tests/component_permissions.rs`、`crates/js-runtime-quickjs/src/api_vm.rs`、`crates/js-runtime-quickjs/src/bridge.rs`、`crates/js-runtime-quickjs/tests/middleware_chain.rs`、`docs/architecture/wx-api-compatibility-matrix.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md`、`docs/plan/production-readiness/phase-1-wx-capability-broker.md`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/steps/01-05-unsupported-api-registry-fail-shape.md`。
- Commit：`8e475dd phase1: add unsupported api registry`。
- Commit 后证据：`git status --short --branch` = `## main...origin/main [ahead 23]`。
- 遗留未提交变更：仅本 Step commit 后回填台账/状态文档，随后创建独立关闭提交。

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 01-05 小 Plan | 将 Phase 1 unsupported stub 覆盖拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：registry 覆盖过宽可能遮蔽真实实现，覆盖过窄会继续暴露 undefined 行为。
- 回滚 / 回退：保留 registry 与已支持 broker 的优先级测试；如某 API 后续实现，先更新 registry 状态和矩阵再迁移到专用 broker。
- 后续文档：Phase 5 可引入自动 API coverage checker，避免矩阵和 registry 漂移。
