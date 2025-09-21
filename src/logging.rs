//! Logging configuration for CT-BRIDGE
//!
//! This module provides centralized logging setup with support for:
//! - Environment-based log level control via RUST_LOG
//! - Configurable output to console, file, or both
//! - Structured logging with tracing
//! - Development-friendly formatting

use std::env;
use std::path::Path;
use tracing_appender::rolling::RollingFileAppender;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Initialize logging based on environment variables
///
/// Environment variables:
/// - RUST_LOG: Controls log level (trace, debug, info, warn, error)
/// - CT_LOG_FILE: Optional path to log file directory (enables file logging)
/// - CT_LOG_FORMAT: "json" for structured logs, "pretty" for human-readable (default)
pub fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("ct_bridge=info"))
        .unwrap();

    let log_format = env::var("CT_LOG_FORMAT").unwrap_or_else(|_| "pretty".to_string());
    let log_file_dir = env::var("CT_LOG_FILE").ok();

    let stdout_layer = match log_format.as_str() {
        "json" => fmt::layer()
            .json()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .boxed(),
        _ => fmt::layer()
            .pretty()
            .with_target(true)
            .with_thread_ids(false)
            .with_thread_names(false)
            .boxed(),
    };

    if let Some(log_dir) = log_file_dir {
        // File + console logging
        let log_path = Path::new(&log_dir);
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file_appender = RollingFileAppender::builder()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("ct-bridge")
            .filename_suffix("log")
            .build(log_path)?;

        let file_layer = fmt::layer()
            .json()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_writer(file_appender)
            .boxed();

        tracing_subscriber::registry()
            .with(env_filter)
            .with(stdout_layer)
            .with(file_layer)
            .init();

        tracing::info!("Logging initialized with file output: {}", log_dir);
    } else {
        // Console-only logging
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stdout_layer)
            .init();

        tracing::info!("Logging initialized (console only)");
    }

    Ok(())
}

/// Helper macros for common logging patterns
#[macro_export]
macro_rules! log_request {
    ($method:expr, $id:expr) => {
        tracing::debug!(method = $method, request_id = $id, "Processing request")
    };
}

#[macro_export]
macro_rules! log_response {
    ($method:expr, $id:expr, $success:expr) => {
        if $success {
            tracing::debug!(
                method = $method,
                request_id = $id,
                "Request completed successfully"
            )
        } else {
            tracing::warn!(method = $method, request_id = $id, "Request failed")
        }
    };
}
