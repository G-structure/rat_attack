//! CLI authentication handling
//!
//! This module handles authentication via the Claude CLI, including process spawning,
//! PTY management, and login URL extraction.

use agent_client_protocol as acp;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

// Global mutex to serialize CLI resolution during tests to prevent env var races
static CLI_RESOLUTION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Handles the auth/cli_login ACP method
///
/// This function launches the Claude CLI login process and captures the login URL
/// from its output. It uses a PTY to interact with the CLI and automates the
/// initial interaction by sending carriage returns.
///
/// # Returns
/// - `Ok(String)` with the login URL if successful
/// - `Err(acp::Error)` if the CLI could not be found or login failed
pub async fn handle_auth_cli_login() -> Result<String, acp::Error> {
    let (cli_path, args) = resolve_claude_login_command()?;

    let project_root = std::env::current_dir()
        .map_err(|_| acp::Error::internal_error().with_data("failed to get current directory"))?;

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| {
            acp::Error::internal_error().with_data(format!("failed to open pty: {err}"))
        })?;

    let cli_command = cli_path
        .to_str()
        .ok_or_else(|| acp::Error::internal_error().with_data("invalid Claude CLI path"))?
        .to_string();

    let mut builder = CommandBuilder::new(cli_command);
    for arg in &args {
        builder.arg(arg);
    }
    builder.arg("/login");
    builder.cwd(&project_root);
    for (key, value) in std::env::vars() {
        builder.env(key, value);
    }

    let child = pair.slave.spawn_command(builder).map_err(|err| {
        acp::Error::internal_error().with_data(format!("failed to spawn login CLI: {err}"))
    })?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(|err| {
        acp::Error::internal_error().with_data(format!("failed to clone pty reader: {err}"))
    })?;
    let mut writer = pair.master.take_writer().map_err(|err| {
        acp::Error::internal_error().with_data(format!("failed to take pty writer: {err}"))
    })?;

    let automation_stop = Arc::new(AtomicBool::new(false));
    let writer_stop = automation_stop.clone();
    let writer_thread = std::thread::spawn(move || {
        while !writer_stop.load(Ordering::Relaxed) {
            if writer.write_all(b"\r").is_err() {
                break;
            }
            let _ = writer.flush();
            std::thread::sleep(Duration::from_millis(250));
        }
    });

    let (tx, mut rx) = mpsc::unbounded_channel();
    let reader_stop = automation_stop.clone();
    let reader_thread = std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        while !reader_stop.load(Ordering::Relaxed) {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    if tx.send(buffer[..read].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let capture_stop = automation_stop.clone();
    let capture = async move {
        let mut collected = String::new();
        while let Some(chunk) = rx.recv().await {
            let text = String::from_utf8_lossy(&chunk);
            collected.push_str(&text);
            if let Some(url) = extract_login_url(&collected) {
                capture_stop.store(true, Ordering::Relaxed);
                return Ok::<String, acp::Error>(url);
            }
        }

        capture_stop.store(true, Ordering::Relaxed);
        Err(acp::Error::internal_error().with_data("login CLI exited before emitting a login URL"))
    };

    let capture_result = timeout(Duration::from_secs(30), capture).await;

    automation_stop.store(true, Ordering::Relaxed);
    let _ = writer_thread.join();
    let _ = reader_thread.join();

    let result = capture_result.map_err(|_| {
        acp::Error::internal_error().with_data("timed out waiting for Claude login URL")
    })?;

    // Detach the child process; the CLI continues running until the user completes login.
    drop(child);

    result
}

/// Resolves the Claude CLI command path and arguments
///
/// This function implements a priority-based search for the Claude CLI:
/// 1. Test-specific override (TEST_CLAUDE_CLI_PATH)
/// 2. CLAUDE_ACP_BIN environment variable
/// 3. Claude Code CLI from node_modules
/// 4. Fallback to `claude` in PATH
///
/// # Returns
/// - `Ok((PathBuf, Vec<String>))` with the CLI path and arguments
/// - `Err(acp::Error)` if no CLI could be found
fn resolve_claude_login_command() -> Result<(PathBuf, Vec<String>), acp::Error> {
    // Serialize access to environment variables during resolution to prevent test interference
    let lock = CLI_RESOLUTION_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().unwrap();

    // Check for test failure mode
    if std::env::var("TEST_MODE_FAIL").is_ok() {
        return Err(acp::Error::new((-32000, "Unable to locate Claude login CLI. Try installing @zed-industries/claude-code-acp or ensure `claude` is in PATH.".to_string())));
    }

    // Check for test-specific override first (highest priority for tests)
    if let Ok(path) = std::env::var("TEST_CLAUDE_CLI_PATH") {
        return Ok((PathBuf::from(path), vec![]));
    }

    // Check CLAUDE_ACP_BIN environment variable
    if let Ok(path) = std::env::var("CLAUDE_ACP_BIN") {
        let path_buf = PathBuf::from(path);
        if path_buf.exists() {
            return Ok((path_buf, vec![]));
        }
    }

    // Try to find Claude Code CLI from node_modules (like Zed does)
    if let Some((path, args)) = find_claude_code_cli_from_node_modules() {
        return Ok((path, args));
    }

    // Fallback: try a `claude` executable in PATH
    if let Ok(path) = which::which("claude") {
        return Ok((path, vec![]));
    }

    Err(acp::Error::new((-32000, "Unable to locate Claude login CLI. Try installing @zed-industries/claude-code-acp or ensure `claude` is in PATH.".to_string())))
}

/// Finds the Claude Code CLI from node_modules
///
/// This function looks for the Claude Code CLI installation similar to how Zed does it,
/// checking for @zed-industries/claude-code-acp and then finding the corresponding
/// @anthropic-ai/claude-code CLI.
///
/// # Returns
/// - `Some((PathBuf, Vec<String>))` with the CLI path and arguments if found
/// - `None` if not found in node_modules
fn find_claude_code_cli_from_node_modules() -> Option<(PathBuf, Vec<String>)> {
    // Look for the Claude Code CLI in node_modules, similar to Zed's approach
    // Check if we have @zed-industries/claude-code-acp installed locally
    let acp_entry = PathBuf::from("node_modules/@zed-industries/claude-code-acp/dist/index.js");
    if acp_entry.exists() {
        // Walk up to find the @anthropic-ai/claude-code/cli.js
        let node_modules_dir = acp_entry
            .parent() // dist
            .and_then(|p| p.parent()) // @zed-industries/claude-code-acp
            .and_then(|p| p.parent()) // @zed-industries
            .and_then(|p| p.parent()); // node_modules

        if let Some(node_modules_dir) = node_modules_dir {
            let cli_js = node_modules_dir
                .join("@anthropic-ai")
                .join("claude-code")
                .join("cli.js");
            if cli_js.exists() {
                return Some((
                    PathBuf::from("node"),
                    vec![cli_js.to_string_lossy().to_string()],
                ));
            }
        }
    }

    None
}

/// Extracts a login URL from CLI output buffer
///
/// This function parses the CLI output looking for HTTPS URLs and extracts
/// the first complete URL found, handling various terminal escape sequences
/// and whitespace characters.
///
/// # Arguments
/// - `buffer`: The accumulated CLI output text
///
/// # Returns
/// - `Some(String)` with the extracted URL if found
/// - `None` if no valid URL was found
fn extract_login_url(buffer: &str) -> Option<String> {
    let start = buffer.find("https://")?;
    let tail = &buffer[start..];
    let mut end = tail.len();
    for (idx, ch) in tail.char_indices() {
        if ch.is_whitespace() || ch == '"' || ch == '\'' || ch == '\u{7}' || ch == '\u{1b}' {
            end = idx;
            break;
        }
    }
    let mut url = tail[..end].to_string();
    if let Some(pos) = url.find('\u{7}') {
        url.truncate(pos);
    }
    if let Some(pos) = url.find('\u{1b}') {
        url.truncate(pos);
    }
    if url.is_empty() {
        None
    } else {
        Some(url)
    }
}
