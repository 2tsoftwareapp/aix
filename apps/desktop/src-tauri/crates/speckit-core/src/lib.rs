//! speckit-core: vendored Spec Kit templates, artifact state machine, gate engine.
//!
//! The prompts ARE the spec: templates are loaded verbatim and hash-verified
//! against SHA256SUMS. Logic here enforces pipeline structure; it never
//! reimplements what the templates instruct the agent to do.

pub mod state;
pub mod steps;
pub mod templates;

pub use state::{ArtifactState, FeatureArtifacts, PipelineState};
pub use steps::Step;
pub use templates::{TemplateStore, TemplateStoreError};
