# Zed ACP User Interface

## Overview

Zed’s ACP UI is implemented in `zed/crates/agent_ui`. The primary conversation view for ACP threads is `zed/crates/agent_ui/src/acp/thread_view.rs`, which renders agent messages, tool calls, streaming content, plans, and permission flows. The ACP panel container and thread orchestration live in `zed/crates/agent_ui/src/agent_panel.rs`. A developer log view for ACP I/O is implemented in `zed/crates/acp_tools/src/acp_tools.rs`.

## Core Components

- `agent_panel.rs`
  - Hosts the Agent Panel, thread list, toolbar, model/profile selectors, and wiring to the workspace. Registers panel actions (open history, onboarding, model selector, etc.).
  - Constructs a `ToolWorkingSet` and passes it into threads so tool availability reflects configuration and MCP servers.
  - Persists panel state (width, selected agent) via `KEY_VALUE_STORE`.

- `acp/thread_view.rs` (AcpThreadView)
  - Renders the active thread: user messages, assistant streamed chunks, and tool calls.
  - Tracks expansion state for tool calls with `expanded_tool_calls: HashSet<ToolCallId>`.
  - Maps `acp::ToolKind` to icons (Execute → `ToolTerminal`, Edit → `ToolPencil`, etc.).
  - Handles “WaitingForConfirmation” with interactive approval UI; shows Pending/InProgress/Completed/Failed/Rejected/Canceled states.
  - Integrates with diff and terminal subsystems for rich tool visualization.

- `acp/entry_view_state.rs`
  - Maintains per-entry UI state (e.g., terminal/diff components) and synchronizes them with thread entries as they stream in.

- `acp_tools.rs`
  - Developer/debug UI for ACP RPC traffic with direction (Incoming/Outgoing) and message lists.

## Thread View Architecture (thread_view.rs)

- Renders each thread entry type (user message, assistant message, tool call) and appends streamed `AgentMessageChunk` content as it arrives.
- Assistant messages render Markdown via `render_markdown` with theme-aware `MarkdownStyle`. “Thought” chunks receive a distinct “thinking” block with disclosure controls.
- Tool calls route through `render_tool_call` (generic) or `render_terminal_tool_call` (terminal-specific). The decision is made per `ToolCallContent` item.
- Expansion is controlled per tool call via `expanded_tool_calls` and a header disclosure.

### Tool Call Content Mapping

- `render_tool_call_content` handles each `acp::ToolCallContent` variant:
  - `ContentBlock`:
    - If it’s a `ResourceLink`, render a clickable chip. For `file://` URIs, the label is normalized to a project-relative path; clicking opens the file buffer. For other URIs, open externally.
    - If it’s Markdown, render via `render_markdown_output` with card or inline styling, plus a collapse button for inline layout.
  - `Diff`:
    - Render with `render_diff_editor`, backed by `buffer_diff::BufferDiff`. For complex scenarios, “Open Agent Diff” launches `crate::agent_diff::AgentDiffPane` bound to the thread.
  - `Terminal`:
    - Render with `render_terminal_tool_call` (see below).

### Permission and Confirmation Flow

- When a tool call is `ToolCallStatus::WaitingForConfirmation { options, .. }`, the content area is followed by `render_permission_buttons`.
- Buttons correspond to `acp::PermissionOptionKind`:
  - AllowOnce (check icon), AllowAlways (double-check icon), RejectOnce/RejectAlways (close icon), with semantic colors.
- Clicking a button invokes `authorize_tool_call(..)` which calls `thread.authorize_tool_call(..)` to update the `acp_thread` state and proceed/cancel.

### Terminal Tool Call UI

- Implemented by `render_terminal_tool_call(entry_ix, terminal, tool_call, window, cx)`.
- Header includes:
  - Working directory (buffer font, muted), running indicator, Stop button (kills active task), elapsed time when >10s.
  - “Truncated” badge with tooltip explaining whether truncation was due to returned-output capping or terminal scrollback limit (`terminal::MAX_SCROLL_HISTORY_LINES`).
  - Error indicator (exit code tooltip) when the exit status is non-zero or tool call failed.
  - Disclosure to expand/collapse the embedded terminal view.
- Body includes:
  - The executed command rendered as Markdown (code block renderer without copy button to reduce noise).
  - When expanded and a `TerminalView` is available from `entry_view_state`, an embedded terminal output area reflects the live PTY content captured by `acp_thread::Terminal`.
- Background integration:
  - For first-party flows like Claude login, the view demonstrates spawning commands in the docked Terminal Panel via `task::SpawnInTerminal` and `terminal_view::terminal_panel::TerminalPanel`, while keeping results linked to the thread.

### Plans and Streaming

- Plans: Renders `acp::Plan` entries with label styling per `PlanEntryStatus` (completed entries are struck through). The current plan is shown with a compact, readable layout.
- Streaming: Assistant text is appended chunk-by-chunk. “Thought” blocks are grouped under a header and are collapsible to reduce noise.

### Export and Notifications

- Export: “Open as Markdown” exports the current thread by calling `thread.to_markdown(cx)` and opens it in an editor buffer (`open_thread_as_markdown`).
- Notifications: Helper methods show contextual notifications and callouts for authentication, usage limits, and errors (caption + icon consistency), including “tool use limit reached” and Burn Mode toggles.

## Agent Panel (agent_panel.rs)

- Registers panel actions (new thread, open history, onboarding modals, model selector, follow, expand editor, etc.) and wires them to workspace focus and panel visibility.
- Modes:
  - Thread: `ActiveThread` with a message editor and title editor. Subscribes to thread events, displays history, and toolbars.
  - ExternalAgentThread: embeds `AcpThreadView` directly for external agents.
  - TextThread / History / Configuration: auxiliary modes within the same panel shell.
- Tool working set: constructs a `ToolWorkingSet` on panel load, subscribes to model changes to adjust web search tool availability, and propagates to threads.
- Burn Mode and usage: renders provider plan/limit banners and quick actions for enabling Burn Mode or retrying when limits are reached.
- Rules surface: when available, “User Rules” and “Project Rules” chips open the Rules Library (`zed_actions::OpenRulesLibrary`).

## ACP Debugging Tools (acp_tools.rs)

- `AcpTools` shows a live log of ACP stream messages with direction (Incoming/Outgoing) and message kinds (Request/Response/Notification). Tracks active connections via `AcpConnectionRegistry` and subscribes to push updates. Useful for debugging tool authorization, content rendering, and agent-server communication.

## How UI State Mirrors ACP State

- Status: `acp_thread` drives `ToolCallStatus`. The UI maps statuses to header treatments, disclosure defaults, and badges.
- Locations: tool calls can attach `locations` in `ToolCallUpdateFields`. The UI uses these paths to pick file icons, show labels, open editors, and anchor diffs.
- Content: `ToolCallContent` (ContentBlock/Markdown/ResourceLink, Diff, Terminal) selects the rendering path described above.
- Authorization: the permission buttons call back into `acp_thread` via `authorize_tool_call`, unblocking the tool run or rejecting it.

## Where to Look in the Code

- Conversation view: `zed/crates/agent_ui/src/acp/thread_view.rs`
- Panel shell/container: `zed/crates/agent_ui/src/agent_panel.rs`
- Entry view state and wiring: `zed/crates/agent_ui/src/acp/entry_view_state.rs`
- ACP debug tools/logs: `zed/crates/acp_tools/src/acp_tools.rs`
- Tool/use status model: `zed/crates/acp_thread/src/acp_thread.rs` (ToolCall, ToolCallStatus, ToolCallContent, Plan)

These files implement the ACP UI flow end‑to‑end: thread creation, message rendering, tool authorization, live terminal/diff views, plans, debugging, export, and notifications.

