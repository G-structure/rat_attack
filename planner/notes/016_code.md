§ CODE 2025-09-18T12:25Z — analysis
- failing vitest due to missing dependencies; need to install vitest + @solidjs/testing-library and configure Vite for testing
- implement BridgeSwitcher component to satisfy tests (accessible buttons with aria-pressed and onSelect)
- plan to expose component within App.tsx for smoke rendering (optional)
§ CODE 2025-09-18T12:35Z — implementation
- installed vitest, @solidjs/testing-library, @testing-library/jest-dom, jsdom and added pnpm scripts for test execution
- configured tsconfig and vite.config with vitest settings plus setup file importing jest-dom matchers
- implemented src/components/BridgeSwitcher.tsx rendering accessible buttons with aria-pressed and selection guard
- added src/test/setup.ts for vitest environment
§ CODE 2025-09-18T12:40Z — test hygiene fix
- added explicit afterEach(cleanup) in BridgeSwitcher tests to avoid DOM leakage between cases; documented as minimal correction to prevent false positives
§ CODE 2025-09-18T12:50Z — validation
- `pnpm vitest --run` (ct-web) → PASS (2 tests)
- `cargo test` → PASS (33 tests) after prior flaky failure rechecked
- `cargo clippy --fix -q` required `--allow-dirty` due to pending changes; reran with flag successfully, followed by `cargo fmt`
- `pnpm lint --fix` and `pnpm format` scripts are not defined yet; commands fail with ERR_PNPM_RECURSIVE_EXEC_FIRST_FAIL — noted for future tooling work
