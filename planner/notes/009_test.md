§ TEST [2025-09-18 11:30]
- Added two integration tests in `tests/bridge_handshake.rs` for `auth/cli_login`:
  • `auth_cli_login_launches_claude_cli_from_path` prepends a temp bin directory to PATH containing a fake `claude` shim that records cwd/args and keeps running, asserting the bridge spawns `/login` from the project root and returns `{ status: "started" }` immediately.
  • `auth_cli_login_errors_when_cli_unavailable` verifies the bridge surfaces an internal error when no Claude CLI can be resolved on PATH.
- Helpers (`EnvVarGuard`, `TestTempDir`, `wait_for_path`) provide deterministic PATH manipulation and async polling without touching production code.
- `cargo test auth_cli_login` currently fails as expected: bridge still returns `method not found` (`result` missing) and `-32601` errors because `auth/cli_login` isn't implemented yet.
- Next: implement CLI resolution (PATH + npm-derived location) and process launching so the success test sees `{ status: "started" }` and missing CLI maps to `-32000` with an explanatory message.

§ TEST [2025-09-18 16:45]
- Expanded auth/cli_login test suite to 7 comprehensive tests inspired by Zed ACP documentation:
  • `auth_cli_login_resolves_claude_acp_bin_override` - Tests CLAUDE_ACP_BIN environment variable override
  • `auth_cli_login_downloads_claude_code_acp_package` - Tests npm package resolution from node_modules
  • `auth_cli_login_handles_virtual_terminal_like_zed` - Tests terminal-based authentication flow like Zed
  • `auth_cli_login_launches_claude_cli_from_path` - Tests PATH-based CLI resolution (original)
  • `auth_cli_login_returns_immediately_before_process_completion` - Tests immediate return behavior
  • `auth_cli_login_resolves_package_from_workspace` - Tests workspace-level package resolution
  • `auth_cli_login_errors_when_cli_unavailable` - Error handling (passes)
- Added test utilities: `DirGuard` for directory changes, enhanced `TestTempDir` for complex setups
- Test results: 6 failing, 1 passing (error case works)
- All failing tests expect `{ "status": "started" }` result but get error responses
- Bridge has auth/cli_login method implemented but not working correctly with all resolution strategies
- Ready for implementation phase to make tests pass
