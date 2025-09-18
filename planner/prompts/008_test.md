Purpose: produce failing tests first, nothing else. Keep the working set minimal. Never modify planner/spec.md.

Title: Step 008: Write failing tests for fs/write_text_file permission policy caching

Context (read-only inputs):
- Spec: planner/spec.md (do not edit)
- Human hints: planner/human.md (follow constraints)
- Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
- Bridge caches permission decisions for `fs/write_text_file` requests: after an agent responds with `allow_always`, subsequent writes to the same canonical path are automatically permitted without another `session/request_permission` round-trip.
- `reject_always` decisions are remembered and cause future writes to that path to fail immediately.
- When no policy entry exists, the bridge requests permission, and decisions are scoped to project-root canonical paths.

Scope & files:
- Target area: tests/bridge_handshake.rs
- You may create/modify only test files and light test scaffolding.
- Rust: integration tests under tests/, using existing BridgeHarness utilities.

What to deliver:
1. Add failing tests that prove `allow_always` skips subsequent permission requests for the same canonical path while still writing successfully.
2. Add failing tests that prove `reject_always` is cached and causes later writes to the same path to error without contacting the agent again.
3. Extend or reuse FakePermissionAgentTransport instrumentation as needed to assert how many times `request_permission` was invoked.

Notes & logging (append-only):
- Append observations, decisions, and evidence to planner/notes/008_test.md using `§ TEST` blocks with timestamps.
- Include the commands you ran, failing output snippets, and links to any generated artifacts.
- Create the file if missing; otherwise append without altering earlier sections.

Commands you will run:
```
cargo test
```

Constraints:
- Do not change application code.
- Keep test names descriptive (<module>: <behavior>).
- Maintain existing test isolation (each test must clean up any files it writes under the project root sandbox).

Exit condition:
- You leave the repo in a state where cargo test fails because the new caching tests expect behavior not yet implemented.
