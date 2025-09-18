§ TEST 2025-09-18T[timestamp]

**Task**: Enhance FakeStreamingAgentTransport to use NotificationSender during prompt() calls

**Problem**: Two streaming tests were failing:
- `bridge_streams_session_prompt_updates`: Expected session/update notifications but received none
- `bridge_session_update_preserves_json_rpc_format`: Expected session/update notifications with proper JSON-RPC format

**Root Cause**: The `FakeStreamingAgentTransport.prompt()` method was ignoring the `notification_sender` parameter and not sending any session/update notifications during prompt execution.

**Solution**: Modified the `prompt()` method in `FakeStreamingAgentTransport` to:
1. Use the `notification_sender.send_notification()` method to send session/update notifications
2. Send configured streaming updates if they exist (via `configure_streaming_updates()`)
3. Send default streaming updates if no specific updates were configured
4. Properly handle async notification sending before returning the final prompt response

**Key Changes**:
- Changed `_notification_sender` parameter to `notification_sender` (removed underscore)
- Added logic to clone streaming_updates and check if they exist before releasing the lock
- Implemented actual notification sending using `notification_sender.send_notification("session/update", update)`
- Added default streaming behavior when no specific updates are configured

**Test Results**:
- All 8 tests now pass (was 6 passed, 2 failed)
- Both previously failing streaming tests now pass:
  - `bridge_streams_session_prompt_updates` ✓
  - `bridge_session_update_preserves_json_rpc_format` ✓

**Commands Run**:
```bash
cargo test bridge_streams_session_prompt_updates
cargo test bridge_session_update_preserves_json_rpc_format
cargo test  # Full test suite
```

**Artifacts**: Modified `/Users/luc/projects/vibes/rat_attack/tests/bridge_handshake.rs` - specifically the `FakeStreamingAgentTransport::prompt()` method implementation.

**Evidence**: The tests exercise the full flow: prompt request → agent sends notifications → bridge relays to CT-WEB, validating that agents can now send session/update notifications through the NotificationSender interface during prompt execution.