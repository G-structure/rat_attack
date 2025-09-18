Title: Step 006: Make tests pass for sandboxed fs/read_text_file

Context (read-only inputs):
	• Spec: planner/spec.md
	• Human hints: planner/human.md
	• Progress: planner/progress.md
	• New failing tests from: planner/prompts/006_test.md

Acceptance (must satisfy):
	• Bridge must handle `fs/read_text_file` JSON-RPC requests by returning file contents when the absolute `path` lies within a configured project root (per RAT-LWS-REQ-040).
	• Requests for paths outside every project root must be rejected with a JSON-RPC error explaining the sandbox violation (RAT-LWS-REQ-044).
	• Each error response must include both `code`/`message` and a `data.details` string to satisfy structured error expectations (RAT-LWS-REQ-132).

Plan (you follow this order):
	1. GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
	2. RE-RUN TESTS — `pnpm vitest --run && cargo test`
	3. REFACTOR — Improve clarity/structure without changing behavior.
	4. LINT/FORMAT — `cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format`
	5. FINAL TEST — `pnpm vitest --run && cargo test` must be fully green.

Notes & logging (append-only):
	• Document implementation details, command outputs, and evidence in planner/notes/006_code.md using `§ CODE` blocks with timestamps.
	• Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
	• Reference any follow-up tasks or regressions discovered during the step.

Constraints:
	• Do not edit tests unless they are objectively incorrect; if you must, note it in planner/progress.md under this step.
	• Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

```
step(006): sandboxed fs/read_text_file — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: <1 line>
```

Exit condition:
	• All tests pass; lints/format pass; acceptance demonstrably true.
