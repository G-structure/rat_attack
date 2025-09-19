Purpose: produce failing tests first, nothing else. Keep the working set minimal. Never modify planner/spec.md.

Title: Step 013: Write failing tests for fs/write_text_file permission audit trail

Context (read-only inputs):
- Spec: planner/spec.md (do not edit)
- Human hints: planner/human.md (follow constraints)
- Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
- Whenever the bridge requests permission for `fs/write_text_file`, it must append an audit entry to a configured log file describing the prompt without recording the file contents.
- After the agent replies, a second audit entry must capture the resulting decision (selected option or cancellation) alongside the session id while keeping sensitive data redacted.
- Log entries MUST provide a stable hash derived from the canonical path so that repeated prompts for the same file can be correlated without exposing the raw path.

Scope & files:
- Target area: tests/bridge_handshake.rs (integration tests driving permission flows)
- You may create/modify only test files and light test scaffolding under tests/.
- Rust: integration tests under tests/, using existing BridgeHarness utilities; update test-only helpers as needed.

What to deliver:
1. Add a new integration test that drives a `fs/write_text_file` permission approval and asserts two audit log entries are written: one for the prompt and one for the outcome.
2. The test should verify the log entries include the session id, tool name, a stable hash for the canonical path, and that the raw file content string never appears in the log file.
3. Extend the test harness minimally (e.g., allow configuring an audit log path) so the test can read and assert against a temporary log file without affecting other suites.

Notes & logging (append-only):
- Append observations, decisions, and evidence to planner/notes/013_test.md using `§ TEST` blocks with timestamps.
- Include the commands you ran, failing output snippets, and links to any generated artifacts.
- Create the file if missing; otherwise append without altering earlier sections.

Commands you will run:
```
pnpm vitest --run
cargo test
```

Constraints:
- Do not change application code.
- Keep test names descriptive (<module>: <behavior>).
- Maintain existing filesystem isolation by cleaning up any files under the project root sandbox.

Exit condition:
- You leave the repo in a state where pnpm vitest --run and/or cargo test fail because the new audit trail test asserts behavior the bridge has not implemented yet.
