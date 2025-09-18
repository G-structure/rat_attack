# Notes for Step 005: Write failing tests for session/prompt streaming notifications

## § TEST [2025-09-18T18:45:00Z]

### Objective
Create failing tests for session/prompt streaming notifications per RAT-LWS-REQ-031 to drive implementation in RED state.

### Analysis of Current State

**Existing AgentTransport Trait:**
- Currently only supports `initialize()` and `new_session()` methods
- No support for `session/prompt` method
- No streaming notification infrastructure in place

**Missing Functionality Identified:**
1. `session/prompt` method in AgentTransport trait
2. Streaming `session/update` notifications from bridge to CT-WEB
3. Final response with `stopReason` field
4. Transparent forwarding of session/prompt requests to agent per RAT-LWS-REQ-011

### Tests Created

**3 failing tests added to `tests/bridge_handshake.rs`:**

1. **`bridge_streams_session_prompt_updates()`**
   - Tests the full streaming flow: session/prompt request → multiple session/update notifications → final result
   - Expects multiple `session/update` notifications followed by final response with `stopReason`
   - Currently fails because bridge doesn't handle `session/prompt` method

2. **`bridge_forwards_session_prompt_transparently()`**
   - Tests RAT-LWS-REQ-011 requirement for transparent JSON-RPC forwarding
   - Verifies agent receives session/prompt requests without modification
   - Currently fails because `session/prompt` method returns method not found

3. **`bridge_session_update_preserves_json_rpc_format()`**
   - Tests RAT-LWS-REQ-011 requirement for JSON-RPC notification format preservation
   - Verifies `session/update` notifications maintain proper JSON-RPC 2.0 structure
   - Currently fails because no streaming notifications are implemented

### Test Infrastructure Added

**New Test Types:**
- `PromptRequest` struct to represent session/prompt parameters
- `FakeStreamingAgentTransport` that supports future streaming behavior
- Helper functions `send_initialize_request()` and `send_session_new_request()`

**Mock Streaming Behavior:**
- `configure_streaming_updates()` method to setup expected streaming responses
- Support for capturing `prompt_calls` for verification
- Maintains compatibility with existing test structure

### Failure Details

```bash
running 8 tests
test bridge_forwards_session_prompt_transparently ... FAILED
test bridge_session_update_preserves_json_rpc_format ... FAILED
test bridge_streams_session_prompt_updates ... FAILED

failures:
---- bridge_forwards_session_prompt_transparently stdout ----
assertion `left == right` failed: session/prompt should be forwarded to agent
  left: 0
 right: 1

---- bridge_session_update_preserves_json_rpc_format stdout ----
assertion `left == right` failed
  left: None
 right: Some(String("session/update"))

---- bridge_streams_session_prompt_updates stdout ----
final response should have result
```

### Expected Behavior vs Current Behavior

**Expected (per spec):**
1. Bridge accepts `session/prompt` JSON-RPC requests
2. Bridge forwards requests to agent via `session_prompt()` method
3. Agent streams `session/update` notifications back through bridge
4. Bridge relays notifications to CT-WEB transparently
5. Final response includes `stopReason` field

**Current:**
1. Bridge returns "method not found" for `session/prompt` requests
2. No `session_prompt()` method exists in AgentTransport trait
3. No streaming notification infrastructure
4. No `stopReason` handling

### Commands Run
```bash
cargo test
# Result: 5 passed, 3 failed (expected - tests are properly RED)
```

### Files Modified
- `/Users/luc/projects/vibes/rat_attack/tests/bridge_handshake.rs` - Added 3 failing tests + infrastructure

### Next Steps for Implementation Agent
The failing tests clearly define the required behavior:
1. Add `session_prompt()` method to `AgentTransport` trait
2. Implement `session/prompt` handler in bridge message processing
3. Add streaming notification relay mechanism
4. Ensure `stopReason` field in final responses
5. Maintain JSON-RPC 2.0 transparency per RAT-LWS-REQ-011

Repository is now in proper RED state with failing tests that exercise the acceptance criteria.