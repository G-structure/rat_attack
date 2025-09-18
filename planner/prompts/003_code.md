Title: Step 003: Make tests pass for WS initialize handshake

Context (read-only inputs):
    • Spec: planner/spec.md
    • Human hints: planner/human.md
    • Progress: planner/progress.md
    • New failing tests from: planner/prompts/003_test.md

Acceptance (must satisfy):
    • Bridge only accepts WebSocket upgrades from allow-listed origins offering `acp.jsonrpc.v1`, forwards `initialize` through `agent-client-protocol` APIs, and injects `_meta.bridgeId` while rejecting other methods.

Plan (follow this order):
    1. GREEN — Implement the smallest change touching at most 1–3 files to pass the new tests.
    2. RE-RUN TESTS — `pnpm vitest --run && cargo test`
    3. REFACTOR — Improve clarity/structure without changing behavior.
    4. LINT/FORMAT — `cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format`
    5. FINAL TEST — `pnpm vitest --run && cargo test` must be fully green.

Notes & logging (append-only):
    • Document implementation details, command outputs, and evidence in planner/notes/003_code.md using `§ CODE` blocks with timestamps.
    • Create the notes file if necessary; otherwise append without editing earlier content.
    • Record any follow-up tasks or regressions you discover.

Constraints:
    • Do not edit tests unless they are objectively incorrect; if so, adjust minimally and note it in planner/progress.md for this step.
    • Prefer localized changes and avoid sweeping refactors.
    • Implementation must leverage the `agent-client-protocol` crate for request/response handling (no bespoke JSON parsing beyond the WebSocket envelope).

Commit message (format exactly):

step(003): WS initialize handshake — green

- tests: <list failing test names>
- touched: <files>
- acceptance: Bridge accepts allowed WS upgrade, forwards initialize, adds bridgeId.

Exit condition:
    • All tests pass; lints/format pass; acceptance demonstrably true.
