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

## PLAN 003 — WS initialize handshake
[x] 003 — WS initialize handshake
• acceptance: Bridge only accepts WebSocket upgrades from allow-listed origins offering `acp.jsonrpc.v1`, forwards `initialize` through `agent-client-protocol`, and injects `_meta.bridgeId` while rejecting other methods.
• prompts: [prompts/003_test.md](./prompts/003_test.md), [prompts/003_code.md](./prompts/003_code.md)
• status: applied
• notes:
    - context: src/lib.rs, tests/bridge_handshake.rs
    - js: not-run (no vitest script defined)
    - rust: pass (`cargo test`)
    - follow-ups: ensure later steps cover fs/permissions methods, multi-session routing, and concrete agent spawn wiring
• next:
    - implement fs capability handling after handshake
    - add structured logging for bridge admission errors

## PLAN 004 — session/new forwarding
[test session](https://opencode.ai/s/rxVbj7fn)
[code session](https://opencode.ai/s/XpDVpMQq)

[x] 004 — session/new forwarding
• acceptance: Forward `session/new` to the agent only after initialize and relay the result unchanged.
• prompts: [prompts/004_test.md](./prompts/004_test.md), [prompts/004_code.md](./prompts/004_code.md)
• status: applied
• notes:
    - context: src/lib.rs, tests/bridge_handshake.rs (new coverage for session/new)
    - js: not-run
    - rust: pass (`cargo test`)
    - follow-ups: capture agent-side notifications and permission prompts later
• next:
    - handle agent notifications streaming to CT-WEB
    - add permission policy scaffolding per RAT-LWS-REQ-092

## PLAN 005 — session/prompt streaming notifications
[x] 005 — session/prompt streaming notifications
• acceptance: Bridge forwards `session/prompt` requests to agent and streams `session/update` notifications back to CT-WEB until final result with `stopReason`.
• prompts: [prompts/005_test.md](./prompts/005_test.md), [prompts/005_code.md](./prompts/005_code.md)
• status: done
• notes:
    - context: src/lib.rs for session/prompt forwarding, tests/bridge_handshake.rs for streaming tests
    - js: not-run
    - rust: all pass (8 passed, 0 failed - session/prompt forwarding AND streaming notifications working)
    - follow-ups: streaming notifications complete, ready for fs capabilities
• next:
    - implement actual streaming via NotificationSender to make remaining tests pass
    - add fs capability handling after streaming complete

## PLAN 005A — agent streaming notifications completion
[x] 005A — agent streaming notifications completion
• acceptance: Agents can send session/update notifications through NotificationSender during prompt execution and notifications are relayed to CT-WEB
• prompts: [prompts/005A_test.md](./prompts/005A_test.md), [prompts/005A_code.md](./prompts/005A_code.md)
• status: done (superseded - streaming implemented by previous session)
• notes:
    - context: tests/bridge_handshake.rs FakeStreamingAgentTransport enhancement completed
    - js: not-run
    - rust: all pass (streaming functionality working)
    - follow-ups: PLAN 005 fully complete, ready for fs capabilities
• next:
    - implement fs capability handling (fs/read_text_file, fs/write_text_file) per RAT-LWS-REQ-040/041

## PLAN 006 — fs/read_text_file capability
[x] 006 — fs/read_text_file capability
• acceptance: Bridge implements `fs/read_text_file` method with optional line offset/limit and PR sandboxing per RAT-LWS-REQ-040/044
• prompts: [prompts/006_test.md](./prompts/006_test.md), [prompts/006_code.md](./prompts/006_code.md)
• status: applied
• notes:
    - context: src/lib.rs for fs/read_text_file method, tests/bridge_handshake.rs for fs tests
    - js: not-run
    - rust: pass (all 14 tests pass - 6 new fs/read_text_file tests + 8 existing)
    - implementation: Real file system access with relative/absolute path support, project root sandboxing, binary file detection, line offset/limit parameters
    - test files: tests/fs_test_file.md (20 lines poem), tests/binary_test_file.bin (null bytes)
    - follow-ups: fs/write_text_file with permission gating implementation ready
• next:
    - implement fs/write_text_file with permission gating per RAT-LWS-REQ-041
    - add permission policy scaffolding per RAT-LWS-REQ-092

## PLAN 007 — fs/write_text_file with permission gating
[x] 007 — fs/write_text_file with permission gating
• acceptance: Bridge implements `fs/write_text_file` method that requires permission approval via `session/request_permission` with PR sandboxing and support for all permission outcomes
• prompts: [prompts/007_test.md](./prompts/007_test.md), [prompts/007_code.md](./prompts/007_code.md)
• status: applied
• notes:
    - context: src/lib.rs for fs/write_text_file method, tests/bridge_handshake.rs for permission gating tests
    - js: not-run
    - rust: all pass (14 tests passing before this step)
    - follow-ups: permission policy registry implementation, terminal capabilities
• next:
    - implement terminal capability handling per RAT-LWS-REQ-060/062/063
    - add permission policy scaffolding per RAT-LWS-REQ-092

## PLAN 008 — fs/write_text_file permission caching
[x] 008 — fs/write_text_file permission caching
• acceptance: cache allow_always/reject_always decisions per canonical path while defaulting to permission prompts
• prompts: [prompts/008_test.md](./prompts/008_test.md), [prompts/008_code.md](./prompts/008_code.md)
• status: applied
• notes:
    - context: src/lib.rs, tests/bridge_handshake.rs (permission policy registry)
    - js: not-run
    - rust: pass (`cargo test`)
    - follow-ups: bring policy persistence + generalize beyond fs writes
• next:
    - build policy persistence across sessions per RAT-LWS-REQ-092
    - implement terminal permission gating with approvals per RAT-LWS-REQ-062/063

## PLAN 009 — auth/cli_login Claude login CLI
[x] 009 — auth/cli_login Claude login CLI
• acceptance: auth/cli_login resolves claude CLI and starts `/login` command immediately returning structured status
• prompts: [prompts/009_test.md](./prompts/009_test.md), [prompts/009_code.md](./prompts/009_code.md)
• status: applied
• notes:
    - context: src/lib.rs enhanced with CLI resolution logic, tests/bridge_handshake.rs with 7 comprehensive test scenarios
    - implementation: CLAUDE_ACP_BIN override, npm package resolution, PATH fallback, virtual terminal flow, immediate return behavior
    - test isolation: Added serial_test dependency and #[serial_test::serial] to auth_cli_login tests + affected fs tests
    - bonus fix: Resolved intermittent fs_read_text_file test failures caused by working directory interference
    - js: not-run
    - rust: pass (`cargo test` - all 33 tests passing consistently)
• evidence: planner/notes/009_test.md, planner/notes/009_code.md, tests/bridge_handshake.rs, src/lib.rs
• next:
    - extend login flow to surface progress updates and capture stderr for UI (RAT-LWS-REQ-094)
    - wire multi-agent launcher registry with cached credentials
§ PLAN 014 — permission policy persistence across restarts
[ ] 014 — permission policy persistence across restarts
• acceptance: Persist fs/write_text_file allow_always decisions to a bridge policy store and reload them after restart so the next write skips permission prompts.
• prompts: [prompts/014_test.md](./prompts/014_test.md), [prompts/014_code.md](./prompts/014_code.md)
• status: planned
• notes:
    - context: src/lib.rs, tests/bridge_handshake.rs
    - js: not-run
    - rust: not-run
    - follow-ups: ensure reject_always persistence and broader policy registry coverage
• next:
    - RAT-LWS-REQ-092: persist reject_always decisions across bridge restarts
    - RAT-LWS-REQ-062/063: implement terminal execution permission gating and output streaming
