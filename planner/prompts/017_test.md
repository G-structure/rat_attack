Title: Step 017: Write failing tests for fs/read_text_file project-root escape guard

Context (read-only inputs):
    • Spec: planner/spec.md (do not edit)
    • Human hints: planner/human.md (follow constraints)
    • Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
    • The bridge MUST reject fs/read_text_file requests whose path resolves outside the project root (e.g., an absolute /tmp path) before touching permission flows, responding with a JSON-RPC error instead of method-not-found.
    • The rejection proves RAT-LWS-REQ-044 by showing the agent transport never sees a permission request when such an escape is attempted.

Scope & files:
    • Target area: tests/bridge_handshake.rs (new regression covering absolute-path escapes).
    • You may create/modify only test files and light test scaffolding.
    • SolidJS: co-located tests *.test.tsx (Vitest + Testing Library).
    • Rust: unit tests beside code (mod tests {}) or tests/{name}.rs for integration.

What to deliver:
    1. Minimal RED tests that fail against current code.
    2. Tests must pin observable behavior (no over-mocking; avoid brittle implementation details).
    3. For UI: prefer Testing Library queries of accessible roles/labels; if snapshots are needed, keep them tiny and stable.

Notes & logging (append-only):
    • Append observations, decisions, and evidence to planner/notes/017_test.md using `§ TEST` blocks with timestamps.
    • Include the commands you ran, failing output snippets, and links to any generated artifacts.
    • Create the file if missing; otherwise append without altering earlier sections.

Commands you will run:

pnpm vitest --run
cargo test

Constraints:
    • Do not change application code.
    • Keep test names descriptive (<module>: <behavior>).
    • If you must introduce test utilities, put them in tests/utils/ (Rust) or test/utils/ (JS) with the smallest viable footprint.

Exit condition:
    • You leave the repo in a state where pnpm vitest --run and/or cargo test fail due to the new tests.
