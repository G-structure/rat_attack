//! Authentication module
//!
//! This module handles Claude CLI login automation and authentication workflows.
//! It provides support for automated CLI interactions and login URL extraction.

pub mod cli;
pub mod discovery;
pub mod url_extract;

pub use cli::handle_auth_cli_login;