# anp-miniapp-dock

`anp-miniapp-dock` is a DID-native Rust Skill runtime for running MiniApp MCP-compatible agent skills over ANP.

The project is currently in architecture-baseline status. Source crates, tests, examples, and runtime assets will be added under clearly named top-level directories such as `crates/`, `tests/`, and `examples/` as implementation starts.

## Architecture Documents

- [Agentic MiniApp Container MVP PRD](docs/architecture/agentic-miniapp-container-prd.md)
- [anp-miniapp-dock System Architecture](docs/architecture/anp-skill-dock-architecture.md)
- [MiniApp MCP Compatibility MVP](docs/architecture/miniapp-mcp-compatibility-mvp.md)
- [MiniApp MCP Component Runtime](docs/architecture/miniapp-mcp-component-runtime.md)
- [MiniApp MCP protocol notes](docs/weichat-miniapp-mcp-protocol/weichat-miniapp-mcp.txt)

## Current Development Status

No Cargo workspace or automated test runner is checked in yet. Do not run or document Rust build commands as active project commands until the corresponding `Cargo.toml` files exist.

The planned MVP keeps the MiniApp MCP interface contract compatible at the Skill boundary while replacing identity, authorization, network, sandboxing, and high-risk action handling with an independent Rust Runtime backed by ANP DID and the ANP Rust SDK.
