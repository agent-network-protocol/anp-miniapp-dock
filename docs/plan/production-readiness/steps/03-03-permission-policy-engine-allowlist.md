# Step 03-03：权限策略引擎与 allowlist decision

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：03-03
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 14:12:53 +0800 |
| Completed | 2026-06-13 14:48:46 +0800 |
| Commit | `32ada09` |
| Review evidence | 2026-06-13 14:38:45 +0800 commit 前 Review 已记录：修复 Host allow 可绕过未声明敏感权限、通用 boolean permission 可能误声明任意 capability 的问题；确认 Host deny override 优先、mock 仅 dev/headless、Prompt 进入 ConsentGate、decision audit 脱敏、allowlist mismatch 在 transport 前失败。 |
| Verification evidence | `cargo fmt --check` 通过；`cargo test -p wx-compat permission` 7 passed；`cargo test -p dock-core permission` 2 passed；`cargo test -p anp-adapter allowlist` 5 passed；`cargo test -p wx-compat` 29 passed；`cargo test -p anp-adapter` 44 passed；`cargo test -p dock-core` 11 passed；`cargo test -p js-runtime-quickjs wx_request` 4 passed；`cargo test -p component-runtime dynamic` 7 passed；`cargo test -p dock-cli --test coffee_order_flow` 4 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/wx-compat crates/dock-core crates/anp-adapter crates/consent-audit crates/js-runtime-quickjs crates/component-runtime crates/dock-cli docs/architecture docs/security docs/runbook docs/plan` 无输出；敏感词抽样仅命中测试假值、文档安全说明和 `AuthMode::HttpSignatures` 常量。 |
| Next action | 进入 Step 03-04 DID / Token 生命周期与 Resolver 信任锚 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：建立统一 permission policy engine，覆盖 `mcp.json`、`_meta.anp` / `x_anp`、component dynamic scope、Host override、mock/dev-only 和网络 allowlist。
- 用户 / 系统可见行为：每次敏感 capability 调用都有 `Allow`、`Deny`、`Prompt` 或 `MockAllowed(dev_only)` 的决策记录和审计摘要。
- 非目标：不实现真实 Host provider UI；不替代 Step 03-05 的 consent adapter。
- 完成标准：未声明敏感权限默认 deny，mock provider 必须显式 dev/headless flag，网络 allowlist 支持 scheme/host/port/path/method/scope。

## 3. 设计方法

- 设计边界：permission decision 在 provider/request/executor 前执行，不能由 Skill JS 绕过。
- 核心决策：策略输入统一来自 manifest、Host policy、用户 consent、merchant trust policy 和 runtime profile；策略输出必须可审计。
- 契约 / API / 数据流：Capability call -> PermissionContext -> PolicyEngine -> Decision -> ConsentGate/provider/broker -> Audit summary。
- 兼容性：保留现有 component capability profile 和 RequestBroker allowlist 行为；向统一决策层收敛。
- 风险控制：policy override 不允许把 production 禁止项静默放开；mock 只能 dev/headless 且 release gate 阻断 production。

## 4. 实现方法

1. 阅读 `wx-compat` permissions、RequestBroker allowlist、`dock-core` orchestrator consent enforcement 和 manifest validation。
2. 设计 `PermissionDecision`：`Allow`、`Deny(reason)`、`Prompt(consent_request)`、`MockAllowed(dev_only)`，并定义 stable reason code。
3. 实现或收敛 policy engine 输入：API name、risk level、manifest permissions、component path、dynamic scope、URL/method/scope、Host override、runtime profile。
4. 网络 allowlist 支持 scheme、host、port、path prefix、method、scope；默认 deny。
5. 增加 tests：未声明敏感权限 deny、Host override deny 优先、mock dev-only、allowlist path/method/scope、permission decision audit。
6. 更新 API/组件矩阵、threat model、release gates 和 Phase 3 文档。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/wx-compat` | permission profile、policy decision、allowlist matching | 代码实现 |
| `anp/anp-miniapp-dock/crates/dock-core` | Orchestrator permission enforcement order | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/anp-adapter` | RequestBroker allowlist scope/path/method tests | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/consent-audit` | decision audit summary | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` | 同步 permission/allowlist 状态 | 必须 |
| `anp/anp-miniapp-dock/docs/architecture/component-compatibility-matrix.md` | 同步 dynamic/component permission 状态 | 必须 |
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 同步 policy engine 控制 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | 同步 permission gate | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/03-03-permission-policy-engine-allowlist.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 01-08、Step 02-05、Step 03-01。
- 外部文档或决策：Threat Model、API/组件矩阵、Release Gates。
- 环境前提：Rust toolchain 1.88.0；真实 Host policy UI 可后置。

## 7. 验收标准

- [x] 所有敏感 capability 都经过统一 permission decision。
- [x] 未声明权限默认 deny；Host deny override 优先于 Skill manifest。
- [x] Mock provider 只能在显式 dev/headless profile 下返回 `MockAllowed(dev_only)`，production release gate 阻断。
- [x] 网络 allowlist 支持 scheme、host、port、path prefix、method、scope，并有 deny-by-default 测试。
- [x] Permission decision 进入脱敏 audit summary。
- [x] 相关矩阵、Threat Model、Release Gates 与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Permission tests | `cd anp/anp-miniapp-dock && cargo test -p wx-compat permission` | policy decision/profile tests 通过 |
| Core enforcement tests | `cd anp/anp-miniapp-dock && cargo test -p dock-core permission` | Orchestrator enforcement tests 通过；若 filter 不匹配，记录实际命令 |
| Request allowlist tests | `cd anp/anp-miniapp-dock && cargo test -p anp-adapter allowlist` | allowlist path/method/scope tests 通过 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/wx-compat crates/dock-core crates/anp-adapter crates/consent-audit docs/architecture docs/security docs/runbook docs/plan` | 无空白错误 |
| 安全抽样 | 手工检查 audit/CLI/error output | 不含 token、Authorization、signature、private key path 或隐私原文 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：enforcement order 是否在 provider 前；allowlist 是否默认 deny；mock 是否无法进 production；decision reason 是否稳定且脱敏。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已记录 | 1. 初版 Host allow override 可能把未声明敏感 capability 放行，不满足“未声明默认 deny”；2. 初版 JSON permission parser 把通用 boolean 当作任意 capability 声明，可能扩大权限；3. 需要确认新增 `Prompt` / `MockAllowed` 分支不会落到真实 RequestBroker 出站。 |
| 已修复问题 | 已修复 | Host allow 改为只能在 manifest/meta/dynamic 已声明后生效；普通 boolean 不再声明任意敏感 capability，dynamic scope 仍单独支持 boolean；真实 ANP/本地 DID RequestBroker 对 `MockAllowed` 返回 unsupported，对 `Prompt` 返回 denied，不会静默出站。 |
| 剩余风险 | 已记录 | 生产 Host policy UI/config、生产 RequestBroker transport、registry 配置来源、provider conformance、persistent request/audit 仍在 Phase 4/Step 03-05；本 Step 只完成本地 required gate 和可审计 decision。 |
| 新增或缺失测试 | 已新增 | 新增/扩展 `wx-compat` permission tests、`anp-adapter` allowlist tests、`dock-core` permission audit tests；缺失的生产 Host UI/config 和 persistent audit tests 按后续 Step 记录。 |
| 已更新或缺失文档 | 已更新 | 已同步 `docs/architecture/wx-api-compatibility-matrix.md`、`docs/architecture/component-compatibility-matrix.md`、`docs/security/threat-model.md`、`docs/runbook/release-gates.md`、Phase 3 文档和本 Step 文档。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 permission policy engine、allowlist、直接 tests 和相关文档。
- Commit 前状态：记录 `git status --short`。
- 纳入文件：记录本步骤 commit 包含的文件。
- Commit 后证据：记录 commit hash 和 commit 后 `git status --short --branch`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`phase3: add permission policy engine`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 03-03 小 Plan | 将权限策略与 allowlist decision 拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：policy 规则过于分散会导致 provider 绕过或 CLI/Host 行为不一致。
- 回滚 / 回退：发现不一致时优先 deny；Host override 必须可审计。
- 后续文档：Phase 4 Host adapter contract 必须实现同一 policy decision 接口或声明 unsupported。
