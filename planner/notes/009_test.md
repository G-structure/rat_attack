§ TEST [2025-09-18 11:30]
- Added two integration tests in `tests/bridge_handshake.rs` for `auth/cli_login`:
  • `auth_cli_login_launches_claude_cli_from_path` prepends a temp bin directory to PATH containing a fake `claude` shim that records cwd/args and keeps running, asserting the bridge spawns `/login` from the project root and returns `{ status: "started" }` immediately.
  • `auth_cli_login_errors_when_cli_unavailable` verifies the bridge surfaces an internal error when no Claude CLI can be resolved on PATH.
- Helpers (`EnvVarGuard`, `TestTempDir`, `wait_for_path`) provide deterministic PATH manipulation and async polling without touching production code.
- `cargo test auth_cli_login` currently fails as expected: bridge still returns `method not found` (`result` missing) and `-32601` errors because `auth/cli_login` isn’t implemented yet.
- Next: implement CLI resolution (PATH + npm-derived location) and process launching so the success test sees `{ status: "started" }` and missing CLI maps to `-32000` with an explanatory message.
