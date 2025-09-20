//! Permission caching and decision types
//!
//! This module implements the permission caching system that allows users to
//! grant or deny permissions persistently, avoiding repeated prompts for the
//! same operations.

use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex as TokioMutex;

/// A cached permission decision that persists across operations
///
/// This enum represents the user's decision to allow or reject operations
/// permanently, implementing the "always allow" and "always reject" options
/// from RAT-LWS-REQ-091.
#[derive(Clone, Debug, PartialEq)]
pub enum PermissionDecision {
    /// Always allow this type of operation without prompting
    AllowAlways,
    /// Always reject this type of operation without prompting
    RejectAlways,
}

/// Type alias for the permission cache
///
/// The cache maps canonical file paths (or other operation identifiers) to
/// cached permission decisions. This allows the bridge to skip permission
/// prompts for operations that have been previously approved or denied.
///
/// # Implementation Notes
/// - Keys should be canonical absolute paths for file operations
/// - The cache is shared across all connections to a bridge instance
/// - Cache entries persist for the lifetime of the bridge process
pub type PermissionCache = Arc<TokioMutex<HashMap<String, PermissionDecision>>>;