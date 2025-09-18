Purpose: produce failing tests first, nothing else. Keep the working set minimal. Never modify planner/spec.md.

Title: Step 005: Write failing tests for session/prompt streaming notifications

Context (read-only inputs):
- Spec: planner/spec.md (do not edit)
- Human hints: planner/human.md (follow constraints)
- Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
- Bridge forwards `session/prompt` requests to agent and streams `session/update` notifications back to CT-WEB until final result with `stopReason`.
- Agent notifications are relayed transparently without modification except for any required bridgeId injection.
- JSON-RPC notification format preserved per RAT-LWS-REQ-011.

Scope & files:
- Target area: src/lib.rs for session/prompt forwarding, tests/bridge_handshake.rs for streaming tests
- You may create/modify only test files and light test scaffolding.
- Rust: integration tests in tests/bridge_handshake.rs covering agent streaming behavior.

What to deliver:
1. Minimal RED tests that fail against current code.
2. Tests must pin observable behavior (no over-mocking; avoid brittle implementation details).
3. Cover the full streaming flow: session/prompt request → multiple session/update notifications → final result.

Notes & logging (append-only):
- Append observations, decisions, and evidence to planner/notes/005_test.md using `§ TEST` blocks with timestamps.
- Include the commands you ran, failing output snippets, and links to any generated artifacts.
- Create the file if missing; otherwise append without altering earlier sections.

Commands you will run:
```
cargo test
```

Constraints:
- Do not change application code.
- Keep test names descriptive (<module>: <behavior>).
- If you must introduce test utilities, put them in tests/utils/ (Rust) or test/utils/ (JS) with the smallest viable footprint.

Exit condition:
- You leave the repo in a state where `cargo test` fails due to the new tests.