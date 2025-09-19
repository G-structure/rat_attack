Title: Step 012: Make tests pass for permission policy persistence

Context (read-only inputs):
• Spec: planner/spec.md
• Human hints: planner/human.md
• Progress: planner/progress.md
• New failing tests from: planner/prompts/012_test.md

Acceptance (must satisfy):
• Permission cache decisions for `fs/write_text_file` MUST persist across bridge restarts.
• When a path is approved with `allow_always`, subsequent writes after restarting the bridge (with the same policy store path) proceed without a new permission request.
• When a path is marked `reject_always`, the bridge must reject subsequent writes immediately without contacting the agent.
• Paths lacking entries continue to request permission.

Plan (you follow this order):
1. GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
2. RE-RUN TESTS — pnpm vitest --run && cargo test
3. REFACTOR — Improve clarity/structure without changing behavior.
4. LINT/FORMAT — cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format
5. FINAL TEST — pnpm vitest --run && cargo test must be fully green.

Notes & logging (append-only):
• Document implementation details, command outputs, and evidence in planner/notes/012_code.md using `§ CODE` blocks with timestamps.
• Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
• Reference any follow-up tasks or regressions discovered during the step.

Constraints:
• Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
• Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(012): permission policy persistence — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: Persist allow/reject always decisions across bridge restarts

Exit condition:
• All tests pass; lints/format pass; acceptance demonstrably true.
