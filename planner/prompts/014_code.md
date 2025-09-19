Title: Step 014: Make tests pass for persisted fs/write_text_file policies

Context (read-only inputs):
• Spec: planner/spec.md
• Human hints: planner/human.md
• Progress: planner/progress.md
• New failing tests from: planner/prompts/014_test.md

Acceptance (must satisfy):
• When fs/write_text_file is approved with the `allow_always` option, the bridge must persist that decision and reload it after a restart so the very next write to the same canonical path succeeds without triggering another permission request. The policy store location must be configurable so tests can point it at a temporary file, and the bridge should skip touching the agent during the second write. The test must assert both the persisted success response and that the agent observed no additional `request_permission` calls on the second bridge run.

Plan (you follow this order):
1. GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
2. RE-RUN TESTS — pnpm vitest --run && cargo test
3. REFACTOR — Improve clarity/structure without changing behavior.
4. LINT/FORMAT — cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format
5. FINAL TEST — pnpm vitest --run && cargo test must be fully green.

Notes & logging (append-only):
• Document implementation details, command outputs, and evidence in planner/notes/014_code.md using `§ CODE` blocks with timestamps.
• Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
• Reference any follow-up tasks or regressions discovered during the step.

Constraints:
• Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
• Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(014): persisted fs/write policies — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: <1 line>

Exit condition:
• All tests pass; lints/format pass; acceptance demonstrably true.
