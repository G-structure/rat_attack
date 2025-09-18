Purpose: produce failing tests first, nothing else. Keep the working set minimal. Never modify planner/spec.md.

Title: Step 009: Write failing tests for auth/cli_login launching Claude login CLI

Context (read-only inputs):
- Spec: planner/spec.md (do not edit)
- Human hints: planner/human.md (follow constraints)
- Progress checklist: planner/progress.md (current step)
- Zed references: planner/docs/zed_acp.md, planner/docs/zed_acp_agent_launching.md, planner/docs/zed_acp_login.md (for inspiration only)

Acceptance (authoritative for this step):
- Bridge exposes an `auth/cli_login` JSON-RPC method that resolves the `claude-code-acp` CLI (via `CLAUDE_ACP_BIN` override or the bundled npm package) and spawns it with the `/login` argument inside the project root.
- The method returns immediately with a structured success payload (e.g., `{ status: "started" }`) once the login process is launched, and propagates errors if the launcher fails.
- The npm workspace pulls in the Claude ACP adapter so the `claude` binary is available (via `pnpm` / `node_modules/.bin`).

Scope & files:
- Target area: tests/bridge_login.rs (new) or extend tests/bridge_handshake.rs with a dedicated auth section.
- Auxiliary fixtures: You may add lightweight helpers under tests/ (e.g., tests/utils/login_stub.rs) to stub the CLI execution.
- JS workspace metadata (ct-web/package.json) must remain unmodified in tests; only assertions via filesystem setup.

What to deliver:
1. Integration-style test(s) that start the bridge, set a temporary `CLAUDE_ACP_BIN` stub script, invoke `auth/cli_login`, and expect a success result plus side-effects proving the stub executed (e.g., sentinel file written).
2. Negative test covering command launch failure (e.g., stub exits with non-zero) asserting the bridge surfaces an error response with meaningful code/data.
3. Any required harness utilities to observe the spawned process without touching production code (e.g., using temp dirs, helper structs).

Notes & logging (append-only):
- Append observations, decisions, and evidence to planner/notes/009_test.md using `§ TEST` blocks with timestamps.
- Include the commands you ran, failing output snippets, and links to any generated artifacts.
- Create the file if missing; otherwise append without altering earlier sections.

Commands you will run:
```
cargo test auth_cli_login
```

Constraints:
- Do not change application code.
- Keep tests deterministic; clean up any temporary files you create.
- Assume the login CLI is long-running: your tests should not wait for process completion—only for the stub side-effect.

Exit condition:
- Repository left where `cargo test` fails because `auth/cli_login` is not implemented and/or the CLI is not being launched.
