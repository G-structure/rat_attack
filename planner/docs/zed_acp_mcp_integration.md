# Zed ACP MCP Integration

## Overview

Zed integrates Model Context Protocol (MCP) servers with Agent Client Protocol (ACP) agents through a sophisticated proxy architecture. This allows ACP agents to leverage tools and resources from MCP servers while maintaining security boundaries and providing seamless user experience.

**Key Implementation Components:**
- **Context Server Store**: `zed/crates/project/src/context_server_store.rs` - Manages MCP server lifecycle
- **Protocol Layer**: `zed/crates/context_server/src/protocol.rs` - MCP protocol implementation
- **Tool Integration**: `zed/crates/assistant_tool/src/tool_working_set.rs` - Tool delegation and routing
- **Configuration**: `zed/crates/project/src/project_settings.rs` - Server configuration management

## MCP Server Discovery and Configuration

### Configuration Sources

MCP servers can be configured through multiple mechanisms:

**1. Project Settings (`settings.json`)**
```json
{
  "context_servers": {
    "my-mcp-server": {
      "source": "custom",
      "enabled": true,
      "command": {
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
      }
    }
  }
}
```

**2. Extension-Provided Servers**
Extensions can declare MCP servers in their `extension.json`:
```json
{
  "context_servers": {
    "database-tools": {
      "description": "Database query and schema tools",
      "command": "db-mcp-server"
    }
  }
}
```

**3. VS Code Compatibility**
Zed automatically imports MCP server configurations from VS Code's `mcp` settings section.

### Server Types

**Custom Servers** (`ContextServerSettings::Custom`):
- Executable command with arguments
- Environment variables
- Working directory specification

**Extension Servers** (`ContextServerSettings::Extension`):
- Defined by Zed extensions
- Version-managed and sandboxed
- Automatic updates through extension system

### Automatic Detection

**Extension Scanning**:
- Extensions declare MCP servers in their manifests
- Automatic registration during extension loading
- Version compatibility checking

**VS Code Migration**:
- Automatic import of VS Code MCP configurations
- Settings transformation and validation
- User notification of migrated servers

## Proxy Implementation Architecture

### Communication Flow

```
ACP Agent → Zed Proxy → MCP Server
     ↑             ↓
     ←───── Response ──────
```

**Request Flow**:
1. **ACP Agent** sends tool call request to Zed
2. **Tool Router** identifies MCP-sourced tool
3. **Context Server Store** locates appropriate MCP server
4. **Protocol Layer** translates ACP request to MCP format
5. **Transport Layer** sends request to MCP server process
6. **Response Translation** converts MCP response back to ACP format
7. **Agent** receives tool execution result

### Protocol Translation

**ACP to MCP Mapping**:
```rust
// ACP Tool Call
pub struct ToolCall {
    pub id: ToolCallId,
    pub name: String,
    pub arguments: Value,
}

// Maps to MCP Tool Call
pub struct CallToolRequest {
    pub method: "tools/call",
    pub params: CallToolRequestParams {
        pub name: String,
        pub arguments: Value,
    },
}
```

**Response Translation**:
- MCP tool results → ACP tool content blocks
- Error handling and status code mapping
- Content type preservation (text, images, etc.)

### Transport Layer

**Supported Transports**:
- **Stdio Transport**: Direct process communication via stdin/stdout
- **HTTP Transport**: REST API communication (planned)
- **WebSocket Transport**: Bidirectional streaming (planned)

**Stdio Implementation** (`zed/crates/context_server/src/transport/stdio_transport.rs`):
```rust
pub struct StdioTransport {
    command: ContextServerCommand,
    child_process: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
}
```

### Security Considerations

**Process Isolation**:
- Each MCP server runs as separate OS process
- No direct memory sharing with Zed
- Resource limits and timeout enforcement

**Permission Scoping**:
- MCP servers declare required capabilities
- User authorization for sensitive operations
- Tool-level permission checking

**Network Security**:
- HTTP-based servers require explicit user consent
- Certificate validation for HTTPS connections
- No automatic external server connections

## Server Management Lifecycle

### Server States

```rust
pub enum ContextServerStatus {
    Starting,    // Process launching
    Running,     // Active and responding
    Stopped,     // Gracefully shut down
    Error(Arc<str>), // Failed with error message
}
```

### Lifecycle Management

**Initialization Sequence**:
1. **Configuration Loading**: Read server settings from project configuration
2. **Process Launch**: Start MCP server process with configured command
3. **Protocol Handshake**: Exchange initialization messages
4. **Capability Discovery**: Query server capabilities
5. **Tool Registration**: Register discovered tools with tool working set

**Runtime Monitoring**:
- Health checks via MCP ping messages
- Automatic restart on process failure
- Resource usage monitoring
- Timeout handling for unresponsive servers

**Shutdown Handling**:
- Graceful termination signals
- Process cleanup and resource deallocation
- State persistence for restart scenarios

### Error Handling

**Process-Level Errors**:
- Exit code monitoring and logging
- Stderr capture for diagnostic information
- Automatic restart with exponential backoff

**Protocol-Level Errors**:
- JSON-RPC error code translation
- Timeout handling with configurable limits
- Connection recovery mechanisms

**Configuration Errors**:
- Settings validation at load time
- Migration handling for outdated configurations
- User-friendly error messages with recovery suggestions

## Tool Delegation and Routing

### Tool Source Identification

**Tool Registration**:
```rust
pub enum ToolSource {
    Native,                    // Built-in Zed tools
    ContextServer { id: String }, // MCP server tools
}
```

**Tool Discovery**:
- MCP servers advertise available tools during initialization
- Tool metadata includes name, description, and parameter schema
- Automatic registration with global tool registry

### Request Routing

**Tool Resolution**:
1. **Name Lookup**: Match tool name against registered tools
2. **Source Identification**: Determine if tool is native or MCP-sourced
3. **Server Selection**: Route to appropriate MCP server instance
4. **Parameter Validation**: Validate arguments against tool schema

**Load Balancing**:
- Multiple servers can provide same tool
- Round-robin distribution for high availability
- Fallback mechanisms for failed servers

### Response Handling

**Content Translation**:
- MCP tool responses contain content blocks
- Translation to ACP-compatible format
- Preservation of rich content types (text, images, files)

**Error Propagation**:
- MCP error codes mapped to ACP error types
- Detailed error messages with context
- User-friendly error presentation

## Advanced Features

### Tool Caching and Optimization

**Response Caching**:
- Configurable caching for deterministic tool calls
- Cache invalidation on server restarts
- Memory-bounded cache with LRU eviction

**Batch Processing**:
- Multiple tool calls batched to single server request
- Parallel execution across multiple servers
- Result aggregation and ordering

### Monitoring and Observability

**Metrics Collection**:
- Tool call latency and success rates
- Server health and uptime statistics
- Resource usage tracking per server

**Logging Integration**:
- Structured logging of MCP protocol messages
- Debug interfaces for protocol inspection
- Performance profiling capabilities

### Extension Ecosystem

**Extension APIs**:
- Extensions can define MCP servers
- Declarative server configuration
- Automatic dependency management

**Version Compatibility**:
- MCP protocol version negotiation
- Backward compatibility handling
- Feature detection and graceful degradation

## Configuration Examples

### File System Server
```json
{
  "context_servers": {
    "filesystem": {
      "source": "custom",
      "enabled": true,
      "command": {
        "command": "npx",
        "args": ["@modelcontextprotocol/server-filesystem", "/allowed/path"]
      }
    }
  }
}
```

### Database Tools Server
```json
{
  "context_servers": {
    "database": {
      "source": "extension",
      "enabled": true,
      "settings": {
        "connection_string": "postgresql://localhost/mydb"
      }
    }
  }
}
```

### Git Repository Server
```json
{
  "context_servers": {
    "git-tools": {
      "source": "custom",
      "enabled": true,
      "command": {
        "command": "git-mcp-server",
        "args": ["--repo", "."],
        "env": {
          "GIT_AUTHOR_NAME": "Zed Agent",
          "GIT_AUTHOR_EMAIL": "agent@zed.dev"
        }
      }
    }
  }
}
```

## Troubleshooting and Debugging

### Common Issues

**Server Startup Failures**:
- Check command path and permissions
- Verify environment variables
- Review stderr output in logs

**Tool Call Failures**:
- Validate tool parameter schemas
- Check server capability declarations
- Monitor network connectivity for HTTP servers

**Performance Issues**:
- Profile server response times
- Check resource limits and allocations
- Monitor for memory leaks in server processes

### Debug Tools

**Protocol Logging**:
- Enable MCP message logging via debug settings
- Inspect JSON-RPC message flows
- Analyze protocol compliance

**Server Health Checks**:
- Built-in ping/pong mechanisms
- Server capability introspection
- Connection pool monitoring

This comprehensive MCP integration enables ACP agents to leverage a rich ecosystem of tools while maintaining Zed's security, performance, and user experience standards.