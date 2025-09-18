# Zed's Agent Client Protocol (ACP) Implementation

## Overview

Quick links to deeper guides (relative paths):
- [Permissions and Authorization](./zed_acp_premissions.md)
- [Tools Catalog](./zed_acp_tools.md)
- [MCP Integration](./zed_acp_mcp_integration.md)
- [Agent Launching](./zed_acp_agent_launching.md)
- [Session Management](./zed_acp_session_management.md)
- [Debugging Guide](./zed_acp_debugging.md)
- [Extensibility](./zed_acp_extensibility.md)
- [Authentication / Login](./zed_acp_login.md)
- [User Interface](./zed_acp_ui.md)

Zed implements the Agent Client Protocol (ACP) to enable integration with external AI coding agents. ACP is a standardized protocol that allows code editors to communicate with AI agents over JSON-RPC, providing a decoupled architecture where agents run as subprocesses and interact with the editor through a well-defined API.

The protocol addresses the interoperability challenges in the AI coding ecosystem by providing a common interface that any compatible agent can use to work with any compatible editor. In Zed's implementation, this allows users to leverage powerful AI agents like Claude Code and Gemini CLI directly within the editor environment.

ACP is initialized in Zed's main application startup sequence in `zed/crates/zed/src/main.rs`, where `acp_tools::init(cx)` sets up the ACP infrastructure. The protocol follows the JSON-RPC 2.0 specification with bidirectional communication over stdin/stdout, as defined in the `agent-client-protocol` crate (`agent-client-protocol/rust/acp.rs`).

### Protocol Architecture

The ACP architecture follows a client-server model where:

- **Client (Zed)**: Implements the `Client` trait (`agent-client-protocol/rust/client.rs`) and provides the user interface, file system access, and terminal management
- **Server (Agent)**: Implements the `Agent` trait (`agent-client-protocol/rust/agent.rs`) and handles AI reasoning, tool calls, and conversation management

The protocol uses JSON-RPC 2.0 over stdio for transport, with the schema defined in `agent-client-protocol/schema/schema.json`. All communication is asynchronous and bidirectional, allowing agents to stream responses and request permissions in real-time.

### Key Protocol Concepts

- [**Sessions**](./zed_acp_session_management.md): Independent conversation contexts with unique IDs (`SessionId`)
- [**Tool Calls**](./zed_acp_tools.md): Structured requests for operations like file editing or terminal commands
- [**Permissions**](./zed_acp_premissions.md): User authorization system for sensitive operations
- [**MCP Integration**](./zed_acp_mcp_integration.md): Proxy support for Model Context Protocol servers
- **Streaming**: Real-time updates via notifications during agent processing

### Zed's ACP Entry Points

Zed's ACP integration begins with several key initialization points:

1. **Main Application**: `acp_tools::init(cx)` in `main.rs` registers ACP tools and UI components
2. **Agent Servers**: `agent_servers::init(cx)` sets up agent launching infrastructure (details: [Agent Launching](./zed_acp_agent_launching.md))
3. **Agent UI**: `agent_ui::init(cx)` creates the conversation interface (details: [ACP UI](./zed_acp_ui.md))
4. **Agent Settings**: `agent_settings::init(cx)` manages agent configurations

These initialization functions set up the complete ACP ecosystem within Zed's architecture.

## Required Packages and Dependencies

Zed's ACP implementation relies on several key components organized across multiple repositories:

### Core Protocol Library (`agent-client-protocol`)

The foundational crate providing the ACP protocol implementation:

- **Location**: `agent-client-protocol/rust/`
- **Key Files**:
  - `acp.rs`: Main protocol implementation with `ClientSideConnection` and `AgentSideConnection`
  - `agent.rs`: Defines the `Agent` trait with methods like `initialize`, `new_session`, `prompt`
  - `client.rs`: Defines the `Client` trait with methods like `request_permission`, `write_text_file`, `create_terminal`
  - `rpc.rs`: JSON-RPC 2.0 transport layer implementation
  - `error.rs`: Structured error types and codes
- **Schema**: `agent-client-protocol/schema/schema.json` defines the complete protocol structure
- **Documentation**: `agent-client-protocol/docs/` contains comprehensive protocol documentation

The protocol uses JSON-RPC 2.0 over stdio with the following key types:
- `InitializeRequest/Response`: Protocol negotiation
- `NewSessionRequest/Response`: Session creation
- `PromptRequest/Response`: User message handling
- `SessionNotification`: Real-time updates
- `RequestPermissionRequest/Response`: Authorization flows

### Zed-Specific Crates

#### `acp_thread` - Thread Management
- **Location**: `zed/crates/acp_thread/src/`
- **Key Components**:
  - `AcpThread` struct: Manages individual conversation threads
  - `AgentConnection` trait: Abstract interface for agent connections
  - `Diff` and `Mention` types: Rich content representations
  - `Terminal` management: Interactive terminal sessions
- **Responsibilities**:
  - Message history management (`AssistantMessage`, `UserMessage`)
  - Session state coordination
  - Tool call execution and authorization
  - Real-time UI updates via GPUI entities

#### `agent2` - Core Agent Logic
- **Location**: `zed/crates/agent2/src/`
- **Key Components**:
  - `Agent` struct: Main agent implementation
  - Tool implementations in `/tools/` subdirectory
  - `HistoryStore`: Conversation persistence
  - `Thread`: Internal conversation representation
- **Tool Categories**:
  - File operations: `read_file_tool.rs`, `edit_file_tool.rs`, `grep_tool.rs`
  - Terminal: `terminal_tool.rs`
  - Web: `web_search_tool.rs`, `fetch_tool.rs`
  - Development: `diagnostics_tool.rs`
  - Context: `find_path_tool.rs`, `list_directory_tool.rs`

#### `agent_servers` - Agent Launching
- **Location**: `zed/crates/agent_servers/src/`
- **Key Components**:
  - `AcpConnection`: Manages JSON-RPC connections to agents
  - `AgentServer` enum: Different agent types (ClaudeCode, Gemini)
  - `ClientDelegate`: Implements `Client` trait for Zed
- **Features**:
  - Process spawning with proper environment setup
  - Connection lifecycle management
  - MCP server proxying
  - Authentication handling

#### `agent_ui` - User Interface
- **Location**: `zed/crates/agent_ui/src/`
- **Key Components**:
  - `AcpThreadView`: Main conversation interface
  - `MessageEditor`: Rich prompt composition
  - Permission dialogs and onboarding flows
- **UI Features**:
  - Syntax-highlighted code blocks
  - @-mention support for file references
  - Real-time streaming display
  - Tool call visualization

#### `acp_tools` - ACP Tools Integration
- **Location**: `zed/crates/acp_tools/src/`
- **Key Components**:
  - `AcpConnectionRegistry`: Global connection management
  - `AcpTools`: Developer tools for ACP debugging
- **Features**:
  - Connection state tracking
  - Debug logging and inspection
  - Performance monitoring

### External Agent Adapters

#### `@zed-industries/claude-code-acp` - Claude Code Adapter
- **Location**: `claude-code-acp/src/`
- **Key Files**:
  - `acp-agent.ts`: Main ACP agent implementation
  - `mcp-server.ts`: MCP server proxy for Claude Code
  - `tools.ts`: Tool call handling and conversion
- **Architecture**:
  - Wraps Anthropic's Claude Code SDK
  - Implements ACP `Agent` interface
  - Provides MCP compatibility layer
  - Handles authentication and session management

#### Gemini CLI Integration
- **Location**: `zed/crates/agent_servers/src/gemini.rs`
- **Features**:
  - Launches Gemini CLI with `--experimental-acp` flag
  - Version checking and capability detection
  - Environment variable configuration

### Integration Points

These components integrate through several key interfaces:

1. **Initialization**: `acp_tools::init()` sets up global state
2. **Connection**: `AcpConnection::stdio()` establishes agent processes
3. **Session**: `AcpThread::new()` creates conversation contexts
4. **UI**: `AcpThreadView` renders conversations
5. **Tools**: Individual tool implementations handle specific operations

The architecture ensures clean separation between protocol concerns, agent management, and user interface while maintaining high performance and reliability.

## How ACP Works in Zed

### Architecture Overview

Zed acts as the **client** in the ACP architecture, implementing the `Client` trait from `agent-client-protocol`, while AI agents run as **servers** (subprocesses) implementing the `Agent` trait. Communication occurs over stdin/stdout using JSON-RPC 2.0, with the transport layer implemented in `agent-client-protocol/rust/rpc.rs`.

The architecture follows a layered approach:

1. **Transport Layer**: JSON-RPC 2.0 over stdio with bidirectional async communication
2. **Protocol Layer**: Structured request/response/notification patterns defined in the schema
3. **Application Layer**: Zed-specific implementations handling UI, file system, and terminal operations
4. **Integration Layer**: Bridges between ACP protocol and Zed's internal APIs

### Detailed Connection Flow

The complete ACP connection lifecycle follows this detailed sequence:

#### 1. Process Launch and Setup
When a user initiates an ACP session through Zed's UI:

```rust
// From agent_servers/src/acp.rs:61-69
let mut child = util::command::new_smol_command(command.path)
    .args(command.args.iter().map(|arg| arg.as_str()))
    .envs(command.env.iter().flatten())
    .current_dir(root_dir)
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .kill_on_drop(true)
    .spawn()?;
```

For Claude Code: launches `claude-code-acp` with API key environment variables
For Gemini CLI: launches with `--experimental-acp` flag and version checking

#### 2. Connection Establishment
Zed establishes the JSON-RPC connection:

```rust
// From agent_servers/src/acp.rs:82-87
let (connection, io_task) = acp::ClientSideConnection::new(
    client,  // ClientDelegate implementing Client trait
    stdin,
    stdout,
    foreground_executor.spawn(fut).detach()
);
```

The `ClientDelegate` struct implements all required `Client` trait methods, providing the bridge between ACP protocol calls and Zed's internal operations.

#### 3. Protocol Initialization
Zed sends the initialization request to negotiate capabilities:

```rust
// From agent_servers/src/acp.rs:129-140
let response = connection
    .initialize(acp::InitializeRequest {
        protocol_version: acp::VERSION,
        client_capabilities: acp::ClientCapabilities {
            fs: acp::FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
            },
            terminal: true,
        },
    })
    .await?;
```

The agent responds with its capabilities, authentication methods, and supported protocol version. Zed validates the protocol version and stores the agent capabilities for later use.

#### 4. Authentication (if required)
For agents requiring authentication:

```rust
// From agent_servers/src/acp.rs:245-255
let result = conn
    .authenticate(acp::AuthenticateRequest {
        method_id: method_id.clone(),
    })
    .await?;
```

Claude Code authentication involves API key validation through the adapter, while other agents may use different methods.

For a full walkthrough of the authentication flow and UX, see [Authentication Flow](./zed_acp_login.md).

#### 5. Session Creation
Zed creates a new conversation session:

```rust
// From agent_servers/src/acp.rs:199-202
let response = conn
    .new_session(acp::NewSessionRequest { mcp_servers, cwd })
    .await
```

The `NewSessionRequest` includes:
- `mcp_servers`: List of configured MCP servers with their connection details
- `cwd`: Current working directory for the session

The agent responds with a unique `SessionId` and session capabilities.

#### 6. Thread Creation and UI Setup
Zed creates the internal thread representation:

```rust
// From agent_servers/src/acp.rs:217-229
let thread = cx.new(|cx| {
    AcpThread::new(
        self.server_name.clone(),
        self.clone(),
        project,
        action_log,
        session_id.clone(),
        watch::Receiver::constant(self.agent_capabilities.prompt_capabilities),
        cx,
    )
})?;
```

This creates the `AcpThread` entity that manages the conversation state, message history, and coordinates with the UI.

#### 7. Message Exchange and Streaming
User prompts are sent through the UI and forwarded to the agent:

```rust
// From agent_servers/src/acp.rs:262-266
let result = conn.prompt(params).await;
```

The agent processes the prompt and streams responses via `SessionNotification`s containing:
- `MessageChunk`: Incremental text responses
- `ToolCall`: Requests for tool execution
- `ToolCallUpdate`: Progress updates on tool execution
- `PlanEntry`: Structured task planning

#### 8. Tool Call Authorization
When agents request tool execution:

```rust
// From agent_servers/src/acp.rs:350
thread.request_tool_call_authorization(arguments.tool_call, arguments.options, cx)
```

This triggers Zed's permission system, presenting users with approval dialogs for sensitive operations.

#### 9. Tool Execution
Approved tool calls are executed through the `ClientDelegate` implementation:

- File operations: `write_text_file`, `read_text_file`
- Terminal commands: `create_terminal`, `terminal_output`
- Other operations: Handled through appropriate client methods

### Connection Management Details

The `AcpConnection` struct provides comprehensive connection management:

#### Persistent Connection Handling
- Maintains `ClientSideConnection` for JSON-RPC communication
- Uses background tasks for I/O operations, process monitoring, and error logging
- Implements connection pooling and reuse for multiple sessions

#### Session Tracking
```rust
// From agent_servers/src/acp.rs:26-30
pub struct AcpConnection {
    server_name: SharedString,
    connection: Rc<acp::ClientSideConnection>,
    sessions: Rc<RefCell<HashMap<acp::SessionId, AcpSession>>>,
    // ... other fields
}
```

Each session tracks its associated thread and cancellation state.

#### Error Handling and Recovery
- Process crash detection through wait tasks
- JSON-RPC error parsing and user-friendly display
- Connection retry logic for transient failures
- Structured error codes from `agent-client-protocol/rust/error.rs`

### Session Lifecycle Management

Each ACP thread follows a detailed lifecycle managed by the `AcpThread` struct:

#### Initialization Phase
- Creates message history structures (`AssistantMessage`, `UserMessage`)
- Sets up tool call tracking and authorization state
- Initializes terminal and file operation contexts

#### Active Conversation Phase
- Processes incoming `SessionNotification`s
- Updates UI with streaming content
- Manages tool call authorization flows
- Handles cancellation requests

#### Termination Phase
- Cleans up terminal sessions
- Persists conversation history if configured
- Releases system resources
- Updates connection registry

### Advanced Features

#### MCP Server Proxying
Zed discovers configured MCP servers and proxies them to agents:

```rust
// From agent_servers/src/acp.rs:174-196
let mcp_servers = context_server_store
    .configured_server_ids()
    .iter()
    .filter_map(|id| {
        let configuration = context_server_store.configuration_for_server(id)?;
        let command = configuration.command();
        Some(acp::McpServer {
            name: id.0.to_string(),
            command: command.path.clone(),
            args: command.args.clone(),
            // ... env setup
        })
    })
    .collect();
```

This allows agents to access additional tools and context through the MCP ecosystem. More: [MCP Integration](./zed_acp_mcp_integration.md)

#### Real-time Streaming
The protocol supports rich streaming through notifications:

- **Message Streaming**: Incremental text updates during generation
- **Tool Progress**: Real-time updates on long-running operations
- **Terminal Output**: Live command output streaming
- **Status Updates**: Agent state and progress indicators

#### Cancellation and Interruption
Users can cancel ongoing operations:

```rust
// From agent_servers/src/acp.rs:317-327
let params = acp::CancelNotification {
    session_id: session_id.clone(),
};
cx.foreground_executor()
    .spawn(async move { conn.cancel(params).await })
    .detach();
```

This allows graceful interruption of agent processing while maintaining UI responsiveness.

## Features Supported

### Core Protocol Features

Zed provides complete implementation of the ACP protocol specification:

#### JSON-RPC Communication Layer
- **Transport**: Stdio-based JSON-RPC 2.0 implementation (`agent-client-protocol/rust/rpc.rs`)
- **Message Types**: Full support for requests, responses, and notifications
- **Async Processing**: Non-blocking communication using Tokio/Smol runtimes
- **Error Handling**: Structured error responses with codes and messages

#### Session Management
- **Session Isolation**: Each conversation maintains independent state via unique `SessionId`
- **Concurrent Sessions**: Multiple active conversations per agent connection
- **Session Persistence**: Optional history storage and resume capabilities
- **Lifecycle Tracking**: Proper cleanup and resource management

#### Streaming and Real-time Updates
- **Incremental Responses**: Message chunks streamed during generation
- **Tool Progress**: Live updates on long-running operations
- **Status Notifications**: Agent state changes and progress indicators
- **Cancellation Support**: Graceful interruption of ongoing operations

#### Protocol Version Negotiation
- **Version Compatibility**: Automatic negotiation of supported protocol versions
- **Capability Exchange**: Dynamic feature detection and capability advertising
- **Backward Compatibility**: Support for multiple protocol versions

### ACP Agent Tools (Zed implementation)

Only the tools defined under `zed/crates/agent2/src/tools` are used by Zed’s ACP agent implementation. These are exposed to ACP agents via `agent2`’s `AgentTool` interface and are surfaced over ACP as `ToolKind` variants. This is distinct from Zed’s non‑ACP “assistant tools” (`crates/assistant_tools`), which are not invoked by ACP agents. For a complete catalog with parameters and examples, see [ACP Tools](./zed_acp_tools.md).

Below is the ACP tool set with direct code references and their ACP kinds.

#### File and Project Tools

- `read_file` — `ToolKind::Read`
  - Path: `zed/crates/agent2/src/tools/read_file_tool.rs`
  - Reads file content with optional line range; enforces exclusion/private path rules.
- `list_directory` — `ToolKind::Read`
  - Path: `zed/crates/agent2/src/tools/list_directory_tool.rs`
  - Lists directory entries; special handling for `.` and wildcard inputs.
- `find_path` — `ToolKind::Search`
  - Path: `zed/crates/agent2/src/tools/find_path_tool.rs`
  - Glob/path search with pagination; returns sorted matches.
- `grep` — `ToolKind::Search`
  - Path: `zed/crates/agent2/src/tools/grep_tool.rs`
  - Regex search across the project; supports include globs, pagination, and context lines.
- `edit_file` — `ToolKind::Edit`
  - Path: `zed/crates/agent2/src/tools/edit_file_tool.rs`
  - Performs edits via Zed’s `EditAgent`; supports edit/create/overwrite modes and returns a diff summary.
- `create_directory` — `ToolKind::Read`
  - Path: `zed/crates/agent2/src/tools/create_directory_tool.rs`
  - Creates a directory (mkdir -p semantics) inside the project; validates project paths.
- `move_path` — `ToolKind::Move`
  - Path: `zed/crates/agent2/src/tools/move_path_tool.rs`
  - Move/rename files or directories inside the project.
- `copy_path` — `ToolKind::Move`
  - Path: `zed/crates/agent2/src/tools/copy_path_tool.rs`
  - Copies files/directories (recursive for dirs) within the project.
- `delete_path` — `ToolKind::Delete`
  - Path: `zed/crates/agent2/src/tools/delete_path_tool.rs`
  - Deletes files/directories (recursive) with project boundary checks.

#### Execution and External Access

- `terminal` — `ToolKind::Execute`
  - Path: `zed/crates/agent2/src/tools/terminal_tool.rs`
  - Runs shell one‑liners inside a Zed‑managed terminal; emits `ToolCallContent::Terminal { terminal_id }`, then streams and summarizes output. Uses `ThreadEnvironment::create_terminal(...)` which is implemented for ACP threads in `zed/crates/agent2/src/agent.rs` via `AcpThreadEnvironment` delegating to `acp_thread::AcpThread`.
- `open` — `ToolKind::Execute`
  - Path: `zed/crates/agent2/src/tools/open_tool.rs`
  - Opens a file or URL with the OS default application; requires user authorization.
- `fetch` — `ToolKind::Fetch`
  - Path: `zed/crates/agent2/src/tools/fetch_tool.rs`
  - HTTP fetch with content‑type aware conversion to Markdown.

#### Introspection and Utility

- `diagnostics` — `ToolKind::Read`
  - Path: `zed/crates/agent2/src/tools/diagnostics_tool.rs`
  - Surfaces LSP diagnostics either project‑wide or for a file.
- `thinking` — `ToolKind::Think`
  - Path: `zed/crates/agent2/src/tools/thinking_tool.rs`
  - Emits model “thinking” content as tool output for planning.
- `now` — `ToolKind::Other`
  - Path: `zed/crates/agent2/src/tools/now_tool.rs`
  - Returns current time in RFC3339, with timezone selection.

#### File System Operations (`zed/crates/agent2/src/tools/`)

**Read File Tool** (`read_file_tool.rs`):
- Implements `fs/read_text_file` protocol method
- Supports line range specification and offset/limit parameters
- Handles encoding detection and binary file rejection
- Integrates with Zed's buffer system for efficient access

**Edit File Tool** (`edit_file_tool.rs`):
- Handles `fs/write_text_file` requests with diff-based editing
- Uses Zed's `EditAgent` for intelligent code modifications
- Supports both full file replacement and targeted edits
- Provides detailed change descriptions for user approval

```rust
// From edit_file_tool.rs:25-50
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EditFileToolInput {
    /// A one-line, user-friendly markdown description of the edit
    pub display_description: String,
    /// The full path of the file to create or modify
    pub file_path: String,
    /// The old string to replace (empty for new files)
    pub old_string: String,
    /// The new string to insert
    pub new_string: String,
}
```

**Search and Navigation Tools**:
- **Grep Tool** (`grep_tool.rs`): Regex search across codebase using ripgrep
- **Find Path Tool** (`find_path_tool.rs`): File discovery with glob patterns
- **List Directory Tool** (`list_directory_tool.rs`): Directory browsing with filtering

#### How terminal execution is wired for ACP

- Tool entry point: `agent2/src/tools/terminal_tool.rs` implements `AgentTool` with `ToolKind::Execute` and calls `ThreadEnvironment::create_terminal`.
- Environment bridge: `agent2/src/agent.rs` implements `ThreadEnvironment` for `AcpThreadEnvironment::create_terminal(...)`, which calls `acp_thread::AcpThread::create_terminal(...)` and returns a `TerminalHandle` (`AcpTerminalHandle`).
- Output/UI: The tool updates the ACP tool call with `ToolCallContent::Terminal { terminal_id }` so the UI can show a live terminal view, and then summarizes the exit status and output when complete.

#### Development and Analysis Tools

**Language Server Integration** (`diagnostics_tool.rs`):
- Access to LSP diagnostics and error reporting
- Code analysis and linting results
- Type checking and compilation feedback

**Web and External Data** (`fetch_tool.rs`):
- HTTP requests for documentation and research
- Web scraping capabilities for context gathering
- API integration for external services

**Utility Tools**:
- **Now Tool** (`now_tool.rs`): Timestamp generation for logging
- **Thinking Tool** (`thinking_tool.rs`): Internal reasoning visualization

#### MCP Server Integration

Zed acts as an MCP proxy, allowing agents to access additional tools:

```rust
// From agent_servers/src/acp.rs:174-196
let mcp_servers = context_server_store
    .configured_server_ids()
    .iter()
    .filter_map(|id| {
        let configuration = context_server_store.configuration_for_server(id)?;
        Some(acp::McpServer {
            name: id.0.to_string(),
            command: command.path.clone(),
            args: command.args.clone(),
            env: /* environment variables */,
        })
    })
    .collect();
```

This enables seamless integration with the broader MCP ecosystem.

### Advanced Permission and Security System

Zed implements a sophisticated permission framework for tool call authorization:

#### Permission Request Flow
1. Agent sends `session/request_permission` with tool call details
2. Zed evaluates the request against security policies
3. User is presented with approval dialog showing operation details
4. Permission decision is cached for future similar requests

#### Permission Levels
- **Allow Once**: Single-use permission for specific operation
- **Allow Always**: Persistent permission for operation type
- **Reject Once**: Temporary denial with re-prompt capability
- **Reject Always**: Permanent denial for operation type

#### Security Features
- **Operation Validation**: Syntax and safety checking of tool parameters
- **Path Sandboxing**: Restriction to project directory boundaries
- **Resource Limits**: Memory and execution time constraints
- **Audit Logging**: Complete permission decision history

See [zed_acp_premissions.md](./zed_acp_premissions.md) for comprehensive permission system documentation.

### Authentication and Identity Management

ACP supports multiple authentication methods for agent access:

#### Authentication Methods
- **API Key Authentication**: For cloud-based agents like Claude Code
- **OAuth Flows**: For integrated service authentication
- **Token-based Auth**: For enterprise and custom deployments

#### Authentication Flow
```rust
// From agent_servers/src/acp.rs:245-255
let result = conn
    .authenticate(acp::AuthenticateRequest {
        method_id: method_id.clone(),
    })
    .await?;
```

#### Claude Code Integration
The `@zed-industries/claude-code-acp` adapter handles:
- API key validation and storage
- Session-scoped authentication
- Token refresh and renewal
- Error handling for auth failures

See [zed_acp_login.md](./zed_acp_login.md) for detailed authentication workflows.

### Rich User Interface Integration

Zed provides a comprehensive UI for ACP interactions:

#### Agent Panel (`agent_ui/src/agent_panel.rs`)
- Conversation thread management
- Agent selection and switching
- Session history and bookmarks
- Performance monitoring and statistics

#### Thread View (`agent_ui/src/acp/thread_view.rs`)
- **Message Rendering**: Syntax-highlighted code blocks and markdown
- **@-Mention Support**: File and symbol references with preview
- **Streaming Display**: Real-time response rendering
- **Tool Call Visualization**: Interactive tool execution displays

```rust
// From thread_view.rs:3000-3050 (authentication UI)
fn render_auth_required_state(&self, ...) {
    // Rich authentication UI with method selection
    // Error display and retry capabilities
    // Configuration dialogs for API keys
}
```

#### Message Composition
- **Rich Editor**: Multi-line input with syntax highlighting
- **Context Insertion**: Drag-and-drop file references
- **Template Support**: Reusable prompt templates
- **History Recall**: Previous message suggestions

#### Permission Dialogs
- **Contextual Approval**: Operation details and impact assessment
- **Batch Operations**: Multiple permission requests handling
- **Remember Decisions**: Persistent permission storage
- **Audit Trail**: Permission decision history

#### Onboarding and Guidance
- **First-time Setup**: Interactive configuration wizards
- **Feature Discovery**: Progressive disclosure of capabilities
- **Help Integration**: Context-sensitive documentation links
- **Error Recovery**: Guided troubleshooting flows

### MCP Server Proxy Architecture

Zed implements sophisticated MCP server proxying:

#### Server Discovery and Configuration
- Automatic detection of configured MCP servers
- Dynamic capability advertisement to agents
- Connection pooling and health monitoring
- Version compatibility checking

#### Proxy Implementation
- **Bidirectional Communication**: Seamless agent-to-MCP request routing
- **Protocol Translation**: ACP to MCP message conversion
- **Error Handling**: Graceful degradation on MCP failures
- **Performance Optimization**: Connection reuse and caching

#### Integration Features
- **Tool Discovery**: Dynamic tool availability based on MCP servers
- **Context Sharing**: Unified context across ACP and MCP tools
- **Security Mediation**: Permission enforcement for proxied operations

This architecture allows agents to leverage the entire MCP ecosystem while maintaining Zed's security and performance standards.

## Implementation Details

### Thread Management Architecture

The `AcpThread` struct serves as the central coordinator for ACP conversations:

#### Core Structure (`acp_thread/src/acp_thread.rs:40`)
```rust
pub struct AcpThread {
    session_id: acp::SessionId,
    connection: Rc<dyn AgentConnection>,
    messages: Vec<Message>,
    tool_calls: Vec<ToolCall>,
    status: ThreadStatus,
    // ... additional fields
}
```

#### Message History Management
- **User Messages**: Store prompts with content, timestamps, and context
- **Assistant Messages**: Track AI responses, tool calls, and reasoning
- **Message Chunking**: Handle streaming responses with incremental updates
- **History Persistence**: Optional storage for conversation continuity

#### Session Update Processing
```rust
// From acp_thread/src/acp_thread.rs:396
fn handle_session_update(&mut self, update: acp::SessionNotification, cx: &mut Context<Self>) {
    match update.update {
        acp::SessionUpdate::MessageChunk { chunk } => {
            // Handle incremental message updates
        }
        acp::SessionUpdate::ToolCall { tool_call } => {
            // Process tool execution requests
        }
        acp::SessionUpdate::ToolCallUpdate { update } => {
            // Update tool execution progress
        }
    }
}
```

#### UI Coordination
- **GPUI Integration**: Reactive updates to UI components
- **Entity Management**: Proper lifecycle handling for UI elements
- **Event Emission**: Notification of state changes to observers
- **Focus Management**: Keyboard navigation and accessibility

#### Terminal Lifecycle Management
- **Terminal Creation**: Spawn interactive sessions on demand
- **Output Streaming**: Real-time display of command results
- **Resource Cleanup**: Proper termination and resource release
- **Session Persistence**: Terminal state across conversation turns

### Tool Call Authorization System

Zed implements a comprehensive authorization framework for tool execution (details and UI flows: [Permissions Guide](./zed_acp_premissions.md)):

#### Authorization Request Flow
```rust
// From acp_thread/src/acp_thread.rs:1302
pub fn request_tool_call_authorization(
    &mut self,
    tool_call: acp::ToolCall,
    options: Vec<acp::PermissionOption>,
    cx: &mut Context<Self>,
) -> Task<Result<acp::RequestPermissionResponse>> {
    // Evaluate request against security policies
    // Present user interface for approval
    // Cache permission decisions
}
```

#### Permission Evaluation
- **Security Assessment**: Validate operation safety and scope
- **User Context**: Consider user preferences and history
- **Operation Impact**: Analyze potential system changes
- **Risk Classification**: Categorize operations by risk level

#### User Interface Integration
- **Modal Dialogs**: Rich permission request presentations
- **Operation Details**: Clear description of proposed changes
- **Approval Options**: One-time vs. persistent permissions
- **Audit Logging**: Complete decision history tracking

#### Permission Caching
- **Decision Storage**: Persistent storage of user choices
- **Pattern Matching**: Similar operation recognition
- **Expiration Handling**: Time-based permission revocation
- **Override Mechanisms**: Administrative permission management

### Error Handling and Recovery Mechanisms

Zed provides multi-layered error handling throughout the ACP stack (tracing and debugging guide: [Debugging Guide](./zed_acp_debugging.md)):

#### Transport Layer Errors
- **Connection Failures**: Automatic retry with exponential backoff
- **Protocol Violations**: Graceful degradation and error reporting
- **Timeout Handling**: Configurable timeouts for operations
- **Resource Limits**: Memory and CPU usage constraints

#### Application Layer Errors
- **Agent Crashes**: Process monitoring and automatic restart
- **Invalid Responses**: Schema validation and error recovery
- **Permission Denials**: User-friendly error messages and guidance
- **Resource Exhaustion**: Graceful handling of system limits

#### Error Classification (`agent-client-protocol/rust/error.rs`)
```rust
pub enum ErrorCode {
    // Standard JSON-RPC errors
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,

    // ACP-specific errors
    AuthRequired = 1000,
    SessionNotFound = 1001,
    ToolCallRejected = 1002,
    // ... additional codes
}
```

#### Recovery Strategies
- **Automatic Retry**: Transient failure recovery
- **Fallback Modes**: Degraded functionality on partial failures
- **User Notification**: Clear error communication and recovery options
- **Diagnostic Logging**: Comprehensive error context for debugging

### Performance Optimizations

#### Connection Pooling
- **Persistent Connections**: Reuse established agent processes
- **Load Balancing**: Distribute requests across multiple agent instances
- **Connection Health**: Monitor and recycle unhealthy connections

#### Caching and Memoization
- **Response Caching**: Avoid redundant operations
- **Metadata Storage**: Cache file and directory information
- **Permission Decisions**: Store user authorization preferences

#### Asynchronous Processing
- **Non-blocking I/O**: All operations use async/await patterns
- **Background Tasks**: Long-running operations don't block UI
- **Streaming Processing**: Incremental result delivery

#### Memory Management
- **Buffer Pooling**: Reuse memory for common operations
- **Lazy Loading**: On-demand resource allocation
- **Garbage Collection**: Automatic cleanup of unused resources

### Security Architecture

#### Sandboxing and Isolation
- **Process Isolation**: Agents run in separate processes
- **File System Restrictions**: Path validation and sandboxing
- **Network Controls**: Configurable network access policies
- **Resource Limits**: CPU, memory, and I/O constraints

#### Input Validation
- **Schema Validation**: All protocol messages validated against schema
- **Sanitization**: Input cleaning and malicious content detection
- **Type Safety**: Rust's type system prevents common vulnerabilities
- **Boundary Checking**: Array and string length limits

#### Audit and Monitoring
- **Operation Logging**: Complete audit trail of all operations
- **Performance Metrics**: Monitoring of system resource usage
- **Security Events**: Detection and reporting of suspicious activities
- **Compliance Tracking**: Regulatory requirement fulfillment

### Extensibility Framework

For extension patterns, hooks, and examples, see [Extensibility](./zed_acp_extensibility.md).

#### Custom Tool Development
- **Tool Registration**: Plugin system for custom tools
- **Capability Declaration**: Dynamic feature advertisement
- **Version Management**: Backward compatibility handling
- **Testing Framework**: Tool validation and integration testing

#### Protocol Extensions
- **Custom Methods**: Extension points for new functionality
- **Metadata Fields**: Additional context in protocol messages
- **Capability Negotiation**: Dynamic feature detection
- **Version Evolution**: Smooth protocol upgrades

#### UI Customization
- **Theme Integration**: Custom styling for ACP components
- **Layout Extensions**: Pluggable UI components
- **Behavior Customization**: Configurable interaction patterns
- **Accessibility**: Screen reader and keyboard navigation support

This comprehensive implementation provides a robust, secure, and extensible foundation for AI agent integration in Zed.
