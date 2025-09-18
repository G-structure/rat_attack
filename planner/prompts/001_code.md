Purpose: write the smallest diffs that make the 001 tests pass, then refactor safely. Never modify planner/spec.md.

Title: Step 001: Make tests pass for WS upgrade with origin validation

Context (read-only inputs):
	•	Spec: planner/spec.md
	•	Human hints: planner/human.md
	•	Progress: planner/progress.md
	•	New failing tests from: planner/prompts/001_test.md

Acceptance (must satisfy):
	•	CT-BRIDGE starts a WS server on port 8137 that validates the Origin header against a configurable allow-list (defaulting to ["http://localhost:5173"]), returning HTTP 403 for invalid origins and proceeding with upgrade for valid ones, per RAT-LWS-REQ-001.

Plan (you follow this order):
	1.	GREEN — Implement the smallest change touching 1–3 files max to pass the new tests: Add tokio-tungstenite to Cargo.toml, implement basic WS server in src/main.rs with origin validation logic.
	2.	RE-RUN TESTS — cargo test --test ws_upgrade
	3.	REFACTOR — Improve clarity/structure without changing behavior (extract config, error handling).
	4.	LINT/FORMAT — cargo clippy --fix -q && cargo fmt
	5.	FINAL TEST — cargo test --test ws_upgrade must be fully green.

Constraints:
	•	Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
	•	Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(001): Implement WS upgrade with origin validation — green

- tests: test_valid_origin_upgrade, test_invalid_origin_rejection
- touched: src/main.rs, Cargo.toml
- acceptance: WS server validates Origin header per RAT-LWS-REQ-001

Exit condition:
	•	All tests pass; lints/format pass; acceptance demonstrably true.