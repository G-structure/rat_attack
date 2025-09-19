Title: Step 011: Make tests pass for auth/cli_login progress streaming

Context (read-only inputs):
• Spec: planner/spec.md
• Human hints: planner/human.md
• Progress: planner/progress.md
• New failing tests from: planner/prompts/011_test.md

Acceptance (must satisfy):
• Invoking `auth/cli_login` must immediately return `{status:"started"}` but also begin streaming CLI stderr output to CT-WEB via JSON-RPC notifications while the CLI runs.
• Each stderr line is forwarded as an `auth/cli_login/progress` notification with a `message` string.
• When the CLI process exits, the bridge emits a final `auth/cli_login/complete` notification containing the integer `exitCode`.
• The CLI continues to launch via the existing resolution rules without blocking for process completion.

Plan (you follow this order):
1. GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
2. RE-RUN TESTS — pnpm vitest --run && cargo test
3. REFACTOR — Improve clarity/structure without changing behavior.
4. LINT/FORMAT — cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format
5. FINAL TEST — pnpm vitest --run && cargo test must be fully green.

Notes & logging (append-only):
• Document implementation details, command outputs, and evidence in planner/notes/011_code.md using `§ CODE` blocks with timestamps.
• Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
• Reference any follow-up tasks or regressions discovered during the step.

Constraints:
• Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
• Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(011): auth/cli_login progress streaming — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: <1 line>

Exit condition:
• All tests pass; lints/format pass; acceptance demonstrably true.
