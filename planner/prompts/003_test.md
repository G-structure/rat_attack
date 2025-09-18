Title: Step 003: Write failing tests for subprotocol list handling

Context (read-only inputs):
	• Spec: planner/spec.md (do not edit)
	• Human hints: planner/human.md (follow constraints)
	• Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
	• CT-BRIDGE must accept WebSocket upgrades when the client offers multiple subprotocol tokens including `acp.jsonrpc.v1`, and it must echo `acp.jsonrpc.v1` in the handshake response. Connections that do not include `acp.jsonrpc.v1` should continue to be rejected with the existing policy close.

Scope & files:
	• Target area: `tests/ws_upgrade.rs`
	• You may create/modify only test files and light test scaffolding.
	• SolidJS: co-located tests *.test.tsx (Vitest + Testing Library).
	• Rust: unit tests beside code (mod tests {}) or tests/{name}.rs for integration.

What to deliver:
	1. Minimal RED tests that fail against current code.
	2. Tests must pin observable behavior (no over-mocking; avoid brittle implementation details).
	3. For UI: prefer Testing Library queries of accessible roles/labels; if snapshots are needed, keep them tiny and stable.

Commands you will run:

cargo test --test ws_upgrade

Constraints:
	• Do not change application code.
	• Keep test names descriptive (<module>: <behavior>).
	• If you must introduce test utilities, put them in tests/utils/ (Rust) or test/utils/ (JS) with the smallest viable footprint.

Exit condition:
	• You leave the repo in a state where cargo test --test ws_upgrade fails due to the new tests.
