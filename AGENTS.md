# Repository Guidelines

## Project Overview
This repository powers a local-first remote control stack: `ct-web` (SolidJS) talks to one or more `ct-bridge` processes that proxy ACP agents over WebSockets, with an Authentication WebView Proxy keeping logins on the agent host.

- Multi-bridge sessions from a single dashboard
- Permission-gated file edits inside project roots
- Streaming terminal execution
- Mobile-optimized responsive UI
- Authentication that maintains IP alignment for provider checks

## Project Structure & Module Organization
The Rust bridge lives in `src/`; `main.rs` is the entrypoint and `lib.rs` houses the bridge and transport helpers. Integration tests under `tests/` (e.g., `bridge_handshake.rs`) assert ACP flows alongside their fixtures. `ct-web/` contains the SolidJS dashboard, and `planner/` tracks specs and prompts. Build outputs land in `target/`; leave the root cache sentinels untouched.

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
