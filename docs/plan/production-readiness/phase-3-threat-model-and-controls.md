# Phase 3 子文档：Threat Model 与安全控制

## 1. 资产清单

| 资产 | 保护目标 |
|---|---|
| DID private key | 永不进入 JS、日志、Render IR、audit export |
| capability token | 只在 host/request boundary 使用，短期有效，可撤销 |
| user DID / agent DID / merchant DID | 正确绑定 session 和 token scope |
| Skill package | 完整性、来源、版本、路径边界 |
| scoped storage | DID + merchant + Skill 隔离 |
| audit records | 完整、可追溯、默认脱敏 |
| Render IR | 不含私密 `_meta` 或 token |
| Host providers | 不能被 Skill 绕过 consent 调用 |

## 2. 攻击者模型

### 2.1 恶意 Skill

能力：提交含恶意 JS、路径逃逸、无限循环、读取 token、发起外部请求、伪造高风险 action。

控制措施：

- QuickJS sandbox；
- 包内 require；
- request allowlist；
- token 不暴露给 JS；
- action 回 Orchestrator；
- resource limits；
- package signing。

### 2.2 被篡改 Skill 包

能力：替换 API JS、组件、manifest、组件路径。

控制措施：

- digest/signature；
- publisher DID；
- cache quarantine；
- manifest validation；
- path canonicalization。

### 2.3 恶意商家 Agent

能力：返回恶意 component metadata、诱导请求、发错 scope、诱导泄露隐私。

控制措施：

- trusted merchant policy；
- token audience/scope；
- Host consent；
- model-visible filtering；
- audit。

### 2.4 网络中间人

能力：篡改 challenge/login/business response。

控制措施：

- HTTPS production requirement；
- DID HTTP signature；
- challenge nonce/audience/TTL；
- token signature verification；
- response size/type validation。

### 2.5 日志读取者

能力：读取 CLI 输出、server logs、audit export。

控制措施：

- centralized redaction；
- sensitive key/value detection；
- no raw token/proof/private path output；
- audit export redacted by default。

## 3. 控制矩阵

| 威胁 | 控制 | 测试证据 |
|---|---|---|
| JS escape | 禁用 eval/Function/prototype constructor | sandbox tests |
| Path traversal | canonicalize + validate inside root | skill-loader tests |
| Unauthorized network | allowlist + broker only | request broker tests |
| Token leakage | host-only token + redaction | CLI/log/audit tests |
| Consent bypass | Orchestrator enforcement order | dock-core tests |
| Replay challenge | nonce one-time + TTL | demo-server/anp-adapter tests |
| Scope mismatch | token verifier expected capability | demo API tests |
| Package tamper | digest/signature | package integrity tests |

## 4. 安全红线

以下情况不得发布：

- 任何 API 可绕过 broker 直接网络出站；
- raw token/signature/private key 出现在 stdout/log/audit/Render IR；
- L3/L4 API 可在无 consent proof 下执行；
- package path 可逃逸 Skill root；
- sandbox escape regression 失败；
- unsupported API 静默成功。

## 5. 残余风险记录模板

```text
Risk:
Impact:
Likelihood:
Control:
Residual risk:
Owner:
Review date:
Release blocker: yes/no
```
