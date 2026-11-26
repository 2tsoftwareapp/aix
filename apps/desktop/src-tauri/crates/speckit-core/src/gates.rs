//! Gate engine: enforcement rules the app adds over raw speckit.
//!
//! These are structural rules — the LLM is never trusted to enforce them itself.
//! - implement requires non-empty tasks.md (hard block)
//! - unchecked custom checklist items require explicit user confirmation (soft gate)
//! - analyze is read-only by construction (no write tools issued — see runner)
//! - checklist checkboxes are reviewer-owned (agent writes never toggle them — see writes)
//! - converge terminates the implement/converge loop only on an explicit verdict

use crate::state::FeatureArtifacts;
use crate::steps::Step;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GateVerdict {
    Allow,
    /// Soft gate: user must confirm before proceeding. Carries the reason.
    RequireConfirmation { reason: String },
    /// Hard block: step cannot run, period.
    Block { reason: String },
}

impl GateVerdict {
    pub fn is_blocked(&self) -> bool {
        matches!(self, GateVerdict::Block { .. })
    }
}

/// Evaluate whether `step` may run against the current feature artifacts.
pub fn evaluate(step: Step, feature: Option<&FeatureArtifacts>, has_constitution: bool) -> GateVerdict {
    match step {
        Step::Constitution => GateVerdict::Allow,

        Step::Specify => {
            if !has_constitution {
                GateVerdict::RequireConfirmation {
                    reason: "no project constitution yet — specify works better after /speckit.constitution".into(),
                }
            } else {
                GateVerdict::Allow
            }
        }

        Step::Clarify | Step::Plan | Step::Checklist => {
            let Some(f) = feature else {
                return GateVerdict::Block {
                    reason: format!("no feature selected — run /speckit.specify first"),
                };
            };
            if !f.artifacts.spec {
                return GateVerdict::Block {
                    reason: format!("spec.md missing — {step:?} requires a specification"),
                };
            }
            GateVerdict::Allow
        }

        Step::Tasks => {
            let Some(f) = feature else {
                return GateVerdict::Block {
                    reason: "no feature selected — run /speckit.specify first".into(),
                };
            };
            if !f.artifacts.plan {
                return GateVerdict::Block {
                    reason: "plan.md missing — tasks derive from the technical plan".into(),
                };
            }
            GateVerdict::Allow
        }

        Step::Analyze => {
            let Some(f) = feature else {
                return GateVerdict::Block {
                    reason: "no feature selected".into(),
                };
            };
            if !(f.artifacts.spec && f.artifacts.plan && f.artifacts.tasks) {
                return GateVerdict::RequireConfirmation {
                    reason: "analyze is most useful with spec+plan+tasks all present".into(),
                };
            }
            GateVerdict::Allow
        }

        Step::Implement => {
            let Some(f) = feature else {
                return GateVerdict::Block {
                    reason: "no feature selected".into(),
                };
            };
            if let Err(block) = f.implement_allowed() {
                return GateVerdict::Block {
                    reason: match block {
                        crate::state::GateBlock::NoTasks => {
                            "tasks.md missing — run /speckit.tasks first".into()
                        }
                        crate::state::GateBlock::EmptyTasks => {
                            "tasks.md has no task items".into()
                        }
                    },
                };
            }
            let unchecked = f.unchecked_checklist_items();
            if unchecked > 0 {
                return GateVerdict::RequireConfirmation {
                    reason: format!(
                        "{unchecked} unchecked checklist item(s) — reviewer approval is your call, not the agent's"
                    ),
                };
            }
            GateVerdict::Allow
        }

        Step::Converge => {
            let Some(f) = feature else {
                return GateVerdict::Block {
                    reason: "no feature selected".into(),
                };
            };
            let (total, done) = f.artifacts.task_progress;
            if total == 0 {
                return GateVerdict::Block {
                    reason: "no tasks to converge against".into(),
                };
            }
            if done < total {
                return GateVerdict::RequireConfirmation {
                    reason: format!("{} of {} tasks still unchecked", total - done, total),
                };
            }
            GateVerdict::Allow
        }
    }
}

/// Converge verdict parsed from agent output — loop ends only on explicit "Converged".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvergeVerdict {
    Converged,
    Remaining(usize),
    Unknown,
}

pub fn parse_converge_verdict(output: &str) -> ConvergeVerdict {
    for line in output.lines() {
        let t = line.trim();
        if t.eq_ignore_ascii_case("converged") || t.starts_with("**Converged**") {
            return ConvergeVerdict::Converged;
        }
        if let Some(rest) = t.strip_prefix("Remaining:") {
            if let Ok(n) = rest.trim().parse::<usize>() {
                return ConvergeVerdict::Remaining(n);
            }
        }
    }
    ConvergeVerdict::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ArtifactState, ChecklistState};
    use std::path::PathBuf;

    fn feature(artifacts: ArtifactState) -> FeatureArtifacts {
        FeatureArtifacts {
            slug: "001-x".into(),
            dir: PathBuf::from("/tmp/x"),
            artifacts,
        }
    }

    fn full_artifacts() -> ArtifactState {
        ArtifactState {
            spec: true,
            plan: true,
            tasks: true,
            checklists: vec![],
            task_progress: (3, 3),
        }
    }

    #[test]
    fn implement_blocked_without_tasks() {
        let f = feature(ArtifactState {
            spec: true,
            plan: true,
            ..Default::default()
        });
        let v = evaluate(Step::Implement, Some(&f), true);
        assert!(v.is_blocked());
    }

    #[test]
    fn implement_soft_gate_on_unchecked_checklist() {
        let mut a = full_artifacts();
        a.checklists.push(ChecklistState {
            name: "requirements.md".into(),
            total: 3,
            checked: 1,
        });
        let f = feature(a);
        let v = evaluate(Step::Implement, Some(&f), true);
        assert!(matches!(v, GateVerdict::RequireConfirmation { .. }));
    }

    #[test]
    fn implement_allowed_when_clean() {
        let f = feature(full_artifacts());
        assert_eq!(evaluate(Step::Implement, Some(&f), true), GateVerdict::Allow);
    }

    #[test]
    fn plan_blocked_without_spec() {
        let f = feature(ArtifactState::default());
        assert!(evaluate(Step::Plan, Some(&f), true).is_blocked());
    }

    #[test]
    fn specify_soft_without_constitution() {
        assert!(matches!(
            evaluate(Step::Specify, None, false),
            GateVerdict::RequireConfirmation { .. }
        ));
        assert_eq!(evaluate(Step::Specify, None, true), GateVerdict::Allow);
    }

    #[test]
    fn converge_requires_all_tasks_done() {
        let mut a = full_artifacts();
        a.task_progress = (3, 1);
        let f = feature(a);
        assert!(matches!(
            evaluate(Step::Converge, Some(&f), true),
            GateVerdict::RequireConfirmation { .. }
        ));
    }

    #[test]
    fn converge_verdict_parsing() {
        assert_eq!(parse_converge_verdict("Converged"), ConvergeVerdict::Converged);
        assert_eq!(
            parse_converge_verdict("**Converged** — all checks pass"),
            ConvergeVerdict::Converged
        );
        assert_eq!(
            parse_converge_verdict("Remaining: 4"),
            ConvergeVerdict::Remaining(4)
        );
        assert_eq!(parse_converge_verdict("unclear output"), ConvergeVerdict::Unknown);
    }
}
