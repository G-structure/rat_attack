# Zed ACP Debugging

## Overview

Zed provides comprehensive debugging tools and techniques for troubleshooting Agent Client Protocol (ACP) issues. The debugging infrastructure includes built-in logging, protocol message inspection, performance profiling, and specialized debugging tools. This document covers debugging approaches across the Zed, claude-code-acp, and agent-client-protocol codebases.

## Development Setup

### Local Development Environment

For effective ACP debugging, set up a local development environment:

#### Zed Development Setup
```bash
# Clone and build Zed
git clone https://github.com/zed-industries/zed.git
cd zed
cargo build --release
```

#### Agent Development Setup
```bash
# For Claude Code adapter
git clone https://github.com/zed-industries/claude-code-acp.git
cd claude-code-acp
npm install

# For Gemini CLI (if developing)
# Follow Gemini CLI development setup
```

### Debug Build Configuration

Enable debug features in Zed (`zed/crates/zed/src/main.rs`):

```rust
// Debug assertions affect ACP behavior
#[cfg(debug_assertions)]
// Additional debug logging and validation
```

## Logging Infrastructure

### Zed Logging System

Zed uses a structured logging system with multiple levels and scopes. ACP-related logging is distributed across several crates:

#### Agent Servers Logging (`zed/crates/agent_servers/src/acp.rs`)

**Process Launch Logging**:
```rust
log::trace!("Spawned (pid: {})", child.id());
```
- Logs when agent processes are successfully spawned
- Includes process ID for tracking

**Stderr Capture** (`acp.rs:91-101`):
```rust
let stderr_task = cx.background_spawn(async move {
    let mut stderr = BufReader::new(stderr);
    let mut line = String::new();
    while let Ok(n) = stderr.read_line(&mut line).await
        && n > 0
    {
        log::warn!("agent stderr: {}", &line);
        line.clear();
    }
    Ok(())
});
```
- Captures and logs all agent stderr output
- Uses `log::warn!` level for visibility
- Runs in background task to avoid blocking

#### Gemini-Specific Logging (`zed/crates/agent_servers/src/gemini.rs`)

**Version Validation** (`gemini.rs:106`):
```rust
log::error!("connected to gemini, but missing prompt_capabilities.image (version is {current_version})");
```

**Connection Diagnostics** (`gemini.rs:140-142`):
```rust
log::error!("failed to create ACP connection to gemini (version is {current_version}, supported: {supported}): {e}");
log::debug!("gemini --help stdout: {help_stdout:?}");
log::debug!("gemini --help stderr: {help_stderr:?}");
```

#### Agent2 Thread Logging (`zed/crates/agent2/src/thread.rs`)

**Detailed Operation Logging**:
```rust
log::debug!("Total messages in thread: {}", self.messages.len());
log::info!("Thread::send called with model: {}", model.name().0);
log::debug!("Thread::send content: {:?}", content);
log::debug!("Starting agent turn execution");
log::debug!("Turn execution completed");
log::error!("Turn execution failed: {:?}", error);
```

**Tool Execution Tracking**:
```rust
log::debug!("Running tool {}", tool_use.name);
log::debug!("Tool finished {:?}", tool_result);
```

### Log Level Configuration

Configure logging levels using Zed's settings:

```json
{
  "log": {
    "level": "debug",
    "filters": {
      "agent_servers": "trace",
      "acp_thread": "debug",
      "agent2": "info"
    }
  }
}
```

### Log Analysis Techniques

#### Filtering ACP Logs
```bash
# Filter for ACP-related logs
zed --log-filter "acp|agent" 2>&1 | grep -E "(acp|agent)"

# Focus on specific components
zed --log-filter "agent_servers=trace" 2>&1
```

#### Common Log Patterns
- **Process Launch**: `Spawned (pid: XXX)` - Agent process started
- **Connection Issues**: `failed to create ACP connection` - Connection problems
- **Protocol Errors**: `agent stderr:` - Agent-side errors
- **Tool Execution**: `Running tool XXX` - Tool call execution

## Protocol Debugging

### ACP Logs Tool

Zed includes a built-in ACP protocol inspector (`zed/crates/acp_tools/src/acp_tools.rs`):

#### Accessing ACP Logs
```rust
actions!(dev, [OpenAcpLogs]);
```
- Available as developer action `OpenAcpLogs`
- Opens dedicated pane showing live protocol messages

#### Message Stream Inspection (`acp_tools.rs:135-142`)
```rust
let mut receiver = connection.subscribe();
let task = cx.spawn(async move |this, cx| {
    while let Ok(message) = receiver.recv().await {
        this.update(cx, |this, cx| {
            this.push_stream_message(message, cx);
        })
        .ok();
    }
});
```

#### Message Display (`acp_tools.rs:163-200`)
The tool displays messages with:
- **Direction indicators**: Arrows showing incoming/outgoing
- **Message types**: Request, Response, Notification
- **Request IDs**: For correlating requests/responses
- **JSON payloads**: Collapsible/expandable parameter display

#### Message Structure
```rust
enum StreamMessageDirection {
    Incoming,  // From agent to Zed
    Outgoing,  // From Zed to agent
}

struct StreamMessage {
    direction: StreamMessageDirection,
    message: StreamMessageContent,
}
```

### Protocol Message Inspection

#### Request/Response Correlation
The ACP logs tool correlates requests and responses using request IDs:

```rust
// Track outgoing requests
method_map.insert(id, method.clone());

// Match incoming responses
if let Some(method) = method_map.remove(&id) {
    // Correlate with original request
}
```

#### JSON-RPC Message Format
All messages follow JSON-RPC 2.0 format:
```json
{
  "jsonrpc": "2.0",
  "id": 123,
  "method": "session/prompt",
  "params": { ... }
}
```

### Manual Protocol Debugging

#### Using curl for Testing
```bash
# Test agent directly (if it supports stdio)
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{...}}' | agent-binary
```

#### Wireshark/tcpdump for Network Agents
```bash
# Capture ACP traffic (if using network transport)
tcpdump -i lo0 port 8080 -w acp_traffic.pcap
```

## Performance Profiling

### Token Usage Monitoring

Zed tracks token usage for performance analysis (`zed/crates/acp_thread/src/acp_thread.rs`):

```rust
let warning_threshold: f32 = std::env::var("ZED_THREAD_WARNING_THRESHOLD")
    .ok()
    .and_then(|s| s.parse().ok())
    .unwrap_or(0.8);

if self.used_tokens as f32 / self.max_tokens as f32 >= warning_threshold {
    // Show token usage warning
}
```

#### Token Usage Metrics
- **Warning threshold**: 80% by default (configurable)
- **Real-time tracking**: Updates during streaming responses
- **UI indicators**: Visual warnings in thread view

### Tool Execution Profiling

Track tool execution performance (`zed/crates/agent2/src/thread.rs`):

```rust
log::debug!("Tool finished {:?}", tool_result);
```

#### Performance Metrics
- **Execution time**: Start/end timestamps
- **Success/failure rates**: Tool completion status
- **Resource usage**: Memory and CPU during execution

### Connection Performance

Monitor connection health and performance:

#### Connection Pooling
- **Reuse connections**: Avoid setup overhead for multiple sessions
- **Health checks**: Detect and recover from connection issues
- **Load balancing**: Distribute requests across agent instances

#### Latency Measurement
```rust
// Measure request/response round-trip time
let start = Instant::now();
// ... make request ...
let duration = start.elapsed();
log::debug!("ACP request took {:?}", duration);
```

## Common Issues and Solutions

### Connection Problems

#### Agent Won't Start
**Symptoms**: Connection fails immediately, "Spawned" log missing
**Causes**:
- Missing agent binary
- Incorrect PATH
- Permission issues
- Missing dependencies

**Debug Steps**:
1. Check agent installation: `which claude-code-acp` or `which gemini`
2. Verify permissions: `ls -la /path/to/agent`
3. Test manual execution: Run agent directly
4. Check Zed logs for spawn errors

#### Protocol Handshake Fails
**Symptoms**: "Unsupported version" or initialization errors
**Causes**:
- Version mismatch between Zed and agent
- Missing protocol capabilities
- Invalid capability negotiation

**Debug Steps**:
1. Check agent version: `agent --version`
2. Review capability requirements in logs
3. Compare with supported versions in code

### Authentication Failures

#### API Key Issues
**Symptoms**: "Authentication required" errors
**Causes**:
- Missing or invalid API keys
- Incorrect key format
- Expired credentials

**Debug Steps** (`zed/crates/agent_servers/src/claude.rs`):
```rust
command
    .env
    .get_or_insert_default()
    .insert("ANTHROPIC_API_KEY".to_owned(), "".to_owned());
```
- Verify API key configuration
- Check key format and validity
- Test with known working credentials

#### Claude Code Login Flow
**Symptoms**: Authentication prompts not working
**Debug Steps**:
1. Use login command: `claude /login`
2. Check for `.claude.json` file
3. Verify login status in Claude Code

### Tool Execution Errors

#### Permission Denied
**Symptoms**: Tool calls rejected despite approval
**Causes**:
- Path validation failures
- Security policy violations
- Resource access issues

**Debug Steps**:
1. Check file permissions: `ls -la /target/path`
2. Verify path is within project directory
3. Review permission logs in ACP tools

#### Tool Timeouts
**Symptoms**: Tools hang or timeout
**Causes**:
- Long-running operations
- Network issues
- Resource constraints

**Debug Steps**:
1. Monitor tool execution logs
2. Check for background process issues
3. Adjust timeout settings if needed

### Performance Issues

#### High Latency
**Symptoms**: Slow response times, UI freezing
**Causes**:
- Network latency to external services
- Large context processing
- Resource contention

**Debug Steps**:
1. Profile with ACP logs tool
2. Monitor token usage
3. Check network connectivity

#### Memory Usage
**Symptoms**: High memory consumption, crashes
**Causes**:
- Large file processing
- Memory leaks in agents
- Excessive context retention

**Debug Steps**:
1. Monitor process memory usage
2. Check for file size limits
3. Review context management

## Testing and Validation

### End-to-End Tests

Zed includes comprehensive E2E tests (`zed/crates/agent_servers/src/e2e_tests.rs`):

#### Test Categories
- **Basic functionality**: Message sending, response handling
- **Path mentions**: File reference processing
- **Tool calls**: Tool execution and permission handling
- **Cancellation**: Request interruption
- **Thread lifecycle**: Creation, usage, cleanup

#### Running Tests
```bash
# Run all agent E2E tests
cargo test -p agent_servers e2e

# Run specific agent tests
cargo test -p agent_servers gemini::tests::basic
```

#### Test Infrastructure
```rust
crate::common_e2e_tests!(async |_, _, _| Gemini, allow_option_id = "proceed_once");
```
- Shared test framework across agents
- Configurable test scenarios
- Automated validation of core functionality

### Unit Testing

Individual components have unit tests:

#### Protocol Testing (`agent-client-protocol/rust/rpc_tests.rs`)
- JSON-RPC message serialization
- Error handling validation
- Protocol compliance checks

#### Component Testing
- Tool implementation validation
- UI component testing
- Connection management tests

### Integration Testing

#### Manual Testing Setup
```bash
# Test with local agent
export ZED_AGENT_COMMAND="path/to/local/agent"
zed --dev
```

#### CI/CD Integration
- Automated test runs on PRs
- Multi-agent compatibility testing
- Performance regression detection

## Error Codes and Troubleshooting

### ACP Error Codes (`agent-client-protocol/rust/error.rs`)

#### Standard JSON-RPC Errors
- **-32700 (PARSE_ERROR)**: Invalid JSON received
- **-32600 (INVALID_REQUEST)**: Malformed request object
- **-32601 (METHOD_NOT_FOUND)**: Unknown method called
- **-32602 (INVALID_PARAMS)**: Invalid method parameters
- **-32603 (INTERNAL_ERROR)**: Server-side error

#### ACP-Specific Errors
- **-32000 (AUTH_REQUIRED)**: Authentication needed before operation

### Error Context Gathering

#### Diagnostic Information
```rust
// Include context in errors
Error::internal_error().with_data(additional_context)
```

#### Error Propagation
- Errors bubble up through the call stack
- Context preserved for debugging
- User-friendly messages generated

## Advanced Debugging Techniques

### Protocol-Level Tracing

#### Message Interception
```rust
// Hook into message stream for custom analysis
let mut receiver = connection.subscribe();
while let Ok(message) = receiver.recv().await {
    analyze_message(&message);
}
```

#### Custom Debug Tools
- Build custom protocol analyzers
- Add debug logging to specific methods
- Create specialized test harnesses

### Performance Analysis

#### Profiling Tools
```rust
// Add timing instrumentation
let start = std::time::Instant::now();
// ... operation ...
let duration = start.elapsed();
log::debug!("Operation took {:?}", duration);
```

#### Memory Profiling
- Track object allocations
- Monitor garbage collection
- Identify memory leaks

### Network Debugging

#### For Network-Based Agents
```bash
# Use netcat for simple testing
echo '{"jsonrpc":"2.0","id":1,"method":"initialize"}' | nc localhost 8080

# Wireshark capture
wireshark -i lo0 -f "tcp port 8080"
```

### Custom Agent Development

#### Debug Agent Implementation
```typescript
// In claude-code-acp
console.error("Debug: Processing request", request);
```

#### Zed-Side Debugging
```rust
// Add debug logging in Zed
log::debug!("Processing agent response: {:?}", response);
```

## Best Practices

### Debugging Workflow
1. **Reproduce the issue** in a controlled environment
2. **Enable appropriate logging** levels
3. **Use ACP logs tool** for protocol inspection
4. **Isolate components** through targeted testing
5. **Profile performance** if latency is an issue
6. **Check error codes** for specific failure modes

### Logging Guidelines
- Use appropriate log levels (trace/debug/info/warn/error)
- Include relevant context in log messages
- Avoid logging sensitive information
- Use structured logging where possible

### Testing Strategy
- Write tests for both success and failure cases
- Include integration tests for end-to-end flows
- Test with different agent implementations
- Validate error handling and recovery

### Performance Monitoring
- Set up alerts for performance regressions
- Monitor key metrics in production
- Profile regularly during development
- Optimize based on real usage patterns

This comprehensive debugging guide provides the tools and techniques needed to effectively troubleshoot ACP issues across the entire Zed ecosystem.
