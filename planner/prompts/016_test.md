Title: Step 016: Write failing tests for bridge switcher toggle UI

Context (read-only inputs):
•Spec: planner/spec.md (do not edit)
•Human hints: planner/human.md (follow constraints)
•Progress checklist: planner/progress.md (current step)

Acceptance (authoritative for this step):
•Render a `BridgeSwitcher` component that receives a list of bridges `{ id, name }`, the currently active `bridgeId`, and an `onSelect` callback. It must display each bridge name as a mobile-friendly button with `aria-pressed` reflecting whether it is active. Clicking an inactive bridge triggers `onSelect` with that bridge's id.

Scope & files:
•Target area: `ct-web/src/components/BridgeSwitcher.test.tsx`
•You may create/modify only test files and light test scaffolding.
•SolidJS: co-located tests *.test.tsx (Vitest + Testing Library).
•Rust: unit tests beside code (mod tests {}) or tests/{name}.rs for integration.

What to deliver:
1.Minimal RED tests that fail against current code.
2.Tests must pin observable behavior (no over-mocking; avoid brittle implementation details).
3.For UI: prefer Testing Library queries of accessible roles/labels; if snapshots are needed, keep them tiny and stable.

Notes & logging (append-only):
•Append observations, decisions, and evidence to planner/notes/016_test.md using `§ TEST` blocks with timestamps.
•Include the commands you ran, failing output snippets, and links to any generated artifacts.
•Create the file if missing; otherwise append without altering earlier sections.

Commands you will run:

pnpm vitest --run
cargo test

Constraints:
•Do not change application code.
•Keep test names descriptive (<module>: <behavior>).
•If you must introduce test utilities, put them in tests/utils/ (Rust) or test/utils/ (JS) with the smallest viable footprint.

Exit condition:
•You leave the repo in a state where pnpm vitest --run and/or cargo test fail due to the new tests.
