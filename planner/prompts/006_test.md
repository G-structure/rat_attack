Title: Step 006: Write failing tests for sandboxed fs/read_text_file

Context (read-only inputs):
	• Spec: planner/spec.md (do not edit)
	• Human hints: planner/human.md (follow constraints)
	• Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
	• Bridge must handle `fs/read_text_file` JSON-RPC requests by returning file contents when the absolute `path` lies within a configured project root (per RAT-LWS-REQ-040).
	• Requests for paths outside every project root must be rejected with a JSON-RPC error explaining the sandbox violation (RAT-LWS-REQ-044).
	• Each error response must include both `code`/`message` and a `data.details` string to satisfy structured error expectations (RAT-LWS-REQ-132).

Scope & files:
	• Target area: tests/ws_upgrade.rs (you may split into an additional integration test file if helpful) plus minimal test scaffolding under tests/.
	• You may create/modify only test files and light test scaffolding.
	• SolidJS is out of scope for this step.
	• Rust integration tests: prefer starting the compiled `ct-bridge` binary inside the test so `cargo test` can run standalone (e.g., spawn `env!("CARGO_BIN_EXE_ct-bridge")` and tear it down).

What to deliver:
	1. Minimal RED tests that fail against current code for each acceptance clause (inside-root success, outside-root rejection, structured error data).
	2. Tests must pin observable behavior: assert on JSON-RPC payloads (result text content, error code/message/data).
	3. Use reasonable timeouts so tests fail fast if no response arrives; avoid brittle sleeps.

Notes & logging (append-only):
	• Append observations, decisions, and evidence to planner/notes/006_test.md using `§ TEST` blocks with timestamps.
	• Include the commands you ran, failing output snippets, and links to any generated artifacts.
	• Create the file if missing; otherwise append without altering earlier sections.

Commands you will run:
	• cargo test --test ws_upgrade
	• cargo test

Constraints:
	• Do not change application code.
	• Keep test names descriptive (`fs_read: returns file content within project root`).
	• If you introduce helpers (e.g., to boot the server), keep them minimal and local to tests/.

Exit condition:
	• Repo left with new tests causing `cargo test` to fail because the implementation does not yet satisfy the acceptance criteria.
