# Release Notes: 2026-06-14 Local Canary

> 状态：本文件是 Step 06-05 的本地 release notes / canary dry-run 证据，用于验证 `scripts/release-gates.sh --release-notes`。它不表示已经完成真实 production Host rollout。

## Version

Release candidate：`2026-06-14-local-canary`

Versioned objects reviewed：

| Object | Version / schema | Status |
|---|---|---|
| Runtime API | `dock.runtime.v1` | Stable local contract |
| Render IR | `dock.render-ir.v1` | Stable local contract |
| Host adapter contract | `dock.host-adapter.v1` | Headless/local contract; production Host conformance remains required |
| Validate report | `dock.validate-report.v1` | Local gate evidence |
| Test Skill report | `dock.test-skill-report.v1` | Local fixture evidence |
| Doctor report | `dock.doctor-report.v1` | Local environment evidence |
| Perf report | `dock.perf-baseline-report.v1` | Local hardware-dependent baseline only |
| Release gates report | `dock.release-gates-report.v1` | Local release gate evidence |

## Compatibility Changes

- Adds the local vendor-neutral release gate runner `scripts/release-gates.sh`.
- Adds release gate report schema `dock.release-gates-report.v1`.
- Keeps Runtime API, Render IR, Host adapter, capability token, Skill package, and report schemas compatible with the existing local tests.
- No Render IR schema bump is required for this local canary.
- No Skill package contract breaking change is introduced by this release note.

## Security Changes

- Release gates now include hard blockers for redaction failure, consent bypass, sandbox escape, token leakage, and `Authorization` / `Signature` leakage.
- The gate runner scans checked-in Render IR/perf artifacts and generated release artifacts for sensitive markers.
- The release process requires rollback without deleting audit evidence.
- Local demo/headless/mock providers remain non-production evidence and cannot be used as production approval.

## Risk And Release Blockers

This local canary still has production release blockers:

- No real production Host provider/renderer conformance has been executed.
- No real deployment platform or staged traffic router is connected.
- No production encrypted token/storage/audit/cache backend is configured.
- No real remote registry download or production signing verifier is connected.
- Step 06-06 provides local operations/troubleshooting/privacy deletion runbooks, but no production Host privacy deletion job or approval workflow has been executed yet.
- Perf numbers are local and hardware-dependent, not production SLOs.

Hard stop conditions:

- token leakage regression;
- consent bypass;
- sandbox escape;
- fallback/error/auth failure spike;
- Host crash or Render IR incompatible;
- audit write failure;
- package digest/signature mismatch;
- release notes or gate report leaking secrets, local private paths, or privacy data.

## Migration Notes

No code migration is required for existing local developers.

Operational migration:

- Use `./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md` instead of manually copying release gate evidence.
- Treat `releaseDecision = "needs-review"` as not approved; skipped gates must be resolved or explicitly reviewed.
- Before production rollout, add real Host/deploy/platform-specific release notes and rerun the release gate with that file.

## Rollback Plan

Rollback target:

- Use the last known good Runtime commit, Skill package digest, and Host adapter contract evidence recorded in the release gate report.

Rollback actions:

- stop rollout for the affected Host channel, merchant DID, publisher DID, Skill id, and Skill version;
- revert Runtime/API/Host adapter commit if the regression is runtime-side;
- roll back or disable the affected Skill version using registry/cache pin and rollback target;
- quarantine package/cache entries with digest/signature mismatch;
- revoke affected token scopes or jti values without exposing raw token material;
- preserve audit evidence, release gate reports, cache quarantine records, and rollback decision logs.

Cache purge rule:

- dry-run first;
- scope purge by merchant DID, publisher DID, Skill id, version, digest, and cache kind;
- preserve rollback pin, active retain, quarantine evidence, and audit evidence.

## Gate Evidence

Required local command:

```bash
./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md
```

Expected evidence:

- `dock.release-gates-report.v1` parses as JSON.
- `releaseDecision = "pass"` for Stage 0 local canary.
- `requiredFailed = 0`.
- `hardBlockerFailed = 0`.
- release notes completeness gate is `pass`.
- generated artifacts do not contain raw token, `Authorization`, `Signature`, capability token, private key material/path, local private absolute path, phone number, real address, file content, or precise coordinates.

## Canary Plan

Stage 0 local canary:

- run full release gates with this release notes file;
- confirm coffee Skill and four compatibility fixtures pass;
- confirm Render IR snapshots and performance smoke baseline pass;
- confirm redaction scan and docs link checks pass.

Stage 1 internal Host canary:

- blocked until real Host adapter conformance and rollout channel are available;
- must verify Host crash handling, unsupported action fail-closed, renderer fallback, redaction, and audit propagation.

Stage 2 allowlisted merchant canary:

- blocked until real merchant DID, publisher DID, Skill package digest/signature policy, registry allowlist, and rollback target are recorded.

Stage 3 expansion:

- blocked until Stage 2 is stable and production metrics thresholds are defined from real traffic.
