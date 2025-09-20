//! File system operations
//!
//! This module handles file system operations with project root sandboxing as required
//! by RAT-LWS-REQ-044. It provides secure file read/write operations within allowed boundaries.

pub mod sandbox;
pub mod operations;

pub use operations::{handle_read_text_file, handle_write_text_file, validate_and_resolve_path};