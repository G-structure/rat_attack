//! File system operations
//!
//! This module implements the file system operations (read and write)
//! with proper sandboxing and permission checking.

use agent_client_protocol as acp;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex as TokioMutex;
use tokio_tungstenite::WebSocketStream;

use crate::bridge::BridgeSharedConfig;
use crate::permissions::PermissionDecision;
use crate::transport::AgentTransport;

/// Validates and resolves a file path with security checks
///
/// This function implements project root sandboxing per RAT-LWS-REQ-044,
/// blocking access to sensitive system paths and resolving relative paths
/// against the configured project root.
///
/// # Arguments
/// - `path`: The file path to validate and resolve
/// - `project_root`: The configured project root for sandboxing
/// - `for_write`: Whether this path will be used for write operations
///
/// # Returns
/// - `Ok(PathBuf)` with the canonical path if validation passes
/// - `Err(acp::Error)` if the path is invalid or outside allowed areas
pub fn validate_and_resolve_path(
    path: &str,
    project_root: &Path,
    for_write: bool,
) -> Result<PathBuf, acp::Error> {
    tracing::debug!(path = %path, for_write = %for_write, project_root = %project_root.display(), "Validating file path");

    let project_root = project_root.canonicalize().map_err(|_| {
        acp::Error::internal_error().with_data("failed to canonicalize project root")
    })?;

    let path_buf = PathBuf::from(path);

    // Block access to sensitive system paths (additional safety)
    if path.starts_with("/etc/")
        || path.starts_with("/var/")
        || path.starts_with("/root/")
        || path.starts_with("/usr/")
        || path.starts_with("/boot/")
        || path.starts_with("/proc/")
    {
        tracing::warn!(path = %path, "Path access blocked - sensitive system path");
        return Err(acp::Error::internal_error().with_data("path outside project root"));
    }

    // For relative paths, resolve against project root
    let resolved_path = if path_buf.is_absolute() {
        path_buf
    } else {
        project_root.clone().join(&path_buf)
    };

    // Canonicalize path, handling the case where file doesn't exist for writes
    let canonical_path = if for_write {
        // For write operations, validate the parent directory since the file may not exist yet
        let parent = resolved_path
            .parent()
            .ok_or_else(|| acp::Error::internal_error().with_data("invalid path"))?;
        let canonical_parent = parent
            .canonicalize()
            .map_err(|_| acp::Error::internal_error().with_data("invalid parent path"))?;

        // Ensure parent is within project root
        if !canonical_parent.starts_with(&project_root) {
            tracing::warn!(path = %path, parent = %canonical_parent.display(), "Path access blocked - parent outside project root");
            return Err(acp::Error::internal_error().with_data("path outside project root"));
        }

        let file_name = resolved_path
            .file_name()
            .ok_or_else(|| acp::Error::internal_error().with_data("invalid path"))?;
        let full_path = canonical_parent.join(file_name);

        // Ensure full path is within project root
        if !full_path.starts_with(&project_root) {
            tracing::warn!(path = %path, full_path = %full_path.display(), "Path access blocked - full path outside project root");
            return Err(acp::Error::internal_error().with_data("path outside project root"));
        }

        full_path
    } else {
        // For reads, canonicalize the full path (file must exist)
        let canonical = resolved_path
            .canonicalize()
            .map_err(|_| acp::Error::internal_error().with_data("file not found"))?;

        // Ensure full path is within project root
        if !canonical.starts_with(&project_root) {
            tracing::warn!(path = %path, canonical = %canonical.display(), "Path access blocked - outside project root");
            return Err(acp::Error::internal_error().with_data("path outside project root"));
        }

        canonical
    };

    // Additional safety check on the final canonical path
    let canonical_str = canonical_path.to_string_lossy();
    if canonical_str.starts_with("/etc/")
        || canonical_str.starts_with("/var/")
        || canonical_str.starts_with("/root/")
        || canonical_str.starts_with("/usr/")
        || canonical_str.starts_with("/boot/")
        || canonical_str.starts_with("/proc/")
    {
        tracing::warn!(path = %path, "Path access blocked - resolved to sensitive system path");
        return Err(acp::Error::internal_error().with_data("path outside project root"));
    }

    tracing::debug!(original_path = %path, canonical_path = %canonical_path.display(), "Path validated successfully");
    Ok(canonical_path)
}

/// Handles reading a text file with optional line filtering
///
/// Implements RAT-LWS-REQ-040 by providing file reading with optional
/// line offset and limit parameters. Includes binary file detection
/// and UTF-8 validation.
///
/// # Arguments
/// - `path`: The file path to read
/// - `project_root`: The configured project root for sandboxing
/// - `line_offset`: Optional 1-based line offset to start reading from
/// - `line_limit`: Optional maximum number of lines to read
///
/// # Returns
/// - `Ok(String)` with the file content (possibly filtered)
/// - `Err(acp::Error)` if the file cannot be read or contains binary data
pub fn handle_read_text_file(
    path: &str,
    project_root: &Path,
    line_offset: Option<u32>,
    line_limit: Option<u32>,
) -> Result<String, acp::Error> {
    tracing::info!(path = %path, line_offset = ?line_offset, line_limit = ?line_limit, "Reading text file");
    let canonical_path = validate_and_resolve_path(path, project_root, false)?;

    // First read as bytes to check for binary content
    let bytes = std::fs::read(&canonical_path).map_err(|err| {
        tracing::warn!(path = %canonical_path.display(), error = %err, "Failed to read file");
        acp::Error::internal_error().with_data("file not found")
    })?;

    // Check if it's likely a binary file (contains null bytes)
    if bytes.contains(&0) {
        tracing::warn!(path = %canonical_path.display(), "Binary file detected");
        return Err(acp::Error::internal_error().with_data("binary file not supported"));
    }

    // Convert to string
    let content = String::from_utf8(bytes)
        .map_err(|err| {
            tracing::warn!(path = %canonical_path.display(), error = %err, "File contains invalid UTF-8");
            acp::Error::internal_error().with_data("file contains invalid UTF-8")
        })?;

    tracing::debug!(path = %canonical_path.display(), content_len = content.len(), "File read successfully");
    apply_line_filter(&content, line_offset, line_limit)
}

/// Applies line filtering to file content
///
/// This function handles line offset and limit parameters for file reading,
/// supporting various combinations of offset and limit values.
///
/// # Arguments
/// - `content`: The full file content
/// - `line_offset`: Optional 1-based line offset to start from
/// - `line_limit`: Optional maximum number of lines to include
///
/// # Returns
/// - `Ok(String)` with the filtered content
/// - `Err(acp::Error)` if filtering fails (should not happen in practice)
fn apply_line_filter(
    content: &str,
    line_offset: Option<u32>,
    line_limit: Option<u32>,
) -> Result<String, acp::Error> {
    let lines: Vec<&str> = content.lines().collect();
    let mut filtered_lines = lines.as_slice();

    if let Some(offset) = line_offset {
        let start_idx = (offset.saturating_sub(1) as usize).min(filtered_lines.len());
        if start_idx >= filtered_lines.len() {
            return Ok(String::new());
        }
        filtered_lines = &filtered_lines[start_idx..];
    }

    if let Some(limit) = line_limit {
        let end_idx = (limit as usize).min(filtered_lines.len());
        filtered_lines = &filtered_lines[..end_idx];
    }

    Ok(filtered_lines.join("\n"))
}

/// Handles writing a text file with permission gating
///
/// Implements RAT-LWS-REQ-041 by providing permission-gated file writing
/// with project root sandboxing. Requests permission from the agent transport
/// if not already cached, and supports various permission outcomes.
///
/// # Arguments
/// - `_stream`: WebSocket stream (unused in current implementation)
/// - `shared`: Bridge configuration containing permission cache
/// - `transport`: Agent transport for permission requests
/// - `session_id`: Session ID for permission context
/// - `path`: File path to write to
/// - `content`: Content to write to the file
///
/// # Returns
/// - `Ok(())` if the file was written successfully
/// - `Err(acp::Error)` if permission was denied or write failed
pub async fn handle_write_text_file(
    _stream: Arc<TokioMutex<WebSocketStream<TcpStream>>>,
    shared: &BridgeSharedConfig,
    transport: &Arc<dyn AgentTransport>,
    session_id: &str,
    path: &str,
    content: &str,
) -> Result<(), acp::Error> {
    use std::fs;

    tracing::info!(path = %path, session_id = %session_id, content_len = content.len(), "Writing text file");

    // First, check sandboxing
    let canonical_path = validate_and_resolve_path(path, &shared.project_root, true)?;
    let canonical_path_str = canonical_path.to_string_lossy().to_string();

    // Create parent directories if they don't exist
    if let Some(parent) = canonical_path.parent() {
        fs::create_dir_all(parent).map_err(|_| {
            acp::Error::internal_error().with_data("failed to create parent directories")
        })?;
    }

    // Check permission cache first
    let cached_decision = {
        let cache = shared.permission_cache.lock().await;
        let key = (session_id.to_string(), canonical_path_str.clone());
        cache.get(&key).cloned()
    };

    match cached_decision {
        Some(PermissionDecision::AllowAlways) => {
            // Cached allow_always - proceed with write without requesting permission
            fs::write(&canonical_path, content)
                .map_err(|_| acp::Error::internal_error().with_data("failed to write file"))?;
            return Ok(());
        }
        Some(PermissionDecision::RejectAlways) => {
            // Cached reject_always - return error immediately
            return Err(acp::Error::new((-32000, "Permission denied".to_string())));
        }
        None => {
            // No cached decision - request permission from agent
        }
    }

    // Request permission from the agent
    let permission_request = acp::RequestPermissionRequest {
        session_id: acp::SessionId(session_id.to_string().into()),
        tool_call: acp::ToolCallUpdate {
            id: acp::ToolCallId("fs_write_text_file".to_string().into()),
            fields: acp::ToolCallUpdateFields {
                kind: Some(acp::ToolKind::Edit),
                title: Some(format!("Write file: {path}")),
                status: Some(acp::ToolCallStatus::InProgress),
                ..Default::default()
            },
            meta: None,
        },
        options: vec![
            acp::PermissionOption {
                id: acp::PermissionOptionId("allow_once".to_string().into()),
                name: "Allow this write operation".to_string(),
                kind: acp::PermissionOptionKind::AllowOnce,
                meta: None,
            },
            acp::PermissionOption {
                id: acp::PermissionOptionId("allow_always".to_string().into()),
                name: "Allow all write operations".to_string(),
                kind: acp::PermissionOptionKind::AllowAlways,
                meta: None,
            },
            acp::PermissionOption {
                id: acp::PermissionOptionId("reject_once".to_string().into()),
                name: "Reject this write operation".to_string(),
                kind: acp::PermissionOptionKind::RejectOnce,
                meta: None,
            },
            acp::PermissionOption {
                id: acp::PermissionOptionId("reject_always".to_string().into()),
                name: "Reject all write operations".to_string(),
                kind: acp::PermissionOptionKind::RejectAlways,
                meta: None,
            },
        ],
        meta: None,
    };

    let permission_response = transport
        .request_permission(permission_request)
        .await
        .map_err(|_| acp::Error::internal_error().with_data("permission request failed"))?;

    // Check the permission outcome and update cache
    match permission_response.outcome {
        acp::RequestPermissionOutcome::Selected { option_id } => {
            match option_id.0.as_ref() {
                "allow_once" => {
                    // Permission granted for this write only, proceed with write
                    fs::write(&canonical_path, content).map_err(|_| {
                        acp::Error::internal_error().with_data("failed to write file")
                    })?;
                    Ok(())
                }
                "allow_always" => {
                    // Permission granted always, cache the decision and proceed with write
                    {
                        let mut cache = shared.permission_cache.lock().await;
                        let key = (session_id.to_string(), canonical_path_str.clone());
                        cache.insert(key, PermissionDecision::AllowAlways);
                    }
                    fs::write(&canonical_path, content).map_err(|_| {
                        acp::Error::internal_error().with_data("failed to write file")
                    })?;
                    Ok(())
                }
                "reject_once" => {
                    // Permission denied for this write only
                    Err(acp::Error::new((-32000, "Permission denied".to_string())))
                }
                "reject_always" => {
                    // Permission denied always, cache the decision
                    {
                        let mut cache = shared.permission_cache.lock().await;
                        let key = (session_id.to_string(), canonical_path_str.clone());
                        cache.insert(key, PermissionDecision::RejectAlways);
                    }
                    Err(acp::Error::new((-32000, "Permission denied".to_string())))
                }
                _ => {
                    // Unknown option
                    Err(acp::Error::new((
                        -32000,
                        "Unknown permission option".to_string(),
                    )))
                }
            }
        }
        acp::RequestPermissionOutcome::Cancelled => {
            // Permission request was cancelled
            Err(acp::Error::new((
                -32000,
                "Permission request cancelled".to_string(),
            )))
        }
    }
}
