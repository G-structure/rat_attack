Title: Step 015: Make tests pass for fs/read_text_file truncation meta

Context (read-only inputs):
• Spec: planner/spec.md
• Human hints: planner/human.md
• Progress: planner/progress.md
• New failing tests from: planner/prompts/015_test.md

Acceptance (must satisfy):
• Flag truncated file reads in `fs/read_text_file` responses.
• When a request supplies `line_limit` and the file has more content beyond the returned slice, include `_meta.truncated: true` in the JSON-RPC result.
• When the requested range covers the full file contents, omit the truncated flag so callers know the read was complete.

Plan (you follow this order):
1. GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
2. RE-RUN TESTS — pnpm vitest --run && cargo test
3. REFACTOR — Improve clarity/structure without changing behavior.
4. LINT/FORMAT — cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format
5. FINAL TEST — pnpm vitest --run && cargo test must be fully green.

Notes & logging (append-only):
• Document implementation details, command outputs, and evidence in planner/notes/015_code.md using `§ CODE` blocks with timestamps.
• Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
• Reference any follow-up tasks or regressions discovered during the step.

Constraints:
• Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
• Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(015): fs/read_text_file truncation meta — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: <1 line>

Exit condition:
• All tests pass; lints/format pass; acceptance demonstrably true.
