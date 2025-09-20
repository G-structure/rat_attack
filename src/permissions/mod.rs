//! Permission system
//!
//! This module implements the permission model for gating file operations and other
//! sensitive actions as required by RAT-LWS-REQ-041. It handles permission caching
//! and approval workflows.

pub mod cache;

pub use cache::{PermissionDecision, PermissionCache};