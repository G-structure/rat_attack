# Zed ACP Extensibility

## Overview

Zed's ACP implementation provides multiple layers of extensibility, allowing developers to customize and extend agent capabilities, user interfaces, and protocol interactions. This document covers the extensibility mechanisms available across the Zed, claude-code-acp, and agent-client-protocol codebases.

## Custom Tool Development

### Tool Interface Architecture

Zed's tool system is built around the `AgentTool` trait (`zed/crates/agent2/src/thread.rs`):

```rust
pub trait AgentTool
where
    Self: 'static + Sized,
{
    type Input: for<'de> Deserialize<'de> + Serialize + JsonSchema;
    type Output: for<'de> Deserialize<'de> + Serialize + Into<LanguageModelToolResultContent>;

    fn name() -> &'static str;
    fn description(&self) -> SharedString;
    fn kind() -> acp::ToolKind;
    fn initial_title(&self, input: Result<Self::Input, serde_json::Value>) -> SharedString;
    fn input_schema(&self, format: LanguageModelToolSchemaFormat) -> Schema;
    fn supported_provider(&self, _provider: &LanguageModelProviderId) -> bool { true }
    fn run(
        self: Arc<Self>,
        input: Self::Input,
        event_stream: ToolCallEventStream,
        cx: &mut App,
    ) -> Task<Result<Self::Output>>;
    fn replay(
        &self,
        _input: Self::Input,
        _output: Self::Output,
        _event_stream: ToolCallEventStream,
        _cx: &mut App,
    ) -> Result<()> { Ok(()) }
}
```

#### Tool Registration System

Tools are registered through the `AnyAgentTool` trait and managed by the context server registry:

```rust
// From context_server_registry.rs:17-20
pub struct RegisteredContextServer {
    tools: BTreeMap<SharedString, Arc<dyn AnyAgentTool>>,
    load_tools: Task<Result<()>>,
}
```

#### Implementing Custom Tools

To create a custom tool, implement the `AgentTool` trait:

```rust
use crate::{AgentTool, AgentToolOutput, ToolCallEventStream};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MyCustomToolInput {
    pub parameter: String,
}

pub struct MyCustomTool {
    // Tool state
}

impl AgentTool for MyCustomTool {
    type Input = MyCustomToolInput;
    type Output = String; // Or a struct that implements Into<LanguageModelToolResultContent>

    fn name() -> &'static str { "my_custom_tool" }
    fn description(&self) -> SharedString { "Description of what my tool does".into() }
    fn kind() -> ToolKind { ToolKind::Other }
    fn initial_title(&self, input: Result<Self::Input, serde_json::Value>) -> SharedString {
        match input { Ok(_) => "My Custom Tool".into(), Err(_) => "My Custom Tool".into() }
    }
    fn input_schema(&self, format: LanguageModelToolSchemaFormat) -> Schema {
        crate::tool_schema::root_schema_for::<Self::Input>(format)
    }
    fn run(self: Arc<Self>, input: Self::Input, _event_stream: ToolCallEventStream, _cx: &mut App) -> Task<Result<Self::Output>> {
        Task::ready(Ok(format!("Ran with {}", input.parameter)))
    }
}
```

#### Tool Event Streaming

Tools communicate progress through the `ToolCallEventStream`:

```rust
// From agent-client-protocol/rust/tool_call.rs
pub enum ToolCallUpdate {
    Started { content: ToolCallContent },
    Progress { content: ToolCallContent },
    Completed { content: ToolCallContent },
    Error { error: String },
}
```

### Built-in Tool Examples

#### File System Tools

**ReadFileTool** (`zed/crates/agent2/src/tools/read_file_tool.rs`):

```rust
impl AgentTool for ReadFileTool {
    fn run(&self, input: serde_json::Value, event_stream: ToolCallEventStream, cx: &mut AsyncApp) -> Task<Result<AgentToolOutput>> {
        let input: ReadFileToolInput = serde_json::from_value(input)?;
        // Implementation uses project.read_buffer to access files
    }
}
```

**EditFileTool** (`zed/crates/agent2/src/tools/edit_file_tool.rs`):

```rust
impl AgentTool for EditFileTool {
    fn run(&self, input: serde_json::Value, event_stream: ToolCallEventStream, cx: &mut AsyncApp) -> Task<Result<AgentToolOutput>> {
        // Uses EditAgent for intelligent code modifications
        // Supports diff-based editing with conflict resolution
    }
}
```

#### Terminal Tools

**TerminalTool** (`zed/crates/agent2/src/tools/terminal_tool.rs`):

```rust
impl AgentTool for TerminalTool {
    fn run(&self, input: serde_json::Value, event_stream: ToolCallEventStream, cx: &mut AsyncApp) -> Task<Result<AgentToolOutput>> {
        // Spawns processes using util::command
        // Streams output in real-time
        // Handles process lifecycle management
    }
}
```

#### Development Tools

**DiagnosticsTool** (`zed/crates/agent2/src/tools/diagnostics_tool.rs`):

```rust
impl AgentTool for DiagnosticsTool {
    fn run(&self, input: serde_json::Value, event_stream: ToolCallEventStream, cx: &mut AsyncApp) -> Task<Result<AgentToolOutput>> {
        // Accesses LSP diagnostics through project.lsp_store
        // Provides code analysis and error reporting
    }
}
```

### Tool Registration and Discovery

Tools are registered through the context server system:

```rust
// From context_server_registry.rs:66-84
let registered_server = self.registered_servers
    .entry(server_id)
    .or_insert_with(|| RegisteredContextServer {
        tools: BTreeMap::new(),
        load_tools: Task::ready(Ok(())),
    });

registered_server.load_tools = cx.spawn(async move |this, cx| {
    let tools = server.list_tools().await?;
    for tool in tools {
        registered_server.tools.insert(tool.name(), tool);
    }
    Ok(())
});
```

## UI Customization

### Component Extension Points

Zed's ACP UI is built with GPUI and provides several extension points:

#### Thread View Customization

The `AcpThreadView` (`zed/crates/agent_ui/src/acp/thread_view.rs`) supports:

- **Message Rendering**: Custom renderers for different content types
- **Tool Call Visualization**: Pluggable UI for tool execution displays
- **Status Indicators**: Customizable progress and error displays

#### Message Editor Extensions

The message editor supports:
- **@-Mention Providers**: Custom mention completion
- **Content Blocks**: Support for images, files, and custom content
- **Template System**: Extensible prompt templates

### Theme Integration

ACP UI components integrate with Zed's theming system:

```rust
// From thread_view.rs
let theme_settings = ThemeSettings::get_global(cx);
let text_style = window.text_style();
let colors = cx.theme().colors();
```

#### Custom Styling

Components can be styled using GPUI's styling system:

```rust
v_flex()
    .bg(cx.theme().colors().editor_background)
    .border_color(cx.theme().colors().border)
    .text_size(base_size)
    .font_buffer(cx)
```

### Event Handling Extensions

UI components support custom event handlers:

```rust
// From thread_view.rs:2665-2680
.on_click(cx.listener(move |this, _, _, cx| {
    if this.expanded.contains(&index) {
        this.expanded.remove(&index);
    } else {
        this.expanded.insert(index);
        // Custom expansion logic
    }
    cx.notify()
}))
```

## Protocol Extensions

### ACP Protocol Extensibility

The ACP protocol supports extensions through the `ext` module (`agent-client-protocol/rust/ext.rs`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct ExtRequest {
    #[serde(skip)]
    pub method: Arc<str>,
    pub params: Arc<RawValue>,
}
```

#### Extension Methods

Agents and clients can define custom methods:

```rust
// Agent trait includes extension support
fn ext_method(&self, args: ExtRequest) -> impl Future<Output = Result<ExtResponse, Error>>;
fn ext_notification(&self, args: ExtNotification) -> impl Future<Output = Result<(), Error>>;
```

#### Extension Notifications

One-way extension notifications:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct ExtNotification {
    #[serde(skip)]
    pub method: Arc<str>,
    pub params: Arc<RawValue>,
}
```

### Capability Negotiation

The protocol supports dynamic capability advertisement:

```rust
// From agent.rs:148-167
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InitializeRequest {
    pub protocol_version: ProtocolVersion,
    pub client_capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClientCapabilities {
    pub fs: FileSystemCapability,
    pub terminal: bool,
    #[serde(flatten)]
    pub _meta: HashMap<String, serde_json::Value>,  // Extension point
}
```

#### Custom Capabilities

Extensions can define custom capabilities:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClientCapabilities {
    pub fs: FileSystemCapability,
    pub terminal: bool,
    #[serde(flatten)]
    pub _meta: HashMap<String, serde_json::Value>,  // Custom capabilities
}
```

### Claude Code Adapter Extensions

The Claude Code adapter (`claude-code-acp/src/acp-agent.ts`) demonstrates protocol extensions:

```typescript
async initialize(request: InitializeRequest): Promise<InitializeResponse> {
    this.clientCapabilities = request.clientCapabilities;
    return {
        protocolVersion: 1,
        agentCapabilities: {
            promptCapabilities: {
                image: true,
                embeddedContext: true,
            },
            // Custom capabilities can be added here
        },
        authMethods: [...],
    };
}
```

## Integration Patterns

### Third-party Agents

#### Custom Agent Server Implementation

To integrate a third-party agent, implement the `AgentServer` trait:

```rust
// From custom.rs:20-43
impl crate::AgentServer for CustomAgentServer {
    fn telemetry_id(&self) -> &'static str {
        "custom"
    }

    fn name(&self) -> SharedString {
        self.name.clone()
    }

    fn logo(&self) -> IconName {
        IconName::Terminal
    }

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

#### Agent Server Registration

Custom agents are registered through Zed's settings system:

```rust
// From settings.rs:73-78
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CustomAgentServerSettings {
    pub command: AgentServerCommand,
    pub icon: Option<IconName>,
}
```

#### Configuration Example

```json
{
  "agent": {
    "custom": {
      "my-agent": {
        "command": {
          "path": "/path/to/agent",
          "args": ["--acp"],
          "env": {
            "API_KEY": "secret"
          }
        }
      }
    }
  }
}
```

### Custom MCP Servers

#### MCP Server Integration

Zed integrates MCP servers through the context server system:

```rust
// From context_server_registry.rs:22-33
impl ContextServerRegistry {
    pub fn new(server_store: Entity<ContextServerStore>, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            server_store: server_store.clone(),
            registered_servers: HashMap::default(),
            _subscription: cx.subscribe(&server_store, Self::handle_context_server_store_event),
        };
        for server in server_store.read(cx).running_servers() {
            this.reload_tools_for_server(server.id(), cx);
        }
        this
    }
}
```

#### Tool Loading from MCP Servers

```rust
// From context_server_registry.rs:48-84
fn reload_tools_for_server(&mut self, server_id: ContextServerId, cx: &mut Context<Self>) {
    let Some(server) = self.server_store.read(cx).get_running_server(&server_id) else {
        return;
    };

    let registered_server = self.registered_servers
        .entry(server_id)
        .or_insert_with(|| RegisteredContextServer {
            tools: BTreeMap::new(),
            load_tools: Task::ready(Ok(())),
        });

    registered_server.load_tools = cx.spawn(async move |this, cx| {
        let tools = server.list_tools().await?;
        for tool in tools {
            registered_server.tools.insert(tool.name(), tool);
        }
        Ok(())
    });
}
```

#### MCP Server Configuration

MCP servers are configured through Zed's settings:

```json
{
  "context_servers": {
    "my-mcp-server": {
      "command": {
        "path": "npx",
        "args": ["-y", "@modelcontextprotocol/server-everything"]
      }
    }
  }
}
```

## Advanced Extensibility

### Plugin Architecture

Zed supports extensions through its extension system, which can include ACP-related functionality:

#### Extension Points
- **Language Extensions**: Custom language support with ACP integration
- **Theme Extensions**: Custom styling for ACP UI components
- **Command Extensions**: Custom commands that interact with ACP

#### Extension Development

Extensions can register custom agent servers:

```rust
// Extension can provide custom AgentServer implementations
cx.update(|cx| {
    agent_servers::register_custom_server(
        "my-extension-agent",
        MyCustomAgentServer::new(),
        cx
    );
});
```

### Protocol Evolution

#### Version Negotiation

ACP supports protocol version negotiation:

```rust
// From acp.rs:51-53
const MINIMUM_SUPPORTED_VERSION: acp::ProtocolVersion = acp::V1;

// Version checking during initialization
if response.protocol_version < MINIMUM_SUPPORTED_VERSION {
    return Err(UnsupportedVersion.into());
}
```

#### Backward Compatibility

Extensions should maintain backward compatibility:

```rust
// Use _meta fields for extensions
#[derive(Serialize, Deserialize)]
pub struct MyExtendedCapabilities {
    pub standard_field: bool,
    #[serde(flatten)]
    pub _meta: HashMap<String, serde_json::Value>,
}
```

### Testing Extensions

#### Unit Testing Custom Tools

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    async fn test_my_custom_tool(cx: &mut TestAppContext) {
        let tool = MyCustomTool::new();
        let input = serde_json::json!({
            "parameter": "test_value"
        });

        let result = tool.run(input, ToolCallEventStream::test().0, cx).await;
        assert!(result.is_ok());
    }
}
```

#### Integration Testing

```rust
#[gpui::test]
async fn test_custom_agent_integration(cx: &mut TestAppContext) {
    let server = CustomAgentServer::new("test-agent".into(), test_command());
    let connection = server.connect(Path::new("/tmp"), delegate, cx).await;
    assert!(connection.is_ok());
}
```

## Best Practices

### Tool Development

#### Error Handling
- Provide clear, actionable error messages
- Handle edge cases gracefully
- Use appropriate error types from `anyhow`

#### Performance Considerations
- Avoid blocking operations in tool execution
- Use streaming for long-running operations
- Implement proper resource cleanup

#### Security
- Validate all inputs thoroughly
- Respect file system boundaries
- Implement proper permission checks

### UI Extensions

#### Responsive Design
- Support different screen sizes and layouts
- Follow Zed's design system
- Handle theme changes gracefully

#### Accessibility
- Support keyboard navigation
- Provide screen reader compatibility
- Follow WCAG guidelines

### Protocol Extensions

#### Naming Conventions
- Use reverse domain notation for custom methods: `com.example.myMethod`
- Document extension points clearly
- Maintain backward compatibility

#### Capability Declaration
- Clearly document required capabilities
- Provide fallbacks for missing capabilities
- Handle capability negotiation failures

### Maintenance

#### Version Management
- Track compatibility across Zed versions
- Provide migration guides for breaking changes
- Test against multiple Zed versions

#### Documentation
- Maintain comprehensive documentation
- Provide examples and tutorials
- Keep API documentation up-to-date

This extensibility framework allows developers to customize and extend Zed's ACP capabilities while maintaining compatibility with the core protocol and ecosystem.
