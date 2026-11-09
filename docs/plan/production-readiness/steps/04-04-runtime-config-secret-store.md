# Step 04-04：Runtime Config 与 Secret Store 边界

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：04-04
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 19:33:40 +0800 |
| Completed | 2026-06-13 19:49:21 +0800 |
| Commit | `189ad87` |
| Review evidence | 2026-06-13 19:47:25 +0800 commit 前 Review 已记录：修复 diagnostics 未脱敏 `issuer` 中 `merchant secret` 文本、`schemaVersion` 可回显异常敏感串、`cargo test -p dock-core config` 初始只命中 2 个新增测试的问题；确认本 Step 只冻结 config/secret contract，不实现真实 secret resolve 或 token/storage/audit/cache 持久化 backend |
| Verification evidence | 启动前 `git status --short --branch` = `## main...origin/main [ahead 68]`；`cargo fmt --check` 通过；`cargo test -p dock-core config` 8 passed；`cargo test -p dock-core` 27 passed；`cargo test -p dock-cli config` 2 passed under filter，CLI 未新增参数，仅复用既有 credential redaction/config tests；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/dock-core crates/dock-cli crates/anp-adapter docs/runbook docs/security docs/plan` 无输出；敏感串扫描仅命中文档红线、redaction marker/test 假值和既有 redaction 回归测试；post-commit `git status --short --branch` = `## main...origin/main [ahead 69]` |
| Next action | 进入 04-05 Token Cache 持久化与恢复 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：定义 production runtime config schema、加载优先级、profile 语义和 Secret Store / Host credential provider 边界。
- 用户 / 系统可见行为：容器可以用同一套配置启动 CLI、Runtime API 和 Host sidecar；secret material 只通过 env、secret store 或 Host credential provider 注入，不进入普通 config、日志、CLI JSON 或 audit。
- 非目标：不绑定单一云厂商 secret store；不实现 token cache、scoped storage、audit sink 或 Skill cache 的持久化 backend。
- 完成标准：runtime config 有 schema、默认值、profile 校验、secret reference 规则、redaction 和 release blocker；token/storage/audit/cache 的实际持久化拆到 Step 04-05 至 04-08。

## 3. 设计方法

- 设计边界：本 Step 只冻结 config/secret contract 和 provider wiring，不承载具体持久化 backend。
- 核心决策：config 文件只允许 non-secret value 和 secret reference；真实 secret 通过 env、secret store 或 Host credential provider resolve；dev/mock provider 必须显式标记。
- 契约 / API / 数据流：config file/env/CLI args -> runtime config loader -> validation/redaction -> provider handles -> later persistence steps。
- 兼容性：保留当前 CLI/demo 默认值，但 production profile 缺少 required provider 时必须 fail closed 或产生 release blocker。
- 风险控制：private key material、capability token、Authorization、HTTP signature、merchant secret 和真实用户数据不得进入 config 文件或错误输出。

## 4. 实现方法

1. 阅读 Runtime API facade、Skill registry/cache、DID/session、RequestBroker、Consent/Audit 和 release gates 中的配置项。
2. 定义 runtime config model：profile、identity provider、trusted DID/resolver、network allowlist、token issuer reference、storage/audit/cache path reference、Host provider registry、log/observability level、mock provider flags。
3. 定义 secret reference 语义：env var、secret store key、Host credential provider handle；禁止在 config 中内联 private key、token、Authorization、merchant secret 或 raw DID credential。
4. 实现或规划 config loader / validator / redactor，并让 CLI 和 Runtime API 共享同一套配置结构。
5. 增加 tests：缺失 required config、unknown field、dev/mock provider 标记、production profile release blocker、secret value redaction、path redaction。
6. 更新 runbook、Threat Model、Release Gates 和 Phase 4 文档，说明 Step 04-05 至 04-08 分别实现具体持久化 backend。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/dock-core` | runtime config model、loader、validator、redactor、provider handles | 代码实现 |
| `anp/anp-miniapp-dock/crates/dock-cli` | CLI config 参数和 redacted config diagnostics | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/anp-adapter` | identity/token issuer secret reference boundary | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/runbook/local-demo.md` | 同步 config/profile 用法 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | production config/secret gate | 必须 |
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 同步 secret/config 风险 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-4-runtime-host-integration.md` | 同步 config 与后续持久化拆分 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/04-04-runtime-config-secret-store.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 04-01。
- 外部文档或决策：Threat Model、Release Gates、Runtime API facade。
- 环境前提：Rust toolchain 1.88.0；生产 secret store 可用 trait + Host boundary 先行。

## 7. 验收标准

- [x] runtime config schema 覆盖 identity、resolver、allowlist、token issuer reference、storage/audit/cache path reference、Host providers、profile、mock provider flags 和 observability level。
- [x] config loader 有校验、默认值、unknown field 处理和 profile-specific release blocker。
- [x] config 文件不包含 secret；secret reference 只能指向 env、secret store key 或 Host credential provider handle。
- [x] private key material、raw token、Authorization、signature、merchant secret 和真实用户数据在 logs/CLI/audit/config diagnostics 中 redacted。
- [x] Release Gates 明确 production profile 缺少 required secret/provider 时 fail closed 或 release blocked。
- [x] Step 04-05 至 04-08 的持久化 backend scope 与本 Step 的 config/provider handle 对齐。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Config tests | `cd anp/anp-miniapp-dock && cargo test -p dock-core config` | config loader/validator/redaction tests 通过；若 filter 不匹配，记录实际命令 |
| CLI/config diagnostics | `cd anp/anp-miniapp-dock && cargo test -p dock-cli config` | redacted diagnostics 或配置参数 tests 通过；若未触及 CLI，记录原因 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/dock-core crates/dock-cli crates/anp-adapter docs/runbook docs/security docs/plan` | 无空白错误 |
| 敏感信息扫描 | 手工或 `rg` 检查 config/log/test output | 不含 private key material、raw token、Authorization、signature 或真实 secret |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：secret 是否不进 config；production profile 是否 fail closed；mock/dev provider 是否被显式标记；redaction 是否覆盖 CLI/log/audit/config diagnostics。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已发现并修复 | 1. `redacted_diagnostics()` 最初对 `tokenIssuer.issuer` 中的 `merchant secret` 文本未脱敏；2. `schemaVersion` 来自配置文件，异常敏感串不应原样出现在 diagnostics；3. `cargo test -p dock-core config` 初始只命中 2 个新增测试，过滤范围不够强。 |
| 已修复问题 | 已修复 | `redact_runtime_config_text()` 增加 `secret` marker；diagnostics 对 `schemaVersion` 也走 redaction；新增测试命名统一 `runtime_config_*`，并补充结构化 secret reference JSON 反序列化测试。 |
| 剩余风险 | 已记录 | 本 Step 只冻结 schema、validator、redactor、provider/path/secret reference 和 release blockers；不实现 config file/env/CLI merge、真实 secret resolve、token cache persistence、scoped storage persistence、audit backend 配置/retention/export、Skill cache cleanup 或 Host provider conformance。 |
| 新增或缺失测试 | 已新增 focused tests | 新增 `crates/dock-core/tests/runtime_config.rs` 覆盖默认值、unknown field、结构化 secret reference、production release blockers、secret/path diagnostics redaction、mock/dev-only production blocker。未新增 CLI 参数，因此 `cargo test -p dock-cli config` 仅作为既有 CLI credential redaction/config 替代证据。 |
| 已更新或缺失文档 | 已更新 | 已更新 Phase 4 文档、Release Gates、Threat Model、主 Plan 台账和本 Step 文档；未更新 `local-demo.md`，因为本 Step 没有新增 CLI/config 文件使用方式。 |

### 9.1 Review 记录

| 时间 | 范围 | 发现 / 处理 | 结论 |
|---|---|---|---|
| 2026-06-13 19:47:25 +0800 | `dock-core` runtime config schema/validator/redactor/tests，Phase 4 文档、Release Gates、Threat Model、执行台账 | 修复 diagnostics redaction 与测试过滤覆盖问题；确认 production profile blocker 覆盖 identity/resolver/allowlist/token issuer/storage/audit/cache/render/consent/mock/dev-only；确认文档没有把真实 secret resolve、持久化 backend 或 Host 能力写成已完成 | 无阻塞问题；可以创建 focused commit |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 runtime config、secret boundary、direct tests 和相关文档。
- Commit 前状态：记录 `git status --short`。
- 纳入文件：记录本步骤 commit 包含的文件。
- Commit 后证据：记录 commit hash 和 commit 后 `git status --short --branch`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`phase4: add runtime config secret boundary`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 04-04 初版小 Plan | 最初覆盖过宽的配置与持久化边界，后续按 Review 拆分 | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |
| 2026-06-12 | 收窄 Step 04-04 范围 | 按 Review 发现将持久化拆成 focused slices，本 Step 只保留 runtime config 与 secret boundary | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：config 为了便捷而接受 inline secret，会造成长期凭据泄露。
- 回滚 / 回退：缺少 production secret provider 时 fail closed 或标为 release blocker；dev/mock profile 只能用于测试和 demo。
- 后续文档：Step 04-05 至 04-08 分别实现 token、storage、audit 和 Skill cache 持久化；Phase 6 runbook 必须引用本配置边界。
