# Step 01-07：Device/App Info Atomic API

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：01-07
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 20:45:17 +0800 |
| Completed | 2026-06-12 20:53:52 +0800 |
| Commit | `50cc245` |
| Review evidence | 本文 Review 环节已记录：未发现阻塞问题；确认 Atomic API 与 Component VM 使用 `wx-compat` shared defaults，字段最小化，不返回真实设备指纹或 Host credential 信息，sync API 不返回 Promise。 |
| Verification evidence | `cargo fmt --check` 通过；`cargo test -p js-runtime-quickjs info` 2 passed；`cargo test -p wx-compat device` 1 passed；`cargo test -p component-runtime` 26 passed；`git diff --check -- crates/wx-compat crates/js-runtime-quickjs crates/component-runtime docs/architecture docs/plan docs/runbook` 无输出；`cargo clippy --workspace --all-targets -- -D warnings` 通过；敏感字段抽样仅命中文档红线、测试假值、禁用字段断言和既有 redaction 代码。 |
| Next action | 进入 Step 01-08 高风险 API Host Boundary 与 fail-closed |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：在 Atomic API VM 中补齐 `wx.getDeviceInfo()` 和 `wx.getAppBaseInfo()` 的最小实现。
- 用户 / 系统可见行为：Skill 原子接口可读取容器和 Host 提供的最小 app/runtime/device snapshot，且不暴露真实设备指纹。
- 非目标：不复刻微信客户端完整环境；不实现 `wx.getAccountInfoSync`、网络状态监听或完整设备传感器 API。
- 完成标准：Atomic API 与 Component VM 的 device/app info 字段口径一致，字段最小化，默认 headless fallback deterministic。

## 3. 设计方法

- 设计边界：device/app info 只能暴露低风险、最小化、非指纹化字段；真实 Host 可覆盖但必须受字段 allowlist 限制。
- 核心决策：优先复用 `wx-compat` 现有 device/app info helper，Atomic API VM 和 Component VM 保持同一结构来源或有防漂移测试。
- 契约 / API / 数据流：Atomic API JS 调用 sync API -> runtime snapshot provider -> 返回脱敏 object；provider 缺失时返回 deterministic headless/container 默认值。
- 兼容性：按 Step 01-01 sync API 规则，不使用 callback，不返回 Promise，失败抛出脱敏 Error。
- 风险控制：不得返回真实 device id、local IP、MAC、广告标识、文件路径、Host 用户账号或精细地理信息。

## 4. 实现方法

1. 阅读 `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` 中 device/app info 状态。
2. 阅读 `anp/anp-miniapp-dock/crates/wx-compat` 和 `anp/anp-miniapp-dock/crates/component-runtime` 中 Component VM 已支持的 device/app info helper。
3. 在 `wx-compat` 中冻结 Atomic API 和 Component VM 共用字段结构、默认值和 redaction 规则。
4. 在 `js-runtime-quickjs` 注入 `wx.getDeviceInfo()` 和 `wx.getAppBaseInfo()` 同步 API。
5. 增加 tests：字段最小化、headless fallback、Component VM 与 Atomic API 防漂移、callback 被忽略或 invalid、敏感字段不出现。
6. 更新 API 矩阵和必要 runbook。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/wx-compat` | device/app info schema、snapshot helper、tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/js-runtime-quickjs` | Atomic API VM sync bridge 和 tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/component-runtime` | 如需共用 helper 或防漂移测试 | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` | 同步 Atomic API device/app info 状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/01-07-device-app-info-atomic-api.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 01-01。
- 外部文档或决策：wx API Bridge Contract、wx API 兼容矩阵、Threat Model。
- 环境前提：Rust toolchain 1.88.0；无需外部 Host provider。
- 独立性说明：本 Step 不依赖 Step 01-06 storage bridge 的实现。若 Step 01-06 被标记为 `blocked`，执行者可以在主 Plan Blocked 记录中说明原因、确认无未提交完成工作后，串行转入本 Step；这不授权并行执行。

## 7. 验收标准

- [x] Atomic API VM 支持 `wx.getDeviceInfo()` 和 `wx.getAppBaseInfo()` 同步调用。
- [x] 返回字段最小化且 deterministic，缺省 headless 环境不返回真实设备指纹。
- [x] Atomic API 与 Component VM 的 runtime/app info 字段不漂移，已有测试或共用 helper。
- [x] 不返回 local IP、MAC、device id、广告标识、Host 账号、credential path 或私钥路径。
- [x] API 矩阵与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Focused VM tests | `cd anp/anp-miniapp-dock && cargo test -p js-runtime-quickjs info` | 2 passed，覆盖 Atomic API device/app info 同步返回、冻结对象和 unsupported registry 不覆盖 |
| Compat tests | `cd anp/anp-miniapp-dock && cargo test -p wx-compat device` | 1 passed，覆盖 shared defaults 最小字段和 forbidden field 缺失 |
| Component 回归 | `cd anp/anp-miniapp-dock && cargo test -p component-runtime` | 26 passed，包含组件 VM 与 shared defaults 防漂移测试 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/wx-compat crates/js-runtime-quickjs crates/component-runtime docs/architecture docs/plan docs/runbook` | 无输出 |
| 敏感字段抽样 | 手工检查返回 JSON 和测试 fixture | 敏感字段扫描仅命中文档红线、测试假值、禁用字段断言和既有 redaction 代码 |

补充验证：`cargo clippy --workspace --all-targets -- -D warnings` 通过。

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：字段是否过多；是否暴露 fingerprint；Atomic API 与 Component VM 是否漂移；sync API 是否误走 Promise/callback。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 未发现阻塞问题 | Review 覆盖字段最小化、指纹风险、Atomic/Component 防漂移和 sync API 行为。 |
| 已修复问题 | 修复组件 VM 与 Atomic API 默认 `model` 字段漂移；移除 `getDeviceInfo` / `getAppBaseInfo` 的 unsupported registry 条目；测试中改为验证冻结对象写入不生效而非依赖非 strict assignment 抛异常。 | focused tests 已覆盖。 |
| 剩余风险 | 真实 Host 可覆盖字段、provider policy 和 conformance tests 仍在 Phase 4；本 Step 仅提供 deterministic headless/runtime default snapshot。 | API 矩阵已记录 Phase 4 边界。 |
| 新增或缺失测试 | 新增 `wx-compat` shared defaults test、Atomic VM info tests 和 component-runtime 防漂移测试；未新增真实 Host provider test。 | Host provider 不属于 01-07。 |
| 已更新或缺失文档 | 已更新 API 兼容矩阵、release gates、本 Step 和主 Plan 台账。 | 未更新 Host adapter 文档，留到 Phase 4。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 device/app info Atomic API、直接 tests 和相关文档。
- Commit 前状态：`git status --short --branch` = `## main...origin/main [ahead 26]`，包含本 Step device/app info Atomic API、shared helper、直接 tests 和相关文档。
- 纳入文件：`crates/wx-compat/src/model_context.rs`、`crates/wx-compat/src/lib.rs`、`crates/wx-compat/src/unsupported.rs`、`crates/wx-compat/tests/component_permissions.rs`、`crates/js-runtime-quickjs/src/api_vm.rs`、`crates/js-runtime-quickjs/src/bridge.rs`、`crates/js-runtime-quickjs/tests/middleware_chain.rs`、`crates/component-runtime/src/component_vm.rs`、`crates/component-runtime/tests/component_lifecycle.rs`、`docs/architecture/wx-api-compatibility-matrix.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/steps/01-07-device-app-info-atomic-api.md`、`docs/plan/production-readiness-roadmap.md`。
- Commit 后证据：实现提交 `50cc245 phase1: add device app info atomic api`；提交后 `git status --short --branch` = `## main...origin/main [ahead 27]`，工作区无未提交变更。
- 遗留未提交变更：无。
- 建议消息：`phase1: add device app info atomic api`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 01-07 小 Plan | 将 Phase 1 device/app info Atomic API 拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |
| 2026-06-12 | 标注 01-07 独立性 | 按 Review 发现，避免 storage blocked 时错误阻塞独立 device/app info work | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：过度兼容微信字段可能变成设备指纹面。
- 回滚 / 回退：若字段口径不清，先保持 unsupported 或最小 headless snapshot，不引入真实 Host 字段。
- 后续文档：Phase 4 Host adapter contract 需要定义 Host 可覆盖字段和 conformance tests。
