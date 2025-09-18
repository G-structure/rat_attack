# Step 006: Write failing tests for fs/read_text_file capability

## § TEST 2025-09-18 - Initial Test Implementation

### Objective
Produce failing tests for `fs/read_text_file` capability per RAT-LWS-REQ-040, covering:
- Basic file reading functionality
- Line offset and limit parameters
- Project root sandboxing enforcement (RAT-LWS-REQ-044)
- Error cases: missing files, binary files, out-of-bounds access

### Implementation
Added 6 test functions to `tests/bridge_handshake.rs`:

1. **`fs_read_text_file_basic_functionality`** - Tests basic file reading
2. **`fs_read_text_file_with_line_offset_and_limit`** - Tests optional offset/limit parameters
3. **`fs_read_text_file_enforces_project_root_sandbox`** - Tests PR sandbox per RAT-LWS-REQ-044
4. **`fs_read_text_file_rejects_missing_files`** - Tests missing file handling
5. **`fs_read_text_file_rejects_binary_files`** - Tests binary file rejection per RAT-LWS-REQ-111
6. **`fs_read_text_file_handles_out_of_bounds_line_parameters`** - Tests edge cases

### Commands Run
```bash
cargo test fs_read_text_file
```

### Test Failure Output
All 6 tests fail as expected:

```
running 6 tests
test fs_read_text_file_rejects_binary_files ... FAILED
test fs_read_text_file_enforces_project_root_sandbox ... FAILED
test fs_read_text_file_rejects_missing_files ... FAILED
test fs_read_text_file_handles_out_of_bounds_line_parameters ... FAILED
test fs_read_text_file_with_line_offset_and_limit ... FAILED
test fs_read_text_file_basic_functionality ... FAILED

test result: FAILED. 0 passed; 6 failed; 0 ignored; 0 measured; 8 filtered out
```

### Failure Analysis
- **Basic functionality tests** expect successful result with content, but get method not found (-32601)
- **Error case tests** expect specific error codes (permission denied, file not found, binary file error) but get method not found (-32601)
- This confirms `fs/read_text_file` method is not yet implemented in the bridge

### Test Design Rationale
Tests are structured to verify:
- **RAT-LWS-REQ-040**: Basic `fs/read_text_file` with optional line/limit
- **RAT-LWS-REQ-044**: Project root sandbox enforcement
- **RAT-LWS-REQ-111**: Binary file rejection
- Error handling for missing files and out-of-bounds parameters

### Exit Condition Met
✅ Repository now in state where `cargo test` fails due to new `fs/read_text_file` tests

The tests provide clear specification for the implementation team:
- Expected JSON-RPC method: `fs/read_text_file`
- Required parameters: `path` (string), optional `line_offset` and `line_limit` (numbers)
- Expected result: `{ "content": "file_content_string" }`
- Required error handling for sandbox violations, missing files, and binary files