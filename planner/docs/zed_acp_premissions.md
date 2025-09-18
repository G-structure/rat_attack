# Zed ACP Tool Call Permissions: Comprehensive Implementation Guide

This guide provides an exhaustive analysis of Zed's Agent Client Protocol (ACP) tool call permission system, including detailed code flows, data structures, UI components, and integration points.

## Overview

Zed's ACP permission system implements a sophisticated user-controlled mechanism for managing external agent tool calls. The system provides granular control over agent actions while maintaining usability through configurable bypass settings and clear user interfaces.

## Core Architecture

### 1. Protocol Foundation

**Agent Client Protocol (ACP)**: External crate `agent-client-protocol` (see `agent-client-protocol/Cargo.toml` for the current version)
- Defines standardized communication between Zed and external agents (Claude Code, Gemini CLI)
- Handles tool call requests, responses, and permission management
- Uses JSON-RPC 2.0-style message passing over stdio

### 2. Key Components

#### Permission Request Flow Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   External      │    │   ACP Thread     │    │   UI Layer      │
│   Agent         │────│   (acp_thread)   │────│   (agent_ui)    │
│                 │    │                  │    │                 │
│ • Tool Call     │    │ • Permission     │    │ • Permission    │
│   Request       │    │   Evaluation     │    │   Dialog        │
│                 │    │                  │    │                 │
│ • Wait for      │    │ • Settings Check │    │ • User Choice   │
│   Permission    │    │                  │    │                 │
│                 │    │ • Dialog Display │    │ • Response      │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

## Detailed Implementation Analysis

### 1. ACP Protocol Integration

#### Client-Side Connection (`zed/crates/agent_servers/src/acp.rs`)

The ACP connection establishes the communication channel with external agents:

```rust
impl acp::Client for ClientDelegate {
    async fn request_permission(
        &self,
        arguments: acp::RequestPermissionRequest,
    ) -> Result<acp::RequestPermissionResponse, acp::Error> {
        let cx = &mut self.cx.clone();

        let task = self
            .session_thread(&arguments.session_id)?
            .update(cx, |thread, cx| {
                thread.request_tool_call_authorization(arguments.tool_call, arguments.options, cx)
            })??;

        let outcome = task.await;

        Ok(acp::RequestPermissionResponse { outcome })
    }
}
```

**Key Protocol Messages**:
- `RequestPermissionRequest`: Contains tool call details and permission options
- `RequestPermissionResponse`: Returns user's permission decision
- `RequestPermissionOutcome`: Either `Selected { option_id }` or `Cancelled`

### 2. Permission Evaluation and Request

#### Core Permission Logic (`zed/crates/acp_thread/src/acp_thread.rs`)

The main permission evaluation happens in the ACP thread:

```rust
// zed/crates/acp_thread/src/acp_thread.rs
pub fn request_tool_call_authorization(
    &mut self,
    tool_call: acp::ToolCallUpdate,
    options: Vec<acp::PermissionOption>,
    cx: &mut Context<Self>,
) -> Result<BoxFuture<'static, acp::RequestPermissionOutcome>> {
    let (tx, rx) = oneshot::channel();

    // Global bypass check
    if AgentSettings::get_global(cx).always_allow_tool_actions {
        // Auto-select AllowOnce option
        if let Some(allow_once_option) = options.iter().find_map(|option| {
            if matches!(option.kind, acp::PermissionOptionKind::AllowOnce) {
                Some(option.id.clone())
            } else {
                None
            }
        }) {
            self.upsert_tool_call_inner(tool_call, ToolCallStatus::Pending, cx)?;
            return Ok(async {
                acp::RequestPermissionOutcome::Selected {
                    option_id: allow_once_option,
                }
            }
            .boxed());
        }
    }

    // Create waiting state with permission options
    let status = ToolCallStatus::WaitingForConfirmation {
        options,
        respond_tx: tx,
    };

    self.upsert_tool_call_inner(tool_call, status, cx)?;
    cx.emit(AcpThreadEvent::ToolAuthorizationRequired);

    // Return future that resolves when user responds
    let fut = async {
        match rx.await {
            Ok(option) => acp::RequestPermissionOutcome::Selected { option_id: option },
            Err(oneshot::Canceled) => acp::RequestPermissionOutcome::Cancelled,
        }
    }
    .boxed();

    Ok(fut)
}
```

#### Tool Call Status State Machine

```rust
// crates/acp_thread/src/acp_thread.rs:377-396
#[derive(Debug)]
pub enum ToolCallStatus {
    /// Tool call pending execution
    Pending,
    /// Waiting for user permission decision
    WaitingForConfirmation {
        options: Vec<acp::PermissionOption>,
        respond_tx: oneshot::Sender<acp::PermissionOptionId>,
    },
    /// Tool call actively executing
    InProgress,
    /// Tool call completed successfully
    Completed,
    /// Tool call failed
    Failed,
    /// User rejected the tool call
    Rejected,
    /// Tool call canceled (e.g., generation stopped)
    Canceled,
}
```

### 3. User Authorization Handler

#### Permission Decision Processing

```rust
// zed/crates/acp_thread/src/acp_thread.rs
pub fn authorize_tool_call(
    &mut self,
    id: acp::ToolCallId,
    option_id: acp::PermissionOptionId,
    option_kind: acp::PermissionOptionKind,
    cx: &mut Context<Self>,
) {
    let Some((ix, call)) = self.tool_call_mut(&id) else {
        return;
    };

    // Update status based on user choice
    let new_status = match option_kind {
        acp::PermissionOptionKind::RejectOnce | acp::PermissionOptionKind::RejectAlways => {
            ToolCallStatus::Rejected
        }
        acp::PermissionOptionKind::AllowOnce | acp::PermissionOptionKind::AllowAlways => {
            ToolCallStatus::InProgress
        }
    };

    let curr_status = mem::replace(&mut call.status, new_status);

    // Send response back to waiting agent
    if let ToolCallStatus::WaitingForConfirmation { respond_tx, .. } = curr_status {
        respond_tx.send(option_id).log_err();
    } else if cfg!(debug_assertions) {
        panic!("tried to authorize an already authorized tool call");
    }

    cx.emit(AcpThreadEvent::EntryUpdated(ix));
}
```

### 4. UI Components and Rendering

#### Permission Dialog Rendering (`zed/crates/agent_ui/src/acp/thread_view.rs`)

The UI renders permission buttons based on the tool call status:

```rust
// zed/crates/agent_ui/src/acp/thread_view.rs
fn render_permission_buttons(
    &self,
    options: &[acp::PermissionOption],
    entry_ix: usize,
    tool_call_id: acp::ToolCallId,
    cx: &Context<Self>,
) -> Div {
    h_flex()
        .py_1()
        .pl_2()
        .pr_1()
        .gap_1()
        .justify_between()
        .flex_wrap()
        .border_t_1()
        .border_color(self.tool_card_border_color(cx))
        .child(
            div()
                .min_w(rems_from_px(145.))
                .child(LoadingLabel::new("Waiting for Confirmation").size(LabelSize::Small)),
        )
        .child(h_flex().gap_0p5().children(options.iter().map(|option| {
            let option_id = SharedString::from(option.id.0.clone());
            Button::new((option_id, entry_ix), option.name.clone())
                .map(|this| match option.kind {
                    acp::PermissionOptionKind::AllowOnce => {
                        this.icon(IconName::Check).icon_color(Color::Success)
                    }
                    acp::PermissionOptionKind::AllowAlways => {
                        this.icon(IconName::CheckDouble).icon_color(Color::Success)
                    }
                    acp::PermissionOptionKind::RejectOnce => {
                        this.icon(IconName::Close).icon_color(Color::Error)
                    }
                    acp::PermissionOptionKind::RejectAlways => {
                        this.icon(IconName::Close).icon_color(Color::Error)
                    }
                })
                .icon_position(IconPosition::Start)
                .icon_size(IconSize::XSmall)
                .label_size(LabelSize::Small)
                .on_click(cx.listener({
                    let tool_call_id = tool_call_id.clone();
                    let option_id = option.id.clone();
                    let option_kind = option.kind;
                    move |this, _, window, cx| {
                        this.authorize_tool_call(
                            tool_call_id.clone(),
                            option_id.clone(),
                            option_kind,
                            window,
                            cx,
                        );
                    }
                }))
        })))
}
```

#### UI Integration Context

The permission buttons are rendered as part of the tool call display:

```rust
// zed/crates/agent_ui/src/acp/thread_view.rs
ToolCallStatus::WaitingForConfirmation { options, .. } => v_flex()
    .w_full()
    .children(tool_call.content.iter().map(|content| {
        div()
            .child(self.render_tool_call_content(
                entry_ix,
                content,
                tool_call,
                use_card_layout,
                window,
                cx,
            ))
            .into_any_element()
    }))
    .child(self.render_permission_buttons(
        options,
        entry_ix,
        tool_call.id.clone(),
        cx,
    ))
    .into_any(),
```

### 5. Agent-Side Permission Requests

#### Tool Authorization in Agent2 (`zed/crates/agent2/src/thread.rs`)

Native Zed agents use `ToolCallEventStream` to request permissions:

```rust
// zed/crates/agent2/src/thread.rs
pub fn authorize(&self, title: impl Into<String>, cx: &mut App) -> Task<Result<()>> {
    if agent_settings::AgentSettings::get_global(cx).always_allow_tool_actions {
        return Task::ready(Ok(()));
    }

    let (response_tx, response_rx) = oneshot::channel();
    self.stream
        .0
        .unbounded_send(Ok(ThreadEvent::ToolCallAuthorization(
            ToolCallAuthorization {
                tool_call: acp::ToolCallUpdate {
                    id: acp::ToolCallId(self.tool_use_id.to_string().into()),
                    fields: acp::ToolCallUpdateFields {
                        title: Some(title.into()),
                        ..Default::default()
                    },
                },
                options: vec![
                    acp::PermissionOption {
                        id: acp::PermissionOptionId("always_allow".into()),
                        name: "Always Allow".into(),
                        kind: acp::PermissionOptionKind::AllowAlways,
                    },
                    acp::PermissionOption {
                        id: acp::PermissionOptionId("allow".into()),
                        name: "Allow".into(),
                        kind: acp::PermissionOptionKind::AllowOnce,
                    },
                    acp::PermissionOption {
                        id: acp::PermissionOptionId("deny".into()),
                        name: "Deny".into(),
                        kind: acp::PermissionOptionKind::RejectOnce,
                    },
                ],
                response: response_tx,
            },
        )))
        .ok();

    // Handle "Always Allow" by updating settings
    let fs = self.fs.clone();
    cx.spawn(async move |cx| match response_rx.await?.0.as_ref() {
        "always_allow" => {
            if let Some(fs) = fs.clone() {
                cx.update(|cx| {
                    update_settings_file::<AgentSettings>(fs, cx, |settings, _| {
                        settings.set_always_allow_tool_actions(true);
                    });
                })?;
            }
            Ok(())
        }
        "allow" => Ok(()),
        "deny" => Err(anyhow!("Tool call denied by user")),
        _ => Err(anyhow!("Unknown permission option")),
    })
}
```

#### Agent2 Permission Flow

```rust
// crates/agent2/src/agent.rs:768-786
ThreadEvent::ToolCallAuthorization(ToolCallAuthorization {
    tool_call,
    options,
    response,
}) => {
    let outcome_task = acp_thread.update(cx, |thread, cx| {
        thread.request_tool_call_authorization(tool_call, options, cx)
    })??;
    cx.background_spawn(async move {
        if let acp::RequestPermissionOutcome::Selected { option_id } =
            outcome_task.await
        {
            response
                .send(option_id)
                .map(|_| anyhow!("authorization receiver was dropped"))
                .log_err();
        }
    })
    .detach();
}
```

### 6. Settings Integration

#### Global Settings Management (`crates/agent_settings/src/agent_settings.rs`)

```rust
// crates/agent_settings/src/agent_settings.rs:67
pub always_allow_tool_actions: bool,

// crates/agent_settings/src/agent_settings.rs:171-173
pub fn set_always_allow_tool_actions(&mut self, allow: bool) {
    self.always_allow_tool_actions = Some(allow);
}
```

#### Settings Loading with Security Constraints

```rust
// crates/agent_settings/src/agent_settings.rs:514-517
// For security reasons, only trust the user's global settings for whether to always allow tool actions.
// If this could be overridden locally, an attacker could (e.g. by committing to source control and
// convincing you to switch branches) modify your project-local settings to disable the agent's safety checks.
settings.always_allow_tool_actions = sources
    .user
    .and_then(|setting| setting.always_allow_tool_actions)
    .unwrap_or(false);
```

#### Default Settings

```json
// assets/settings/default.json:841
"always_allow_tool_actions": false,
```

#### UI Settings Integration (`crates/agent_ui/src/agent_configuration.rs`)

```rust
// crates/agent_ui/src/agent_configuration.rs:402-419
fn render_command_permission(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
    let always_allow_tool_actions = AgentSettings::get_global(cx).always_allow_tool_actions;
    let fs = self.fs.clone();

    SwitchField::new(
        "always-allow-tool-actions-switch",
        "Allow running commands without asking for confirmation",
        Some(
            "The agent can perform potentially destructive actions without asking for your confirmation.".into(),
        ),
        always_allow_tool_actions,
        move |state, _window, cx| {
            let allow = state == &ToggleState::Selected;
            update_settings_file::<AgentSettings>(fs.clone(), cx, move |settings, _| {
                settings.set_always_allow_tool_actions(allow);
            });
        },
    )
}
```

### 7. Permission Option Types

#### ACP Protocol Permission Options

```rust
// From agent-client-protocol crate
pub enum PermissionOptionKind {
    AllowOnce,    // Allow this specific tool call
    AllowAlways,  // Allow this type of tool call always
    RejectOnce,   // Reject this specific tool call
    RejectAlways, // Reject this type of tool call always
}

pub struct PermissionOption {
    pub id: PermissionOptionId,
    pub name: String,
    pub kind: PermissionOptionKind,
}
```

### 8. Error Handling and Edge Cases

#### Connection Error Handling (`zed/crates/agent_servers/src/acp.rs`)

```rust
// zed/crates/agent_servers/src/acp.rs
match result {
    Ok(response) => Ok(response),
    Err(err) => {
        if err.code != ErrorCode::INTERNAL_ERROR.code {
            anyhow::bail!(err)
        }

        let Some(data) = &err.data else {
            anyhow::bail!(err)
        };

        // Handle Gemini CLI abort errors
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ErrorDetails {
            details: Box<str>,
        }

        match serde_json::from_value(data.clone()) {
            Ok(ErrorDetails { details }) => {
                if suppress_abort_err
                    && (details.contains("This operation was aborted")
                        || details.contains("The user aborted a request"))
                {
                    Ok(acp::PromptResponse {
                        stop_reason: acp::StopReason::Cancelled,
                    })
                } else {
                    Err(anyhow!(details))
                }
            }
            Err(_) => Err(anyhow!(err)),
        }
    }
}
```

#### Cancellation Handling

```rust
// zed/crates/acp_thread/src/acp_thread.rs
pub fn cancel(&mut self, cx: &mut Context<Self>) -> Task<()> {
    let Some(send_task) = self.send_task.take() else {
        return Task::ready(());
    };

    for entry in self.entries.iter_mut() {
        if let AgentThreadEntry::ToolCall(call) = entry {
            let cancel = matches!(
                call.status,
                ToolCallStatus::Pending
                    | ToolCallStatus::WaitingForConfirmation { .. }
                    | ToolCallStatus::InProgress
            );

            if cancel {
                call.status = ToolCallStatus::Canceled;
            }
        }
    }

    self.connection.cancel(&self.session_id, cx);
    cx.foreground_executor().spawn(send_task)
}
```

### 9. Complete Permission Request Flow

#### End-to-End Flow

1. **Agent Initiates Tool Call**
   - Agent calls `ToolCallEventStream::authorize()` or ACP sends `request_permission`
   - Checks `always_allow_tool_actions` setting

2. **Permission Evaluation**
   - If bypass enabled: Auto-allow with "AllowOnce"
   - If bypass disabled: Create permission dialog

3. **UI Display**
   - Render permission buttons with options
   - Show loading state while waiting

4. **User Decision**
   - User clicks Allow/Reject button
   - UI calls `authorize_tool_call()` on thread

5. **Response Processing**
   - Update tool call status
   - Send response via oneshot channel
   - Handle "Always Allow" by updating settings

6. **Agent Continuation**
   - Agent receives permission outcome
   - Proceeds with tool call or handles rejection

### 10. Security Considerations

#### Defense in Depth

1. **Default Secure**: `always_allow_tool_actions` defaults to `false`
2. **User-Only Settings**: Only user global settings can enable bypass
3. **Scoped Permissions**: Permissions are tool-call specific
4. **Audit Trail**: All decisions logged in ACP debug logs
5. **UI Confirmation**: Clear visual feedback for all decisions

#### Attack Vector Mitigation

- **Local Settings Override**: Prevented by only trusting user global settings
- **Social Engineering**: Clear UI prevents confusion about actions
- **Race Conditions**: Proper state management prevents double-authorization

### 11. Debugging and Monitoring

#### ACP Debug Logs

Access via `dev: open acp logs` command to monitor:
- Permission requests from agents
- User permission decisions
- Tool call execution status
- Protocol message flow

#### Debug Log Structure

```rust
// crates/acp_tools/src/acp_tools.rs - Message logging
let message = WatchedConnectionMessage {
    name: method,
    message_type,
    request_id,
    direction: stream_message.direction,
    // ... parameters and status
};
```

### 12. Integration Points

#### Settings System
- `AgentSettings::always_allow_tool_actions`
- User preference persistence
- Security-conscious loading logic

#### UI System
- Permission dialog rendering
- Button state management
- Loading indicators

#### Agent System
- Tool call event streaming
- Authorization request handling
- Response processing

#### ACP Protocol
- Message serialization/deserialization
- Error handling
- Connection management

## Library Integration Analysis

### 1. Claude Code ACP Implementation (`claude-code-acp`)

#### Permission Tool Integration

The Claude Code ACP agent implements a sophisticated permission system that bridges Claude Code's tool execution with Zed's ACP protocol:

```typescript
// claude-code-acp/src/mcp-server.ts:666-704
server.registerTool(
  "permission",
  {
    title: "Permission Tool",
    description: "Used to request tool permissions",
    inputSchema: {
      tool_name: z.string(),
      input: z.any(),
      tool_use_id: z.string().optional(),
    },
  },
  async (input) => {
    const session = agent.sessions[sessionId];
    if (!session) {
      return {
        content: [
          {
            type: "text",
            text: JSON.stringify({
              behavior: "deny",
              message: "Session not found",
            }),
          },
        ],
      };
    }
    if (alwaysAllowedTools[input.tool_name]) {
      return {
        content: [
          {
            type: "text",
            text: JSON.stringify({
              behavior: "allow",
              updatedInput: input.input,
            }),
          },
        ],
      };
    }
    const response = await agent.client.requestPermission({
      options: [
        {
          kind: "allow_always",
          name: "Always Allow",
          optionId: "allow_always",
        },
        { kind: "allow_once", name: "Allow", optionId: "allow" },
        { kind: "reject_once", name: "Reject", optionId: "reject" },
      ],
      sessionId,
      toolCall: {
        toolCallId: input.tool_use_id!,
        rawInput: input.input,
      },
    });
    if (
      response.outcome?.outcome === "selected" &&
      (response.outcome.optionId === "allow" || response.outcome.optionId === "allow_always")
    ) {
      if (response.outcome.optionId === "allow_always") {
        alwaysAllowedTools[input.tool_name] = true;
      }
      return {
        content: [
          {
            type: "text",
            text: JSON.stringify({
              behavior: "allow",
              updatedInput: input.input,
            }),
          },
        ],
      };
    } else {
      return {
        content: [
          {
            type: "text",
            text: JSON.stringify({
              behavior: "deny",
              message: "User refused permission to run tool",
            }),
          },
        ],
      };
    }
  },
);
```

#### Tool Mapping and Permission Handling

The Claude Code agent maps native Claude Code tools to ACP-compatible tools with permission handling:

```typescript
// claude-code-acp/src/tools.ts:29-43
export function toolInfoFromToolUse(
  toolUse: any,
  cachedFileContent: { [key: string]: string },
): ToolInfo {
  const name = toolUse.name;
  const input = toolUse.input;

  switch (name) {
    case "Task":
      return {
        title: input?.description ? input.description : "Task",
        kind: "think",
        content:
          input && input.prompt
            ? [
                {
                  type: "content",
                  content: { type: "text", text: input.prompt },
                },
              ]
            : [],
      };
```

#### MCP Server with Permission Proxy

The implementation creates an MCP server that proxies tool calls through Zed's permission system:

```typescript
// claude-code-acp/src/acp-agent.ts:124-137
const server = await createMcpServer(this, sessionId, this.clientCapabilities);
const address = server.address() as AddressInfo;
mcpServers["acp"] = {
  type: "http",
  url: "http://127.0.0.1:" + address.port + "/mcp",
  headers: {
    "x-acp-proxy-session-id": sessionId,
  },
};
```

### 2. Agent Client Protocol Library (`agent-client-protocol`)

#### Core Protocol Types

The ACP library provides the standardized communication layer:

```rust
// agent-client-protocol/rust/tool_call.rs:20-51
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    /// Unique identifier for this tool call within the session.
    #[serde(rename = "toolCallId")]
    pub id: ToolCallId,
    /// Human-readable title describing what the tool is doing.
    pub title: String,
    /// The category of tool being invoked.
    #[serde(default, skip_serializing_if = "ToolKind::is_default")]
    pub kind: ToolKind,
    /// Current execution status of the tool call.
    #[serde(default, skip_serializing_if = "ToolCallStatus::is_default")]
    pub status: ToolCallStatus,
    /// Content produced by the tool call.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<ToolCallContent>,
    /// File locations affected by this tool call.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<ToolCallLocation>,
    /// Raw input parameters sent to the tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_input: Option<serde_json::Value>,
    /// Raw output returned by the tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<serde_json::Value>,
}
```

#### Permission Request Protocol

The library defines the permission request/response cycle:

```rust
// agent-client-protocol/rust/client.rs:249-267
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(extend("x-side" = "client", "x-method" = SESSION_REQUEST_PERMISSION_METHOD_NAME))]
#[serde(rename_all = "camelCase")]
pub struct RequestPermissionRequest {
    /// The session ID for this request.
    pub session_id: SessionId,
    /// Details about the tool call requiring permission.
    pub tool_call: ToolCallUpdate,
    /// Available permission options for the user to choose from.
    pub options: Vec<PermissionOption>,
    /// Extension point for implementations
    #[serde(skip_serializing_if = "Option::is_none", rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}
```

### 3. Integration Points Between Zed and Libraries

#### Protocol Message Flow

```
Claude Code Agent → MCP Server → ACP Protocol → Zed Client → UI Permission Dialog → User Decision → ACP Response → Claude Code
```

#### Tool Call Lifecycle in External Agents

1. **Agent Initiates Tool Call**: Claude Code generates a tool call (e.g., `Bash`, `Read`, `Edit`)
2. **Permission Check**: MCP server intercepts tool call and requests permission via ACP
3. **Zed Permission Evaluation**: Zed checks `always_allow_tool_actions` setting
4. **UI Presentation**: If needed, Zed displays permission dialog to user
5. **User Decision**: User selects Allow/Reject option
6. **Response Routing**: Decision flows back through ACP to MCP server
7. **Tool Execution**: If allowed, MCP server executes the tool call
8. **Result Processing**: Tool results are formatted and returned to Claude Code

#### Key Integration Differences

**Native Zed Agents vs External ACP Agents**:

| Aspect | Native Zed Agents | External ACP Agents |
|--------|-------------------|---------------------|
| Permission Request | Direct `ToolCallEventStream::authorize()` | Via MCP server proxy |
| Tool Mapping | Direct Rust types | TypeScript ↔ Rust conversion |
| UI Integration | Native Zed UI components | ACP protocol serialization |
| Settings Scope | User global settings only | Per-agent always-allowed tools |
| Error Handling | Direct Rust error types | JSON-RPC error codes |

### 4. Permission Handling Differences

#### Native Zed Agent Permissions

Native agents use Zed's internal permission system:

```rust
// crates/agent2/src/thread.rs:2421-2470
pub fn authorize(&self, title: impl Into<String>, cx: &mut App) -> Task<Result<()>> {
    if agent_settings::AgentSettings::get_global(cx).always_allow_tool_actions {
        return Task::ready(Ok(()));
    }
    // ... permission dialog creation
}
```

#### External Agent Permissions

External agents use the MCP permission tool with per-tool allowlists:

```typescript
// claude-code-acp/src/mcp-server.ts:667-669
if (alwaysAllowedTools[input.tool_name]) {
  return {
    content: [
      {
        type: "text",
        text: JSON.stringify({
          behavior: "allow",
          updatedInput: input.input,
        }),
      },
    ],
  };
}
```

### 5. Special Handling and Gaps

#### Current Limitations

1. **Per-Agent Settings**: External agents maintain their own `alwaysAllowedTools` separate from Zed's global settings
2. **Tool Name Mapping**: Claude Code tool names must be mapped to ACP-compatible tool kinds
3. **Content Caching**: File content caching is handled differently between native and external agents
4. **Error Propagation**: Error handling differs between direct Rust calls and JSON-RPC protocol

#### Security Considerations for External Agents

1. **Tool Validation**: MCP server validates tool calls before forwarding to ACP
2. **Session Isolation**: Each session maintains separate permission state
3. **Content Sanitization**: File content includes security reminders for external agents
4. **Capability Negotiation**: Agents negotiate available tools based on client capabilities

#### Future Enhancement Opportunities

1. **Unified Settings**: Integrate external agent permissions with Zed's global settings
2. **Enhanced Tool Mapping**: More sophisticated tool kind detection and UI presentation
3. **Audit Integration**: Centralized logging of all permission decisions across agents
4. **Batch Permissions**: Support for requesting multiple permissions in a single dialog

## Conclusion

Zed's ACP permission system provides a robust, user-controlled framework for managing external agent tool calls. The implementation demonstrates careful attention to:

- **Security**: Default-deny with user-controlled bypass
- **Usability**: Clear UI with appropriate feedback
- **Reliability**: Comprehensive error handling and state management
- **Extensibility**: Clean separation of concerns and modular design
- **Auditability**: Complete logging and monitoring capabilities

The system successfully balances the need for agent autonomy with user control, providing a foundation for safe and productive AI-assisted development workflows.

The integration with `claude-code-acp` and `agent-client-protocol` libraries demonstrates how Zed extends its native permission system to external agents while maintaining consistent security guarantees and user experience.
