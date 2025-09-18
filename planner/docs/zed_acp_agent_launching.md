# Zed ACP Agent Launching

## Overview

Zed's ACP agent launching system provides a robust framework for spawning and managing external AI agent processes. The system supports multiple agent types (Claude Code, Gemini CLI, custom agents) with sophisticated process management, environment setup, and lifecycle monitoring. This document details the technical implementation across the Zed codebase.

## Core Architecture

### Agent Server Abstraction

The agent launching system is built around the `AgentServer` trait (`zed/crates/agent_servers/src/lib.rs`):

```rust
pub trait AgentServer {
    fn telemetry_id(&self) -> &'static str;
    fn name(&self) -> SharedString;
    fn logo(&self) -> ui::IconName;
    fn connect(
        &self,
        root_dir: &Path,
        delegate: AgentServerDelegate,
        cx: &mut App,
    ) -> Task<Result<Rc<dyn AgentConnection>>>;
}
```

Each agent type implements this trait to provide its specific launching logic.

### Command Structure

All agent launches use the `AgentServerCommand` structure (`zed/crates/agent_servers/src/agent_servers.rs`):

```rust
pub struct AgentServerCommand {
    pub path: PathBuf,
    pub args: Vec<String>,
    pub env: Option<HashMap<String, String>>,
}
```

This structure encapsulates the executable path, command-line arguments, and environment variables needed to launch an agent.

## Agent-Specific Launching Implementations

### Claude Code Launching

Claude Code launching is implemented in `zed/crates/agent_servers/src/claude.rs`. The process involves:

#### Command Construction
```rust
let mut command = if let Some(settings) = settings {
    settings.command  // Use user-configured command
} else {
    // Use built-in npm package
    cx.update(|cx| {
        delegate.get_or_npm_install_builtin_agent(
            Self::BINARY_NAME.into(),  // "claude-code-acp"
            Self::PACKAGE_NAME.into(), // "@zed-industries/claude-code-acp"
            format!("node_modules/{}/dist/index.js", Self::PACKAGE_NAME).into(),
            true,  // ignore_system_version
            None,  // minimum_version
            cx,
        )
    })?
    .await?
};
```

#### Environment Setup
```rust
project_env.extend(command.env.take().unwrap_or_default());
command.env = Some(project_env);

// Ensure ANTHROPIC_API_KEY is set (even if empty)
command
    .env
    .get_or_insert_default()
    .insert("ANTHROPIC_API_KEY".to_owned(), "".to_owned());
```

#### Connection Establishment
```rust
crate::acp::connect(server_name, command.clone(), &root_dir, cx).await
```

The Claude Code adapter (`claude-code-acp/src/acp-agent.ts`) implements the ACP `Agent` interface and handles the actual agent logic.

### Gemini CLI Launching

Gemini CLI launching is implemented in `zed/crates/agent_servers/src/gemini.rs` with additional complexity for version checking and capability validation.

#### Command Construction
```rust
let mut command = if let Some(settings) = settings
    && let Some(command) = settings.custom_command()
{
    command  // Use custom command
} else {
    // Install built-in package
    cx.update(|cx| {
        delegate.get_or_npm_install_builtin_agent(
            Self::BINARY_NAME.into(),  // "gemini"
            Self::PACKAGE_NAME.into(), // "@google/gemini-cli"
            format!("node_modules/{}/dist/index.js", Self::PACKAGE_NAME).into(),
            ignore_system_version,
            Some(Self::MINIMUM_VERSION.parse().unwrap()), // "0.2.1"
            cx,
        )
    })?
    .await?
};
```

#### ACP Argument Injection
```rust
if !command.args.contains(&ACP_ARG.into()) {
    command.args.push(ACP_ARG.into());  // "--experimental-acp"
}
```

#### API Key Setup
```rust
if let Some(api_key) = cx.update(GoogleLanguageModelProvider::api_key)?.await.ok() {
    project_env.insert("GEMINI_API_KEY".to_owned(), api_key.key);
}
```

#### Version and Capability Validation
The Gemini launcher performs extensive validation:

1. **Post-Connection Validation** (`gemini.rs:94-113`):
   - Checks if `prompt_capabilities.image` is supported
   - Runs version command if validation fails
   - Returns `LoadError::Unsupported` for incompatible versions

2. **Pre-Connection Diagnostics** (`gemini.rs:115-152`):
   - Runs `--version` and `--help` commands
   - Parses version output and checks ACP support
   - Provides detailed error messages with version information

### Custom Agent Launching

Custom agents are launched through `zed/crates/agent_servers/src/custom.rs`:

```rust
impl crate::AgentServer for CustomAgentServer {
    fn connect(
        &self,
        root_dir: &Path,
        _delegate: AgentServerDelegate,
        cx: &mut App,
    ) -> Task<Result<Rc<dyn AgentConnection>>> {
        let server_name = self.name();
        let command = self.command.clone();
        let root_dir = root_dir.to_path_buf();
        cx.spawn(async move |cx| crate::acp::connect(server_name, command, &root_dir, cx).await)
    }
}
```

Custom agents use user-provided `AgentServerCommand` configurations directly.

## Core Launching Infrastructure

### ACP Connection Establishment

The core launching logic is in `zed/crates/agent_servers/src/acp.rs`:

```rust
pub async fn connect(
    server_name: SharedString,
    command: AgentServerCommand,
    root_dir: &Path,
    cx: &mut AsyncApp,
) -> Result<Rc<dyn AgentConnection>> {
    let conn = AcpConnection::stdio(server_name, command.clone(), root_dir, cx).await?;
    Ok(Rc::new(conn) as _)
}
```

### Process Spawning (`acp.rs:55-75`)

The `AcpConnection::stdio` method handles the actual process creation:

```rust
pub async fn stdio(
    server_name: SharedString,
    command: AgentServerCommand,
    root_dir: &Path,
    cx: &mut AsyncApp,
) -> Result<Self> {
    // Create child process
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

#### Process Configuration
- **Standard I/O**: All stdio streams are piped for ACP communication
- **Working Directory**: Set to the project root directory
- **Environment**: Inherits project environment plus agent-specific variables
- **Lifecycle**: `kill_on_drop(true)` ensures cleanup on process termination

### Communication Setup

After spawning, the system establishes JSON-RPC communication:

```rust
let client = ClientDelegate { sessions: sessions.clone(), cx: cx.clone() };
let (connection, io_task) = acp::ClientSideConnection::new(
    client,  // Implements Client trait
    stdin,
    stdout,
    foreground_executor.spawn(fut).detach()  // Async executor
);
```

### Background Task Management

Multiple background tasks handle different aspects:

1. **I/O Task** (`acp.rs:89`): Manages JSON-RPC message serialization/deserialization
2. **Stderr Logging** (`acp.rs:91-101`): Captures and logs agent stderr output
3. **Process Monitoring** (`acp.rs:103-118`): Waits for process exit and handles cleanup

### Protocol Initialization

After connection establishment, Zed initializes the ACP protocol:

```rust
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

## Environment and Context Management

### Project Environment Integration

All agents inherit the project's environment:

```rust
let mut project_env = project
    .update(cx, |project, cx| {
        project.directory_environment(root_dir.as_path().into(), cx)
    })?
    .await
    .unwrap_or_default();
```

This includes:
- Shell environment variables
- PATH settings
- Project-specific configurations
- User-defined environment variables

### Directory Validation

Before launching, all implementations validate the working directory:

```rust
let root_dir_exists = fs.is_dir(&root_dir).await;
anyhow::ensure!(
    root_dir_exists,
    "Session root {} does not exist or is not a directory",
    root_dir.to_string_lossy()
);
```

## Built-in Agent Management

### NPM Package Installation

For built-in agents, Zed uses `zed/crates/agent_servers/src/agent_servers.rs` for automatic installation:

```rust
pub fn get_or_npm_install_builtin_agent(
    &self,
    binary_name: SharedString,
    package_name: SharedString,
    entrypoint_path: SharedString,
    ignore_system_version: bool,
    minimum_version: Option<semver::Version>,
    cx: &mut AsyncApp,
) -> Task<Result<AgentServerCommand>>
```

#### Installation Process
1. **System Binary Check** (`agent_servers.rs:88-96`): Look for existing system installation
2. **Package Installation** (`agent_servers.rs:98-130`): Download and install npm package
3. **Version Management** (`agent_servers.rs:125-145`): Handle multiple versions and cleanup
4. **Entrypoint Resolution** (`agent_servers.rs:200-210`): Locate executable within package

#### Package Storage
Agents are installed to `paths::data_dir()/external_agents/{binary_name}/` with versioned directories.

## Error Handling and Diagnostics

### Launch Failure Handling

Different agent types handle launch failures with specific diagnostics:

#### Gemini Version Validation
- Post-launch capability checking
- Version command execution
- Detailed error messages with version information
- Fallback to help command parsing

#### Claude Code Authentication
- Login command generation for authentication
- Separate login flow handling

### Process Monitoring and Recovery

#### Crash Detection (`acp.rs:103-118`)
```rust
let wait_task = cx.spawn({
    let sessions = sessions.clone();
    async move |cx| {
        let status = child.status().await?;
        for session in sessions.borrow().values() {
            session.thread.update(cx, |thread, cx| {
                thread.emit_load_error(LoadError::Exited { status }, cx)
            }).ok();
        }
        anyhow::Ok(())
    }
});
```

#### Resource Cleanup
- Automatic process termination on drop
- Session cleanup on process exit
- Connection registry updates

## Security Considerations

### Process Isolation
- Agents run as separate processes with limited privileges
- No direct memory sharing with Zed
- Sandboxed execution environment

### Environment Sanitization
- Controlled environment variable inheritance
- API key management through secure storage
- Path validation and canonicalization

### Resource Limits
- Process spawning restrictions
- Memory and CPU monitoring
- Timeout handling for hung processes

## Configuration and Customization

### Settings Integration

Agent launching integrates with Zed's settings system (`zed/crates/agent_servers/src/settings.rs`):

```rust
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AllAgentServersSettings {
    pub claude: Option<ClaudeCodeSettings>,
    pub gemini: Option<GeminiSettings>,
    pub custom: Option<HashMap<SharedString, CustomAgentServerSettings>>,
}
```

### Custom Agent Support

Users can define custom agents through configuration:

```rust
pub struct CustomAgentServerSettings {
    pub command: AgentServerCommand,
    pub icon: Option<IconName>,
}
```

## Performance Optimization

### Connection Reuse
- Persistent connections for multiple sessions
- Connection pooling for frequently used agents
- Lazy initialization to reduce startup time

### Asynchronous Operations
- Non-blocking process spawning
- Background task execution
- Streaming I/O handling

### Caching and Memoization
- Version information caching
- Package installation state tracking
- Environment computation optimization

## Monitoring and Telemetry

### Launch Metrics
- Success/failure rates tracking
- Launch time measurement
- Agent version distribution

### Error Reporting
- Structured error logging
- Diagnostic information collection
- User-friendly error messages

### Performance Monitoring
- Process resource usage tracking
- Connection health monitoring
- Startup time analytics

## Future Extensions

### Additional Agent Types
- Support for more AI providers
- Custom protocol implementations
- Container-based agent execution

### Enhanced Security
- Process sandboxing improvements
- Network access controls
- Resource quota management

### Advanced Features
- Agent hot-swapping
- Version rollback capabilities
- Parallel agent execution

This comprehensive launching system provides robust, secure, and flexible agent management while maintaining high performance and user experience standards.
