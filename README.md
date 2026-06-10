# anp-miniapp-dock

`anp-miniapp-dock` is a DID-native Rust Skill runtime for running MiniApp MCP-compatible agent skills over ANP.

The project is currently in Rust workspace scaffold status. Core runtime crates live under `crates/`; examples and end-to-end tests will be added under `examples/` and `tests/` as the MVP implementation progresses.

## Architecture Documents

- [Agentic MiniApp Container MVP PRD](docs/architecture/agentic-miniapp-container-prd.md)
- [anp-miniapp-dock System Architecture](docs/architecture/anp-skill-dock-architecture.md)
- [MiniApp MCP Compatibility MVP](docs/architecture/miniapp-mcp-compatibility-mvp.md)
- [MiniApp MCP Component Runtime](docs/architecture/miniapp-mcp-component-runtime.md)
- [MiniApp MCP protocol notes](docs/weichat-miniapp-mcp-protocol/weichat-miniapp-mcp.txt)

## Current Development Status

The workspace is intentionally thin at this stage: crates expose only compile-time scaffold entry points until the feature steps fill in MCP schema, Skill loading, orchestration, runtime, ANP adapter, component rendering, CLI, and demo server behavior.

The planned MVP keeps the MiniApp MCP interface contract compatible at the Skill boundary while replacing identity, authorization, network, sandboxing, and high-risk action handling with an independent Rust Runtime backed by ANP DID and the ANP Rust SDK.

## Development Commands

The repository pins Rust `1.88.0` through `rust-toolchain.toml` to match the ANP Rust SDK.

```bash
cargo metadata --format-version 1 --no-deps
cargo fmt --check
cargo test --workspace
```
