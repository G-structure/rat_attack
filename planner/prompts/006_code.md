Purpose: write the smallest diffs that make the 006 tests pass, then refactor safely. Never modify planner/spec.md.

Title: Step 006: Make tests pass for fs/read_text_file capability

Context (read-only inputs):
- Spec: planner/spec.md
- Human hints: planner/human.md
- Progress: planner/progress.md
- New failing tests from: planner/prompts/006_test.md

Acceptance (must satisfy):
- Bridge implements `fs/read_text_file` method per RAT-LWS-REQ-040
- Supports optional line offset and limit parameters
- Reads are restricted to declared project roots (PR sandbox) per RAT-LWS-REQ-044
- Returns appropriate errors for out-of-bounds access, binary files, missing files

Plan (you follow this order):
1. GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
2. RE-RUN TESTS — `cargo test`
3. REFACTOR — Improve clarity/structure without changing behavior.
4. LINT/FORMAT — `cargo clippy --fix -q && cargo fmt`
5. FINAL TEST — `cargo test` must be fully green.

Notes & logging (append-only):
- Document implementation details, command outputs, and evidence in planner/notes/006_code.md using `§ CODE` blocks with timestamps.
- Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
- Reference any follow-up tasks or regressions discovered during the step.

Constraints:
- Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
- Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(006): fs/read_text_file capability — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: Bridge implements fs/read_text_file with PR sandboxing

Exit condition:
- All tests pass; lints/format pass; acceptance demonstrably true.