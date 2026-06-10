# Repository Guidelines

## Project Structure & Module Organization

This repository is currently documentation-first for `anp-miniapp-dock`, a DID-native Skill runtime for MiniApp MCP-compatible agent skills over ANP.

- `README.md` gives the short project summary.
- `docs/architecture/` contains product and system architecture documents.
- `docs/weichat-miniapp-mcp-protocol/` contains the MiniApp MCP protocol reference notes used by the architecture docs.
- Source code, tests, and runtime assets are not present yet. When adding them, keep implementation under clearly named top-level directories such as `src/`, `tests/`, `examples/`, or `assets/`, and update this guide plus `README.md`.

## Build, Test, and Development Commands

No package manifest, build system, or test runner is checked in yet. Do not invent dependency-manager commands until the corresponding project files exist.

Useful repository commands:

- `rg "QuickJS|MCP|DID" docs README.md` searches project docs quickly.
- `git status --short` checks pending local changes before editing.
- `git diff -- README.md docs/ AGENTS.md` reviews documentation changes.

When code is introduced, document the exact local commands here, for example `cargo test`, `npm test`, or `make build`.

## Coding Style & Naming Conventions

For Markdown, use ATX headings (`#`, `##`), concise paragraphs, and fenced code blocks with language labels where applicable. Prefer lower-kebab-case filenames for new docs, matching existing files such as `anp-skill-dock-architecture.md`.

Keep terminology consistent with the architecture docs: use `Skill`, `MCP`, `ANP DID`, `ANP Rust SDK`, `wx Compatibility Layer`, `QuickJS-NG`, `MiniApp MCP Component Runtime`, `Render IR`, `CardSpec`, and `Agentic MiniApp Container`. Preserve exact protocol field names in backticks, for example `SKILL.md`, `mcp.json`, `structuredContent`, `_meta.ui.componentPath`, `sendFollowUpMessage`, and `api/call`.

## Testing Guidelines

There is no automated test framework yet. For documentation changes, verify headings, relative links, and command snippets manually. If source code is added, include focused tests alongside the implementation or under `tests/`, and add the test command to this file.

## Commit & Pull Request Guidelines

The current history only contains `Initial commit`, so no detailed convention has been established. Use short, imperative commit subjects, for example `Add Skill loader architecture notes` or `Document MCP card rendering flow`.

Pull requests should include a concise summary, changed files or areas, verification performed, and screenshots only when UI or rendered documentation output changes. Link related issues or design notes when available.

## Security & Configuration Tips

Do not commit private keys, DID credentials, capability tokens, merchant secrets, or real user data. Keep protocol examples mock-only unless a secure configuration path is documented.
