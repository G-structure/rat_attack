//! Bridge server core
//!
//! This module contains the core WebSocket server logic that handles CT-BRIDGE functionality.
//! It implements the server lifecycle and connection management for ACP over WebSocket.

pub mod config;
pub mod handle;
pub mod server;

pub use config::BridgeConfig;
pub(crate) use config::BridgeSharedConfig;
pub use handle::{BridgeHandle, BridgeError};
pub use server::serve;