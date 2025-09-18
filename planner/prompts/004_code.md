Title: Step 004: Make tests pass for session/new forwarding

Context (read-only inputs):
    • Spec: planner/spec.md
    • Human hints: planner/human.md
    • Progress: planner/progress.md
    • New failing tests from: planner/prompts/004_test.md

Acceptance (must satisfy):
    • After a successful initialize handshake, CT-BRIDGE must forward a JSON-RPC `session/new` request to the connected agent and relay the agent's result back to CT-WEB without altering the payload other than prior bridge metadata requirements. The bridge must not forward `session/new` before `initialize` has completed. This behavior is exercised end-to-end over the WebSocket connection.

Plan (you follow this order):
    1. GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
    2. RE-RUN TESTS — pnpm vitest --run && cargo test
    3. REFACTOR — Improve clarity/structure without changing behavior.
    4. LINT/FORMAT — cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format
    5. FINAL TEST — pnpm vitest --run && cargo test must be fully green.

Notes & logging (append-only):
    • Document implementation details, command outputs, and evidence in planner/notes/004_code.md using `§ CODE` blocks with timestamps.
    • Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
    • Reference any follow-up tasks or regressions discovered during the step.

Constraints:
    • Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
    • Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(004): session/new forwarding — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: <1 line>

Exit condition:
    • All tests pass; lints/format pass; acceptance demonstrably true.
