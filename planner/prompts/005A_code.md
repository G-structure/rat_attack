Purpose: write the smallest diffs that make the 005A tests pass, then refactor safely. Never modify planner/spec.md.

Title: Step 005A: Make streaming notification tests pass

Context (read-only inputs):
- Spec: planner/spec.md
- Human hints: planner/human.md
- Progress: planner/progress.md
- New failing tests from: planner/prompts/005A_test.md

Acceptance (must satisfy):
- Agents can send session/update notifications through NotificationSender during prompt execution
- Notifications are relayed from agent through bridge to CT-WEB without modification
- The 2 previously failing streaming tests now pass

Plan (you follow this order):
1. GREEN — Implement any missing bridge infrastructure to support agent notification streaming
2. RE-RUN TESTS — `cargo test`
3. REFACTOR — Improve clarity/structure without changing behavior
4. LINT/FORMAT — `cargo clippy --fix -q && cargo fmt`
5. FINAL TEST — `cargo test` must be fully green

Notes & logging (append-only):
- Document implementation details, command outputs, and evidence in planner/notes/005A_code.md using `§ CODE` blocks with timestamps.
- Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
- Reference any follow-up tasks or regressions discovered during the step.

Constraints:
- Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
- Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(005A): agent streaming notifications — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: Agents send session/update notifications through bridge to CT-WEB

Exit condition:
- All tests pass; lints/format pass; acceptance demonstrably true.