#!/usr/bin/env bash
set -u -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT_DIR="${REPORT_DIR:-$ROOT/target/release-gates}"
REPORT_FILE="${REPORT_FILE:-$REPORT_DIR/release-gates-report.json}"
LOG_DIR="$REPORT_DIR/logs"
ARTIFACT_DIR="$REPORT_DIR/artifacts"
RESULTS_FILE="$REPORT_DIR/results.jsonl"
RELEASE_NOTES_PATH="${RELEASE_NOTES_PATH:-}"
QUICK=0

usage() {
  cat <<'USAGE'
Usage: scripts/release-gates.sh [--quick] [--report <path>] [--release-notes <path>]

Runs production-readiness release gates and writes a machine-readable report.

Options:
  --quick                 Run script/doc/report checks plus metadata/fmt only.
  --report <path>         Override report path. Default: target/release-gates/release-gates-report.json.
  --release-notes <path>  Validate release notes completeness for a specific markdown file.
  -h, --help              Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --quick)
      QUICK=1
      shift
      ;;
    --report)
      if [[ $# -lt 2 ]]; then
        echo "--report requires a path" >&2
        exit 2
      fi
      REPORT_FILE="$2"
      if [[ "$REPORT_FILE" != /* ]]; then
        REPORT_FILE="$ROOT/$REPORT_FILE"
      fi
      REPORT_DIR="$(dirname "$REPORT_FILE")"
      LOG_DIR="$REPORT_DIR/logs"
      ARTIFACT_DIR="$REPORT_DIR/artifacts"
      RESULTS_FILE="$REPORT_DIR/results.jsonl"
      shift 2
      ;;
    --release-notes)
      if [[ $# -lt 2 ]]; then
        echo "--release-notes requires a path" >&2
        exit 2
      fi
      RELEASE_NOTES_PATH="$2"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

mkdir -p "$REPORT_DIR"
rm -rf "$LOG_DIR" "$ARTIFACT_DIR"
mkdir -p "$LOG_DIR" "$ARTIFACT_DIR"
: > "$RESULTS_FILE"
cd "$ROOT" || exit 1
export REPORT_DIR ARTIFACT_DIR

json_escape() {
  python3 -c 'import json, sys; print(json.dumps(sys.argv[1], ensure_ascii=False))' "$1"
}

append_result() {
  local name="$1"
  local status="$2"
  local required="$3"
  local hard_blocker="$4"
  local command="$5"
  local reason="$6"
  local residual_risk="$7"
  local log_path="$8"
  local duration_ms="$9"

  printf '{"name":%s,"status":%s,"required":%s,"hardBlocker":%s,"command":%s,"reason":%s,"residualRisk":%s,"log":%s,"durationMs":%s}\n' \
    "$(json_escape "$name")" \
    "$(json_escape "$status")" \
    "$required" \
    "$hard_blocker" \
    "$(json_escape "$command")" \
    "$(json_escape "$reason")" \
    "$(json_escape "$residual_risk")" \
    "$(json_escape "$log_path")" \
    "$duration_ms" >> "$RESULTS_FILE"
}

duration_ms_since() {
  local start_ms="$1"
  local now_ms
  now_ms="$(python3 - <<'PY'
import time
print(int(time.time() * 1000))
PY
)"
  echo $((now_ms - start_ms))
}

now_ms() {
  python3 - <<'PY'
import time
print(int(time.time() * 1000))
PY
}

run_cmd() {
  local name="$1"
  local required="$2"
  local hard_blocker="$3"
  local command="$4"
  local log="$LOG_DIR/$name.log"
  local start_ms
  start_ms="$(now_ms)"

  echo "==> $name"
  if bash -lc "$command" > "$log" 2>&1; then
    append_result "$name" "pass" "$required" "$hard_blocker" "$command" "" "" "${log#$ROOT/}" "$(duration_ms_since "$start_ms")"
  else
    append_result "$name" "fail" "$required" "$hard_blocker" "$command" "command exited non-zero" "release must not proceed until this gate passes or the blocker is explicitly resolved" "${log#$ROOT/}" "$(duration_ms_since "$start_ms")"
  fi
}

skip_gate() {
  local name="$1"
  local required="$2"
  local hard_blocker="$3"
  local command="$4"
  local reason="$5"
  local residual_risk="$6"
  append_result "$name" "skip" "$required" "$hard_blocker" "$command" "$reason" "$residual_risk" "" 0
}

check_no_match() {
  local name="$1"
  local pattern="$2"
  shift 2
  local log="$LOG_DIR/$name.log"
  local display_targets=()
  local target
  for target in "$@"; do
    if [[ "$target" == "$ROOT/"* ]]; then
      display_targets+=("${target#$ROOT/}")
    else
      display_targets+=("$target")
    fi
  done
  local display_command="rg -n <sensitive-pattern> ${display_targets[*]}"
  local start_ms
  start_ms="$(now_ms)"

  echo "==> $name"
  rg -n "$pattern" "$@" > "$log" 2>&1
  local code=$?
  if [[ $code -eq 1 ]]; then
    append_result "$name" "pass" "true" "true" "$display_command" "" "" "${log#$ROOT/}" "$(duration_ms_since "$start_ms")"
  elif [[ $code -eq 0 ]]; then
    append_result "$name" "fail" "true" "true" "$display_command" "forbidden sensitive marker found" "release must not proceed with token, Authorization, signature, local path, location, or credential leakage" "${log#$ROOT/}" "$(duration_ms_since "$start_ms")"
  else
    append_result "$name" "fail" "true" "true" "$display_command" "sensitive scan command failed" "scanner failure leaves redaction gate unevaluated" "${log#$ROOT/}" "$(duration_ms_since "$start_ms")"
  fi
}

check_markdown_links() {
  local name="docs_markdown_links"
  local log="$LOG_DIR/$name.log"
  local start_ms
  start_ms="$(now_ms)"

  echo "==> $name"
  if python3 - "$ROOT" > "$log" 2>&1 <<'PY'
import os
import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlparse

root = Path(sys.argv[1]).resolve()
skip_dirs = {".git", "target", ".idea", ".vscode", ".venv", "__pycache__", ".build", ".swiftpm"}
link_pattern = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
errors = []
checked = 0

def iter_markdown_files():
    for current_root, dirs, files in os.walk(root):
        dirs[:] = [name for name in dirs if name not in skip_dirs and not name.endswith(".xcuserdata")]
        current = Path(current_root)
        for filename in files:
            if filename.endswith(".md"):
                yield current / filename

for md_path in iter_markdown_files():
    text = md_path.read_text(encoding="utf-8")
    for match in link_pattern.finditer(text):
        raw = match.group(1).strip()
        if not raw or raw.startswith("#"):
            continue
        if raw.startswith("<") and raw.endswith(">"):
            raw = raw[1:-1]
        parsed = urlparse(raw)
        if parsed.scheme or raw.startswith("//"):
            continue
        target = raw.split("#", 1)[0].split("?", 1)[0]
        if not target:
            continue
        target = unquote(target)
        if target.startswith("/"):
            errors.append(f"{md_path.relative_to(root)}: absolute local markdown link is not portable: {raw}")
            continue
        resolved = (md_path.parent / target).resolve()
        checked += 1
        try:
            resolved.relative_to(root)
        except ValueError:
            errors.append(f"{md_path.relative_to(root)}: link escapes repo: {raw}")
            continue
        if not resolved.exists():
            errors.append(f"{md_path.relative_to(root)}: missing link target: {raw}")

print(f"checked {checked} local markdown links")
if errors:
    print("\n".join(errors))
    sys.exit(1)
PY
  then
    append_result "$name" "pass" "true" "false" "python markdown relative link checker" "" "" "${log#$ROOT/}" "$(duration_ms_since "$start_ms")"
  else
    append_result "$name" "fail" "true" "false" "python markdown relative link checker" "markdown link check failed" "broken docs links make release evidence non-auditable" "${log#$ROOT/}" "$(duration_ms_since "$start_ms")"
  fi
}

check_matrix_statuses() {
  local name="compatibility_matrix_statuses"
  local log="$LOG_DIR/$name.log"
  local start_ms
  start_ms="$(now_ms)"

  echo "==> $name"
  if python3 - "$ROOT" > "$log" 2>&1 <<'PY'
import sys
from pathlib import Path

root = Path(sys.argv[1])
allowed = {
    "supported",
    "host-boundary",
    "planned-p1",
    "planned-p2",
    "demo-only",
    "unsupported-by-design",
}
files = [
    root / "docs/architecture/wx-api-compatibility-matrix.md",
    root / "docs/architecture/component-compatibility-matrix.md",
]
errors = []
checked = 0

for path in files:
    lines = path.read_text(encoding="utf-8").splitlines()
    status_index = None
    for line in lines:
        stripped = line.strip()
        if not stripped.startswith("|"):
            status_index = None
            continue
        cells = [cell.strip() for cell in stripped.strip("|").split("|")]
        if all(set(cell) <= {"-", ":"} for cell in cells):
            continue
        lowered = [cell.lower() for cell in cells]
        if "status" in lowered:
            status_index = lowered.index("status")
            continue
        if "状态" in cells:
            status_index = cells.index("状态")
            continue
        if status_index is None or status_index >= len(cells):
            continue
        raw_status = cells[status_index].replace("`", "").strip()
        if not raw_status or raw_status in {"含义", "当前处理", "当前决策"}:
            continue
        if raw_status in allowed:
            checked += 1
        else:
            errors.append(f"{path.relative_to(root)}: unsupported status `{raw_status}` in row `{stripped}`")

print(f"checked {checked} compatibility matrix status cells")
if checked == 0:
    errors.append("no compatibility matrix status cells were checked")
if errors:
    print("\n".join(errors))
    sys.exit(1)
PY
  then
    append_result "$name" "pass" "true" "false" "python compatibility matrix status checker" "" "" "${log#$ROOT/}" "$(duration_ms_since "$start_ms")"
  else
    append_result "$name" "fail" "true" "false" "python compatibility matrix status checker" "compatibility matrix status check failed" "unknown status values can hide production-readiness drift" "${log#$ROOT/}" "$(duration_ms_since "$start_ms")"
  fi
}

check_release_notes() {
  local name="release_notes_completeness"
  local log="$LOG_DIR/$name.log"
  local start_ms
  start_ms="$(now_ms)"

  if [[ -z "$RELEASE_NOTES_PATH" ]]; then
    skip_gate "$name" "true" "false" "RELEASE_NOTES_PATH=<path> scripts/release-gates.sh" "no release notes path provided" "release notes must be checked before canary or production release"
    return
  fi

  echo "==> $name"
  if python3 - "$ROOT" "$RELEASE_NOTES_PATH" > "$log" 2>&1 <<'PY'
import re
import sys
from pathlib import Path

root = Path(sys.argv[1]).resolve()
path = Path(sys.argv[2])
if not path.is_absolute():
    path = (root / path).resolve()
try:
    path.relative_to(root)
except ValueError:
    print(f"release notes path escapes repository: {path}")
    sys.exit(1)
if not path.is_file():
    print(f"release notes file does not exist: {path.relative_to(root)}")
    sys.exit(1)

text = path.read_text(encoding="utf-8")
required_terms = {
    "version": r"(?i)\bversion\b|版本",
    "compatibility": r"(?i)compat|兼容",
    "risk": r"(?i)risk|风险",
    "rollback": r"(?i)rollback|回滚",
    "migration": r"(?i)migration|迁移|breaking",
}
missing = [name for name, pattern in required_terms.items() if not re.search(pattern, text)]
print(f"checked release notes: {path.relative_to(root)}")
if missing:
    print("missing required release-note topics: " + ", ".join(missing))
    sys.exit(1)
PY
  then
    append_result "$name" "pass" "true" "false" "python release notes completeness checker" "" "" "${log#$ROOT/}" "$(duration_ms_since "$start_ms")"
  else
    append_result "$name" "fail" "true" "false" "python release notes completeness checker" "release notes completeness check failed" "release notes must include version, compatibility, risk, rollback, and migration information" "${log#$ROOT/}" "$(duration_ms_since "$start_ms")"
  fi
}

write_report() {
  python3 - "$ROOT" "$RESULTS_FILE" "$REPORT_FILE" "$QUICK" <<'PY'
import json
import subprocess
import sys
import time
from pathlib import Path

root = Path(sys.argv[1])
results_file = Path(sys.argv[2])
report_file = Path(sys.argv[3])
quick = sys.argv[4] == "1"
results = []
if results_file.exists():
    with results_file.open(encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if line:
                results.append(json.loads(line))

def git(args, default):
    try:
        return subprocess.check_output(["git", *args], cwd=root, text=True).strip()
    except Exception:
        return default

required_failed = [item for item in results if item["required"] and item["status"] == "fail"]
hard_failed = [item for item in results if item["hardBlocker"] and item["status"] == "fail"]
skipped = [item for item in results if item["status"] == "skip"]
if required_failed or hard_failed:
    status = "failed"
    release_decision = "blocked"
elif skipped:
    status = "warning"
    release_decision = "needs-review"
else:
    status = "ok"
    release_decision = "pass"

report = {
    "schemaVersion": "dock.release-gates-report.v1",
    "status": status,
    "commandStatus": status,
    "releaseDecision": release_decision,
    "mode": "quick" if quick else "full",
    "generatedAtMs": int(time.time() * 1000),
    "environment": {
        "commit": git(["rev-parse", "--short", "HEAD"], "unknown"),
        "branch": git(["branch", "--show-current"], "unknown"),
        "workingTreeDirty": bool(git(["status", "--short"], "")),
        "rustc": subprocess.getoutput("rustc --version").strip() or "unknown",
    },
    "summary": {
        "total": len(results),
        "pass": sum(1 for item in results if item["status"] == "pass"),
        "fail": sum(1 for item in results if item["status"] == "fail"),
        "skip": len(skipped),
        "requiredFailed": len(required_failed),
        "hardBlockerFailed": len(hard_failed),
        "skipCountsAsPass": False,
    },
    "hardBlockers": [
        "redaction failure",
        "consent bypass",
        "sandbox escape",
        "token leakage",
        "Authorization or Signature leakage",
    ],
    "gates": results,
}
report_file.parent.mkdir(parents=True, exist_ok=True)
report_file.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
print(report_file)
PY
}

run_cmd "cargo_metadata" "true" "false" "cargo metadata --format-version 1 --no-deps"
run_cmd "cargo_fmt" "true" "false" "cargo fmt --check"

if [[ "$QUICK" -eq 0 ]]; then
  run_cmd "cargo_clippy_workspace" "true" "false" "cargo clippy --workspace --all-targets -- -D warnings"
  run_cmd "cargo_test_workspace" "true" "false" "cargo test --workspace"
  run_cmd "coffee_e2e" "true" "true" "cargo test -p dock-cli --test coffee_order_flow"
  run_cmd "validate_coffee_json" "true" "false" "cargo run -p dock-cli -- validate examples/coffee-skill > \"\$ARTIFACT_DIR/validate-coffee.json\" && python3 -m json.tool \"\$ARTIFACT_DIR/validate-coffee.json\" >/dev/null"
  run_cmd "doctor_json" "true" "false" "cargo run -p dock-cli -- doctor > \"\$ARTIFACT_DIR/doctor.json\" && python3 -m json.tool \"\$ARTIFACT_DIR/doctor.json\" >/dev/null"
  run_cmd "observability_events" "true" "true" "cargo test -p dock-core observability && cargo test -p dock-core runtime_observability"
  run_cmd "metrics_tracing" "true" "true" "cargo test -p dock-core metrics && cargo test -p dock-core trace && cargo test -p js-runtime-quickjs quickjs_executor_records_vm_request_and_token_metrics_with_trace"
  run_cmd "sandbox_security" "true" "true" "cargo test -p js-runtime-quickjs sandbox && cargo test -p js-runtime-quickjs limit && cargo test -p js-runtime-quickjs console && cargo test -p js-runtime-quickjs invalid_atomic && cargo test -p js-runtime-quickjs pending_job && cargo test -p component-runtime sandbox && cargo test -p component-runtime dynamic && cargo test -p component-runtime snapshot_size"
  run_cmd "permission_allowlist" "true" "true" "cargo test -p wx-compat permission && cargo test -p anp-adapter allowlist"
  run_cmd "did_token_lifecycle" "true" "true" "cargo test -p anp-adapter token && cargo test -p anp-adapter session && cargo test -p anp-adapter challenge"
  run_cmd "consent_audit_supply_chain" "true" "true" "cargo test -p consent-audit consent && cargo test -p dock-core consent && cargo test -p skill-loader package && cargo test -p js-runtime-quickjs remote_require_is_rejected"
  run_cmd "component_snapshot_gate" "true" "false" "cargo test -p component-runtime snapshot"
  run_cmd "dock_cli_fixture_gate" "true" "false" "cargo test -p dock-cli fixture && cargo test -p dock-cli example"
  run_cmd "test_skill_fixtures" "true" "false" "cargo run -p dock-cli -- test-skill examples/coffee-skill > \"\$ARTIFACT_DIR/test-skill-coffee.json\" && cargo run -p dock-cli -- test-skill examples/fixtures/address-form > \"\$ARTIFACT_DIR/test-skill-address-form.json\" && cargo run -p dock-cli -- test-skill examples/fixtures/media-review > \"\$ARTIFACT_DIR/test-skill-media-review.json\" && cargo run -p dock-cli -- test-skill examples/fixtures/dynamic-status > \"\$ARTIFACT_DIR/test-skill-dynamic-status.json\" && cargo run -p dock-cli -- test-skill examples/fixtures/location-map-preview > \"\$ARTIFACT_DIR/test-skill-location-map-preview.json\""
  run_cmd "performance_baseline" "true" "false" "cargo test -p dock-cli perf && cargo test -p dock-cli --test coffee_order_flow perf_smoke_reports_baselines_and_stress && cargo run -p dock-cli -- perf examples/coffee-skill --iterations 1 > \"\$ARTIFACT_DIR/perf-coffee-smoke.json\" && python3 -m json.tool \"\$ARTIFACT_DIR/perf-coffee-smoke.json\" >/dev/null"
else
  skip_gate "cargo_clippy_workspace" "true" "false" "cargo clippy --workspace --all-targets -- -D warnings" "--quick selected" "full release gate must run this before release"
  skip_gate "cargo_test_workspace" "true" "false" "cargo test --workspace" "--quick selected" "full release gate must run this before release"
  skip_gate "security_and_fixture_gates" "true" "true" "focused security, fixture, snapshot, perf gates" "--quick selected" "quick mode is script validation only, not release approval"
fi

check_markdown_links
check_matrix_statuses
check_release_notes
check_no_match "artifact_redaction_scan" '(/home/|/Users/|Authorization|Signature|capabilityToken|Bearer |fixture-token|perf-token-redacted|private key|token-secret|latitude|longitude)' testdata/render-ir testdata/perf "$ARTIFACT_DIR"
run_cmd "docs_diff_check" "true" "false" "git diff --check -- scripts docs/runbook docs/plan README.md"

write_report

python3 - "$REPORT_FILE" <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
print(f"release gates report: {sys.argv[1]}")
print(f"status: {report['status']} decision: {report['releaseDecision']} summary: {report['summary']}")
sys.exit(1 if report["releaseDecision"] == "blocked" else 0)
PY
