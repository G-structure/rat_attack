# Zed ACP Tools

## Overview

This document provides comprehensive technical documentation for all ACP (Agent Client Protocol) tools available in Zed. Each tool is implemented as a Rust struct that implements the `AgentTool` trait, providing a standardized interface for agent interactions with the Zed editor and project system. Only the tools in `zed/crates/agent2/src/tools` are included here.

Tools are categorized by functionality and include detailed implementation notes, security considerations, and code references to the Zed codebase.

## File System Tools

### Read File Tool (`read_file`)

**Purpose**: Reads the content of files within the project, with intelligent handling for different file types and sizes.

**Implementation**: `zed/crates/agent2/src/tools/read_file_tool.rs`

**Input Schema**:
```rust
pub struct ReadFileToolInput {
    /// Relative path to the file within the project
    pub path: String,
    /// Optional 1-based line number to start reading
    pub start_line: Option<u32>,
    /// Optional 1-based line number to end reading (inclusive)
    pub end_line: Option<u32>,
}
```

**Key Features**:
- **Security Filtering**: Respects both global and worktree-specific `file_scan_exclusions` and `private_files` settings
- **Image Support**: Automatically detects image files and returns language model image objects
- **Large File Handling**: For files exceeding `outline::AUTO_OUTLINE_SIZE` (typically 100KB), returns a structural outline instead of full content
- **Line Range Support**: Allows reading specific line ranges with 1-based indexing
- **Path Resolution**: Validates that paths exist within project worktrees and resolves relative paths

**Security Considerations**:
- Prevents reading files marked as private or excluded in settings
- Validates paths are within project boundaries to prevent directory traversal attacks
- Checks for file existence before attempting to read

**Code Reference**: The tool uses `project.open_buffer()` to load files and `buffer.read_with()` for content access, with comprehensive error handling for security violations.

### Edit File Tool (`edit_file`)

**Purpose**: Performs granular edits to existing files or creates new files with content.

**Implementation**: `zed/crates/agent2/src/tools/edit_file_tool.rs`

**Input Schema**:
```rust
pub struct EditFileToolInput {
    /// User-friendly description of the edit operation
    pub display_description: String,
    /// Full path to the file (must start with project root directory)
    pub path: PathBuf,
    /// Operation mode: Edit, Create, or Overwrite
    pub mode: EditFileMode,
}
```

**Modes**:
- **Edit**: Uses AI-powered diff generation for precise edits
- **Create**: Creates new files, validating parent directory existence
- **Overwrite**: Replaces entire file contents

**Key Features**:
- **AI-Powered Editing**: Uses `EditAgent` with language model completion for intelligent edit generation
- **Authorization**: Requires user confirmation for edit operations
- **Format on Save**: Automatically formats code when `format_on_save` is enabled
- **Diff Tracking**: Maintains unified diffs for change visualization
- **Security Validation**: Prevents unauthorized access to sensitive configuration files

**Security Considerations**:
- Requires authorization for editing local settings files (`.zed/` directories)
- Validates file paths are within project boundaries
- Checks for existing files when in Create mode

**Code Reference**: Integrates with `assistant_tools::edit_agent::EditAgent` for AI-powered editing, using `project.open_buffer()` and buffer manipulation APIs.

### Copy Path Tool (`copy_path`)

**Purpose**: Creates copies of files or directories within the project.

**Implementation**: `zed/crates/agent2/src/tools/copy_path_tool.rs`

**Input Schema**:
```rust
pub struct CopyPathToolInput {
    pub source_path: String,
    pub destination_path: String,
}
```

**Key Features**:
- **Recursive Directory Copying**: Uses `cp -r` semantics for directories
- **Path Validation**: Ensures both source and destination are within project boundaries
- **Entity-Based Operations**: Uses Zed's project entity system for efficient copying

**Code Reference**: Leverages `project.copy_entry()` method for low-level file system operations.

### Move/Rename Path Tool (`move_path`)

**Purpose**: Moves or renames files and directories within the project.

**Implementation**: `zed/crates/agent2/src/tools/move_path_tool.rs`

**Input Schema**:
```rust
pub struct MovePathToolInput {
    pub source_path: String,
    pub destination_path: String,
}
```

**Key Features**:
- **Automatic Rename Detection**: When source and destination directories are the same, performs rename operation
- **Cross-Directory Moves**: Handles moves between different project directories
- **Path Resolution**: Validates all paths are within project boundaries

**Code Reference**: Uses `project.rename_entry()` for atomic move/rename operations.

### Delete Path Tool (`delete_path`)

**Purpose**: Removes files or directories from the project.

**Implementation**: `zed/crates/agent2/src/tools/delete_path_tool.rs`

**Input Schema**:
```rust
pub struct DeletePathToolInput {
    pub path: String,
}
```

**Key Features**:
- **Recursive Deletion**: Removes directories and all contents
- **Action Logging**: Updates the action log for undo/redo functionality
- **Buffer Management**: Closes any open buffers for deleted files

**Code Reference**: Uses `project.delete_file()` with comprehensive buffer cleanup and action logging.

### Create Directory Tool (`create_directory`)

**Purpose**: Creates new directories within the project structure.

**Implementation**: `zed/crates/agent2/src/tools/create_directory_tool.rs`

**Input Schema**:
```rust
pub struct CreateDirectoryToolInput {
    pub path: String,
}
```

**Key Features**:
- **Parent Directory Creation**: Automatically creates parent directories (`mkdir -p` behavior)
- **Path Validation**: Ensures new directories are within project boundaries

**Code Reference**: Uses `project.create_entry()` with `is_dir = true` for directory creation.

## Search Tools

### Grep Tool (`grep`)

**Purpose**: Performs regex-based content search across all project files.

**Implementation**: `zed/crates/agent2/src/tools/grep_tool.rs`

**Input Schema**:
```rust
pub struct GrepToolInput {
    /// Rust regex pattern to search for
    pub regex: String,
    /// Optional glob pattern to limit search scope
    pub include_pattern: Option<String>,
    /// 0-based offset for pagination (20 results per page)
    pub offset: u32,
    /// Whether regex is case-sensitive
    pub case_sensitive: bool,
}
```

**Key Features**:
- **Full Regex Support**: Uses Rust's `regex` crate for pattern matching
- **Syntax-Aware Context**: Shows surrounding code structure using language server syntax trees
- **Pagination**: Returns results in pages of 20 matches with offset support
- **Security Filtering**: Respects `file_scan_exclusions` and `private_files` settings
- **Performance**: Uses Zed's optimized search infrastructure

**Code Reference**: Integrates with `project.search()` using `SearchQuery::regex()` and processes results through syntax tree analysis for contextual display.

### Find Path Tool (`find_path`)

**Purpose**: Searches for files and directories by name patterns using glob matching.

**Implementation**: `zed/crates/agent2/src/tools/find_path_tool.rs`

**Input Schema**:
```rust
pub struct FindPathToolInput {
    /// Glob pattern to match against file paths
    pub glob: String,
    /// 0-based offset for pagination (50 results per page)
    pub offset: usize,
}
```

**Key Features**:
- **Glob Pattern Support**: Uses standard glob syntax (`**/*.rs`, `src/**/*`, etc.)
- **Pagination**: Returns up to 50 results per page
- **Alphabetical Sorting**: Results are sorted for consistent ordering
- **Worktree Iteration**: Searches across all project worktrees

**Code Reference**: Uses `PathMatcher` for glob matching and iterates through `worktree.snapshot().entries()` for efficient file discovery.

## Terminal Tools

### Terminal Tool (`terminal`)

**Purpose**: Executes shell commands in the project environment.

**Implementation**: `zed/crates/agent2/src/tools/terminal_tool.rs`

**Input Schema**:
```rust
pub struct TerminalToolInput {
    /// Shell command to execute
    pub command: String,
    /// Working directory (must be project root directory)
    pub cd: String,
}
```

**Key Features**:
- **Shell Integration**: Uses user's default shell environment
- **Working Directory Control**: Changes to specified project root directory (validated)
- **Live View**: Emits `ToolCallContent::Terminal { terminal_id }` for inline terminal UI
- **Output Truncation**: Returns a summarized result with a 16 KiB output cap (live terminal shows full stream)
- **Authorization**: Requires user confirmation for command execution

**Security Considerations**:
- Commands are executed via Zed's terminal subsystem per invocation
- Working directories are restricted to project roots or absolute paths inside worktrees
- User authorization required for all command execution

**Code Reference**: Uses `ThreadEnvironment::create_terminal()` and integrates with Zed's terminal infrastructure for secure command execution.

## Development Tools

### Diagnostics Tool (`diagnostics`)

**Purpose**: Retrieves compiler errors and warnings for project files.

**Implementation**: `zed/crates/agent2/src/tools/diagnostics_tool.rs`

**Input Schema**:
```rust
pub struct DiagnosticsToolInput {
    /// Optional path to specific file (shows project summary if omitted)
    pub path: Option<String>,
}
```

**Key Features**:
- **File-Specific Diagnostics**: Shows all errors/warnings for a specific file
- **Project Summary**: When no path provided, shows error/warning counts across all files
- **Severity Filtering**: Only shows ERROR and WARNING level diagnostics
- **Real-time Updates**: Reflects current state of language server diagnostics

**Code Reference**: Uses `buffer.diagnostic_groups()` and `project.diagnostic_summaries()` to access language server diagnostic information.

### Now Tool (`now`)

**Purpose**: Returns the current date and time in RFC 3339 format.

**Implementation**: `zed/crates/agent2/src/tools/now_tool.rs`

**Input Schema**:
```rust
pub struct NowToolInput {
    pub timezone: Timezone, // Utc or Local
}
```

**Key Features**:
- **Timezone Support**: Returns time in UTC or local timezone
- **RFC 3339 Format**: Standard timestamp format for APIs and logging

**Code Reference**: Uses `chrono::Utc::now()` and `chrono::Local::now()` for timestamp generation.

## Web Tools

### Fetch Tool (`fetch`)

**Purpose**: Retrieves content from URLs and converts it to markdown.

**Implementation**: `zed/crates/agent2/src/tools/fetch_tool.rs`

**Input Schema**:
```rust
pub struct FetchToolInput {
    pub url: String,
}
```

**Key Features**:
- **Content Type Detection**: Handles HTML, JSON, and plain text responses
- **HTML to Markdown Conversion**: Uses `html_to_markdown` crate with tag handlers
- **Robust Errors**: Validates status codes and content-type; returns descriptive errors

**Code Reference**: Uses `HttpClientWithUrl` for HTTP requests and `convert_html_to_markdown()` with custom tag handlers for content processing.

### Web Search Tool (`web_search`)

**Purpose**: Performs web searches and returns structured results.

**Implementation**: `zed/crates/agent2/src/tools/web_search_tool.rs`

**Input Schema**:
```rust
pub struct WebSearchToolInput {
    pub query: String,
}
```

**Key Features**:
- **Provider Integration**: Uses configured web search providers (currently Zed Cloud)
- **Structured Results**: Returns search results with titles, URLs, and snippets
- **Result Streaming**: Updates UI with search progress and results

**Code Reference**: Integrates with `WebSearchRegistry` and provider-specific search implementations.

### Open Tool (`open`)

**Purpose**: Opens files or URLs with the system's default applications.

**Implementation**: `zed/crates/agent2/src/tools/open_tool.rs`

**Input Schema**:
```rust
pub struct OpenToolInput {
    pub path_or_url: String,
}
```

**Key Features**:
- **Cross-Platform**: Uses appropriate system commands (`open`, `start`, `xdg-open`, etc.)
- **Path Resolution**: Converts project-relative paths to absolute paths
- **URL Support**: Handles both file paths and web URLs

**Security Considerations**:
- Requires explicit user authorization
- Only opens user-requested content

**Code Reference**: Uses `open::that()` crate for cross-platform file/URL opening with path resolution through project APIs.

## Other Tools

### Thinking Tool (`thinking`)

**Purpose**: Provides a mechanism for agents to record their thought processes.

**Implementation**: `zed/crates/agent2/src/tools/thinking_tool.rs`

**Input Schema**:
```rust
pub struct ThinkingToolInput {
    pub content: String,
}
```

**Key Features**:
- **UI Integration**: Displays thinking content in the tool call interface
- **Non-Action Tool**: Purely informational, doesn't modify project state

**Code Reference**: Updates `ToolCallEventStream` with thinking content for UI display.

## Tool Architecture

All ACP tools implement the `AgentTool` trait:

```rust
pub trait AgentTool {
    type Input: DeserializeOwned + JsonSchema;
    type Output: Into<LanguageModelToolResultContent>;

    fn name() -> &'static str;
    fn kind() -> ToolKind;
    fn initial_title(&self, input: Result<Self::Input, Value>) -> SharedString;
    fn run(self: Arc<Self>, input: Self::Input, event_stream: ToolCallEventStream, cx: &mut App) -> Task<Result<Self::Output>>;
}
```

**Common Patterns**:
- **Security**: Most tools include authorization checks and path validation
- **Error Handling**: Comprehensive error handling with user-friendly messages
- **Async Operations**: All tool execution is asynchronous using GPUI's `Task` system
- **Event Streaming**: Tools update `ToolCallEventStream` for real-time UI feedback
- **Project Integration**: Tools interact with Zed's `Project` entity for file system operations

**Security Architecture**:
- **Path Validation**: All file operations validate paths are within project boundaries
- **Settings Respect**: Tools honor `file_scan_exclusions` and `private_files` configurations
- **Authorization**: Sensitive operations require explicit user confirmation
- **Terminal Guardrails**: Commands run via Zed's terminal with working-directory validation and capped returned output

## Context Server (MCP) Tools

- In addition to the native tools above, Zed can surface tools from connected MCP/context servers. These are wired through `zed/crates/agent2/src/tools/context_server_registry.rs` and exposed to ACP threads alongside native tools.
- MCP-provided tools are discovered at runtime and can vary based on configured servers and profiles; names may be prefixed to avoid collisions with native tools.

This comprehensive tool set enables ACP agents to perform sophisticated code analysis, editing, and project management tasks while maintaining security and user control.
