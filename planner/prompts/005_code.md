Title: Step 005: Make tests pass for initialize fs capability enforcement

Context (read-only inputs):
	• Spec: planner/spec.md
	• Human hints: planner/human.md
	• Progress: planner/progress.md
	• New failing tests from: planner/prompts/005_test.md

Acceptance (must satisfy):
	• After a successful ACP WebSocket upgrade, when CT-WEB sends `initialize` with `capabilities.fs.readTextFile` and `capabilities.fs.writeTextFile` both true, the bridge must reply with a JSON-RPC 2.0 result containing `_meta.bridgeId` and a `capabilities.fs` section that echoes both booleans as true.
	• If the client omits either fs capability or sets it false, the bridge must return a JSON-RPC error (use code -32602) describing the missing requirement instead of a success result.
	• Responses must remain well-formed JSON-RPC frames and the connection should stay open for subsequent requests.

Plan (you follow this order):
	1. GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
	2. RE-RUN TESTS — pnpm vitest --run && cargo test
	3. REFACTOR — Improve clarity/structure without changing behavior.
	4. LINT/FORMAT — cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format
	5. FINAL TEST — pnpm vitest --run && cargo test must be fully green.

Notes & logging (append-only):
	• Document implementation details, command outputs, and evidence in `planner/notes/005_code.md` using `§ CODE` blocks with timestamps.
	• Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
	• Reference any follow-up tasks or regressions discovered during the step.

Constraints:
	• Do not edit tests unless they are objectively incorrect; if so, fix them minimally and note it in planner/progress.md under this step.
	• Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(005): initialize fs capability enforcement — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: <1 line>

Exit condition:
	• All tests pass; lints/format pass; acceptance demonstrably true.
