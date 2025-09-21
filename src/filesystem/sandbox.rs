//! File system sandboxing
//!
//! This module implements project root sandboxing for file operations,
//! ensuring that all file access is restricted to allowed directories
//! as required by RAT-LWS-REQ-044.

use agent_client_protocol as acp;
use std::path::PathBuf;

/// Validates and resolves a file path with sandboxing restrictions
///
/// This function implements RAT-LWS-REQ-044 by enforcing project root sandboxing.
/// It blocks access to sensitive system directories and canonicalizes paths to
/// prevent directory traversal attacks.
///
/// # Arguments
/// - `path`: The file path to validate and resolve
/// - `for_write`: Whether this path will be used for writing (affects validation)
///
/// # Returns
/// - `Ok(PathBuf)`: The canonical absolute path if access is allowed
/// - `Err(acp::Error)`: An error if the path is outside the project root or invalid
///
/// # Security Notes
/// This implementation blocks access to system directories like /etc/, /var/, etc.
/// For relative paths, it resolves them against the current working directory.
/// The canonicalization process handles symlinks and ensures no directory traversal.
///
/// # TODO
/// Improve project root determination and overhaul sandboxing logic.
/// The current implementation blocks a set of hardcoded system directories
/// and resolves relative paths against the current working directory.
/// This approach is fragile and may allow directory traversal or
/// unintended access to files outside the intended project root.
/// Future work should compute the actual project root (e.g., via
/// environment variables, a .git directory, or a configuration file)
/// and enforce that all file accesses stay within that root.
pub fn validate_and_resolve_path(path: &str, for_write: bool) -> Result<PathBuf, acp::Error> {
    let path_buf = PathBuf::from(path);

    // Implement project root sandboxing per RAT-LWS-REQ-044
    // Block access to sensitive system paths
    if path.starts_with("/etc/")
        || path.starts_with("/var/")
        || path.starts_with("/root/")
        || path.starts_with("/usr/")
        || path.starts_with("/boot/")
        || path.starts_with("/proc/")
    {
        return Err(acp::Error::internal_error().with_data("path outside project root"));
    }

    // For relative paths, resolve against current working directory
    let resolved_path = if path_buf.is_absolute() {
        path_buf
    } else {
        std::env::current_dir()
            .map_err(|_| acp::Error::internal_error().with_data("failed to get current directory"))?
            .join(&path_buf)
    };

    // Canonicalize path, handling the case where file doesn't exist for writes
    let canonical_path = if for_write && !resolved_path.exists() {
        // For write operations, canonicalize the parent directory since the file may not exist yet
        let parent = resolved_path
            .parent()
            .ok_or_else(|| acp::Error::internal_error().with_data("invalid path"))?;
        let canonical_parent = parent
            .canonicalize()
            .map_err(|_| acp::Error::internal_error().with_data("invalid path"))?;
        canonical_parent.join(
            resolved_path
                .file_name()
                .ok_or_else(|| acp::Error::internal_error().with_data("invalid path"))?,
        )
    } else {
        resolved_path.canonicalize().map_err(|_| {
            acp::Error::internal_error().with_data(if for_write {
                "invalid path"
            } else {
                "file not found"
            })
        })?
    };

    // Additional safety check: ensure the canonical path doesn't escape to system directories
    let canonical_str = canonical_path.to_string_lossy();
    if canonical_str.starts_with("/etc/")
        || canonical_str.starts_with("/var/")
        || canonical_str.starts_with("/root/")
        || canonical_str.starts_with("/usr/")
        || canonical_str.starts_with("/boot/")
        || canonical_str.starts_with("/proc/")
    {
        return Err(acp::Error::internal_error().with_data("path outside project root"));
    }

    Ok(canonical_path)
}
