# Repository Guidelines

## Project Structure & Module Organization

This repository contains `anp-miniapp-dock`, a DID-native Rust Skill runtime for MiniApp MCP-compatible agent skills over ANP.

- `README.md` gives the short project summary.
- `docs/architecture/` contains product and system architecture documents.
- `docs/weichat-miniapp-mcp-protocol/` contains the MiniApp MCP protocol reference notes used by the architecture docs.
- `Cargo.toml` defines the Rust workspace.
- `crates/` contains runtime crates, CLI, and demo server packages.
- `examples/`, `tests/`, and runtime assets should stay under clearly named top-level directories when introduced, and this guide plus `README.md` should be updated when commands or layout change.

## Build, Test, and Development Commands

The Rust workspace pins toolchain `1.88.0` through `rust-toolchain.toml`.

Useful repository commands:

- `cargo metadata --format-version 1 --no-deps` verifies workspace membership.
- `cargo fmt --check` checks Rust formatting.
- `cargo test --workspace` runs all workspace tests.
- `rg "QuickJS|MCP|DID" docs README.md` searches project docs quickly.
- `git status --short` checks pending local changes before editing.
- `git diff -- README.md docs/ AGENTS.md` reviews documentation changes.

When new crates, examples, or integration tests are introduced, document the exact local commands here and in `README.md`.

## Coding Style & Naming Conventions

For Rust, use the repository `rust-toolchain.toml`, run `cargo fmt`, and keep crate names aligned with the workspace package names. For Markdown, use ATX headings (`#`, `##`), concise paragraphs, and fenced code blocks with language labels where applicable. Prefer lower-kebab-case filenames for new docs, matching existing files such as `anp-skill-dock-architecture.md`.

Keep terminology consistent with the architecture docs: use `Skill`, `MCP`, `ANP DID`, `ANP Rust SDK`, `wx Compatibility Layer`, `QuickJS-NG`, `MiniApp MCP Component Runtime`, `Render IR`, `CardSpec`, and `Agentic MiniApp Container`. Preserve exact protocol field names in backticks, for example `SKILL.md`, `mcp.json`, `structuredContent`, `_meta.ui.componentPath`, `sendFollowUpMessage`, and `api/call`.

## Testing Guidelines

Run `cargo test --workspace` for code changes unless a step plan records a narrower crate-level command. For documentation changes, verify headings, relative links, and command snippets manually. Include focused tests alongside implementation or under `tests/` when behavior is added.

## Commit & Pull Request Guidelines

The current history only contains `Initial commit`, so no detailed convention has been established. Use short, imperative commit subjects, for example `Add Skill loader architecture notes` or `Document MCP card rendering flow`.

Pull requests should include a concise summary, changed files or areas, verification performed, and screenshots only when UI or rendered documentation output changes. Link related issues or design notes when available.

## Security & Configuration Tips

Do not commit private keys, DID credentials, capability tokens, merchant secrets, or real user data. Keep protocol examples mock-only unless a secure configuration path is documented.
