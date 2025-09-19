Purpose: write the smallest diffs that make the 013 tests pass, then refactor safely. Never modify planner/spec.md.

Title: Step 013: Make tests pass for fs/write_text_file permission audit trail

Context (read-only inputs):
- Spec: planner/spec.md
- Human hints: planner/human.md
- Progress: planner/progress.md
- New failing tests from: planner/prompts/013_test.md

Acceptance (must satisfy):
- Whenever the bridge requests permission for `fs/write_text_file`, it must append an audit entry to a configured log file describing the prompt without recording the file contents.
- After the agent replies, a second audit entry must capture the resulting decision (selected option or cancellation) alongside the session id while keeping sensitive data redacted.
- Log entries MUST provide a stable hash derived from the canonical path so that repeated prompts for the same file can be correlated without exposing the raw path.

Plan (you follow this order):
1. GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
2. RE-RUN TESTS — pnpm vitest --run && cargo test
3. REFACTOR — Improve clarity/structure without changing behavior.
4. LINT/FORMAT — cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format
5. FINAL TEST — pnpm vitest --run && cargo test must be fully green.

Notes & logging (append-only):
- Document implementation details, command outputs, and evidence in planner/notes/013_code.md using `§ CODE` blocks with timestamps.
- Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
- Reference any follow-up tasks or regressions discovered during the step.

Constraints:
- Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
- Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(013): fs/write_text_file permission audit trail — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: <1 line>

Exit condition:
- All tests pass; lints/format pass; acceptance demonstrably true.
