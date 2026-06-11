# ANP MiniApp Dock Mac App

This folder is the phase-one macOS host project for `anp-miniapp-dock`.
It is intentionally kept outside the Rust Cargo workspace. It now includes both a real Xcode project (`AnpMiniappDockMac.xcodeproj`) and the original Swift Package manifest for command-line builds.

## Project analysis

The existing repository already has the container pipeline that the Mac host needs:

1. `skill-loader` validates and loads `SKILL.md`, `mcp.json`, API JavaScript, and MiniApp MCP component files.
2. `dock-core`, `js-runtime-quickjs`, and `component-runtime` execute APIs, enforce consent/audit boundaries, and convert MiniApp components into Render IR.
3. `demo-server` provides the local coffee merchant Agent endpoints.
4. `dock-cli run-demo` proves the full local flow: challenge/login, `searchDrinks`, `confirmOrder`, `payOrder`, component `api/call`, and card expiration.

For phase one, the Mac app uses that stable Rust CLI boundary through `Process` instead of introducing a premature Swift/Rust FFI layer. The visible demo is a chatbot:

```text
user need, e.g. "我要点一杯咖啡"
  -> OpenAI-compatible intent recognition
  -> local MiniApp container / Coffee Skill API call
  -> Skill component results rendered as SwiftUI chat attachments
```

After the chatbot recognizes the coffee-order intent, the app runs:

```text
dock-cli validate examples/coffee-skill
FastAPI coffee service on http://127.0.0.1:8008, if installed
  or demo-server --port 0 --skill examples/coffee-skill
dock-cli run-demo --skill examples/coffee-skill --server <local-url>
```

It then parses the JSON output and renders the coffee search, order confirmation, payment result, and expiration steps as native SwiftUI cards.

## Acceptance mapping

- **Create the Mac project**: this `mac-app/AnpMiniappDockMac` SwiftUI project.
- **Load the container**: `dock-cli validate` and `dock-cli run-demo` load `examples/coffee-skill` through the Rust MiniApp MCP container.
- **Run the local example pipeline**: the app starts the localhost FastAPI coffee service when its venv is installed, otherwise starts `demo-server` on a random local port, then runs the coffee flow end to end.
- **Chatbot UI**: the user enters a natural-language need, the app recognizes intent through an OpenAI-compatible API, then calls the local Skill/container.
- **Display components**: the parsed container output is shown as SwiftUI chat messages, status, metric, evidence, and flow cards.


## Localhost FastAPI service

The Xcode app first looks for `examples/coffee-fastapi-server/.venv/bin/uvicorn`. If that venv exists, the app starts the Python/FastAPI coffee service on `http://127.0.0.1:8008` and runs the container against it. If the venv is not installed, it falls back to the Rust `demo-server`, which exposes the same localhost HTTP endpoints for smoke tests.

Prepare the FastAPI service from the repository root:

```bash
cd examples/coffee-fastapi-server
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

## Chatbot intent recognition

The chatbot reads OpenAI-compatible settings from the current process environment and also from `source ~/.zshrc` so launches from Xcode/Finder can reuse shell configuration:

```bash
export OPENAI_BASE_URL=https://didhost.cc
export OPENAI_API_KEY=...        # do not commit or log real keys
export OPENAI_MODEL=gpt-5.4
```

If `OPENAI_API_KEY` is empty or the API call fails, the app falls back to a local keyword recognizer so the coffee demo can still run offline. For deterministic smoke tests, force the fallback path:

```bash
ANP_DOCK_DISABLE_OPENAI=1 ANP_DOCK_MAC_HEADLESS=1 swift run
```

## Build and run

Open the real Xcode project:

```bash
open mac-app/AnpMiniappDockMac/AnpMiniappDockMac.xcodeproj
```

Build it from the command line with Xcode:

```bash
xcodebuild -project mac-app/AnpMiniappDockMac/AnpMiniappDockMac.xcodeproj \
  -scheme AnpMiniappDockMac \
  -configuration Debug \
  -derivedDataPath mac-app/AnpMiniappDockMac/.xcode-derived \
  build
```

You can also still use the Swift Package path:

```bash
cd mac-app/AnpMiniappDockMac
swift build
swift run
```

Headless smoke mode runs the same chatbot pipeline without opening a window. Override the default prompt with `ANP_DOCK_CHAT_PROMPT`:

```bash
ANP_DOCK_MAC_HEADLESS=1 ANP_DOCK_CHAT_PROMPT='我要点一杯咖啡' swift run

# or after xcodebuild:
ANP_DOCK_MAC_HEADLESS=1 ANP_DOCK_CHAT_PROMPT='我要点一杯咖啡' \
  .xcode-derived/Build/Products/Debug/AnpMiniappDockMac.app/Contents/MacOS/AnpMiniappDockMac
```

If the app is launched from Finder/Xcode and cannot locate the repository, set:

```bash
export ANP_DOCK_REPO_ROOT=/Users/cs/work/agents/awiki-space/anp/anp-miniapp-dock
swift run
```

To create a `.app` bundle for local manual testing:

```bash
cd mac-app/AnpMiniappDockMac
Scripts/build-app-bundle.sh
open .build/app/AnpMiniappDockMac.app
```

The `.app` still expects a local checkout of this repository because phase one shells out to Cargo and the Rust workspace.

## Notes for the next phase

A production host should replace the `dock-cli` process boundary with either:

- a Rust `cdylib`/C ABI exposing the runtime to Swift, or
- a long-lived local sidecar process with a typed JSON-RPC API.

That would avoid per-run Cargo startup cost and allow the SwiftUI layer to render actual Render IR incrementally instead of consuming the summarized `run-demo` JSON.
