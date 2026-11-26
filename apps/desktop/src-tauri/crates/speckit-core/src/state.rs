use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Per-artifact presence for one feature.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArtifactState {
    pub spec: bool,
    pub plan: bool,
    pub tasks: bool,
    /// Custom checklists: (file name, total items, checked items).
    /// Checkbox state is reviewer-owned; we only read it.
    pub checklists: Vec<ChecklistState>,
    /// tasks.md completion: (total, done)
    pub task_progress: (usize, usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistState {
    pub name: String,
    pub total: usize,
    pub checked: usize,
}

impl ChecklistState {
    pub fn all_checked(&self) -> bool {
        self.total > 0 && self.checked >= self.total
    }
}

/// One feature directory under specs/.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureArtifacts {
    pub slug: String,
    pub dir: PathBuf,
    pub artifacts: ArtifactState,
}

impl FeatureArtifacts {
    /// The furthest pipeline point reached, derived purely from artifacts.
    pub fn furthest_step(&self) -> crate::steps::Step {
        use crate::steps::Step;
        if self.artifacts.tasks {
            Step::Tasks
        } else if self.artifacts.plan {
            Step::Plan
        } else if self.artifacts.spec {
            Step::Specify
        } else {
            Step::Constitution
        }
    }

    /// Gate: implement requires tasks.md to exist and be non-empty.
    pub fn implement_allowed(&self) -> Result<(), GateBlock> {
        if !self.artifacts.tasks {
            return Err(GateBlock::NoTasks);
        }
        if self.artifacts.task_progress.0 == 0 {
            return Err(GateBlock::EmptyTasks);
        }
        Ok(())
    }

    /// Unchecked custom checklist items trigger a confirmation gate (not a hard block).
    pub fn unchecked_checklist_items(&self) -> usize {
        self.artifacts
            .checklists
            .iter()
            .map(|c| c.total - c.checked)
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateBlock {
    NoTasks,
    EmptyTasks,
}

/// Whole-project pipeline state, derived from the filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineState {
    pub project_root: PathBuf,
    pub has_constitution: bool,
    pub features: Vec<FeatureArtifacts>,
}

impl PipelineState {
    /// Scan a project directory. Never fails on missing pieces — absence is state.
    pub fn scan(project_root: impl AsRef<Path>) -> Self {
        let root = project_root.as_ref().to_path_buf();
        let has_constitution = root.join(".specify/memory/constitution.md").is_file();
        let mut features = Vec::new();
        let specs_dir = root.join("specs");
        if specs_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&specs_dir) {
                for entry in entries.flatten() {
                    let dir = entry.path();
                    if !dir.is_dir() {
                        continue;
                    }
                    let slug = entry.file_name().to_string_lossy().into_owned();
                    features.push(FeatureArtifacts {
                        slug,
                        artifacts: scan_feature(&dir),
                        dir,
                    });
                }
            }
        }
        features.sort_by(|a, b| a.slug.cmp(&b.slug));
        PipelineState {
            project_root: root,
            has_constitution,
            features,
        }
    }
}

fn scan_feature(dir: &Path) -> ArtifactState {
    let mut state = ArtifactState {
        spec: dir.join("spec.md").is_file(),
        plan: dir.join("plan.md").is_file(),
        tasks: dir.join("tasks.md").is_file(),
        ..Default::default()
    };
    if state.tasks {
        if let Ok(content) = fs::read_to_string(dir.join("tasks.md")) {
            state.task_progress = count_tasks(&content);
        }
    }
    let checklists_dir = dir.join("checklists");
    if checklists_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&checklists_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        let (total, checked) = count_checkboxes(&content);
                        state.checklists.push(ChecklistState {
                            name: entry.file_name().to_string_lossy().into_owned(),
                            total,
                            checked,
                        });
                    }
                }
            }
        }
    }
    state
}

/// Count `- [ ]` / `- [x]` task items in tasks.md format (`- [ ] T001 ...`).
pub fn count_tasks(content: &str) -> (usize, usize) {
    let mut total = 0;
    let mut done = 0;
    for line in content.lines() {
        let t = line.trim_start();
        if t.starts_with("- [ ]") || t.starts_with("- [x]") || t.starts_with("- [X]") {
            total += 1;
            if !t.starts_with("- [ ]") {
                done += 1;
            }
        }
    }
    (total, done)
}

/// Count checklist `- [ ]` / `- [x]` items (same syntax; distinct fn for semantics).
pub fn count_checkboxes(content: &str) -> (usize, usize) {
    count_tasks(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_project() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".specify/memory")).unwrap();
        fs::write(root.join(".specify/memory/constitution.md"), "# Principles").unwrap();
        let feat = root.join("specs/001-photo-album");
        fs::create_dir_all(feat.join("checklists")).unwrap();
        fs::write(feat.join("spec.md"), "# Spec").unwrap();
        fs::write(feat.join("plan.md"), "# Plan").unwrap();
        fs::write(
            feat.join("tasks.md"),
            "# Tasks\n\n- [x] T001 setup\n- [ ] T002 implement\n- [ ] T003 test\n",
        )
        .unwrap();
        fs::write(
            feat.join("checklists/requirements.md"),
            "# Checklist\n\n- [x] Q1 clear?\n- [ ] Q2 complete?\n",
        )
        .unwrap();
        tmp
    }

    #[test]
    fn scan_derives_full_state() {
        let tmp = fixture_project();
        let state = PipelineState::scan(tmp.path());
        assert!(state.has_constitution);
        assert_eq!(state.features.len(), 1);
        let f = &state.features[0];
        assert_eq!(f.slug, "001-photo-album");
        assert!(f.artifacts.spec && f.artifacts.plan && f.artifacts.tasks);
        assert_eq!(f.artifacts.task_progress, (3, 1));
        assert_eq!(f.artifacts.checklists[0].checked, 1);
    }

    #[test]
    fn implement_gate_requires_nonempty_tasks() {
        let tmp = tempfile::tempdir().unwrap();
        let feat = tmp.path().join("specs/001-x");
        fs::create_dir_all(&feat).unwrap();
        fs::write(feat.join("spec.md"), "# Spec").unwrap();
        let state = PipelineState::scan(tmp.path());
        let f = &state.features[0];
        assert_eq!(f.implement_allowed(), Err(GateBlock::NoTasks));

        fs::write(feat.join("tasks.md"), "# Tasks, no items").unwrap();
        let state = PipelineState::scan(tmp.path());
        assert_eq!(state.features[0].implement_allowed(), Err(GateBlock::EmptyTasks));

        fs::write(feat.join("tasks.md"), "- [ ] T001 do thing\n").unwrap();
        let state = PipelineState::scan(tmp.path());
        assert!(state.features[0].implement_allowed().is_ok());
    }

    #[test]
    fn unchecked_checklist_items_counted() {
        let tmp = fixture_project();
        let state = PipelineState::scan(tmp.path());
        assert_eq!(state.features[0].unchecked_checklist_items(), 1);
    }

    #[test]
    fn empty_project_scans_clean() {
        let tmp = tempfile::tempdir().unwrap();
        let state = PipelineState::scan(tmp.path());
        assert!(!state.has_constitution);
        assert!(state.features.is_empty());
    }
}
