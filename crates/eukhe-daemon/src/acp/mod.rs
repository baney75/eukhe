//! ACP stdio mode served over a daemon session; see `daemon.rs`.

mod config_options;
pub mod daemon;
mod jsonrpc;
mod mcp;
mod meta;
mod producer;
mod types;
mod wire_config;
mod wire_events;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

fn internal_error(id: &Value, details: &str) -> Value {
    jsonrpc::error_response(
        id,
        jsonrpc::INTERNAL_ERROR,
        "Internal error",
        Some(&json!({ "details": details })),
    )
}

/// Two paths are the same cwd when their canonical forms match, or when they
/// are the same directory on disk (dev/inode) — the bind-mount and
/// case-normalized-FS cases a lexical comparison misses.
fn same_cwd(requested: &Path, actual: &Path) -> bool {
    let canonical = |path: &Path| -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    };
    let requested = canonical(requested);
    let actual = canonical(actual);
    if requested == actual {
        return true;
    }
    let identity = |path: &Path| -> Option<(u64, u64)> {
        // Windows reads no dev or ino (0): the canonical paths decide.
        let stat = eukhe_core::platform::stat::stat_fields(&std::fs::metadata(path).ok()?);
        if stat.dev == 0 || stat.ino == 0 {
            return None;
        }
        Some((stat.dev, stat.ino))
    };
    if let (Some(left), Some(right)) = (identity(&requested), identity(&actual)) {
        return left == right;
    }
    false
}
