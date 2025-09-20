//! Bridge server handle and lifecycle management
//!
//! This module provides the BridgeHandle type which represents a running
//! bridge server instance and allows for graceful shutdown.

use std::{future::Future, net::SocketAddr, pin::Pin};
use tokio::{sync::oneshot, task::JoinHandle};

/// Errors that can occur during bridge operations
#[derive(Debug)]
pub enum BridgeError {
    /// I/O error from network operations
    Io(std::io::Error),
    /// Error from background task execution
    Task(tokio::task::JoinError),
}

impl From<std::io::Error> for BridgeError {
    fn from(value: std::io::Error) -> Self {
        BridgeError::Io(value)
    }
}

impl From<tokio::task::JoinError> for BridgeError {
    fn from(value: tokio::task::JoinError) -> Self {
        BridgeError::Task(value)
    }
}

/// Handle to a running bridge server instance
///
/// This struct provides control over a running bridge server, allowing
/// the caller to retrieve the local address and gracefully shut down
/// the server when needed.
///
/// # Usage
/// ```no_run
/// # use ct_bridge::{serve, BridgeConfig, AgentTransport};
/// # use std::net::SocketAddr;
/// # use std::sync::Arc;
/// # async fn example(config: BridgeConfig, transport: Arc<dyn AgentTransport>) {
/// let handle = serve(config, transport).await.expect("failed to start bridge");
/// println!("Bridge listening on: {}", handle.local_addr());
///
/// // Later, shut down the bridge
/// handle.shutdown().await.expect("failed to shutdown bridge");
/// # }
/// ```
pub struct BridgeHandle {
    local_addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    join_handle: Option<JoinHandle<()>>,
}

impl BridgeHandle {
    /// Creates a new bridge handle
    ///
    /// This function is typically called by the bridge server startup code
    /// and not directly by users.
    pub(crate) fn new(
        local_addr: SocketAddr,
        shutdown: oneshot::Sender<()>,
        join_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            local_addr,
            shutdown: Some(shutdown),
            join_handle: Some(join_handle),
        }
    }

    /// Returns the local socket address the bridge is listening on
    ///
    /// This can be useful for logging or for determining the actual port
    /// when the bridge was configured to bind to port 0 (auto-assign).
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Gracefully shuts down the bridge server
    ///
    /// This method signals the bridge server to stop accepting new connections
    /// and waits for all existing operations to complete before returning.
    ///
    /// # Returns
    /// A future that resolves when the bridge has fully shut down, or an error
    /// if the shutdown process failed.
    pub fn shutdown(
        mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), BridgeError>> + Send + 'static>> {
        let shutdown = self.shutdown.take();
        let join_handle = self.join_handle.take();

        Box::pin(async move {
            // Signal the server to shut down
            if let Some(sender) = shutdown {
                let _ = sender.send(());
            }

            // Wait for the server task to complete
            if let Some(handle) = join_handle {
                handle.await?;
            }

            Ok(())
        })
    }
}