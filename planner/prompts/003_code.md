Title: Step 003: Make tests pass for subprotocol list handling

Context (read-only inputs):
	• Spec: planner/spec.md
	• Human hints: planner/human.md
	• Progress: planner/progress.md
	• New failing tests from: planner/prompts/003_test.md

Acceptance (must satisfy):
	• CT-BRIDGE must accept WebSocket upgrades when the client offers multiple subprotocol tokens including `acp.jsonrpc.v1`, and it must echo `acp.jsonrpc.v1` in the handshake response. Connections that do not include `acp.jsonrpc.v1` should continue to be rejected with the existing policy close.

Plan (you follow this order):
	1. GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
	2. RE-RUN TESTS — pnpm vitest --run && cargo test
	3. REFACTOR — Improve clarity/structure without changing behavior.
	4. LINT/FORMAT — cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format
	5. FINAL TEST — pnpm vitest --run && cargo test must be fully green.

Constraints:
	• Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
	• Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(003): subprotocol list handling — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: <1 line>

Exit condition:
	• All tests pass; lints/format pass; acceptance demonstrably true.
