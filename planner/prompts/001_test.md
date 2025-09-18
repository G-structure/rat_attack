Purpose: produce failing tests first, nothing else. Keep the working set minimal. Never modify planner/spec.md.

Title: Step 001: Write failing tests for WS upgrade with origin validation

Context (read-only inputs):
	•	Spec: planner/spec.md (do not edit)
	•	Human hints: planner/human.md (follow constraints)
	•	Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
	•	CT-BRIDGE starts a WS server on port 8137 that validates the Origin header against a configurable allow-list (defaulting to ["http://localhost:5173"]), returning HTTP 403 for invalid origins and proceeding with upgrade for valid ones, per RAT-LWS-REQ-001.

Scope & files:
	•	Target area: WS transport layer in CT-BRIDGE
	•	You may create/modify only test files and light test scaffolding.
	•	Rust: integration tests in tests/ws_upgrade.rs for WS server behavior.
	•	Do not modify src/main.rs or Cargo.toml yet.

What to deliver:
	1.	Minimal RED integration tests that fail against current code (which has no WS server).
	2.	Tests must pin observable behavior: one test for valid origin upgrade success, one for invalid origin 403 rejection.
	3.	Use tokio-tungstenite test client to simulate WS upgrade attempts.

Commands you will run:

cargo test --test ws_upgrade

Constraints:
	•	Do not change application code.
	•	Keep test names descriptive (e.g., test_valid_origin_upgrade, test_invalid_origin_rejection).
	•	If you must introduce test utilities, put them in tests/utils/ with the smallest viable footprint.

Exit condition:
	•	You leave the repo in a state where cargo test --test ws_upgrade fails due to the new tests (no WS server implemented yet).