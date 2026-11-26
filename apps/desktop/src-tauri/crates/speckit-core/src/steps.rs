use serde::{Deserialize, Serialize};

/// The 9 SDD pipeline steps, in canonical order.
/// Quality gates (clarify, checklist, analyze) are optional but ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    Constitution,
    Specify,
    Clarify,
    Plan,
    Checklist,
    Tasks,
    Analyze,
    Implement,
    Converge,
}

impl Step {
    pub const ALL: [Step; 9] = [
        Step::Constitution,
        Step::Specify,
        Step::Clarify,
        Step::Plan,
        Step::Checklist,
        Step::Tasks,
        Step::Analyze,
        Step::Implement,
        Step::Converge,
    ];

    /// Template file name under templates/commands/
    pub fn template_file(&self) -> &'static str {
        match self {
            Step::Constitution => "constitution.md",
            Step::Specify => "specify.md",
            Step::Clarify => "clarify.md",
            Step::Plan => "plan.md",
            Step::Checklist => "checklist.md",
            Step::Tasks => "tasks.md",
            Step::Analyze => "analyze.md",
            Step::Implement => "implement.md",
            Step::Converge => "converge.md",
        }
    }

    pub fn is_quality_gate(&self) -> bool {
        matches!(self, Step::Clarify | Step::Checklist | Step::Analyze)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_steps_have_templates() {
        for step in Step::ALL {
            assert!(!step.template_file().is_empty());
        }
    }

    #[test]
    fn specify_precedes_plan_precedes_tasks_precedes_implement() {
        assert!(Step::Specify < Step::Plan);
        assert!(Step::Plan < Step::Tasks);
        assert!(Step::Tasks < Step::Implement);
    }
}
