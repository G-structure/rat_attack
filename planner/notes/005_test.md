§ TEST 2025-09-18 14:00 — Added failing tests for initialize fs capability enforcement

**Context:** Extended tests/ws_upgrade.rs with 6 new test cases to verify bridge enforces fs.readTextFile and fs.writeTextFile both true in initialize params. Tests cover valid case (echo capabilities) and invalid cases (missing, false, or omitted fields) expecting JSON-RPC error -32602.

**Added Tests:**
- test_initialize_with_valid_fs_capabilities: Expects result with capabilities.fs echoing true/true
- test_initialize_missing_fs_capabilities: Expects error -32602 when params lacks capabilities.fs
- test_initialize_fs_read_false: Expects error -32602 when readTextFile=false
- test_initialize_fs_write_false: Expects error -32602 when writeTextFile=false
- test_initialize_fs_read_missing: Expects error -32602 when readTextFile omitted
- test_initialize_fs_write_missing: Expects error -32602 when writeTextFile omitted

**Command Run:**
```
just test-ws-upgrade
```

**Failing Output Snippets:**
- test_initialize_with_valid_fs_capabilities: assertion failed: result["capabilities"].is_object() (current response lacks capabilities section)
- test_initialize_missing_fs_capabilities: assertion failed: response["error"].is_object() (current response returns result instead of error)
- Similar failures for other invalid cases: all expect error but get result

**Evidence:** All 6 new tests failed as expected; existing 8 tests (upgrade/subprotocol/bridgeId) passed. Server was already running on :8137, tests connected successfully.

**Next:** Implement capability validation in src/main.rs handle_jsonrpc_request to make tests green.