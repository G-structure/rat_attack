Purpose: produce failing tests first, nothing else. Keep the working set minimal. Never modify planner/spec.md.

Title: Step 006: Write failing tests for fs/read_text_file capability

Context (read-only inputs):
- Spec: planner/spec.md (do not edit)
- Human hints: planner/human.md (follow constraints)
- Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
- Bridge implements `fs/read_text_file` method per RAT-LWS-REQ-040
- Supports optional line offset and limit parameters
- Reads are restricted to declared project roots (PR sandbox) per RAT-LWS-REQ-044
- Returns appropriate errors for out-of-bounds access, binary files, missing files

Scope & files:
- Target area: src/lib.rs for fs/read_text_file method implementation, tests/bridge_handshake.rs for fs tests
- You may create/modify only test files and light test scaffolding.
- Rust: integration tests covering fs read behavior with project root sandboxing

What to deliver:
1. Minimal RED tests that fail against current code.
2. Tests must cover: basic file read, line offset/limit, PR sandbox enforcement, error cases
3. Tests should verify RAT-LWS-REQ-040 and RAT-LWS-REQ-044 compliance

Notes & logging (append-only):
- Append observations, decisions, and evidence to planner/notes/006_test.md using `§ TEST` blocks with timestamps.
- Include the commands you ran, failing output snippets, and links to any generated artifacts.
- Create the file if missing; otherwise append without altering earlier sections.

Commands you will run:
```
cargo test
```

Constraints:
- Do not change application code.
- Keep test names descriptive (<module>: <behavior>).
- If you must introduce test utilities, put them in tests/utils/ (Rust) with the smallest viable footprint.

Exit condition:
- You leave the repo in a state where `cargo test` fails due to the new fs/read_text_file tests.