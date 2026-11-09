# Privacy Deletion Runbook

> 状态：Step 06-06 隐私删除流程。本文定义 user/merchant/Skill/session scope 下的 storage、audit、token、cache 和 release evidence 处理顺序；真实生产 Host 需要把本文映射到加密 storage/audit backend、secure token store、deploy cache 和审批系统。
> 上游计划：[`../plan/production-readiness-roadmap.md`](../plan/production-readiness-roadmap.md) Step 06-06。
> 相关文档：[`operations.md`](operations.md)、[`troubleshooting.md`](troubleshooting.md)、[`release-process.md`](release-process.md)、[`security.md`](security.md)。

## 1. 基本原则

- 精确 scope：每次删除必须记录 user DID、merchant DID、Skill id、session id、namespace、publisher DID、Skill version 和 digest 中适用的字段；不能用模糊文本匹配。
- Dry-run first：生产删除前必须先生成 dry-run 计划和审批记录。
- Audit evidence 保留优先：隐私删除不等于删除事故、合规、consent 或 release evidence。需要保留的 audit record 必须 redacted/exported，并记录 retention policy。
- Token revoke 优先于 storage/cache 清理：先阻止继续访问，再清理持久化状态。
- 不输出原文：删除记录只写 scope summary、hash/digest、entry count、redaction policy 和审批 id。

## 2. Scope 定义

| Scope 字段 | 用途 | 当前本地 contract |
|---|---|---|
| user DID | 用户维度删除和 token/session 失效 | storage scope、token scope、audit record；输出时可用 hash 或 redacted summary。 |
| merchant DID | 商家隔离 | storage scope、token scope、request/audit、Skill cache metadata 可记录 merchant scope。 |
| Skill id | Skill 维度删除 | storage scope、audit record、cache cleanup scope。 |
| session id | 会话级 token/audit 定位 | token/session、Runtime API、audit record；不作为长期 storage 隔离维度。 |
| namespace | storage 子空间 | `StorageScope.namespace`，默认 `default`。 |
| publisher DID / version / digest | package/cache 维度 | Skill cache cleanup scope、rollback pin、quarantine。 |

## 3. 删除对象

| 对象 | 删除 / 保留策略 | 当前能力 |
|---|---|---|
| Token cache | revoke affected session/token jti；清理 restore entry；不输出 raw token。 | `anp-adapter` 有 token cache persistence trait、restore filter、revocation/replay policy；生产 secure store 仍 Host-specific。 |
| Scoped storage | 按 `user DID + merchant DID + Skill id + namespace` 删除 scope；记录 deleted count 或 dry-run count。 | `wx-compat::PersistentScopedStorage::try_delete_scope()` 和 `StorageRestoreReport` 已有本地 contract。 |
| Audit records | 默认保留必要 safety/compliance evidence；导出时 redacted；retention 删除需审批。 | `consent-audit::AuditExportReport` / `AuditRetentionReport` 已有本地 contract；生产 backend 和 approval workflow 仍 Host-specific。 |
| Skill cache | 按 publisher DID、merchant DID、Skill id、version、digest 清理；保留 rollback pin、active retain 和 quarantine evidence。 | `skill-loader::SkillCacheCleanupPolicy` 已有 Rust API contract；当前没有生产 CLI。 |
| Release evidence | release gate report、release notes、rollback decision、incident note 保留。 | `scripts/release-gates.sh` 输出 `dock.release-gates-report.v1`。 |

## 4. Dry-run Checklist

| 项 | 记录 |
|---|---|
| request id / approval id |  |
| requester / reviewer |  |
| legal basis / retention exception |  |
| user DID summary |  |
| merchant DID |  |
| Skill id / publisher DID / version / digest |  |
| session id / namespace |  |
| token revoke scope |  |
| storage delete scope |  |
| audit export/retention scope |  |
| cache cleanup scope |  |
| rollback pin / active retain preserved |  |
| commands or Host-specific jobs planned |  |
| redaction review |  |

## 5. 执行顺序

1. 接收删除请求，生成 request id，记录 scope summary。
2. 冻结受影响 session 或 rollout；如果还在 canary，先 stop rollout。
3. Revoke token/session scope；生产环境必须通过 Host secure token store 或 revocation service 执行。
4. 导出必要 audit evidence，默认 redacted；确认 retention exception。
5. Dry-run scoped storage deletion；确认 namespace、merchant DID、Skill id 和 user DID 都匹配。
6. Dry-run Skill cache cleanup；保留 rollback pin、active retain 和 quarantine evidence。
7. 执行 storage/cache/token 清理；audit retention 删除必须按审批窗口执行。
8. 重跑相关 gate 或 Host-specific verification。
9. 记录 closure：deleted/retained counts、redaction policy、residual risk、owner 和时间。

## 6. 本地验证命令

这些命令只能证明本地 contract，不是生产删除执行：

```bash
cargo test -p wx-compat storage
cargo test -p consent-audit audit
cargo test -p skill-loader cache
cargo run -p dock-cli -- doctor
```

生产 Host 必须提供等价命令或作业，用于：

- token revoke / restore-entry cleanup；
- encrypted storage delete-scope；
- audit redacted export / retention approval；
- Skill cache cleanup dry-run / execute；
- release evidence retention。

## 7. 保留与例外

允许保留的内容：

- redacted audit evidence；
- release gate report、release notes、rollback decision；
- cache quarantine metadata、package digest 和 rollback pin；
- legal/compliance 要求保留的 minimal record。

禁止保留的内容：

- raw token、bearer value、`Authorization`、`Signature`、private key material/path；
- storage raw key/value 原文；
- raw Host provider payload；
- 手机号、真实地址、文件内容、精确位置，除非真实 production policy 明确合法保留且访问受控。

## 8. 关闭标准

- token/session 已失效或记录 Host-specific blocker。
- storage delete scope 已执行或记录合法保留原因。
- audit evidence 已 redacted export 或 retention decision 已审批。
- cache cleanup 保留 rollback pin、active retain 和 quarantine evidence。
- 删除报告不含真实 secret、隐私原文、本机私有路径或 raw credential。
- 相关 incident/release notes 已更新。
