Title: Step 005: Write failing tests for initialize fs capability enforcement

Context (read-only inputs):
	• Spec: planner/spec.md (do not edit)
	• Human hints: planner/human.md (follow constraints)
	• Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
	• After a successful ACP WebSocket upgrade, when CT-WEB sends `initialize` with `capabilities.fs.readTextFile` and `capabilities.fs.writeTextFile` both true, the bridge must reply with a JSON-RPC 2.0 result containing `_meta.bridgeId` and a `capabilities.fs` section that echoes both booleans as true.
	• If the client omits either fs capability or sets it false, the bridge must return a JSON-RPC error (use code -32602) describing the missing requirement instead of a success result.
	• Responses must remain well-formed JSON-RPC frames and the connection should stay open for subsequent requests.

Scope & files:
	• Target area: `tests/ws_upgrade.rs` (extend existing integration tests) or a new integration test under `tests/` if isolation helps.
	• You may create/modify only test files and light test scaffolding.
	• SolidJS: co-located tests *.test.tsx (Vitest + Testing Library).
	• Rust: unit tests beside code (mod tests {}) or tests/{name}.rs for integration.

What to deliver:
	1. Minimal RED test coverage that fails because the current bridge neither echoes fs capabilities nor rejects missing ones.
	2. Tests must assert the JSON structure of the success response and verify a JSON-RPC error object (code -32602) when capabilities are absent.
	3. Keep tests async and exercise the live WebSocket handshake, reusing helpers where possible to limit noise.

Notes & logging (append-only):
	• Append observations, decisions, and evidence to `planner/notes/005_test.md` using `§ TEST` blocks with timestamps.
	• Include the commands you ran, failing output snippets, and links to any generated artifacts.
	• Create the file if missing; otherwise append without altering earlier sections.

Commands you will run:

pnpm vitest --run
cargo test

Constraints:
	• Do not change application code.
	• Keep test names descriptive (<module>: <behavior>).
	• If you must introduce test utilities, place them under tests/utils/ with the smallest viable footprint.

Exit condition:
	• Leave the repo with `pnpm vitest --run` and/or `cargo test` failing solely because of the new tests.
