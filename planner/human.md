Zed’s ACP notes are the fastest way to build intuition before touching the bridge code. Start work by sampling these docs so you absorb how a production client wires transport, agents, permissions, and UX.

**ZED NOTES – WHAT TO READ AND WHY**
- `planner/docs/zed_acp.md`: Orientation map. Read first to internalize the bridge/client split and request flow.
- `planner/docs/zed_acp_agent_launching.md`: Launch orchestration. Review when touching process supervision or stdio plumbing.
- `planner/docs/zed_acp_extensibility.md`: Extension patterns. Skim early so `_meta` handling stays consistent.
- `planner/docs/zed_acp_login.md`: Auth + webview proxy. Revisit before implementing or modifying AWP.
- `planner/docs/zed_acp_mcp_integration.md`: MCP proxy pipeline. Check whenever we proxy tool calls or register MCP servers.
- `planner/docs/zed_acp_premissions.md`: Permission UX + policy storage. Use as the template for our approval model.
- `planner/docs/zed_acp_session_management.md`: Session lifecycle and reconnect logic. Consult before changing session routing.
- `planner/docs/zed_acp_tools.md`: Tool catalog and ACP bindings. Reference when exposing or gating file/terminal tools.
- `planner/docs/zed_acp_ui.md`: UX decisions. Useful when syncing web UI affordances with protocol events.
- `planner/docs/zed_acp_debugging.md`: Diagnostics. Borrow ideas whenever we need better logging or troubleshooting hooks.

**WHEN TO DIVE INTO THE EXTERNAL CODEBASES**
The vendored repos under `external_refrence/` are checked out to the exact versions we target. Treat them as living specs—open them whenever you wonder how the real implementations solve a problem.
- `external_refrence/claude-code-acp`: Claude’s ACP adapter. Explore to see how agents structure `initialize`, session streaming, tool invocation, and MCP proxying. Copy interaction patterns rather than guessing.
- `external_refrence/agent-client-protocol`: Canonical ACP library (Rust + TS) plus schema. Use it to confirm field names, error semantics, and helper APIs before writing code. Prefer importing its types over rolling our own.

## Additional Guidance
- CT-BRIDGE is an ACP **client-side** implementation. It forwards ACP JSON-RPC between CT-WEB and downstream agents while owning local capabilities (fs, permissions, terminal). Keep it thin—no bespoke agent logic.
- Always lean on the official `agent-client-protocol` crates for message handling. Typed APIs keep us aligned on negotiated versions, capabilities, and structured errors.
- Filesystem, terminal, and permission methods are invoked by the agent. Implement the ACP `Client` trait methods (`fs/read_text_file`, `fs/write_text_file`, `terminal/*`) with strict project-root sandboxing and policy checks before responding.
- Maintain a routing map keyed by `(bridgeId, sessionId)` so multiple agents or sessions can share one WebSocket. Forward payloads verbatim aside from injecting `_meta.bridgeId` where required.
- Treat `planner/spec.md` as the contract. Update the spec first whenever behavior changes, then implementation, then tests. Surface mismatches immediately.
- Borrow architectural patterns from Zed’s docs—process lifecycle, permission prompts, MCP proxying—but adapt configuration and UX to our requirements.
- Before merging bridge changes, run integration tests that simulate a full agent conversation (`initialize` → `session/new` → tool calls). Add regression coverage whenever routing or capability negotiation shifts.
