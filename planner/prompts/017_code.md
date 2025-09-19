Title: Step 017: Make tests pass for fs/read_text_file project-root escape guard

Context (read-only inputs):
    • Spec: planner/spec.md
    • Human hints: planner/human.md
    • Progress: planner/progress.md
    • New failing tests from: planner/prompts/017_test.md

Acceptance (must satisfy):
    • The bridge MUST reject fs/read_text_file requests whose path resolves outside the project root (e.g., an absolute /tmp path) before touching permission flows, responding with a JSON-RPC error instead of method-not-found.
    • The rejection proves RAT-LWS-REQ-044 by showing the agent transport never sees a permission request when such an escape is attempted.

Plan (you follow this order):
    1. GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
    2. RE-RUN TESTS — pnpm vitest --run && cargo test
    3. REFACTOR — Improve clarity/structure without changing behavior.
    4. LINT/FORMAT — cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format
    5. FINAL TEST — pnpm vitest --run && cargo test must be fully green.

Notes & logging (append-only):
    • Document implementation details, command outputs, and evidence in planner/notes/017_code.md using `§ CODE` blocks with timestamps.
    • Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
    • Reference any follow-up tasks or regressions discovered during the step.

Constraints:
    • Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
    • Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(017): fs/read_text_file escape guard — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: <1 line>

Exit condition:
    • All tests pass; lints/format pass; acceptance demonstrably true.
