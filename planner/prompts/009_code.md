Purpose: write the smallest diffs that make the {009} tests pass, then refactor safely. Never modify planner/spec.md.

Title: Step 009: Make tests pass for auth/cli_login launching Claude login CLI

Context (read-only inputs):
- Spec: planner/spec.md
- Human hints: planner/human.md
- Progress: planner/progress.md
- New failing tests from: planner/prompts/009_test.md
- Reference material: planner/docs/zed_acp.md, planner/docs/zed_acp_agent_launching.md, planner/docs/zed_acp_login.md

Acceptance (must satisfy):
- Bridge exposes an `auth/cli_login` JSON-RPC method that resolves the `claude-code-acp` CLI (via `CLAUDE_ACP_BIN` override or the bundled npm package) and spawns it with the `/login` argument inside the project root.
- The method returns immediately with a structured success payload (e.g., `{ status: "started" }`) once the login process is launched, and propagates errors if the launcher fails.
- The npm workspace pulls in the Claude ACP adapter so the `claude` binary is available (via `pnpm` / `node_modules/.bin`).

Plan (you follow this order):
1. GREEN — Implement the smallest change touching 1–3 files max to pass the new tests (likely `src/lib.rs`, new `src/login.rs`, and supporting config).
2. RE-RUN TESTS — pnpm vitest --run && cargo test
3. REFACTOR — Improve clarity/structure without changing behavior.
4. LINT/FORMAT — cargo clippy --fix -q && cargo fmt && pnpm lint --fix && pnpm format
5. FINAL TEST — pnpm vitest --run && cargo test must be fully green.

Notes & logging (append-only):
- Document implementation details, command outputs, and evidence in planner/notes/009_code.md using `§ CODE` blocks with timestamps.
- Create the notes file if it does not exist; otherwise append new blocks without editing earlier content.
- Record how the CLI path is resolved (node_modules bin lookup, env override) and any error mapping decisions.

Constraints:
- Introduce a trait or injectable command runner if needed for testability; avoid global mutable state.
- Ensure spawned login process inherits sanitized environment (project root, necessary API keys) and runs detached/non-blocking.
- Wire npm dependency changes via pnpm (update lockfiles accordingly) and keep workspace consistent.

Commit message (format exactly):

step(009): auth/cli_login launches Claude login — green

- tests: <list the test names that were failing>
- touched: <files>
- acceptance: <1 line>

Exit condition:
- All tests pass; lints/format pass; acceptance demonstrably true.
