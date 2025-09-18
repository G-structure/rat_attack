Title: Step 004: Write failing tests for initialize bridgeId response

Context (read-only inputs):
	• Spec: planner/spec.md (do not edit)
	• Human hints: planner/human.md (follow constraints)
	• Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
	• After a successful WebSocket upgrade using the ACP subprotocol, CT-BRIDGE must respond to a JSON-RPC `initialize` request with a result payload that includes `_meta.bridgeId`. The returned `bridgeId` must be a non-empty string and remain the same across multiple `initialize` calls on the same connection.

Scope & files:
	• Target area: `tests/ws_upgrade.rs` (add new async test) or a new integration test under `tests/` if clearer.
	• You may create/modify only test files and light test scaffolding.
	• SolidJS: co-located tests *.test.tsx (Vitest + Testing Library).
	• Rust: unit tests beside code (mod tests {}) or tests/{name}.rs for integration.

What to deliver:
	1. Minimal RED test(s) that fail against current code because the bridge does not yet send the expected initialize response.
	2. Tests must exercise the live WebSocket connection (no mocks) and assert the JSON structure of the response using serde_json.
	3. Ensure the test drives two `initialize` calls to verify stability of the `bridgeId`.

Commands you will run:

cargo test --test ws_upgrade

Constraints:
	• Do not change application code.
	• Keep test names descriptive (<module>: <behavior>).
	• If you must introduce test utilities, put them in tests/utils/ with the smallest viable footprint.

Exit condition:
	• You leave the repo in a state where cargo test --test ws_upgrade fails due to the new test.
