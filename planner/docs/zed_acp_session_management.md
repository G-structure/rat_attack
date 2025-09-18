# Zed ACP Session Management

## Overview

Zed's ACP session management provides sophisticated lifecycle handling, concurrency control, and persistence for AI agent conversations. Sessions are managed through a layered architecture combining the internal `Thread` entity, the ACP protocol `AcpThread`, and database persistence via `ThreadsDatabase`.

**Key Implementation Locations:**
- Session coordination: `zed/crates/agent2/src/agent.rs`
- Thread entity: `zed/crates/agent2/src/thread.rs`
- ACP thread: `zed/crates/acp_thread/src/acp_thread.rs`
- Database persistence: `zed/crates/agent2/src/db.rs`
- History store: `zed/crates/agent2/src/history_store.rs`

## Session Lifecycle

### Creation

**Thread Creation Flow:**
1. **Agent Initialization**: `NativeAgent::new()` creates the agent with session registry
2. **Thread Entity**: `Thread::new()` instantiates the internal thread with unique `SessionId`
3. **ACP Thread**: `AcpThread::new()` creates the protocol wrapper with connection binding
4. **Session Registration**: `NativeAgent::register_session()` links thread and ACP thread entities

**Key Components:**
```rust
pub struct Thread {
    id: acp::SessionId,
    messages: Vec<Message>,
    running_turn: Option<RunningTurn>,
    model: Option<Arc<dyn LanguageModel>>,
    // ... state management fields
}

pub struct AcpThread {
    session_id: acp::SessionId,
    entries: Vec<AgentThreadEntry>,
    connection: Rc<dyn AgentConnection>,
    // ... UI state fields
}
```

**Session Linking:**
```rust
struct Session {
    thread: Entity<Thread>,
    acp_thread: WeakEntity<AcpThread>,
    pending_save: Task<()>,
}
```

### Termination

**Graceful Shutdown:**
- **Active Turn Cancellation**: `Thread::cancel()` interrupts running agent interactions
- **Resource Cleanup**: Automatic cleanup of terminals, tool processes, and subscriptions
- **State Persistence**: Final state saved to database before termination
- **Entity Disposal**: Weak references ensure memory cleanup

**Termination Triggers:**
- **User Action**: Explicit thread closure via UI
- **Agent Failure**: Connection errors or protocol violations
- **Resource Limits**: Token usage limits or tool execution quotas
- **Application Exit**: Graceful shutdown with state preservation

## Concurrency

### Multiple Sessions

**Session Isolation:**
- Each session maintains independent state and execution context
- Separate `SessionId` ensures message routing and state consistency
- Isolated tool execution environments prevent cross-session interference
- Independent model connections allow parallel processing

**Concurrent Execution:**
- Multiple threads can run simultaneously across different projects
- Background task management prevents UI blocking
- Shared resource pools (language models, context servers) with proper synchronization
- Event-driven updates maintain UI responsiveness

### Resource Management

**Memory Management:**
- **Entity References**: Weak entity references prevent memory leaks
- **Shared Buffers**: `shared_buffers` in `AcpThread` cache file snapshots
- **Task Cleanup**: Automatic cancellation of background tasks on session termination
- **Subscription Management**: Event subscription cleanup on entity disposal

**Resource Limits:**
- **Token Usage Tracking**: Per-session and cumulative token monitoring
- **Tool Execution Limits**: Configurable tool call quotas
- **Terminal Pooling**: Reused terminal instances with cleanup
- **Database Connection Pooling**: Shared SQLite connections

## Persistence

### State Saving

**Database Schema:**
```rust
pub struct Thread {
    id: acp::SessionId,
    title: Option<SharedString>,
    summary: Option<SharedString>,
    messages: Vec<Message>,
    // ... metadata fields
}
```

**Save Triggers:**
- **Message Events**: New user/assistant messages trigger incremental saves
- **Title Updates**: Thread title changes persist immediately
- **Tool Results**: Tool call completion updates state
- **Periodic Sync**: Background task ensures state consistency

**Serialization:**
- **Message History**: Full conversation history with content blocks
- **Tool States**: Tool call results and execution metadata
- **Model Context**: Selected model and configuration
- **Project Context**: File references and worktree state

### Resume Capabilities

**State Restoration:**
- **Database Loading**: `ThreadsDatabase::load_thread()` reconstructs thread state
- **Entity Recreation**: `Thread::from_db()` rebuilds internal thread entity
- **ACP Thread Replay**: `Thread::replay()` streams historical events to UI
- **Context Reconstruction**: Project files and worktree state restoration

**Resume Process:**
```rust
pub fn open_thread(&mut self, id: acp::SessionId, cx: &mut Context<Self>) -> Task<Result<Entity<AcpThread>>> {
    // Load from database
    let db_thread = database.load_thread(id.clone())?;
    // Reconstruct thread entity
    let thread = Thread::from_db(id, db_thread, ...);
    // Register session
    let acp_thread = self.register_session(thread, cx);
    // Replay events to UI
    let events = thread.replay(cx);
    // Process historical events
    NativeAgentConnection::handle_thread_events(events, acp_thread.downgrade(), cx)
}
```

**Incremental Resume:**
- **Checkpoint Support**: Git checkpoints enable partial rewind
- **Message Truncation**: `Thread::truncate()` removes messages after specific point
- **State Consistency**: Token usage and tool states maintained across resume

## Synchronization

### UI Updates

**Event-Driven Architecture:**
- **Thread Events**: `ThreadEvent` enum for agent interaction updates
- **ACP Events**: `AcpThreadEvent` for UI state changes
- **Subscription Model**: Entity subscriptions propagate changes to UI components
- **Async Processing**: Background tasks prevent UI blocking

**Event Flow:**
```
Agent Response → ThreadEvent → NativeAgentConnection::handle_thread_events()
    ↓
AcpThread Updates → AcpThreadEvent → UI Components
    ↓
Panel Rendering → User Interface Updates
```

**Real-time Streaming:**
- **Message Chunks**: Incremental text updates during generation
- **Tool Progress**: Live tool execution status updates
- **Token Usage**: Real-time usage statistics
- **Plan Updates**: Dynamic plan progression visualization

### State Consistency

**Synchronization Mechanisms:**
- **Entity System**: GPUI's entity system ensures thread-safe state access
- **Watch Channels**: `watch::Receiver/Sender` for capability updates
- **Mutex Protection**: Database connections protected with `Arc<Mutex<>>`
- **Task Coordination**: `Shared<Task<>>` for coordinated async operations

**Consistency Guarantees:**
- **Atomic Updates**: State changes applied atomically
- **Version Tracking**: Message IDs and timestamps prevent conflicts
- **Error Recovery**: Failed operations don't corrupt session state
- **Rollback Support**: Checkpoint-based rewind capabilities

**Cross-Session Coordination:**
- **Shared Resources**: Language model registry shared across sessions
- **Project Context**: Worktree changes propagated to all active sessions
- **Settings Updates**: Global settings changes applied to running sessions
- **History Store**: Centralized conversation history management

## Error Handling and Recovery

### Session Recovery

**Failure Modes:**
- **Connection Loss**: Agent server disconnection with automatic retry
- **Model Errors**: Language model failures with fallback options
- **Tool Failures**: Tool execution errors with user notification
- **Database Corruption**: Recovery from corrupted persistence state

**Recovery Strategies:**
- **Automatic Retry**: Transient failures retried with exponential backoff
- **State Preservation**: Session state maintained across failures
- **Graceful Degradation**: Reduced functionality when services unavailable
- **User Notification**: Clear error messages with recovery options

### Resource Cleanup

**Cleanup Lifecycle:**
- **Terminal Disposal**: Active terminals terminated on session end
- **Task Cancellation**: Background tasks cancelled and awaited
- **Memory Deallocation**: Entity references cleaned up automatically
- **Database Transactions**: Proper transaction handling prevents corruption

## Performance Optimizations

### Session Efficiency

**Lazy Loading:**
- **Message History**: Large conversations loaded on-demand
- **Tool Results**: Tool outputs cached and streamed incrementally
- **Project Context**: Worktree information loaded asynchronously

**Background Processing:**
- **Save Operations**: Non-blocking database writes
- **Model Requests**: Async language model interactions
- **File Operations**: Background file I/O for context loading

### Scalability

**Resource Pooling:**
- **Model Connections**: Reused model instances across sessions
- **Terminal Pooling**: Shared terminal resources
- **Database Connections**: Connection pooling for concurrent access

**Memory Optimization:**
- **Weak References**: Prevent circular references and memory leaks
- **Shared Buffers**: Cached file snapshots reduce I/O
- **Incremental Updates**: Only changed state persisted

This comprehensive session management system enables robust, scalable AI agent interactions while maintaining data integrity, performance, and user experience consistency.