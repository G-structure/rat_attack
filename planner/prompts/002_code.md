Purpose: write the smallest diffs that make the 002 tests pass, then refactor safely. Never modify planner/spec.md.

Title: Step 002: Make tests pass for WS subprotocol echo

Context (read-only inputs):
	•	Spec: planner/spec.md
	•	Human hints: planner/human.md
	•	Progress: planner/progress.md
	•	New failing tests from: planner/prompts/002_test.md

Acceptance (must satisfy):
	•	CT-BRIDGE MUST echo exactly one offered subprotocol token; token is ~acp.jsonrpc.v1~ (RAT-LWS-REQ-002).
	•	If the client offers "acp.jsonrpc.v1" in Sec-WebSocket-Protocol, the bridge MUST echo it back in the 101 response.
	•	If the client offers no subprotocol or a different one, the bridge MUST close the connection with code 1008 (policy violation).

Plan (you follow this order):
	1.	GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
	2.	RE-RUN TESTS — pnpm vitest --run && cargo test
	3.	REFACTOR — Improve clarity/structure without changing behavior.
	4.	LINT/FORMAT — cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format
	5.	FINAL TEST — pnpm vitest --run && cargo test must be fully green.

Constraints:
	•	Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
	•	Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(002): WS subprotocol echo — green

- tests: ws_upgrade::subprotocol_echo_valid, ws_upgrade::subprotocol_echo_invalid
- touched: src/main.rs
- acceptance: Bridge echoes "acp.jsonrpc.v1" if offered, closes 1008 otherwise

Exit condition:
	•	All tests pass; lints/format pass; acceptance demonstrably true.