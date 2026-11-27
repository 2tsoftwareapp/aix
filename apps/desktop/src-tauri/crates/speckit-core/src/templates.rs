use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TemplateStoreError {
    #[error("template directory not found: {0}")]
    MissingDir(PathBuf),
    #[error("SHA256SUMS manifest missing or unreadable")]
    MissingManifest,
    #[error("hash mismatch for {path}: expected {expected}, got {actual}")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("file listed in manifest is missing: {0}")]
    MissingFile(String),
    #[error("template not found: {0}")]
    TemplateNotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Vendored, hash-pinned speckit template store.
/// Verification is mandatory at load time: any drift fails closed.
#[derive(Debug)]
pub struct TemplateStore {
    root: PathBuf,
    files: HashMap<String, String>,
}

impl TemplateStore {
    /// Load from a vendored speckit content dir (containing SHA256SUMS).
    /// Fails if any file hash mismatches the manifest.
    pub fn load_verified(root: impl AsRef<Path>) -> Result<Self, TemplateStoreError> {
        let root = root.as_ref().to_path_buf();
        if !root.is_dir() {
            return Err(TemplateStoreError::MissingDir(root));
        }
        let manifest_path = root.join("SHA256SUMS");
        if !manifest_path.is_file() {
            return Err(TemplateStoreError::MissingManifest);
        }
        let manifest = fs::read_to_string(&manifest_path)?;
        let mut files = HashMap::new();
        for line in manifest.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let (hash, path) = line
                .split_once(char::is_whitespace)
                .ok_or(TemplateStoreError::MissingManifest)?;
            let rel = path.trim_start_matches(['*', ' ']).trim();
            let full = root.join(rel);
            if !full.is_file() {
                return Err(TemplateStoreError::MissingFile(rel.to_string()));
            }
            let bytes = fs::read(&full)?;
            let actual = hex_encode(&Sha256::digest(&bytes));
            if actual != hash {
                return Err(TemplateStoreError::HashMismatch {
                    path: rel.to_string(),
                    expected: hash.to_string(),
                    actual,
                });
            }
            files.insert(rel.to_string(), String::from_utf8_lossy(&bytes).into_owned());
        }
        Ok(Self { root, files })
    }

    /// Get a command template body verbatim (e.g. "templates/commands/specify.md").
    pub fn get(&self, rel_path: &str) -> Result<&str, TemplateStoreError> {
        self.files
            .get(rel_path)
            .map(|s| s.as_str())
            .ok_or_else(|| TemplateStoreError::TemplateNotFound(rel_path.to_string()))
    }

    pub fn command_template(&self, step: crate::steps::Step) -> Result<&str, TemplateStoreError> {
        self.get(&format!("templates/commands/{}", step.template_file()))
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vendored_root() -> PathBuf {
        // speckit-templates inside the desktop app: crates/speckit-core -> src-tauri -> desktop
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for _ in 0..3 {
            p.pop();
        }
        p.join("speckit-templates")
    }

    #[test]
    fn vendored_templates_verify_clean() {
        let store = TemplateStore::load_verified(vendored_root())
            .expect("vendored templates must pass hash verification");
        assert!(store.len() > 50, "expected substantial template set");
    }

    #[test]
    fn all_nine_command_templates_present() {
        let store = TemplateStore::load_verified(vendored_root()).unwrap();
        for step in crate::steps::Step::ALL {
            store
                .command_template(step)
                .unwrap_or_else(|_| panic!("missing template for {step:?}"));
        }
    }

    #[test]
    fn tampered_file_fails_closed() {
        let tmp = tempfile::tempdir().unwrap();
        let src = vendored_root();
        // copy a minimal subset: one file + manifest
        let sub = tmp.path().join("templates/commands");
        fs::create_dir_all(&sub).unwrap();
        let content = fs::read_to_string(src.join("templates/commands/specify.md")).unwrap();
        fs::write(sub.join("specify.md"), &content).unwrap();
        let hash = {
            let actual = hex_encode(&Sha256::digest(content.as_bytes()));
            // corrupt the manifest hash deliberately
            actual.replacen(&actual[0..1], "0", 1)
        };
        fs::write(
            tmp.path().join("SHA256SUMS"),
            format!("{hash}  templates/commands/specify.md\n"),
        )
        .unwrap();
        let err = TemplateStore::load_verified(tmp.path()).unwrap_err();
        assert!(matches!(err, TemplateStoreError::HashMismatch { .. }));
    }

    #[test]
    fn missing_file_in_manifest_fails() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("SHA256SUMS"),
            format!("{}  templates/commands/nope.md\n", "a".repeat(64)),
        )
        .unwrap();
        let err = TemplateStore::load_verified(tmp.path()).unwrap_err();
        assert!(matches!(err, TemplateStoreError::MissingFile(_)));
    }
}
