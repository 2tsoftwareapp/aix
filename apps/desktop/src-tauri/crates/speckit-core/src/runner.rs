//! Step runner: assembles prompts from verbatim templates + artifact context,
//! and sanitizes the agent's artifact writes against structural rules.
//!
//! The runner never improvises process logic: the template body IS the instruction
//! set, passed verbatim to the model. What the runner owns is *enforcement* —
//! which writes are allowed to land on disk.

use crate::steps::Step;
use crate::templates::{TemplateStore, TemplateStoreError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error(transparent)]
    Templates(#[from] TemplateStoreError),
    #[error("write rejected: {0}")]
    WriteRejected(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptBundle {
    pub step: Step,
    /// Verbatim speckit command template — the authoritative instructions.
    pub template: String,
    /// User input for this step (feature description, tech stack, scope...).
    pub user_input: String,
    /// Artifact context snapshot (paths + contents the template operates on).
    pub context: Vec<ArtifactContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactContext {
    pub rel_path: String,
    pub content: String,
}

/// One file the agent wants to write, relative to the feature dir
/// (or project root for constitution).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactWrite {
    pub rel_path: String,
    pub content: String,
}

pub struct StepRunner<'a> {
    store: &'a TemplateStore,
}

impl<'a> StepRunner<'a> {
    pub fn new(store: &'a TemplateStore) -> Self {
        Self { store }
    }

    /// Build the prompt for one step execution.
    pub fn prepare(
        &self,
        step: Step,
        user_input: &str,
        context: Vec<ArtifactContext>,
    ) -> Result<PromptBundle, RunnerError> {
        let template = self.store.command_template(step)?.to_string();
        Ok(PromptBundle {
            step,
            template,
            user_input: user_input.to_string(),
            context,
        })
    }

    /// The primary artifact path a step produces, relative to the feature dir.
    pub fn primary_artifact(step: Step) -> Option<&'static str> {
        match step {
            Step::Constitution => Some(".specify/memory/constitution.md"),
            Step::Specify => Some("spec.md"),
            Step::Plan => Some("plan.md"),
            Step::Tasks => Some("tasks.md"),
            // clarify rewrites spec.md with clarifications encoded (single-shot mode:
            // the model outputs the full updated artifact)
            Step::Clarify => Some("spec.md"),
            // analyze/converge report rather than produce one artifact
            _ => None,
        }
    }

    /// Enforcement boundary for agent writes. Returns the accepted writes.
    ///
    /// Rules:
    /// - analyze: read-only — every write is rejected.
    /// - checklists/*.md: agent may create/update checklist CONTENT, but may not
    ///   flip checkbox state. Any `[x]` introduced by the agent on a previously
    ///   unchecked line is reverted — checkboxes are reviewer-owned.
    /// - everything is confined to the feature dir or .specify/ (no escapes).
    pub fn sanitize_writes(
        step: Step,
        writes: Vec<ArtifactWrite>,
    ) -> Result<Vec<ArtifactWrite>, RunnerError> {
        if step == Step::Analyze {
            if writes.is_empty() {
                return Ok(vec![]);
            }
            return Err(RunnerError::WriteRejected(
                "analyze is read-only — report only, no file writes".into(),
            ));
        }
        let mut accepted = Vec::new();
        for w in writes {
            if w.rel_path.contains("..") || w.rel_path.starts_with('/') {
                return Err(RunnerError::WriteRejected(format!(
                    "path escapes workspace: {}",
                    w.rel_path
                )));
            }
            if w.rel_path.starts_with("checklists/") {
                let content = strip_agent_checkbox_approvals(&w.content);
                accepted.push(ArtifactWrite {
                    rel_path: w.rel_path,
                    content,
                });
            } else {
                accepted.push(w);
            }
        }
        Ok(accepted)
    }
}

/// Revert `- [x]` to `- [ ]` everywhere in agent-written checklist content.
/// Crude by design: the reviewer flips boxes through the dedicated UI action,
/// never through agent prose. (Reviewer flips use `flip_checkbox` below.)
pub fn strip_agent_checkbox_approvals(content: &str) -> String {
    content
        .lines()
        .map(|line| {
            let t = line.trim_start();
            if t.starts_with("- [x]") || t.starts_with("- [X]") {
                let indent = &line[..line.len() - t.len()];
                format!("{}- [ ]{}", indent, &t[5..])
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Reviewer-only checkbox flip. Operates on exact line index; fails if the line
/// is not a checkbox item.
pub fn flip_checkbox(content: &str, line_index: usize, checked: bool) -> Result<String, RunnerError> {
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let Some(line) = lines.get_mut(line_index) else {
        return Err(RunnerError::WriteRejected(format!(
            "line {line_index} out of range"
        )));
    };
    let t = line.trim_start();
    let is_box = t.starts_with("- [ ]") || t.starts_with("- [x]") || t.starts_with("- [X]");
    if !is_box {
        return Err(RunnerError::WriteRejected(format!(
            "line {line_index} is not a checkbox item"
        )));
    }
    let indent_len = line.len() - t.len();
    let mark = if checked { "- [x]" } else { "- [ ]" };
    *line = format!("{}{}{}", &line[..indent_len], mark, &t[5..]);
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_writes_rejected() {
        let writes = vec![ArtifactWrite {
            rel_path: "analysis.md".into(),
            content: "x".into(),
        }];
        assert!(StepRunner::sanitize_writes(Step::Analyze, writes).is_err());
        assert!(StepRunner::sanitize_writes(Step::Analyze, vec![]).is_ok());
    }

    #[test]
    fn escape_writes_rejected() {
        let writes = vec![ArtifactWrite {
            rel_path: "../evil.md".into(),
            content: "x".into(),
        }];
        assert!(StepRunner::sanitize_writes(Step::Specify, writes).is_err());
    }

    #[test]
    fn agent_checkbox_approvals_stripped() {
        let input = "- [x] done thing\n- [ ] open thing\n  - [X] nested done\n";
        let out = strip_agent_checkbox_approvals(input);
        assert!(out.contains("- [ ] done thing"));
        assert!(out.contains("- [ ] open thing"));
        assert!(out.contains("  - [ ] nested done"));
    }

    #[test]
    fn reviewer_can_flip_checkbox() {
        let input = "- [ ] first\n- [ ] second\n";
        let out = flip_checkbox(input, 1, true).unwrap();
        assert!(out.lines().nth(1).unwrap().starts_with("- [x]"));
        // non-checkbox line rejected
        let prose = "just text\n";
        assert!(flip_checkbox(prose, 0, true).is_err());
    }

    #[test]
    fn primary_artifacts_mapped() {
        assert_eq!(StepRunner::primary_artifact(Step::Specify), Some("spec.md"));
        assert_eq!(StepRunner::primary_artifact(Step::Implement), None);
    }
}

#[cfg(test)]
mod golden_tests {
    use super::*;
    use crate::templates::TemplateStore;
    use std::path::PathBuf;

    fn store() -> TemplateStore {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for _ in 0..3 {
            p.pop();
        }
        TemplateStore::load_verified(p.join("speckit-templates")).unwrap()
    }

    #[test]
    fn prepare_assembles_verbatim_template_and_input() {
        let s = store();
        let runner = StepRunner::new(&s);
        let bundle = runner
            .prepare(
                Step::Specify,
                "Build a photo organizer with albums grouped by date.",
                vec![ArtifactContext {
                    rel_path: ".specify/memory/constitution.md".into(),
                    content: "# Principles".into(),
                }],
            )
            .unwrap();
        // template is the verbatim upstream instructions
        assert!(bundle.template.contains("feature specification"));
        assert!(bundle.user_input.contains("photo organizer"));
        assert_eq!(bundle.context.len(), 1);
        // template is byte-identical to the vendored file
        let raw = s.command_template(Step::Specify).unwrap();
        assert_eq!(bundle.template, raw);
    }
}
