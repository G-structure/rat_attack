# Zed ACP Authentication Flow

## Overview
Zed integrates with the Agent Client Protocol (ACP) to enable authentication for AI agents, particularly Claude Code. This document provides a comprehensive analysis of the authentication implementation, mapping the complete login flow from code exploration of Zed, claude-code-acp, and the ACP protocol specification.

## Implementation Architecture

### Zed's ACP Implementation
Zed's ACP integration is built around several key components:

- **AcpThread** (`zed/crates/acp_thread/src/acp_thread.rs`): Core thread management handling session lifecycle, message processing, and tool call execution. Manages connection state, handles session updates, and coordinates between UI and agent.

- **AgentConnection Trait** (`zed/crates/acp_thread/src/connection.rs`): Defines the interface for agent communication with methods like `authenticate()`, `prompt()`, `new_session()`, and session management operations.

- **ThreadView** (`zed/crates/agent_ui/src/acp/thread_view.rs`): UI layer managing thread states (`Loading`, `Ready`, `Unauthenticated`, `LoadError`) and authentication flows. Handles user interactions and state transitions.

### Claude Code ACP Agent
The `ClaudeAcpAgent` class (`claude-code-acp/src/acp-agent.ts`) implements the ACP Agent interface:

- **Credential Management**: Checks for `.claude.json` file existence in the user's home directory
- **Authentication Methods**: Advertises `claude-login` method during initialization
- **Session Management**: Creates Claude SDK query instances and manages tool use caching
- **Message Conversion**: Translates Claude SDK messages to ACP protocol notifications

### ACP Protocol Layer
The protocol layer (`agent-client-protocol/typescript/acp.ts`) defines:

- **Authentication Schema**: `AuthenticateRequest`/`AuthenticateResponse` interfaces
- **Auth Methods**: `AuthMethod` interface with id, name, and description fields
- **Error Handling**: `RequestError.authRequired()` for authentication failures
- **Session Flow**: Complete lifecycle from initialization to authenticated sessions

## Step-by-Step Authentication Flow

### 1. Session Initialization
When a user initiates an ACP session in Zed:

```rust
// From acp_thread.rs
pub struct AcpThread {
    connection: Rc<dyn AgentConnection>,
    session_id: acp::SessionId,
    // ... other fields
}
```

The `AcpThread::new()` method creates a new thread with an `AgentConnection`. The connection is established through `agent_servers::connect()`, which spawns the Claude Code process and sets up the ACP communication channel.

### 2. Authentication Check
During session creation, the Claude ACP agent performs credential validation:

```typescript
// From acp-agent.ts
async newSession(params: NewSessionRequest): Promise<NewSessionResponse> {
    if (
      fs.existsSync(path.resolve(os.homedir(), ".claude.json.backup")) &&
      !fs.existsSync(path.resolve(os.homedir(), ".claude.json"))
    ) {
      throw RequestError.authRequired();
    }
    // ... session creation continues
}
```

If `.claude.json` is missing, `RequestError.authRequired()` is thrown, signaling Zed that authentication is required.

### 3. UI State Transition
Zed's UI responds to authentication requirements:

```rust
// From thread_view.rs
enum ThreadState {
    Loading(Entity<LoadingView>),
    Ready { thread: Entity<AcpThread>, title_editor: Option<Entity<Editor>>, _subscriptions: Vec<Subscription> },
    LoadError(LoadError),
    Unauthenticated { connection: Rc<dyn AgentConnection>, description: Option<Entity<Markdown>>, configuration_view: Option<AnyView>, pending_auth_method: Option<acp::AuthMethodId>, _subscription: Option<Subscription> },
}
```

The thread state transitions to `ThreadState::Unauthenticated`, displaying authentication UI with available auth methods.

### 4. Authentication Method Selection
Zed presents available authentication methods to the user:

```rust
// From thread_view.rs
fn authenticate(&mut self, method: acp::AuthMethodId, window: &mut Window, cx: &mut Context<Self>) {
    // Handle different auth methods
    if method.0.as_ref() == "claude-login" {
        if let Some(workspace) = self.workspace.upgrade() {
            Self::spawn_claude_login(&workspace, window, cx)
        }
        // ... authentication logic
    }
}
```

For `claude-login`, Zed spawns a terminal running `claude /login`.

### 5. Claude Code Login Execution
Zed executes the login command in a terminal:

```rust
// From thread_view.rs
fn spawn_claude_login(workspace: &Entity<Workspace>, window: &mut Window, cx: &mut App) -> Task<Result<()>> {
    // Creates terminal panel, executes login command
    let terminal = terminal_panel.update_in(cx, |terminal_panel, window, cx| {
        terminal_panel.spawn_task(&SpawnInTerminal {
            command: Some(command.into()),
            args,
            // ... other terminal config
        }, window, cx)
    })?;
    // Monitors for "Login successful" output
}
```

The terminal monitors Claude Code's output for successful authentication.

### 6. Authentication Completion
Upon successful login:

1. Claude Code creates/updates `.claude.json` with authentication tokens
2. The ACP agent detects the credential file and allows session creation
3. Zed transitions the thread state back to `ThreadState::Ready`
4. Normal ACP communication resumes

### 7. Error Handling and Recovery
Authentication failures are handled through:

```rust
// From thread_view.rs:3290-3310
fn handle_thread_error(&mut self, error: anyhow::Error, cx: &mut Context<Self>) {
    self.thread_error = Some(ThreadError::from_err(error, &self.agent));
    cx.notify();
}
```

The `ThreadError` enum includes `AuthenticationRequired` variants for different auth methods, allowing users to retry or switch authentication approaches.

## Key Code References

### Zed Core Components
- **AcpThread**: `zed/crates/acp_thread/src/acp_thread.rs:55-90` - Main thread structure and session management
- **AgentConnection**: `zed/crates/acp_thread/src/connection.rs:22-78` - Agent communication interface
- **ThreadView**: `zed/crates/agent_ui/src/acp/thread_view.rs:296-311` - UI state management
- **Authentication Logic**: `zed/crates/agent_ui/src/acp/thread_view.rs:3567-3650` - Auth method handling

### Claude Code Integration
- **ClaudeAcpAgent**: `claude-code-acp/src/acp-agent.ts:62-98` - ACP agent implementation
- **Credential Check**: `claude-code-acp/src/acp-agent.ts:99-106` - Authentication validation
- **Auth Methods**: `claude-code-acp/src/acp-agent.ts:90-96` - Available authentication options

### ACP Protocol
- **Authentication Schema**: `agent-client-protocol/typescript/schema.ts:989-1001` - Auth request/response definitions
- **Auth Methods**: `agent-client-protocol/typescript/schema.ts:1243-1266` - Auth method interface
- **Error Handling**: `agent-client-protocol/typescript/acp.ts:962` - `RequestError.authRequired()`

## Interesting Implementation Details

### Terminal-Based Authentication
Zed's approach of spawning Claude Code in a terminal (`spawn_claude_login`) is notable because:
- It leverages Claude Code's existing authentication flow
- Provides users with familiar CLI authentication experience
- Allows monitoring of authentication progress through terminal output
- Handles both interactive and programmatic authentication scenarios

#### Hidden Terminal and PTY Implementation
Zed uses a pseudo-terminal (PTY) approach to spawn Claude Code in a hidden terminal:

```rust
// From thread_view.rs:1520-1600 (spawn_claude_login implementation)
let terminal = terminal_panel.update_in(cx, |terminal_panel, window, cx| {
    terminal_panel.spawn_task(&SpawnInTerminal {
        command: Some("claude".into()),
        args: vec!["/login".into()],
        cwd: None,
        env: None,
        use_pty: true,  // Uses PTY for interactive terminal behavior
        hide: task::HideStrategy::Always,  // Terminal is hidden from user
        // ... other configuration
    }, window, cx)
})?;
```

Key aspects:
- **PTY Usage**: `use_pty: true` enables pseudo-terminal mode, allowing interactive input/output
- **Hidden Terminal**: `hide: task::HideStrategy::Always` keeps the terminal invisible to the user
- **Command Execution**: Runs `claude /login` command via `ClaudeCode::login_command()` in `claude.rs`

#### Terminal Output Monitoring
Zed monitors the terminal output every second to detect authentication completion:

```rust
// Conceptual monitoring loop (integrated into terminal task)
loop {
    let output = terminal.read_output();
    if output.contains("Login successful") {
        // Authentication completed successfully
        break;
    }
    // Wait 1 second before next check
    sleep(Duration::from_secs(1));
}
```

This approach:
- **No Direct URL Extraction**: Zed does not "grab" sign-in links directly; it relies on Claude Code's internal authentication flow
- **Browser Opening**: Claude Code handles opening the browser to Claude's sign-in page internally
- **Output-Based Detection**: Monitors for specific strings like "Login successful" to determine completion
- **Non-Intrusive**: Allows Claude Code to manage the entire authentication process independently

#### Integration Flow
The integration between components follows this sequence:

1. **Zed UI**: User selects `claude-login` method → Calls `spawn_claude_login()`
2. **Terminal Spawn**: Creates hidden PTY terminal → Executes `claude /login`
3. **Claude Code**: Handles authentication internally → Opens browser if needed → Outputs status messages
4. **Output Monitoring**: Zed polls terminal output → Detects "Login successful" → Updates UI state
5. **State Transition**: Thread state changes from `Unauthenticated` to `Ready` → Normal ACP communication resumes

### Credential File Management
The `.claude.json` file serves as the authentication state:
- Located in user's home directory (`os.homedir()`)
- Contains session tokens and authentication metadata
- Checked on every session creation
- Backup file (`.claude.json.backup`) indicates previous authentication state

### State Synchronization
The authentication flow involves complex state synchronization:
- UI state (`ThreadState`) reflects authentication progress
- ACP protocol messages coordinate between Zed and Claude Code
- Terminal output monitoring detects authentication completion
- Error propagation maintains consistent state across components

### Multiple Auth Method Support
Zed supports various authentication methods:
- `claude-login`: Terminal-based Claude Code authentication
- `gemini-api-key`: Direct API key for Gemini
- `anthropic-api-key`: Direct API key for Claude
- `vertex-ai`: Google Cloud Vertex AI authentication

### Tool Use Integration
Authentication state affects tool execution:
- Unauthenticated sessions cannot proceed with tool calls
- Authentication requirements are checked before tool authorization
- Successful authentication enables full ACP functionality

## Potential Improvements

### Enhanced User Experience
- **Progress Indicators**: Add visual feedback during terminal-based authentication
- **Alternative Auth Methods**: Implement OAuth flows for web-based authentication
- **Credential Validation**: Add proactive credential validation and refresh
- **Error Recovery**: Better handling of partial authentication states

### Security Enhancements
- **Token Storage**: Consider secure credential storage mechanisms
- **Session Management**: Implement token expiration and refresh logic
- **Multi-Factor Auth**: Support for additional authentication factors

### Developer Experience
- **Debugging Tools**: Add authentication state inspection capabilities
- **Testing Infrastructure**: Improve test coverage for authentication flows
- **Configuration**: Allow customization of authentication methods per agent

### Performance Optimizations
- **Connection Reuse**: Cache authenticated connections across sessions
- **Parallel Auth**: Support concurrent authentication for multiple agents
- **Lazy Loading**: Defer authentication until actually needed for tool use
