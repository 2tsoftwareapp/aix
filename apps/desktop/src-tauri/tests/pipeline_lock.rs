//! Feature-locking integration tests: the speckit pipeline contract end-to-end
//! across all four crates, driven through the real vendored templates.
//!
//! These tests encode the behavioral promises from PLAN.md — if any of them
//! breaks, a locked feature has regressed.

use speckit_core::gates::{evaluate, parse_converge_verdict, ConvergeVerdict, GateVerdict};
use speckit_core::runner::{ArtifactWrite, StepRunner};
use speckit_core::{PipelineState, Step, TemplateStore};
use std::path::{Path, PathBuf};

fn template_root() -> PathBuf {
    if let Ok(p) = std::env::var("AIX_SPECKIT_CONTENT") {
        return p.into();
    }
    // tests run with cwd = crate root (src-tauri)
    let rel = PathBuf::from("../../../content/speckit");
    if rel.is_dir() {
        return rel;
    }
    PathBuf::from("content/speckit")
}

fn store() -> TemplateStore {
    TemplateStore::load_verified(template_root()).expect("vendored templates must verify")
}

/// Write an artifact to the feature dir the way run_step does it.
fn write_artifact(feature_dir: &Path, rel: &str, content: &str) {
    let path = feature_dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn scan(root: &Path) -> PipelineState {
    PipelineState::scan(root.to_string_lossy().as_ref())
}

// ---------------------------------------------------------------------------
// LOCK 1 (TASK-05/06): full pipeline progression — gates track artifacts exactly
// ---------------------------------------------------------------------------
#[test]
fn lock_full_pipeline_progression() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    // Empty project: nothing scanned, implement/converge blocked, constitution open.
    let state = scan(root);
    assert!(state.features.is_empty());
    assert!(!state.has_constitution);
    assert!(matches!(evaluate(Step::Constitution, None, false), GateVerdict::Allow));
    assert!(matches!(evaluate(Step::Implement, None, false), GateVerdict::Block { .. }));
    // specify without constitution is a soft gate, not a block
    assert!(matches!(
        evaluate(Step::Specify, None, false),
        GateVerdict::RequireConfirmation { .. }
    ));

    // Constitution written → specify fully open
    std::fs::create_dir_all(root.join(".specify/memory")).unwrap();
    std::fs::write(root.join(".specify/memory/constitution.md"), "# Principles\n").unwrap();
    let state = scan(root);
    assert!(state.has_constitution);
    assert!(matches!(evaluate(Step::Specify, None, true), GateVerdict::Allow));

    // Feature created by specify
    let fdir = root.join("specs/001-test-feature");
    write_artifact(&fdir, "spec.md", "# Spec\n\n## Requirements\n");
    let state = scan(root);
    let f = state.features.iter().find(|f| f.slug == "001-test-feature").unwrap();
    assert!(f.artifacts.spec);
    // plan blocked? no — plan only requires spec; tasks requires plan
    assert!(matches!(evaluate(Step::Plan, Some(f), true), GateVerdict::Allow));
    assert!(matches!(evaluate(Step::Tasks, Some(f), true), GateVerdict::Block { .. }));
    assert!(matches!(evaluate(Step::Implement, Some(f), true), GateVerdict::Block { .. }));

    // plan written → tasks open, implement still blocked
    write_artifact(&fdir, "plan.md", "# Plan\n");
    let state = scan(root);
    let f = &state.features[0];
    assert!(matches!(evaluate(Step::Tasks, Some(f), true), GateVerdict::Allow));
    let implement_verdict = evaluate(Step::Implement, Some(f), true);
    assert!(implement_verdict.is_blocked(), "implement must block without tasks");

    // tasks written but empty of items → still blocked (EmptyTasks)
    write_artifact(&fdir, "tasks.md", "# Tasks\n\nnothing here yet\n");
    let state = scan(root);
    let f = &state.features[0];
    assert!(evaluate(Step::Implement, Some(f), true).is_blocked());

    // real tasks → implement unblocked (no checklists exist, so no soft gate)
    write_artifact(&fdir, "tasks.md", "# Tasks\n\n- [ ] T001 first\n- [ ] T002 second\n");
    let state = scan(root);
    let f = &state.features[0];
    assert_eq!(f.artifacts.task_progress, (2, 0));
    assert!(matches!(evaluate(Step::Implement, Some(f), true), GateVerdict::Allow));

    // converge blocked/soft while tasks incomplete
    assert!(matches!(
        evaluate(Step::Converge, Some(f), true),
        GateVerdict::RequireConfirmation { .. }
    ));

    // tasks all done → converge fully open
    write_artifact(&fdir, "tasks.md", "# Tasks\n\n- [x] T001 first\n- [x] T002 second\n");
    let state = scan(root);
    let f = &state.features[0];
    assert_eq!(f.artifacts.task_progress, (2, 2));
    assert!(matches!(evaluate(Step::Converge, Some(f), true), GateVerdict::Allow));
}

// ---------------------------------------------------------------------------
// LOCK 2 (TASK-19): reviewer-owned checkboxes end-to-end
// ---------------------------------------------------------------------------
#[test]
fn lock_reviewer_owned_checkboxes_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let fdir = root.join("specs/001-feat");
    write_artifact(&fdir, "spec.md", "# Spec\n");
    write_artifact(&fdir, "checklists/requirements.md", "# Checklist\n\n- [ ] requirement one\n- [ ] requirement two\n");

    // Agent tries to approve items in prose during a checklist-step write — stripped.
    let agent_output = "# Checklist\n\n- [x] requirement one\n- [x] requirement two\n";
    let writes = vec![ArtifactWrite {
        rel_path: "checklists/requirements.md".into(),
        content: agent_output.into(),
    }];
    let accepted = StepRunner::sanitize_writes(Step::Checklist, writes).unwrap();
    assert!(
        accepted[0].content.contains("- [ ] requirement one"),
        "agent checkbox approvals must be stripped, got: {}",
        accepted[0].content
    );

    // Reviewer flips through the dedicated primitive — persists.
    let flipped = speckit_core::runner::flip_checkbox(&accepted[0].content, 2, true).unwrap();
    write_artifact(&fdir, "checklists/requirements.md", &flipped);

    // State machine now sees exactly one checked of two → implement soft-gates.
    write_artifact(&fdir, "plan.md", "# Plan\n");
    write_artifact(&fdir, "tasks.md", "# Tasks\n\n- [ ] T001 x\n");
    let state = scan(root);
    let f = &state.features[0];
    assert_eq!(f.unchecked_checklist_items(), 1);
    assert!(matches!(
        evaluate(Step::Implement, Some(f), true),
        GateVerdict::RequireConfirmation { .. }
    ));

    // flip_checkbox rejects non-checkbox lines — no accidental corruption
    assert!(speckit_core::runner::flip_checkbox(&flipped, 0, true).is_err());
}

// ---------------------------------------------------------------------------
// LOCK 3 (TASK-07): prompt assembly uses verbatim vendored template bytes
// ---------------------------------------------------------------------------
#[test]
fn lock_prompt_assembly_verbatim() {
    let store = store();
    let runner = StepRunner::new(&store);
    let bundle = runner
        .prepare(Step::Specify, "build a thing", vec![])
        .unwrap();
    let on_disk = std::fs::read_to_string(template_root().join("templates/commands/specify.md"))
        .expect("upstream template on disk");
    // the template the model sees must be byte-identical to the vendored upstream file —
    // no normalization, no injection, no "improvements"
    assert_eq!(
        bundle.template.trim_end(),
        on_disk.trim_end(),
        "prompt template diverges from vendored upstream"
    );
    assert!(bundle.user_input.contains("build a thing"));
}

// ---------------------------------------------------------------------------
// LOCK 4 (TASK-06): analyze is read-only by construction across the runner
// ---------------------------------------------------------------------------
#[test]
fn lock_analyze_readonly_construction() {
    // any write at all is rejected for analyze — even in-feature paths
    for rel in ["spec.md", "plan.md", "tasks.md", "checklists/x.md", "anything.md"] {
        let writes = vec![ArtifactWrite {
            rel_path: rel.into(),
            content: "x".into(),
        }];
        assert!(
            StepRunner::sanitize_writes(Step::Analyze, writes).is_err(),
            "analyze must never write {rel}"
        );
    }
    // and analyze has no primary artifact
    assert_eq!(StepRunner::primary_artifact(Step::Analyze), None);
}

// ---------------------------------------------------------------------------
// LOCK 5: converge termination only on explicit verdict
// ---------------------------------------------------------------------------
#[test]
fn lock_converge_verdict_contract() {
    assert_eq!(parse_converge_verdict("Converged"), ConvergeVerdict::Converged);
    assert_eq!(parse_converge_verdict("some output\nConverged\n"), ConvergeVerdict::Converged);
    assert_eq!(parse_converge_verdict("Remaining: 3"), ConvergeVerdict::Remaining(3));
    // vague success language is NOT convergence — loop must continue
    assert_eq!(parse_converge_verdict("everything looks good!"), ConvergeVerdict::Unknown);
    assert_eq!(parse_converge_verdict("converged-ish results"), ConvergeVerdict::Unknown);
}

// ---------------------------------------------------------------------------
// LOCK 6 (TASK-13/14): executor enforcement through the agent loop — the
// implement-step contract: tools execute scoped, denials feed back as errors
// ---------------------------------------------------------------------------
#[tokio::test]
async fn lock_implement_loop_executes_scoped_tools() {
    use agent_loop::{run, LoopConfig, ToolExecutor};
    use async_trait::async_trait;
    use futures_util::stream;
    use providers::{
        CompletionRequest, EventStream, Provider, ProviderError, StreamEvent, ToolCall, ToolResult,
    };
    use std::sync::Mutex;

    struct OneShot {
        turns: Mutex<Vec<Vec<StreamEvent>>>,
    }
    #[async_trait]
    impl Provider for OneShot {
        fn id(&self) -> &'static str {
            "oneshot"
        }
        fn display_name(&self) -> &'static str {
            "oneshot"
        }
        async fn complete(&self, _req: CompletionRequest) -> Result<EventStream, ProviderError> {
            let t = self.turns.lock().unwrap().pop().unwrap_or_default();
            Ok(Box::pin(stream::iter(t.into_iter().map(Ok))))
        }
        async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
            Ok(vec![])
        }
    }

    // Executor backed by the real executor crate, no approval friction (full-auto semantics)
    struct RealExecutor {
        root: PathBuf,
    }
    #[async_trait]
    impl ToolExecutor for RealExecutor {
        async fn execute(&self, call: &ToolCall) -> ToolResult {
            match executor::execute_tool(&self.root, &call.name, &call.input, &executor::default_denylist()).await {
                Ok(out) => ToolResult {
                    tool_call_id: call.id.clone(),
                    content: out,
                    is_error: false,
                },
                Err(e) => ToolResult {
                    tool_call_id: call.id.clone(),
                    content: format!("error: {e}"),
                    is_error: true,
                },
            }
        }
        fn describe(&self, call: &ToolCall) -> String {
            call.name.clone()
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();

    let provider = OneShot {
        turns: Mutex::new(vec![
            // first turn: write a file, attempt an escape, run an allowed shell command
            vec![
                StreamEvent::ToolUse {
                    call: ToolCall {
                        id: "t1".into(),
                        name: "write_file".into(),
                        input: serde_json::json!({"path": "src/main.rs", "content": "fn main() {}"}),
                    },
                },
                StreamEvent::ToolUse {
                    call: ToolCall {
                        id: "t2".into(),
                        name: "write_file".into(),
                        input: serde_json::json!({"path": "../escape.rs", "content": "pwned"}),
                    },
                },
                StreamEvent::ToolUse {
                    call: ToolCall {
                        id: "t3".into(),
                        name: "run_shell".into(),
                        input: serde_json::json!({"command": "echo loop-ok"}),
                    },
                },
                StreamEvent::Done,
            ],
            // second turn: finish
            vec![StreamEvent::TextDelta { text: "implementation complete".into() }, StreamEvent::Done],
        ]
        .into_iter()
        .rev()
        .collect()),
    };

    let config = LoopConfig {
        model: "m".into(),
        system: "s".into(),
        tools: executor::tool_defs(),
        max_tokens: 4096,
        max_iterations: 10,
    };
    let ex = RealExecutor { root: root.clone() };
    let (text, transcript) = run(&provider, &config, "implement".into(), &ex, |_| {}, tokio_util::sync::CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(text, "implementation complete");
    // the legitimate write landed inside the project
    assert_eq!(std::fs::read_to_string(root.join("src/main.rs")).unwrap(), "fn main() {}");
    // the escape never happened — and the model was told via tool_result error
    assert!(!root.parent().unwrap().join("escape.rs").exists());
    let results_msg = transcript.iter().find(|m| !m.tool_results.is_empty()).unwrap();
    let escape_result = results_msg.tool_results.iter().find(|r| r.tool_call_id == "t2").unwrap();
    assert!(escape_result.is_error, "escape must report error to the model");
    // shell executed and output captured
    let shell_result = results_msg.tool_results.iter().find(|r| r.tool_call_id == "t3").unwrap();
    assert!(shell_result.content.contains("loop-ok"));
}

// ---------------------------------------------------------------------------
// LOCK 7 (TASK-04): template store fails closed on tampering
// ---------------------------------------------------------------------------
#[test]
fn lock_template_tampering_fails_closed() {
    // copy vendored tree, corrupt one byte, verification must fail
    let tmp = tempfile::tempdir().unwrap();
    let dst = tmp.path().join("speckit");
    copy_dir(&template_root(), &dst);
    let victim = dst.join("templates/commands/specify.md");
    let mut content = std::fs::read_to_string(&victim).unwrap();
    content.push_str("tampered");
    std::fs::write(&victim, content).unwrap();
    assert!(
        TemplateStore::load_verified(&dst).is_err(),
        "tampered template must fail verification"
    );

    // missing file must also fail
    let tmp2 = tempfile::tempdir().unwrap();
    let dst2 = tmp2.path().join("speckit");
    copy_dir(&template_root(), &dst2);
    std::fs::remove_file(dst2.join("templates/commands/implement.md")).unwrap();
    assert!(TemplateStore::load_verified(&dst2).is_err());
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
