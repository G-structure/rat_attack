## 2025-09-18 12:00 — Init Rust CT-BRIDGE Project
[session link not saved]

**Context:** Kick off CT-BRIDGE implementation per spec.md (RAT-LWS-REQ-001 to 305), starting with minimal Cargo project structure and ACP dep. Why now? Establishes foundation for bridge WS server, ACP forwarding, and agent spawning without disrupting existing planner/docs.

**Plan:**
- [x] Create Cargo.toml with agent-client-protocol v0.4.0 dep and basic metadata
- [x] Create src/main.rs with placeholder CT-BRIDGE skeleton (WS server stub, ACP init)
- [x] Run cargo check to verify deps resolve and no syntax errors
- [x] Run cargo build to ensure compiles successfully

**Prompts/Notes:** planner/notes/001.md

**Status:** APPLIED

## 2025-09-18 13:00 — Init CT-WEB SolidJS + Vite App
[session link](https://opencode.ai/s/4fpY1H3R)

**Context:** Initialize CT-WEB web app using pnpm, SolidJS + Vite, TypeScript, and Tailwind CSS as per spec.md (CT-WEB requirements). User chose SolidJS + Vite over Solid Start for simpler setup. Why now? Establishes foundation for SolidJS WebUI to connect to CT-BRIDGEs over WebSockets, enabling ACP control plane implementation.

**Plan:**
- [x] Verify pnpm installed; install if missing
- [x] Run pnpm create solid@latest ct-web with ts + tailwind template (selected SolidJS + Vite)
- [x] Install dependencies with pnpm install
- [x] Approve builds for @tailwindcss/oxide, esbuild
- [x] Re-run pnpm install after approval
- [x] Test dev server starts successfully

**Prompts/Notes:** planner/notes/002.md

**Status:** APPLIED

## 2025-09-18 14:00 — Implement WS Upgrade with Origin Validation
[session test](https://opencode.ai/s/eoBOmRFL)
[session code](https://opencode.ai/s/7VEUOVx2)

**Context:** Implement RAT-LWS-REQ-001 for CT-BRIDGE: start WS server on port 8137 validating Origin header against configurable allow-list (default ["http://localhost:5173"]), return 403 for invalid origins, proceed for valid. Smallest diffs to make tests pass, refactor safely.

**Plan:**
- [x] Add tokio-tungstenite and tungstenite to Cargo.toml
- [x] Implement basic WS server in src/main.rs with origin validation logic
- [x] Run cargo test --test ws_upgrade (with server running)
- [x] Refactor: extract Config struct and run_server function
- [x] Lint/format: cargo clippy --fix && cargo fmt
- [x] Final test: cargo test --test ws_upgrade fully green
- [x] Fix tests: added missing WS handshake headers (Sec-WebSocket-Key, etc.)
- [x] Create justfile for test running
- [x] Document test running in README.md

**Prompts/Notes:** prompts/001_code.md, prompts/001_test.md

**Status:** APPLIED

## PLAN 002 — Implement WS Subprotocol Echo
[test session](https://opencode.ai/s/K777n09f)
[code session](https://opencode.ai/s/6cjeYEUS)

[x] 002 — Implement WS Subprotocol Echo
• acceptance: CT-BRIDGE MUST echo exactly "acp.jsonrpc.v1" when offered in Sec-WebSocket-Protocol; if not offered or different, close with code 1008.
• prompts: prompts/002_test.md, prompts/002_code.md
• status: applied
• notes:
    - context: WS upgrade handling in src/main.rs; tests in tests/ws_upgrade.rs
    - js: N/A
    - rust: cargo test --test ws_upgrade
    - follow-ups: Next could be initialize with bridgeId (REQ-300) or JSON-RPC framing (REQ-011)

# PLAN 003 — Subprotocol list handling
[test session](https://opencode.ai/s/Z7AQ6xxy)
[code session](https://opencode.ai/s/DoZE95iu)

[x] 003 — Subprotocol list handling
• acceptance: Bridge accepts handshake when acp.jsonrpc.v1 is among offered subprotocols and echoes it.
• prompts: [prompts/003_test.md](./prompts/003_test.md), [prompts/003_code.md](./prompts/003_code.md)
• status: applied
• notes:
    - context: WebSocket upgrade handling in src/main.rs; integration coverage in tests/ws_upgrade.rs
    - js: not run
    - rust: cargo test --test ws_upgrade (pass)
    - follow-ups: ensure initialize handshake covers bridgeId (RAT-LWS-REQ-300)

## PLAN 004 — Initialize bridgeId response
[test session](https://opencode.ai/s/39YRKR6d)
[code session](https://opencode.ai/s/ViPGCSvj)

[x] 004 — Initialize bridgeId response
• acceptance: Bridge must return stable non-empty bridgeId in initialize JSON-RPC response across repeated calls.
• prompts: [prompts/004_test.md](./prompts/004_test.md), [prompts/004_code.md](./prompts/004_code.md)
• status: applied
• notes:
    - context: JSON-RPC handling in src/main.rs; integration tests in tests/ws_upgrade.rs (new coverage around initialize)
    - js: not run (no frontend tests configured yet)
    - rust: cargo test --test ws_upgrade (8/8 tests pass)
    - follow-ups: extend initialize payload to advertise bridge capabilities per spec (RAT-LWS-REQ-020)

## PLAN 005 — Initialize fs capabilities enforcement
[test session](https://opencode.ai/s/StHgV5f5)
[code session](https://opencode.ai/s/0jzTvCXx)

[x] 005 — Initialize fs capabilities enforcement
• acceptance: Bridge initialize echoes fs capabilities and rejects missing ones (RAT-LWS-REQ-020)
• prompts: [prompts/005_test.md](./prompts/005_test.md), [prompts/005_code.md](./prompts/005_code.md)
• status: applied
• notes:
    - context: src/main.rs; tests/ws_upgrade.rs
    - js: N/A (no frontend tests configured)
    - rust: cargo test --test ws_upgrade (14/14 tests pass)
    - follow-ups: consider terminal capability advertisement (RAT-LWS-REQ-060)
• next: explore RAT-LWS-REQ-021 agent capability negotiation
• next: prep auth preflight coverage for RAT-LWS-REQ-022
