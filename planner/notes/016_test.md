§ TEST 2025-09-18T12:00Z — setup
- reviewed spec requirement RAT-LWS-REQ-145 and existing ct-web structure (only App.tsx placeholder)
- target is new BridgeSwitcher component test under ct-web/src/components
§ TEST 2025-09-18T12:10Z — tests drafted
- added ct-web/src/components/BridgeSwitcher.test.tsx with two vitest cases covering active state and selection callback
- tests rely on @solidjs/testing-library imports which are not yet installed, ensuring red failure once vitest runs
§ TEST 2025-09-18T12:18Z — command results
- `pnpm vitest --run` (from ct-web) fails immediately because vitest is not yet installed: ERR_PNPM_RECURSIVE_EXEC_FIRST_FAIL Command "vitest" not found
- `cargo test` currently fails with prior suite regression (`bridge_handshake_caches_allow_always_permission_decisions` panic). Need to flag during coordination; unrelated to newly added UI test but must be addressed before green.
