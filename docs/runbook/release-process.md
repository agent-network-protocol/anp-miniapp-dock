# Release Process, Canary, And Rollback Runbook

> 状态：Step 06-05 本地可执行发布流程。本文定义 release candidate、canary、rollback 和 cache purge 的审计规则；不表示已经接入真实生产发布平台或真实 Host rollout UI。
> 上游计划：[`../plan/production-readiness-roadmap.md`](../plan/production-readiness-roadmap.md) Step 06-05。
> 相关门禁：[`release-gates.md`](release-gates.md)、[`operations.md`](operations.md)、[`privacy-deletion.md`](privacy-deletion.md)、[`../developer/host-adapter-guide.md`](../developer/host-adapter-guide.md)。

## 1. 适用范围

本文适用于 `anp-miniapp-dock` runtime、CLI/dev tooling、Render IR schema、Skill package contract、Host adapter contract、release gate runner 和 runbook 的 production-readiness release candidate。

非目标：

- 不接入真实 production deploy platform。
- 不声明 headless CLI、local file backend、mock provider、demo server 或 Mac demo 为 production-ready。
- 不删除 audit evidence、release report、rollback decision 或 cache quarantine 记录。

## 2. 版本化对象

每个 release notes 必须记录以下对象的版本或 schema：

| 对象 | 当前字段 / schema | 变更规则 | Breaking change 要求 |
|---|---|---|---|
| Runtime API | `dock.runtime.v1` | 兼容扩展可保持 v1；删除字段、改错误语义、改 action routing 必须先出 migration note。 | 需要 migration note、Host adapter conformance 和 rollback plan。 |
| Render IR | `dock.render-ir.v1` | 新增 node/style/action 必须保持旧 Host fallback；不兼容变更必须 bump schema。 | 需要 snapshot gate、Host renderer fallback 和 migration note。 |
| Capability token | token claims version / scope derivation | scope、issuer、audience、jti replay policy 变化必须记录。 | 需要 token revoke / cache restore policy 和 auth rollback。 |
| Skill package contract | `SKILL.md`、`mcp.json`、supply-chain metadata | manifest 新字段必须兼容旧包；integrity policy 收紧必须先提示。 | 需要 import/validate guidance 和 package rollback。 |
| Host adapter contract | `dock.host-adapter.v1` | Host action/provider capability 新增必须声明 required/optional/unsupported。 | 需要 Host adapter conformance、fallback 和 rollout stop rule。 |
| Release gates report | `dock.release-gates-report.v1` | report 字段可兼容扩展；release decision 语义不能放松。 | 需要 release tooling migration note。 |

## 3. Release Notes 模板

每个 release notes 文件必须包含这些小节。`scripts/release-gates.sh --release-notes <path>` 会检查版本、兼容变化、风险、rollback 和 migration/breaking-change 信息。

```markdown
# Release Notes: <version-or-date>

## Version

## Compatibility Changes

## Security Changes

## Risk And Release Blockers

## Migration Notes

## Rollback Plan

## Gate Evidence

## Canary Plan
```

Release notes 不得包含 raw token、`Authorization`、`Signature`、private key material/path、手机号、真实地址、文件内容、精确经纬度或本机绝对路径。

## 4. 准入流程

### 4.1 Release Candidate

必须先创建 release candidate evidence：

```bash
./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md
```

准入要求：

- `dock.release-gates-report.v1` 可解析。
- `releaseDecision = "pass"` 才能进入 canary；如果是 `needs-review`，必须记录 skip 原因并禁止 production rollout。
- `requiredFailed = 0` 且 `hardBlockerFailed = 0`。
- release notes completeness gate 不能是 skip。
- artifact redaction scan 通过。
- `git status --short --branch` 和 release commit hash 已记录。

### 4.2 Canary Stages

| Stage | 范围 | 准入条件 | 监控信号 | 退出条件 |
|---|---|---|---|---|
| 0. Headless fixture | `examples/coffee-skill` 和 `examples/fixtures/*` | release gate `pass`；snapshot、fixture、perf、redaction gates 通过 | gate report、fixture pass/fail、perf baseline、fallback count | 任一 hard blocker 或 fixture failure 立即 rollback |
| 1. Internal Host | 内部 Host adapter / Mac demo / headless Host conformance | Stage 0 pass；Host adapter contract 明确 `productionReady = false` 或真实 Host conformance 证据 | Host crash、Render IR unsupported node/action、action routing、redaction scan | Host crash、unknown action silent success 或 sensitive output rollback |
| 2. Allowlisted merchant Skill | 固定 merchant DID、publisher DID、Skill id 和 version | Stage 1 pass；Skill package digest/signature policy、registry/cache pin 和 rollback target 记录 | API latency、fallback rate、consent deny/unavailable、token refresh/fail、audit write | 指标超过阈值或 audit unavailable rollback |
| 3. Publisher / Skill version expansion | 逐步扩展 publisher DID、Skill version 或 Host channel | Stage 2 稳定；release notes 更新 rollout scope | 同 Stage 2，加 cache/quarantine/publisher error | 任一 hard blocker 或 regression spike stop rollout |

当前仓库只完成 Stage 0 本地可执行流程；Stage 1-3 需要真实 Host/deploy 平台接入后再执行。

## 5. 监控阈值

在没有生产基线前，以下阈值作为 canary hard stop；后续可按真实 Host/merchant 数据收紧。

| 信号 | Hard stop 条件 | 来源 |
|---|---|---|
| Token leakage | 任意 raw token、`Authorization`、`Signature`、capability token 或 private key marker 出现在 CLI/report/log/artifact | release gate redaction scan、Host出口扫描 |
| Consent bypass | L3/L4 API/provider 在没有 ConsentGate/audit evidence 时执行 | `dock.consent.total`、audit record、runtime tests |
| Sandbox escape | Component/Atomic VM escape regression、resource limit fail-open | sandbox gate、`dock.sandbox_limit.total` |
| Fallback spike | fallback rate 比本地基线明显上升，或 Host renderer unknown node/action 无 fallback | `dock.fallback.total`、Host adapter report |
| Auth failure spike | `wx.login` / `wx.checkSession` / `wx.request` token refresh fail 激增 | `dock.token_refresh.total`、request status |
| Host crash | Host adapter crash、unsupported action silent success、Render IR incompatible | Host adapter conformance / crash report |
| Audit failure | L3/L4 audit write unavailable、audit export corrupt、audit sink blocked | `dock.audit_record.total`、persistent audit reader |

## 6. Rollback 条件

以下任一条件必须立即 stop rollout，并执行对应 rollback plan：

- token leakage regression；
- consent bypass；
- sandbox escape 或 resource-limit fail-open；
- fallback/error/auth failure spike；
- Host crash 或 Render IR incompatible；
- audit write failure / audit unavailable；
- Skill package digest/signature mismatch；
- registry/cache quarantine、publisher trust policy mismatch；
- release notes 或 gate report 发现真实 secret、本机路径或隐私原文。

## 7. Rollback Actions

| 动作 | 执行方式 | 证据要求 |
|---|---|---|
| Stop rollout | 暂停当前 Host channel、publisher DID、Skill version 或 release candidate 扩展 | 记录时间、actor、scope、原因和 gate/report 链接 |
| Runtime revert | 回退到上一 Runtime/API/Host adapter compatible commit 或 release tag | 记录 target commit、migration impact 和 gate rerun |
| Skill version rollback | 使用 Step 04-03 registry/cache 的 pinned version / rollback target | 记录 Skill id、version、digest、publisher DID 和 rollback pin |
| Disable Skill version | 对 allowlisted merchant / publisher / version 下线或 quarantine | 不删除 audit；保留 package digest 和 quarantine reason |
| Cache purge | 只清理匹配 scope 的 cache sidecar/metadata；保留 rollback pin、active retain 和 audit evidence | 记录 dry-run scope、deleted entries、preserved rollback/evidence |
| Token revoke | revoke affected session/token jti，清理 cache restore entry | 不输出 raw token；记录 issuer/audience/scope summary |
| Audit preservation | 固定 retention/export evidence，禁止删除事故窗口 audit | 记录 retention policy、export approval 和 redaction result |

当前仓库没有生产 cache purge CLI；执行真实 purge 前必须使用 Step 04-08 的 cache cleanup contract 规则：dry-run first、scope precise、保留 rollback pin 和 audit evidence。

涉及用户或商家数据删除时，必须同时执行 [`privacy-deletion.md`](privacy-deletion.md) 的审批、scope dry-run、token revoke、storage delete-scope、audit evidence retention 和 cache cleanup 检查。

## 8. Cache Purge Procedure

1. 记录 incident / release gate report / affected Skill version。
2. 定义 purge scope：merchant DID、publisher DID、Skill id、version、digest、cache kind。
3. 先执行 dry-run 或等价审计，不得删除 audit。
4. 保留 rollback pin、active retain 和 quarantine evidence。
5. 删除或 quarantine 目标 cache entries。
6. 重跑 `./scripts/release-gates.sh --release-notes <release-notes>`。
7. 把结果写回 release notes 或 incident record。

## 9. Dry Run Checklist

| 项 | 记录 |
|---|---|
| release notes path |  |
| release gate report path |  |
| release decision |  |
| git commit / branch / dirty status |  |
| versioned objects reviewed |  |
| canary stage |  |
| rollout scope |  |
| metrics/perf baseline reviewed |  |
| rollback target |  |
| cache purge scope |  |
| token revoke scope |  |
| audit evidence retention |  |
| residual risk / skipped gates |  |

## 10. Step 06-05 本地 Dry Run

当前本地 dry-run 使用：

```bash
./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md
```

预期：本地 Stage 0 release notes completeness 可通过；如果没有真实 Host/deploy 平台证据，release notes 必须明确 Stage 1-3 仍是 release blocker，不能宣称 production rollout 完成。
