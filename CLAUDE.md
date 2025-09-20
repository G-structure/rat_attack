# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Rust (Bridge)
- `cargo fmt --all` - Format code before commits
- `cargo clippy --all-targets --all-features -D warnings` - Lint gate (must pass)
- `cargo test` - Run full async test suite
- `cargo test bridge_handshake` - Focus on bridge handshake tests
- `cargo run --bin ct-bridge` - Run the bridge with handshake logs

### Frontend (ct-web)
- `pnpm install --dir ct-web` - Install dependencies
- `pnpm --dir ct-web dev` - Serve dashboard at http://localhost:5173
- `pnpm --dir ct-web test` - Run Vitest test suite
- `pnpm --dir ct-web build` - Build for production

### Testing Best Practices
- Use `#[tokio::test(flavor = "multi_thread")]` for async tests
- Add `serial_test::serial` for filesystem state or environment variable tests
- Preserve spec callouts like `RAT-LWS-REQ-###` in assertions
- Run `cargo test -- --nocapture` for detailed logging

## Architecture Overview

This project implements a **local-first remote control stack** where a SolidJS web dashboard (`ct-web`) communicates with one or more Rust bridge processes (`ct-bridge`) that proxy ACP (Agent Client Protocol) agents over WebSockets.

### Key Components

#### CT-BRIDGE (Rust)
- **Main entry**: `src/main.rs` (minimal entrypoint)
- **Core logic**: `src/lib.rs` (bridge server, transport, ACP forwarding)
- **Purpose**: WebSocket server that forwards ACP JSON-RPC between web clients and downstream agents
- **Responsibilities**:
  - WebSocket upgrade validation and origin checking
  - ACP message routing with bridge ID injection
  - File system operations (`fs/read_text_file`, `fs/write_text_file`) with project root sandboxing
  - Permission management and approval gating
  - Auth WebView Proxy (AWP) for same-IP authentication flows
  - Optional terminal and MCP proxying

#### CT-WEB (SolidJS)
- **Frontend stack**: SolidJS + Vite + TailwindCSS + TypeScript
- **Entry point**: `ct-web/src/index.tsx`
- **Main component**: `ct-web/src/App.tsx`
- **Purpose**: Mobile-first responsive web dashboard
- **Features**: Multi-bridge sessions, streaming chat, file editor, terminal, permission dialogs

#### Key Architectural Patterns
- **Multi-bridge topology**: One web client connects to N bridge instances
- **Session addressing**: Routes by `(bridgeId, sessionId)` tuple
- **ACP compliance**: Implements Agent Client Protocol over JSON-RPC WebSockets
- **Permission-gated operations**: All file writes and terminal commands require approval
- **Project root sandboxing**: File operations restricted to declared project boundaries

### External References
The `external_refrence/` directory contains vendored repositories for reference:
- `claude-code-acp/`: Claude's ACP adapter implementation
- `agent-client-protocol/`: Canonical ACP library and schema
- `zed/`: Zed editor source (see GPUI patterns and ACP client reference)
- `opencode/`: Terminal-based AI coding agent
- `sst/`: Infrastructure-as-code framework patterns

### Important Documentation
- `planner/spec.md`: **The authoritative contract** - update spec first, then implementation, then tests
- `AGENTS.md`: Contains comprehensive project guidelines and development patterns
- `planner/docs/zed_acp_*.md`: Critical ACP integration patterns from Zed's implementation

## Coding Conventions

### Rust Style
- **Indentation**: 4 spaces (no tabs)
- **Naming**: snake_case for modules/functions, PascalCase for public types, SCREAMING_SNAKE_CASE for constants
- **Error handling**: Favor `Result` and `?` over `unwrap()` or `panic!()`
- **Comments**: Only add "why" comments for non-obvious decisions, not "what" summaries
- **Async patterns**: Use `#[tokio::test(flavor = "multi_thread")]` for tests

### Frontend Style
- **Components**: PascalCase files in `ct-web/src/`
- **Props**: camelCase naming
- **Assets**: Vite-managed imports
- **Testing**: Vitest with SolidJS testing library

### Key Requirements from Spec
- **RAT-LWS-REQ-044**: File operations must be sandboxed to project roots
- **RAT-LWS-REQ-041**: All writes must be approval-gated via `session/request_permission`
- **RAT-LWS-REQ-300**: Each bridge must expose stable `bridgeId` in `initialize` response
- **RAT-LWS-REQ-031**: Agents must stream `session/update` chunks during prompts

## Development Workflow

1. **Before changes**: Read `planner/spec.md` for contract requirements
2. **ACP patterns**: Reference `planner/docs/zed_acp_*.md` for proven implementation patterns
3. **External code**: Check `external_refrence/` repos when unsure about ACP semantics
4. **Testing**: Run integration tests that simulate full agent conversations
5. **Validation**: Ensure `cargo clippy` and `cargo fmt` pass before commits

## Security & Sandboxing

- File operations are restricted to project roots with path canonicalization
- Permission model defaults to deny-all with explicit approval requirements
- Authentication flows use Auth WebView Proxy to maintain IP alignment
- All tool calls require user permission except cached "always allow" decisions

## Multi-Bridge Design

This system supports multiple concurrent bridge connections:
- Each bridge instance has a unique `bridgeId`
- Sessions are addressed as `(bridgeId, sessionId)` tuples
- Project roots, permissions, and resources are isolated per bridge
- Independent reconnect logic with exponential backoff per bridge