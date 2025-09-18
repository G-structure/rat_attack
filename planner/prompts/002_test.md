Purpose: produce failing tests first, nothing else. Keep the working set minimal. Never modify planner/spec.md.

Title: Step 002: Write failing tests for WS subprotocol echo

Context (read-only inputs):
	•	Spec: planner/spec.md (do not edit)
	•	Human hints: planner/human.md (follow constraints)
	•	Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
	•	CT-BRIDGE MUST echo exactly one offered subprotocol token; token is ~acp.jsonrpc.v1~ (RAT-LWS-REQ-002).
	•	If the client offers "acp.jsonrpc.v1" in Sec-WebSocket-Protocol, the bridge MUST echo it back in the 101 response.
	•	If the client offers no subprotocol or a different one, the bridge MUST close the connection with code 1008 (policy violation).

Scope & files:
	•	Target area: WS upgrade handling in CT-BRIDGE.
	•	You may create/modify only test files and light test scaffolding.
	•	Rust: unit tests in tests/ws_upgrade.rs (integration tests for WS server).

What to deliver:
	1.	Minimal RED tests that fail against current code.
	2.	Tests must pin observable behavior (no over-mocking; avoid brittle implementation details).
	3.	For WS: use tungstenite test client to simulate upgrade requests with/without subprotocol.

Commands you will run:

pnpm vitest --run
cargo test

Constraints:
	•	Do not change application code.
	•	Keep test names descriptive (e.g., ws_upgrade::subprotocol_echo_valid).
	•	If you must introduce test utilities, put them in tests/utils/ with the smallest viable footprint.

Exit condition:
	•	You leave the repo in a state where cargo test fails due to the new tests.