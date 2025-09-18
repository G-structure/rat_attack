Title: Step 003: Write failing tests for WS initialize handshake

Context (read-only inputs):
    • Spec: planner/spec.md (do not edit)
    • Human hints: planner/human.md (follow constraints)
    • Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
    • Bridge only accepts WebSocket upgrades from allow-listed origins offering `acp.jsonrpc.v1`, forwards `initialize` through `agent-client-protocol` APIs, and injects `_meta.bridgeId` while rejecting other methods.

Scope & files:
    • Target area: Rust integration tests in `tests/`
    • You may create/modify only test files and light test scaffolding.
    • Rust: prefer integration tests in `tests/bridge_handshake.rs` that drive the bridge via its public API.

What to deliver:
    1. Minimal RED tests that fail against current code.
    2. Tests must assert observable behavior (e.g., handshake rejection, JSON-RPC responses) and verify that `agent-client-protocol` transport methods (e.g., `AgentTransport::initialize`) are invoked with typed requests.
    3. Keep any helper utilities small and co-located in `tests/` if required.

Notes & logging (append-only):
    • Append observations, decisions, and evidence to planner/notes/003_test.md using `§ TEST` blocks with timestamps.
    • Include the commands you ran, failing output snippets, and pointers to generated artifacts.

Commands you will run:

cargo test

Constraints:
    • Do not change application code.
    • Keep test names descriptive (`bridge_handshake: accepts initialize` etc.).
    • Any test doubles must conform to the `AgentTransport` trait and use `agent-client-protocol` request/response types.

Exit condition:
    • Leave the repo in a state where `cargo test` fails because of the newly added tests.
