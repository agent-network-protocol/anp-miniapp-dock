# Step 05-06：示例 Skill 与兼容测试集

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：05-06
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-14 02:59:58 +0800 |
| Completed | 2026-06-14 03:16:21 +0800 |
| Commit | `f3d97cc` |
| Review evidence | 2026-06-14 03:14:20 +0800 commit 前 Review 已记录：确认 05-06 复用既有 `examples/fixtures/*`，避免重复创建 `examples/address-skill` 等包；修复本地 fixture 在 `validate` / `inspect` 中缺少 manifest `id` 时回退为默认 `coffee` 的报告问题，同时保留 coffee fixture 形状继续输出 `coffee`；确认 expected JSON 只记录稳定摘要，不包含易变 audit 时间戳、本机路径、token、Authorization、Signature、fixture-token、真实手机号、地址、文件内容或经纬度；确认 headless provider 仍标 `productionReady = false`，未将 Host provider 或动态网络能力写成 production-ready。 |
| Verification evidence | 启动前 `git status --short --branch` = `## main...origin/main [ahead 96]`；已读取主 Plan、Step 05-06 文档、Phase 5 文档、现有 `examples/fixtures/*` 和 `testdata/render-ir/*`；确认 05-05 implementation commit `9d19744` 与 closure commit `56daf6f`；`cargo fmt --check` 通过；`cargo test -p dock-cli example` 1 unit + 1 integration passed；`cargo test -p dock-cli validate` 4 unit + 2 integration passed；`cargo test -p dock-cli inspect` 2 unit + 1 integration passed；手工 `validate` / `test-skill` 覆盖 address-form、media-review、dynamic-status、location-map-preview，全部 JSON parse 通过，`validate` 输出 `dock.validate-report.v1`、示例 skillId、`warning`、`commandStatus = ok`，`test-skill` 输出 `dock.test-skill-report.v1`、`status = ok`、`failed = 0`、snapshot `match`；`git diff --check -- examples testdata crates/dock-cli docs/architecture docs/runbook docs/plan README.md` 无输出；fixture JSON 样本敏感串扫描未命中 `/home/`、Authorization、Signature、capabilityToken、fixture-token、Bearer、private key material、PEM header、latitude 或 longitude；计划要求的 `rg -n "token|Authorization|signature|private key|phone|address|latitude|longitude" examples testdata docs/plan docs/architecture docs/runbook README.md` 仅命中 mock handle、示例命令、文档红线、测试假值和既有安全说明；`cargo test -p dock-cli --test coffee_order_flow` 12 passed；`cargo clippy -p dock-cli --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；implementation commit `f3d97cc phase5: add compatibility example skills` 后 `git status --short --branch` = `## main...origin/main [ahead 97]`。 |
| Next action | 进入 05-07 开发者文档与迁移指南 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：在 coffee 之外补齐 address、media、dynamic-status、location 等示例 Skill / fixture，并与 validate/test-skill/snapshots 绑定。
- 用户 / 系统可见行为：开发者可以复制示例学习表单、地址/手机号 consent、media/file handle、dynamic component、location map preview。
- 非目标：不使用真实手机号、地址、文件、位置或支付；示例必须 mock-only。
- 完成标准：至少 3 个 coffee 之外示例可跑，每个有 README、run command、expected JSON、Render IR snapshot 和风险说明。

## 3. 设计方法

- 设计边界：示例是开发者教育和兼容回归，不是生产商家数据。
- 核心决策：复用 Step 02-06 fixtures；若 fixtures 已存在，本 Step 强化 README、CLI run commands、expected output 和开发者可读性。
- 契约 / API / 数据流：example Skill -> validate -> test-skill -> snapshot/audit -> README evidence。
- 兼容性：coffee 继续作为交易基线；新增示例覆盖 Phase 1/2/3/4 能力。
- 风险控制：所有示例 DID、手机号、地址、文件、位置、token 都是 mock/redacted。

## 4. 实现方法

1. 阅读 Step 02-06 fixture 输出、Phase 5 示例计划和 developer docs 计划。
2. 整理 `examples/fixtures/address-form`、`examples/fixtures/media-review`、`examples/fixtures/dynamic-status`、`examples/fixtures/location-map-preview`，避免与 Step 02-06 已完成的 fixture 重复建包。
3. 每个示例补 README、expected JSON，并确认既有 `SKILL.md`、`mcp.json`、API JS、components 和 Render IR snapshot 可作为开发者证据。
4. 将示例接入 `dock-cli validate` 和 `test-skill` regression。
5. 增加 tests：每个示例 validate/test-skill 通过或输出 expected planned gap。
6. 更新 README、Phase 5 文档、Release Gates 和兼容矩阵证据。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/examples/fixtures/address-form` | 整理 address/form 示例 README 和 expected JSON | 复用既有 fixture |
| `anp/anp-miniapp-dock/examples/fixtures/media-review` | 整理 image/file/media 示例 README 和 expected JSON | 复用既有 fixture |
| `anp/anp-miniapp-dock/examples/fixtures/dynamic-status` | 整理 dynamic component 示例 README 和 expected JSON | 复用既有 fixture |
| `anp/anp-miniapp-dock/examples/fixtures/location-map-preview` | 整理 location/map-preview 示例 README 和 expected JSON | 复用既有 fixture |
| `anp/anp-miniapp-dock/testdata/render-ir` | 示例 snapshots | 视 Step 02-06 结构修改 |
| `anp/anp-miniapp-dock/crates/dock-cli/tests` | 示例 validate/test-skill regression | 必须 |
| `anp/anp-miniapp-dock/docs/architecture` | 同步示例证据 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | 示例 gate | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-5-developer-experience.md` | 同步示例列表 | 必须 |
| `anp/anp-miniapp-dock/README.md` | 示例入口 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/05-06-example-skills-compatibility-fixtures.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 02-06、Step 05-01、Step 05-03。
- 外部文档或决策：fixture/snapshot rules、API/组件矩阵、Threat Model。
- 环境前提：Rust toolchain 1.88.0；示例必须 mock-only。

## 7. 验收标准

- [ ] coffee 之外至少 3 个示例 Skill 可 validate/test-skill。
- [ ] 每个示例有 README、run command、expected JSON、Render IR snapshot、风险说明。
- [ ] 示例覆盖表单/地址/手机号、media/file、dynamic component、location/map preview 中至少 3 类。
- [ ] 示例不包含真实手机号、地址、文件内容、位置、token、private key material。
- [ ] Release Gates 和 README 将示例纳入开发者验证路径。
- [ ] Review 发现已经修复或明确记录。
- [ ] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Example tests | `cd anp/anp-miniapp-dock && cargo test -p dock-cli example` | 示例 validate/test-skill tests 通过；若 filter 不匹配，记录实际命令 |
| Manual validate | `cd anp/anp-miniapp-dock && cargo run -p dock-cli -- validate examples/fixtures/address-form` | 示例输出 expected report；其它示例同理或记录 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- examples testdata crates/dock-cli docs/architecture docs/runbook docs/plan README.md` | 无空白错误 |
| 敏感信息扫描 | `cd anp/anp-miniapp-dock && rg -n "token|Authorization|signature|private key|phone|address|latitude|longitude" examples testdata docs/plan docs/architecture docs/runbook README.md` | 只命中 mock/dev-only 示例、redaction 规则或安全说明 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：示例、tests、文档同步完成后、commit 前。
- Review 重点：示例是否真正可跑；是否覆盖核心能力；mock/dev-only 是否清晰；是否含敏感数据。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已发现并修复 | 1. 原计划仍指向新增 `examples/address-skill` 等目录，实际 Step 02-06 已有 `examples/fixtures/*`；2. `validate` / `inspect` 对未声明 manifest `id` 的本地 fixture 会输出默认 `coffee`，导致开发者示例报告失真；3. 初次将 `inspect` 改为路径 fallback 时破坏 coffee 既有 `skillId = coffee` 契约，已收敛为 coffee 形状优先。 |
| 已修复问题 | 已修复 | Step 文档、Phase 5 文档和 release gate 改为复用 `examples/fixtures/*`；`validate` / `inspect` 使用 `skill_id_for_path`，并对 coffee fixture 形状保留默认 `coffee`；新增 expected JSON 和集成测试固定四个 fixture 的 schema、skillId、fixtureSet、snapshot、audit boundary 和 redaction。 |
| 剩余风险 | 已记录 | expected JSON 是稳定摘要，不是完整 CLI report snapshot；真实 Host provider、production renderer、dynamic production transport、audit persistence 和 release automation 仍由 Phase 4/6 gate 负责，示例不能作为 production-ready 证明。 |
| 新增或缺失测试 | 已补充 | 新增 `example_compatibility_fixtures_validate_and_test_skill`，覆盖 4 个 fixture 的 README、expected JSON、validate/test-skill、snapshot existence、snapshot match 和敏感串禁用；缺真实 Host provider E2E，因本 Step 是 mock-only developer fixture。 |
| 已更新或缺失文档 | 已同步 | 新增四个 fixture README / expected JSON；更新 README、local demo runbook、release gates、Phase 5 文档、component compatibility matrix、Step 文档和主 Plan 台账；无额外缺失文档。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含示例 Skill、fixtures/snapshots、direct tests 和相关文档。
- Commit 前状态：`git status --short` 显示 05-06 范围内的 README、`crates/dock-cli`、`examples/fixtures/*/README.md`、`expected-test-skill.json`、component matrix、runbook、Phase 5 文档和 Plan 台账变更。
- 纳入文件：`README.md`、`crates/dock-cli/src/commands.rs`、`crates/dock-cli/tests/coffee_order_flow.rs`、`examples/fixtures/*/README.md`、`examples/fixtures/*/expected-test-skill.json`、`docs/architecture/component-compatibility-matrix.md`、`docs/runbook/local-demo.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/phase-5-developer-experience.md`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/steps/05-06-example-skills-compatibility-fixtures.md`。
- Commit 后证据：implementation commit `f3d97cc phase5: add compatibility example skills`；commit 后 `git status --short --branch` = `## main...origin/main [ahead 97]`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`phase5: add compatibility example skills`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 05-06 小 Plan | 将示例 Skill 与兼容测试集拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |
| 2026-06-14 | 复用既有 compatibility fixtures 作为示例体系 | Step 02-06 已创建 address-form、media-review、dynamic-status、location-map-preview Skill packages 和 golden snapshots；本 Step 聚焦 README、expected JSON、回归测试和开发者入口，避免创建重复示例包 | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：示例如果不可跑，会误导开发者和 release gates。
- 回滚 / 回退：示例必须由 CLI tests 覆盖；不能跑的示例标为 planned，不进入 release gate。
- 后续文档：Step 05-07 迁移指南应引用这些示例。
