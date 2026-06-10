# Repository Guidelines

## Project Structure & Module Organization

This repository contains `anp-miniapp-dock`, a DID-native Rust Skill runtime for MiniApp MCP-compatible agent skills over ANP.

- `README.md` gives the short project summary.
- `docs/architecture/` contains product and system architecture documents.
- `docs/weichat-miniapp-mcp-protocol/` contains the MiniApp MCP protocol reference notes used by the architecture docs.
- `docs/runbook/` contains local operation and security runbooks.
- `Cargo.toml` defines the Rust workspace.
- `crates/` contains runtime crates, CLI, and demo server packages.
- `examples/coffee-skill/` contains the mock coffee Skill used by integration tests and local demo runs.
- Integration tests currently live under the owning crate, for example `crates/dock-cli/tests/coffee_order_flow.rs`; add root-level `tests/` only if the workspace root becomes a package or a dedicated harness is introduced.

## Build, Test, and Development Commands

The Rust workspace pins toolchain `1.88.0` through `rust-toolchain.toml`.

Useful repository commands:

- `cargo metadata --format-version 1 --no-deps` verifies workspace membership.
- `cargo fmt --check` checks Rust formatting.
- `cargo clippy --workspace --all-targets -- -D warnings` runs the lint gate.
- `cargo test --workspace` runs all workspace tests.
- `cargo test -p dock-cli --test coffee_order_flow` runs the coffee CLI E2E.
- `cargo run -p demo-server -- --host 127.0.0.1 --port 3000 --skill examples/coffee-skill` starts the local coffee merchant Agent demo.
- `cargo run -p dock-cli -- run-demo --skill examples/coffee-skill --server http://127.0.0.1:3000` runs the local coffee flow through the CLI.
- `rg "QuickJS|MCP|DID" docs README.md` searches project docs quickly.
- `git status --short` checks pending local changes before editing.
- `git diff -- README.md docs/ AGENTS.md` reviews documentation changes.

## Coding Style & Naming Conventions

For Rust, use the repository `rust-toolchain.toml`, run `cargo fmt`, and keep crate names aligned with the workspace package names. For Markdown, use ATX headings (`#`, `##`), concise paragraphs, and fenced code blocks with language labels where applicable. Prefer lower-kebab-case filenames for new docs, matching existing files such as `anp-skill-dock-architecture.md`.

Keep terminology consistent with the architecture docs: use `Skill`, `MCP`, `ANP DID`, `ANP Rust SDK`, `wx Compatibility Layer`, `QuickJS-NG`, `MiniApp MCP Component Runtime`, `Render IR`, `CardSpec`, and `Agentic MiniApp Container`. Preserve exact protocol field names in backticks, for example `SKILL.md`, `mcp.json`, `structuredContent`, `_meta.ui.componentPath`, `sendFollowUpMessage`, and `api/call`.

## Testing Guidelines

Run `cargo test --workspace` for code changes unless a step plan records a narrower crate-level command. Run `cargo clippy --workspace --all-targets -- -D warnings` before commits that touch Rust behavior. For documentation changes, verify headings, relative links, and command snippets manually. Include focused tests alongside implementation or under the owning crate's `tests/` directory when behavior is added.

## Commit & Pull Request Guidelines

The current history only contains `Initial commit`, so no detailed convention has been established. Use short, imperative commit subjects, for example `Add Skill loader architecture notes` or `Document MCP card rendering flow`.

Pull requests should include a concise summary, changed files or areas, verification performed, and screenshots only when UI or rendered documentation output changes. Link related issues or design notes when available.

## Security & Configuration Tips

Do not commit private keys, DID credentials, capability tokens, merchant secrets, or real user data. Keep protocol examples mock-only unless a secure configuration path is documented. CLI and demo output must redact capability tokens, `Authorization` values, HTTP signatures, private key paths, secrets, and privacy-bearing parameters. High-risk actions such as order confirmation and payment must go through consent/audit boundaries rather than direct Skill execution.
