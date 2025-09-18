Purpose: write the smallest diffs that make the 005 tests pass, then refactor safely. Never modify planner/spec.md.

Title: Step 005: Make tests pass for session/prompt streaming notifications

Context (read-only inputs):
- Spec: planner/spec.md
- Human hints: planner/human.md
- Progress: planner/progress.md
- New failing tests from: planner/prompts/005_test.md

Acceptance (must satisfy):
- Bridge forwards `session/prompt` requests to agent and streams `session/update` notifications back to CT-WEB until final result with `stopReason`.
- Agent notifications are relayed transparently without modification except for any required bridgeId injection.
- JSON-RPC notification format preserved per RAT-LWS-REQ-011.

Plan (you follow this order):
1. GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
2. RE-RUN TESTS — `cargo test`
3. REFACTOR — Improve clarity/structure without changing behavior.
4. LINT/FORMAT — `cargo clippy --fix -q && cargo fmt`
5. FINAL TEST — `cargo test` must be fully green.

Notes & logging (append-only):
- Document implementation details, command outputs, and evidence in planner/notes/005_code.md using `§ CODE` blocks with timestamps.
- Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
- Reference any follow-up tasks or regressions discovered during the step.

Constraints:
- Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
- Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(005): session/prompt streaming notifications — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: Bridge forwards session/prompt and streams session/update notifications

Exit condition:
- All tests pass; lints/format pass; acceptance demonstrably true.