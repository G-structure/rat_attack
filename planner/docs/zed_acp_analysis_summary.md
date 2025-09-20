# Zed ACP Analysis Summary

## Overview
This document summarizes the analysis of Zed's implementation of the Agent Client Protocol (ACP), conducted to inform improvements and expansions to our CT-Bridge refactor. The analysis aimed to systematically review the Zed codebase at `/Users/luc/projects/vibes/rat_attack/external_reference/zed`, cross-referencing it with the official ACP specification in `planner/spec.md`, the `claude-code-acp` adapter, and the `agent-client-protocol` library. Existing notes in `planner/docs/` (e.g., `zed_acp.md`, `zed_acp_permissions.md`) were reviewed and intended for enhancement with precise code snippets, cross-references, and insights into Rust patterns.

The goal was to extract actionable details on ACP client integration, agent launching, permissions, session management, tools, extensibility, authentication, UI, MCP integration, and debugging. This would enable us to adapt Zed's approaches—such as its use of Tokio for async handling, trait-based extensibility, and GPUI for UI—while addressing our project's unique needs like multi-bridge sessions, Auth WebView Proxy (AWP), and mobile-optimized UX as defined in `spec.md` (e.g., RAT-LWS-REQ-300 for bridge identity, RAT-LWS-REQ-200 for AWP).

## Key Findings from Existing Documentation
Due to access limitations (detailed below), the analysis relied heavily on the pre-existing notes in `planner/docs/`, which provide a solid foundation based on prior investigations. These notes align well with our `spec.md` and highlight Zed's strengths as a production ACP client:

### Architecture and Protocol Handling
- Zed implements ACP as a client-side system (`agent-client-protocol/rust/client.rs`), forwarding JSON-RPC over stdio to agent subprocesses. This mirrors our CT-Bridge's role in proxying over WebSockets (RAT-LWS-REQ-011).
- Core crates like `acp_thread`, `agent2`, and `agent_servers` manage threads, agents, and launching. For example, `acp_thread/src/acp_thread.rs` defines `AcpThread` for session state, emphasizing structured error propagation via `Result` types— a Rust idiom we should adopt for reliability.
- Initialization flows (e.g., `initialize` negotiation in `zed/src/main.rs`) advertise capabilities like `fs.readTextFile` and `terminal:true`, directly comparable to RAT-LWS-REQ-020.

### Agent Launching and Session Management
- Zed spawns agents via `agent_servers::init(cx)`, using process supervision and stdio plumbing (cross-ref: `zed_acp_agent_launching.md`). This informs our optional agent spawning in CT-Bridge, with sandboxing to project roots (PRs) per RAT-LWS-REQ-044.
- Sessions are addressed by `SessionId` and tracked in `AcpConnection` structs, supporting streaming via `session/update` notifications (RAT-LWS-REQ-031). Zed's reconnect logic (in `zed_acp_session_management.md`) uses exponential backoff, which we can extend for per-BRID independence (RAT-LWS-REQ-304).

### Permissions and Tools
- Permission flows use `session/request_permission` with user dialogs (e.g., `request_tool_call_authorization` in notes), offering options like `allow_once`/`reject_always` (RAT-LWS-REQ-090). Zed caches decisions client-side, a pattern for our per-BRID policies (RAT-LWS-REQ-092).
- Tools in `agent2/src/tools/` (e.g., `EditFileToolInput` for file edits) implement ACP methods with PR sandboxing. Terminal tools stream via PTY (RAT-LWS-REQ-063), and extensibility via traits allows custom tools—ideal for our MCP proxying (RAT-LWS-REQ-070).

### Authentication and UI
- Login uses webview proxies for provider flows (e.g., Claude integration in `zed_acp_login.md`), ensuring IP alignment similar to our AWP (RAT-LWS-REQ-201). Zed's `render_auth_required_state` in `agent_ui/src/acp/thread_view.rs` handles auth states gracefully.
- UI integration via GPUI (e.g., `agent_panel.rs`) supports streaming, diffs, and mobile-like responsiveness, aligning with RAT-LWS-REQ-100 for small screens.

### MCP Integration and Debugging
- Zed proxies MCP servers with permission enforcement ( `zed_acp_mcp_integration.md`), translating to ACP content blocks (RAT-LWS-REQ-072).
- Debugging includes structured logs and error codes (e.g., `ErrorCode` enum in `agent-client-protocol/rust/error.rs`), with recovery strategies like session reloading—recommendations for our RAT-LWS-REQ-131 ACP log view.

### Rust Patterns and Best Practices
- Zed favors async `Result`-based APIs with Tokio, avoiding panics and using `#[tokio::test]` for integration tests. This promotes our guidelines in `AGENTS.md` (e.g., cargo test with multi-thread flavor).
- Extensibility via traits (e.g., `AgentConnection`) and typed errors reduces boilerplate, while GPUI entities enable reactive UI updates.

Potential pitfalls noted: Zed assumes single-host setups, lacking native multi-bridge support (our RAT-LWS-REQ-301 innovation). Some extensions (e.g., custom auth) may deviate from strict ACP spec, requiring careful adaptation.

## Challenges and Limitations
A major obstacle was the inability to directly access and inspect the Zed codebase at `/Users/luc/projects/vibes/rat_attack/external_reference/zed`. Tool calls to `list_directory`, `find_path`, `terminal` (e.g., `ls external_reference`), and `read_file` on Zed paths consistently failed:
- "Path not found" errors for `external_reference/zed` and subpaths.
- No matches for Zed-specific files like `Cargo.toml` via glob searches.
- This prevented extracting precise code snippets (e.g., from `crates/agent2/src/tools/edit_file_tool.rs`) or verifying flows in entry points like `main.rs`.

As a result, updates to individual `.md` files could not incorporate direct Zed code inspections or new discoveries. The analysis built upon existing notes, which appear comprehensive but may contain outdated or high-level references (e.g., "code in X file" without snippets). No modifications were made to the Zed repo, and no build/run attempts were possible due to access issues.

## Recommendations for CT-Bridge Implementation
1. **Adopt Zed's Client Traits**: Mirror `agent-client-protocol`'s `Client` trait for FS/terminal methods in `src/lib.rs`, ensuring PR sandboxing and AWP integration.
2. **Enhance Multi-Bridge Routing**: Extend Zed's session tracking with `(bridgeId, sessionId)` tuples (RAT-LWS-REQ-302), using a `HashMap` for isolation.
3. **Permission and Auth Alignment**: Implement AWP as a reverse proxy (inspired by Zed's webview) with allow-lists (RAT-LWS-REQ-202), and persist policies per-BRID.
4. **Testing and Observability**: Add integration tests simulating full flows (e.g., `initialize` → auth → prompt), borrowing Zed's `serial_test` for stateful cases. Include BRID-labeled metrics (RAT-LWS-REQ-130).
5. **Next Steps**: Resolve codebase access (e.g., verify path/clone status) to enable snippet-based note updates. Prioritize `zed_acp.md` and `zed_acp_agent_launching.md` for CT-Bridge entry points. Cross-reference with `claude-code-acp` for agent-side patterns.

This analysis confirms Zed as a strong reference for ACP clients, but direct code access is essential for deeper precision. Total findings emphasize thin bridging, typed APIs, and user-centric UX—core to our spec.

*Last Updated: [Current Date]*
```
