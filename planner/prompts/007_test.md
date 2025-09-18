Purpose: produce failing tests first, nothing else. Keep the working set minimal. Never modify planner/spec.md.

Title: Step 007: Write failing tests for fs/write_text_file with permission gating

Context (read-only inputs):
- Spec: planner/spec.md (do not edit)
- Human hints: planner/human.md (follow constraints)
- Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
- Bridge implements `fs/write_text_file` method that requires permission approval per RAT-LWS-REQ-041
- All writes must be gated via `session/request_permission` before execution per RAT-LWS-REQ-041
- Writes are restricted to declared project roots (PR sandboxing) per RAT-LWS-REQ-044
- Permission prompts return a definitive outcome (allow_once, allow_always, reject_once, reject_always, cancelled) per RAT-LWS-REQ-091

Scope & files:
- Target area: tests/bridge_handshake.rs
- You may create/modify only test files and light test scaffolding.
- Rust: unit tests beside code (mod tests {}) or tests/{name}.rs for integration.

What to deliver:
1. Minimal RED tests that fail against current code.
2. Tests must pin observable behavior (no over-mocking; avoid brittle implementation details).
3. Test the permission gating flow: permission request → approval → write execution.
4. Test project root sandboxing for writes.
5. Test different permission outcomes (allow_once, reject_once, etc.).

Notes & logging (append-only):
- Append observations, decisions, and evidence to planner/notes/007_test.md using `§ TEST` blocks with timestamps.
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
- You leave the repo in a state where cargo test fails due to the new tests expecting fs/write_text_file with permission gating.