//! Tauri command surface: the bridge between UI and the speckit machine.
//!
//! run_step flow: gate evaluation → context assembly → verbatim template prompt
//! → provider streaming (relayed as Tauri events) → write sanitization → disk.

use providers::live::{AnthropicProvider, OpenAiCompatProvider};
use providers::{Credential, Provider, ToolResult};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;
use serde::{Deserialize, Serialize};
use speckit_core::gates::{evaluate, GateVerdict};
use speckit_core::runner::{ArtifactContext, StepRunner};
use speckit_core::{PipelineState, Step, TemplateStore};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Serialize)]
pub struct GateInfo {
    pub allowed: bool,
    pub needs_confirmation: bool,
    pub reason: Option<String>,
}

#[tauri::command]
pub fn scan_pipeline(project_root: String) -> Result<PipelineState, String> {
    Ok(PipelineState::scan(&project_root))
}

#[tauri::command]
pub fn check_gate(
    step: String,
    project_root: String,
    feature_slug: Option<String>,
) -> Result<GateInfo, String> {
    let step = parse_step(&step)?;
    let state = PipelineState::scan(&project_root);
    let feature = feature_or_active(&state, feature_slug.as_deref());
    Ok(match evaluate(step, feature, state.has_constitution) {
        GateVerdict::Allow => GateInfo {
            allowed: true,
            needs_confirmation: false,
            reason: None,
        },
        GateVerdict::RequireConfirmation { reason } => GateInfo {
            allowed: true,
            needs_confirmation: true,
            reason: Some(reason),
        },
        GateVerdict::Block { reason } => GateInfo {
            allowed: false,
            needs_confirmation: false,
            reason: Some(reason),
        },
    })
}

#[tauri::command]
pub fn store_provider_key(provider_id: String, key: String) -> Result<String, String> {
    providers::credentials::store(&provider_id, &key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn has_provider_key(provider_id: String) -> bool {
    providers::credentials::retrieve(&provider_id).is_ok()
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StepStreamPayload {
    Delta { text: String },
    ToolUse { name: String },
    ToolResult { name: String, ok: bool, detail: String },
    ArtifactWritten { path: String },
    GateBlocked { reason: String },
    Done,
    Error { message: String },
}

/// Pending approval requests: id → resolver channel. The agent loop parks on
/// the oneshot until the UI calls respond_approval.
#[derive(Default)]
pub struct ApprovalRegistry {
    pub pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

/// Token for the currently-running step, if any. One run at a time —
/// starting a new run replaces (and implicitly abandons) the old token.
#[derive(Default)]
pub struct RunCancellation {
    pub current: Mutex<Option<tokio_util::sync::CancellationToken>>,
}

#[tauri::command]
pub fn cancel_run(registry: tauri::State<'_, RunCancellation>) -> Result<(), String> {
    let token = registry.current.lock().map_err(|e| e.to_string())?.clone();
    match token {
        Some(t) => {
            t.cancel();
            Ok(())
        }
        None => Err("no run in progress".into()),
    }
}

#[tauri::command]
pub fn respond_approval(
    registry: tauri::State<'_, ApprovalRegistry>,
    approval_id: String,
    approved: bool,
) -> Result<(), String> {
    let tx = registry
        .pending
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&approval_id);
    match tx {
        Some(tx) => tx.send(approved).map_err(|_| "loop already gone".to_string()),
        None => Err(format!("no pending approval: {approval_id}")),
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalRequestPayload {
    Request {
        approval_id: String,
        tool: String,
        detail: String,
        /// Content preview for write_file (first 2000 chars) so reviewers
        /// never approve blind. None for other tools.
        preview: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
pub struct RunStepInput {
    pub step: String,
    pub project_root: String,
    pub feature_slug: Option<String>,
    pub user_input: String,
    pub provider_id: String,
    pub model: String,
    /// "ask" | "auto-edit" | "full-auto"; defaults to ask.
    pub approval_mode: Option<String>,
    pub confirmed: Option<bool>,
}

fn template_root() -> PathBuf {
    if let Ok(p) = std::env::var("AIX_SPECKIT_CONTENT") {
        return p.into();
    }
    // dev fallback: repo content/speckit relative to src-tauri
    let dev = PathBuf::from("../../../content/speckit");
    if dev.is_dir() {
        return dev;
    }
    // absolute fallback for running from repo root
    PathBuf::from("content/speckit")
}

fn parse_step(s: &str) -> Result<Step, String> {
    for step in Step::ALL {
        if format!("{step:?}").to_lowercase() == s.to_lowercase() {
            return Ok(step);
        }
    }
    Err(format!("unknown step: {s}"))
}

fn feature_or_active<'a>(
    state: &'a PipelineState,
    slug: Option<&str>,
) -> Option<&'a speckit_core::FeatureArtifacts> {
    match slug {
        Some(s) => state.features.iter().find(|f| f.slug == s),
        None => state.features.last(),
    }
}

fn make_provider(provider_id: &str) -> Result<Box<dyn Provider>, String> {
    let key = providers::credentials::retrieve(provider_id)
        .map_err(|e| format!("no API key stored for {provider_id}: {e}"))?;
    Ok(match provider_id {
        "anthropic" => Box::new(AnthropicProvider::new(key)),
        "openai" => Box::new(OpenAiCompatProvider::openai(key)),
        "openrouter" => Box::new(OpenAiCompatProvider::openrouter(key)),
        "qwen" => Box::new(OpenAiCompatProvider::qwen(key)),
        other => return Err(format!("unknown provider: {other}")),
    })
}

/// Per-artifact context budget (~25k tokens at 4 chars/token). Oversized artifacts
/// are truncated with an explicit marker so the model knows content is missing —
/// silent truncation here would corrupt downstream artifacts.
const ARTIFACT_CONTEXT_BUDGET: usize = 100_000;

fn read_context(path: &Path, rel: &str) -> Option<ArtifactContext> {
    let full = path.join(rel);
    std::fs::read_to_string(full).ok().map(|content| ArtifactContext {
        rel_path: rel.to_string(),
        content: if content.len() > ARTIFACT_CONTEXT_BUDGET {
            format!(
                "{}\n\n[TRUNCATED: artifact is {} chars, budget {} — read the file directly if sections are missing]",
                &content[..ARTIFACT_CONTEXT_BUDGET],
                content.len(),
                ARTIFACT_CONTEXT_BUDGET
            )
        } else {
            content
        },
    })
}

fn slugify(text: &str) -> String {
    let mut out = String::new();
    for word in text.split_whitespace().take(4) {
        let clean: String = word
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>()
            .to_lowercase();
        if !clean.is_empty() {
            if !out.is_empty() {
                out.push('-');
            }
            out.push_str(&clean);
        }
    }
    if out.is_empty() {
        "feature".into()
    } else {
        out
    }
}

fn next_feature_dir(project_root: &Path, user_input: &str) -> PathBuf {
    let specs = project_root.join("specs");
    let mut max_n = 0u32;
    if let Ok(entries) = std::fs::read_dir(&specs) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(num) = name.get(..3).and_then(|s| s.parse::<u32>().ok()) {
                max_n = max_n.max(num);
            }
        }
    }
    specs.join(format!("{:03}-{}", max_n + 1, slugify(user_input)))
}

const STREAM_EVENT: &str = "step-stream";

#[tauri::command]
pub async fn run_step(
    app: AppHandle,
    registry: tauri::State<'_, ApprovalRegistry>,
    cancellation: tauri::State<'_, RunCancellation>,
    input: RunStepInput,
) -> Result<(), String> {
    let cancel_token = tokio_util::sync::CancellationToken::new();
    cancellation
        .current
        .lock()
        .map_err(|e| e.to_string())?
        .replace(cancel_token.clone());
    let emit = |p: StepStreamPayload| {
        let _ = app.emit(STREAM_EVENT, p);
    };

    let step = parse_step(&input.step)?;
    let project_root = PathBuf::from(&input.project_root);

    // Gate evaluation — structural enforcement before anything else.
    let state = PipelineState::scan(&project_root);
    let feature = feature_or_active(&state, input.feature_slug.as_deref());
    match evaluate(step, feature, state.has_constitution) {
        GateVerdict::Block { reason } => {
            emit(StepStreamPayload::GateBlocked { reason });
            return Ok(());
        }
        GateVerdict::RequireConfirmation { reason } if !input.confirmed.unwrap_or(false) => {
            emit(StepStreamPayload::GateBlocked {
                reason: format!("confirmation required — {reason}"),
            });
            return Ok(());
        }
        _ => {}
    }

    // Resolve feature dir. Specify always creates a NEW feature dir — upstream
    // spec-kit behavior: each specification is a new numbered feature. Editing an
    // existing spec is done via the artifact editor or clarify, never by re-specifying.
    let feature_dir = if step == Step::Specify {
        next_feature_dir(&project_root, &input.user_input)
    } else {
        match feature {
            Some(f) => f.dir.clone(),
            None => project_root.clone(),
        }
    };

    // Assemble artifact context.
    let mut context = Vec::new();
    for rel in [
        ".specify/memory/constitution.md",
        "spec.md",
        "plan.md",
        "tasks.md",
    ] {
        let base = if rel.starts_with(".specify") {
            project_root.clone()
        } else {
            feature_dir.clone()
        };
        if let Some(ctx) = read_context(&base, rel) {
            context.push(ctx);
        }
    }

    // Verbatim template prompt.
    let store = TemplateStore::load_verified(template_root()).map_err(|e| e.to_string())?;
    let runner = StepRunner::new(&store);
    let bundle = runner
        .prepare(step, &input.user_input, context)
        .map_err(|e| e.to_string())?;

    let system = format!(
        "You are executing the Spec Kit step `/speckit.{step}` strictly according to these instructions. \
         Follow them exactly; do not improvise process steps.\n\n{template}\n\n\
         IMPORTANT OUTPUT CONTRACT: Output ONLY the final artifact content in markdown, \
         or for analysis/report steps, the report. Do not narrate your process.",
        step = input.step.to_lowercase(),
        template = bundle.template,
    );
    let mut user_msg = bundle.user_input.clone();
    for ctx in &bundle.context {
        user_msg.push_str(&format!(
            "\n\n--- Current artifact `{}` ---\n{}",
            ctx.rel_path, ctx.content
        ));
    }

    let provider = make_provider(&input.provider_id)?;
    let mode = match input.approval_mode.as_deref() {
        Some("full-auto") => executor::ApprovalMode::FullAuto,
        Some("auto-edit") => executor::ApprovalMode::AutoEdit,
        _ => executor::ApprovalMode::Ask,
    };
    let denylist = executor::default_denylist();
    // Implement is the execution step — it gets file/shell tools and the agent loop.
    // Artifact steps stay single-shot: their output IS the artifact.
    let use_tools = step == Step::Implement;
    let tools = if use_tools { executor::tool_defs() } else { vec![] };

    struct TauriExecutor<'a> {
        app: &'a AppHandle,
        registry: &'a ApprovalRegistry,
        project_root: &'a Path,
        mode: executor::ApprovalMode,
        denylist: &'a [String],
    }

    #[async_trait::async_trait]
    impl agent_loop::ToolExecutor for TauriExecutor<'_> {
        async fn execute(&self, call: &providers::ToolCall) -> ToolResult {
            let detail = summarize_tool_call(call);
            let approved = if executor::tool_requires_approval(&call.name, self.mode) {
                let approval_id = uuid_simple();
                let (tx, rx) = oneshot::channel::<bool>();
                match self.registry.pending.lock() {
                    Ok(mut pending) => {
                        pending.insert(approval_id.clone(), tx);
                    }
                    Err(_) => {
                        return ToolResult {
                            tool_call_id: call.id.clone(),
                            content: "approval registry poisoned".into(),
                            is_error: true,
                        };
                    }
                }
                let preview = if call.name == "write_file" {
                    call.input
                        .get("content")
                        .and_then(serde_json::Value::as_str)
                        .map(|c| write_preview(self.project_root, call.input.get("path").and_then(serde_json::Value::as_str).unwrap_or(""), c))
                } else {
                    None
                };
                let _ = self.app.emit("approval-request", ApprovalRequestPayload::Request {
                    approval_id,
                    tool: call.name.clone(),
                    detail: detail.clone(),
                    preview,
                });
                match tokio::time::timeout(std::time::Duration::from_secs(600), rx).await {
                    Ok(Ok(v)) => v,
                    _ => false,
                }
            } else {
                true
            };

            executor::audit_append(self.project_root, &call.name, &detail, approved);
            if !approved {
                return ToolResult {
                    tool_call_id: call.id.clone(),
                    content: "denied by user".into(),
                    is_error: true,
                };
            }
            match executor::execute_tool(self.project_root, &call.name, &call.input, self.denylist).await {
                Ok(out) => {
                    let _ = self.app.emit(STREAM_EVENT, StepStreamPayload::ToolResult {
                        name: call.name.clone(),
                        ok: true,
                        detail,
                    });
                    ToolResult {
                        tool_call_id: call.id.clone(),
                        content: out,
                        is_error: false,
                    }
                }
                Err(e) => {
                    let _ = self.app.emit(STREAM_EVENT, StepStreamPayload::ToolResult {
                        name: call.name.clone(),
                        ok: false,
                        detail: e.to_string(),
                    });
                    ToolResult {
                        tool_call_id: call.id.clone(),
                        content: format!("error: {e}"),
                        is_error: true,
                    }
                }
            }
        }
        fn describe(&self, call: &providers::ToolCall) -> String {
            summarize_tool_call(call)
        }
    }

    let tauri_executor = TauriExecutor {
        app: &app,
        registry: &registry,
        project_root: &project_root,
        mode,
        denylist: &denylist,
    };
    let loop_config = agent_loop::LoopConfig {
        model: input.model.clone(),
        system: system.clone(),
        tools,
        max_tokens: 8192,
        max_iterations: 40,
    };

    let emit_ref = &emit;
    let run_result = agent_loop::run(
        provider.as_ref(),
        &loop_config,
        user_msg,
        &tauri_executor,
        |ev| match ev {
            agent_loop::LoopEvent::Delta(text) => emit_ref(StepStreamPayload::Delta { text }),
            agent_loop::LoopEvent::ToolUse { name } => emit_ref(StepStreamPayload::ToolUse { name }),
            agent_loop::LoopEvent::ToolFinished { .. } => {}
            agent_loop::LoopEvent::Usage { .. } => {}
        },
        cancel_token,
    )
    .await;

    let final_text = match run_result {
        Ok((text, _transcript)) => text,
        Err(agent_loop::LoopError::StreamInterrupted) => {
            emit(StepStreamPayload::Error {
                message: "stream interrupted — artifact NOT written to avoid truncation".into(),
            });
            return Ok(());
        }
        Err(agent_loop::LoopError::Cancelled) => {
            emit(StepStreamPayload::Error {
                message: "cancelled by user".into(),
            });
            return Ok(());
        }
        Err(e) => {
            emit(StepStreamPayload::Error {
                message: e.to_string(),
            });
            return Ok(());
        }
    };

    // Write primary artifact through the sanitization boundary.
    if let Some(rel) = StepRunner::primary_artifact(step) {
        if !final_text.trim().is_empty() {
            let writes = vec![speckit_core::runner::ArtifactWrite {
                rel_path: rel.to_string(),
                content: strip_code_fence(&final_text),
            }];
            match StepRunner::sanitize_writes(step, writes) {
                Ok(accepted) => {
                    for w in accepted {
                        let target = if w.rel_path.starts_with(".specify") {
                            project_root.join(&w.rel_path)
                        } else {
                            feature_dir.join(&w.rel_path)
                        };
                        if let Some(parent) = target.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        match std::fs::write(&target, &w.content) {
                            Ok(_) => emit(StepStreamPayload::ArtifactWritten {
                                path: target.to_string_lossy().into_owned(),
                            }),
                            Err(e) => emit(StepStreamPayload::Error {
                                message: format!("write failed: {e}"),
                            }),
                        }
                    }
                }
                Err(e) => emit(StepStreamPayload::Error {
                    message: e.to_string(),
                }),
            }
        }
    }

    // Converge: the report is ephemeral chat output unless persisted. Save it to the
    // feature dir and surface the verdict so the implement↔converge loop is trackable.
    if step == Step::Converge && !final_text.trim().is_empty() {
        let report_path = feature_dir.join("converge.md");
        let _ = std::fs::write(&report_path, strip_code_fence(&final_text));
        emit(StepStreamPayload::ArtifactWritten {
            path: report_path.to_string_lossy().into_owned(),
        });
        let converged = final_text
            .split_whitespace()
            .any(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).eq_ignore_ascii_case("converged"));
        emit(StepStreamPayload::ToolResult {
            name: "verdict".into(),
            ok: converged,
            detail: if converged {
                "CONVERGED — implementation matches artifacts".into()
            } else {
                "not yet converged — see converge.md for remaining work, then re-run implement".into()
            },
        });
    }

    emit(StepStreamPayload::Done);
    Ok(())
}

/// Strip a single outer markdown code fence if the model wrapped the artifact.
fn strip_code_fence(text: &str) -> String {
    let t = text.trim();
    if t.starts_with("```") {
        if let Some(first_nl) = t.find('\n') {
            let inner = &t[first_nl + 1..];
            if let Some(end) = inner.rfind("```") {
                return inner[..end].trim_end().to_string();
            }
        }
    }
    text.to_string()
}

// Credential is re-exported for future OAuth wiring; silence unused warning for now.
#[allow(unused)]
fn _cred_marker(_: Credential) {}

#[tauri::command]
pub fn read_artifact(
    project_root: String,
    feature_slug: Option<String>,
    rel_path: String,
) -> Result<String, String> {
    let root = PathBuf::from(&project_root);
    let base = if rel_path.starts_with(".specify") {
        root
    } else {
        match feature_slug {
            Some(slug) => root.join("specs").join(slug),
            None => {
                let state = PipelineState::scan(&project_root);
                match state.features.last() {
                    Some(f) => f.dir.clone(),
                    None => return Err("no feature yet".into()),
                }
            }
        }
    };
    let path = executor::resolve_scoped(&base, &rel_path).map_err(|e| e.to_string())?;
    std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
}

#[tauri::command]
pub fn write_artifact(
    project_root: String,
    feature_slug: Option<String>,
    rel_path: String,
    content: String,
) -> Result<(), String> {
    // Human edits bypass agent sanitization — the reviewer owns the artifact.
    let root = PathBuf::from(&project_root);
    let base = if rel_path.starts_with(".specify") {
        root
    } else {
        match feature_slug {
            Some(slug) => root.join("specs").join(slug),
            None => return Err("feature slug required".into()),
        }
    };
    let path = executor::resolve_scoped(&base, &rel_path).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

/// Reviewer-only checkbox flip on a checklist artifact. This is the sole path
/// by which checklist items become checked — agents cannot reach it through any
/// tool, and their prose approvals are stripped at the write boundary.
#[tauri::command]
pub fn flip_checklist_item(
    project_root: String,
    feature_slug: String,
    rel_path: String,
    line_index: usize,
    checked: bool,
) -> Result<(), String> {
    let root = PathBuf::from(&project_root);
    let base = root.join("specs").join(&feature_slug);
    let path = executor::resolve_scoped(&base, &rel_path).map_err(|e| e.to_string())?;
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let flipped = speckit_core::runner::flip_checkbox(&content, line_index, checked)
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, flipped).map_err(|e| e.to_string())?;
    executor::audit_append(&root, "reviewer_checkbox_flip", &format!("{rel_path}:{line_index} → {checked}"), true);
    Ok(())
}

/// Approval preview for write_file: unified diff against the existing file when
/// there is one (reviewers care about what changes), truncated new content otherwise.
fn write_preview(root: &Path, rel: &str, content: &str) -> String {
    let existing = executor::resolve_scoped(root, rel)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok());
    match existing {
        Some(old_text) if old_text != content => {
            let diff = similar::TextDiff::from_lines(&old_text, content);
            let mut out = String::new();
            for change in diff.iter_all_changes() {
                let sign = match change.tag() {
                    similar::ChangeTag::Delete => "-",
                    similar::ChangeTag::Insert => "+",
                    similar::ChangeTag::Equal => " ",
                };
                out.push_str(sign);
                out.push_str(&change.to_string());
                if out.len() > 3000 {
                    out.push_str("\n… [diff truncated]");
                    break;
                }
            }
            format!("diff against existing {rel}:\n{out}")
        }
        Some(_) => format!("{rel}: content unchanged"),
        None => {
            if content.len() > 2000 {
                format!("new file:\n{}\n… [{} more bytes]", &content[..2000], content.len() - 2000)
            } else {
                format!("new file:\n{content}")
            }
        }
    }
}

fn summarize_tool_call(call: &providers::ToolCall) -> String {
    match call.name.as_str() {
        "write_file" => format!(
            "write {} ({} bytes)",
            call.input.get("path").and_then(serde_json::Value::as_str).unwrap_or("?"),
            call.input.get("content").and_then(serde_json::Value::as_str).map(|c| c.len()).unwrap_or(0),
        ),
        "run_shell" => format!(
            "run `{}`",
            call.input.get("command").and_then(serde_json::Value::as_str).unwrap_or("?")
        ),
        other => format!("{} {}", other, call.input),
    }
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("appr-{:x}-{:x}", nanos, std::process::id())
}

#[tauri::command]
pub async fn list_models(provider_id: String) -> Result<Vec<providers::models::ModelInfo>, String> {
    let client = reqwest::Client::new();
    providers::models::fetch_models(&provider_id, &client)
        .await
        .map_err(|e| e.to_string())
}
