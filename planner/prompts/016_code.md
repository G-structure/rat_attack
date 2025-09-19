Title: Step 016: Make tests pass for bridge switcher toggle UI

Context (read-only inputs):
•Spec: planner/spec.md
•Human hints: planner/human.md
•Progress: planner/progress.md
•New failing tests from: planner/prompts/016_test.md

Acceptance (must satisfy):
•Render a `BridgeSwitcher` component that receives a list of bridges `{ id, name }`, the currently active `bridgeId`, and an `onSelect` callback. It must display each bridge name as a mobile-friendly button with `aria-pressed` reflecting whether it is active. Clicking an inactive bridge triggers `onSelect` with that bridge's id.

Plan (you follow this order):
1.GREEN — Implement the smallest change touching 1–3 files max to pass the new tests.
2.RE-RUN TESTS — pnpm vitest --run && cargo test
3.REFACTOR — Improve clarity/structure without changing behavior.
4.LINT/FORMAT — cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format
5.FINAL TEST — pnpm vitest --run && cargo test must be fully green.

Notes & logging (append-only):
•Document implementation details, command outputs, and evidence in planner/notes/016_code.md using `§ CODE` blocks with timestamps.
•Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
•Reference any follow-up tasks or regressions discovered during the step.

Constraints:
•Do not edit tests unless they are objectively incorrect; if so, fix them minimally and add a note in planner/progress.md under this step.
•Prefer clear, local changes over rewrites. Avoid renames unless essential.

Commit message (format exactly):

step(016): bridge switcher toggle UI — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: <1 line>

Exit condition:
•All tests pass; lints/format pass; acceptance demonstrably true.
