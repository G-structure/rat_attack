# Repository Guidelines

## Project Overview
This repository powers a local-first remote control stack: `ct-web` (SolidJS) talks to one or more `ct-bridge` processes that proxy ACP agents over WebSockets, with an Authentication WebView Proxy keeping logins on the agent host.

- Multi-bridge sessions from a single dashboard
- Permission-gated file edits inside project roots
- Streaming terminal execution
- Mobile-optimized responsive UI
- Authentication that maintains IP alignment for provider checks

## Project Structure & Module Organization
The Rust bridge lives in `src/`; `main.rs` is the entrypoint and `lib.rs` houses the bridge and transport helpers. Integration tests under `tests/` (e.g., `bridge_handshake.rs`) assert ACP flows alongside their fixtures. `ct-web/` contains the SolidJS dashboard, and `planner/` captures the research corpus: `spec.md` is the contract, `progress.md` logs milestones, `notes/` holds scratch investigations, `prompts/` plus `general_prompt.md` and `main_prompt.md` store orchestrator prompts, `human.md` provides onboarding context, and `docs/` expands on ACP integration details. Build outputs land in `target/`; leave the root cache sentinels untouched.

## Planner Guidance
The guidance below is reproduced verbatim from `planner/human.md` so contributors can align with the original research notes:

Zed’s ACP notes are the fastest way to build intuition before touching the bridge code. Start work by sampling these docs so you absorb how a production client wires transport, agents, permissions, and UX.

**ZED NOTES – WHAT TO READ AND WHY**
- `planner/docs/zed_acp.md`: Orientation map. Read first to internalize the bridge/client split and request flow.
- `planner/docs/zed_acp_agent_launching.md`: Launch orchestration. Review when touching process supervision or stdio plumbing.
- `planner/docs/zed_acp_extensibility.md`: Extension patterns. Skim early so `_meta` handling stays consistent.
- `planner/docs/zed_acp_login.md`: Auth + webview proxy. Revisit before implementing or modifying AWP.
- `planner/docs/zed_acp_mcp_integration.md`: MCP proxy pipeline. Check whenever we proxy tool calls or register MCP servers.
- `planner/docs/zed_acp_premissions.md`: Permission UX + policy storage. Use as the template for our approval model.
- `planner/docs/zed_acp_session_management.md`: Session lifecycle and reconnect logic. Consult before changing session routing.
- `planner/docs/zed_acp_tools.md`: Tool catalog and ACP bindings. Reference when exposing or gating file/terminal tools.
- `planner/docs/zed_acp_ui.md`: UX decisions. Useful when syncing web UI affordances with protocol events.
- `planner/docs/zed_acp_debugging.md`: Diagnostics. Borrow ideas whenever we need better logging or troubleshooting hooks.

**WHEN TO DIVE INTO THE EXTERNAL CODEBASES**
The vendored repos under `external_refrence/` are checked out to the exact versions we target. Treat them as living specs—open them whenever you wonder how the real implementations solve a problem.
- `external_refrence/claude-code-acp`: Claude's ACP adapter. Explore to see how agents structure `initialize`, session streaming, tool invocation, and MCP proxying. Copy interaction patterns rather than guessing.
- `external_refrence/agent-client-protocol`: Canonical ACP library (Rust + TS) plus schema. Use it to confirm field names, error semantics, and helper APIs before writing code. Prefer importing its types over rolling our own.
- `external_refrence/zed`: Zed editor source code. Reference for GPUI patterns, UI architecture, and ACP client implementation. Check `CLAUDE.md` for Rust coding guidelines and GPUI best practices.
- `external_refrence/opencode`: Open-source AI coding agent built for terminals. Study its client/server architecture, TUI design patterns, and multi-provider LLM integration for terminal-based AI tool inspiration.
- `external_refrence/sst`: Infrastructure-as-code framework using Pulumi/Terraform. Reference for build patterns, CLI design, and TypeScript/Go polyglot project structure. See `CLAUDE.md` for build commands and style guidelines.

## Additional Guidance
- CT-BRIDGE is an ACP **client-side** implementation. It forwards ACP JSON-RPC between CT-WEB and downstream agents while owning local capabilities (fs, permissions, terminal). Keep it thin—no bespoke agent logic.
- Always lean on the official `agent-client-protocol` crates for message handling. Typed APIs keep us aligned on negotiated versions, capabilities, and structured errors.
- Filesystem, terminal, and permission methods are invoked by the agent. Implement the ACP `Client` trait methods (`fs/read_text_file`, `fs/write_text_file`, `terminal/*`) with strict project-root sandboxing and policy checks before responding.
- Maintain a routing map keyed by `(bridgeId, sessionId)` so multiple agents or sessions can share one WebSocket. Forward payloads verbatim aside from injecting `_meta.bridgeId` where required.
- Treat `planner/spec.md` as the contract. Update the spec first whenever behavior changes, then implementation, then tests. Surface mismatches immediately.
- Borrow architectural patterns from Zed’s docs—process lifecycle, permission prompts, MCP proxying—but adapt configuration and UX to our requirements.
- Before merging bridge changes, run integration tests that simulate a full agent conversation (`initialize` → `session/new` → tool calls). Add regression coverage whenever routing or capability negotiation shifts.

## Build, Test, and Development Commands
- `cargo fmt --all` format before commits.
- `cargo clippy --all-targets --all-features -D warnings` lint gate.
- `cargo test` async suite; append `bridge_handshake` to focus.
- `cargo run --bin ct-bridge` runs the bridge with handshake logs.
- `pnpm install --dir ct-web && pnpm --dir ct-web dev` serves the dashboard at http://localhost:5173.

## Coding Style & Naming Conventions
Stick to four-space indentation and Rust 2021 idioms: snake_case for modules and functions, PascalCase for public types such as `BridgeConfig`, and SCREAMING_SNAKE_CASE for constants (`ALLOWED_ORIGIN`). Favor `Result`-returning async helpers over panics and add `///` docs when you expand the public API. For front-end work, keep Solid components in PascalCase files under `ct-web/src` and use camelCase props with Vite-managed asset imports.

## Testing Guidelines
Tests run on Tokio's multi-thread runtime; annotate new async cases with `#[tokio::test(flavor = "multi_thread")]`. Use `serial_test::serial` whenever filesystem state or environment variables are involved, mirroring `tests/bridge_handshake.rs`. Preserve spec callouts such as `RAT-LWS-REQ-###` in assertions, and run `cargo test -- --nocapture` when you need more logging. For UI logic, add Vitest suites under `ct-web/src` and expose them through a `pnpm --dir ct-web test` script.

## Commit & Pull Request Guidelines
Git history favors short, prefixed subjects (for example, `step(007): …` or `plan 9 tests`). Keep that pattern when extending roadmap items; otherwise, use an imperative verb under 72 characters. Note intent, trade-offs, and validation in the body for any non-trivial change. PRs should summarize user impact, list the commands you ran (`cargo test`, `pnpm --dir ct-web dev` smoke check), link the relevant spec items or issues, and include screenshots when UI behavior changes.
