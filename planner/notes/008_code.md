§ CODE Thu Sep 18 2025 12:00:00 - fs/write_text_file permission policy caching implementation

## Implementation Details

### Permission Cache Structure
- Added `PermissionDecision` enum with `AllowAlways` and `RejectAlways` variants
- Created `PermissionCache` type alias as `Arc<TokioMutex<HashMap<String, PermissionDecision>>>`
- Integrated cache into `BridgeSharedConfig` for per-bridge instance isolation

### Cache Logic in handle_write_text_file
1. **Cache Check First**: Before requesting permission, check if canonical path has cached decision
2. **AllowAlways**: If cached `AllowAlways`, proceed with write without contacting agent
3. **RejectAlways**: If cached `RejectAlways`, return permission denied error immediately
4. **No Cache**: Request permission from agent as usual
5. **Cache Updates**: Store `AllowAlways`/`RejectAlways` decisions, ignore `AllowOnce`/`RejectOnce`

### Key Design Decisions
- Cache keyed by canonical path string for consistent lookups
- Per-bridge cache ensures isolation between different bridge instances
- Thread-safe using `Arc<TokioMutex<>>` for concurrent access
- Only persistent decisions (`AllowAlways`/`RejectAlways`) are cached per RAT-LWS-REQ-091

### Files Modified
- `src/lib.rs`: Added permission cache structures and logic to `handle_write_text_file`
- `.gitignore`: Added patterns for test output files (`*_test.txt`, `cache_*.txt`, etc.)

### Test Results
- All 26 Rust tests passing including new caching tests
- No TypeScript tests configured (pnpm vitest not available)
- Cache behavior verified: allow_always skips permission requests, reject_always fails immediately

### Command Outputs
```
$ cargo test
running 26 tests
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Acceptance Verification
- ✅ Bridge caches permission decisions for `fs/write_text_file` requests
- ✅ `allow_always` decisions skip subsequent permission round-trips
- ✅ `reject_always` decisions cause immediate failures without contacting agent
- ✅ When no policy exists, permission is requested from agent
- ✅ Decisions scoped to project-root canonical paths via existing sandboxing

### Follow-up Tasks
- Consider persisting cache across bridge restarts (currently in-memory only)
- Generalize caching mechanism for other permission-gated operations (terminal, MCP)
- Add cache size limits and eviction policies for long-running bridges