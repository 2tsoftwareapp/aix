//! Tauri command surface: the bridge between UI and the speckit machine.
//!
//! run_step flow: gate evaluation → context assembly → verbatim template prompt
//! → provider streaming (relayed as Tauri events) → write sanitization → disk.

use futures_util::StreamExt;
use providers::live::{AnthropicProvider, OpenAiCompatProvider};
use providers::{CompletionRequest, Credential, Message, Provider, Role};
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
    ArtifactWritten { path: String },
    GateBlocked { reason: String },
    Done,
    Error { message: String },
}

#[derive(Debug, Deserialize)]
pub struct RunStepInput {
    pub step: String,
    pub project_root: String,
    pub feature_slug: Option<String>,
    pub user_input: String,
    pub provider_id: String,
    pub model: String,
}

fn template_root() -> PathBuf {
    if let Ok(p) = std::env::var("AIX_SPECKIT_CONTENT") {
        return p.into();
    }
    // dev fallback: repo content/speckit relative to src-tauri
    let dev = PathBuf::from("../../content/speckit");
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
        other => return Err(format!("unknown provider: {other}")),
    })
}

fn read_context(path: &Path, rel: &str) -> Option<ArtifactContext> {
    let full = path.join(rel);
    std::fs::read_to_string(full).ok().map(|content| ArtifactContext {
        rel_path: rel.to_string(),
        content,
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
pub async fn run_step(app: AppHandle, input: RunStepInput) -> Result<(), String> {
    let emit = |p: StepStreamPayload| {
        let _ = app.emit(STREAM_EVENT, p);
    };

    let step = parse_step(&input.step)?;
    let project_root = PathBuf::from(&input.project_root);

    // Gate evaluation — structural enforcement before anything else.
    let state = PipelineState::scan(&project_root);
    let feature = feature_or_active(&state, input.feature_slug.as_deref());
    if let GateVerdict::Block { reason } = evaluate(step, feature, state.has_constitution) {
        emit(StepStreamPayload::GateBlocked { reason });
        return Ok(());
    }

    // Resolve feature dir (creating on specify).
    let feature_dir = match feature {
        Some(f) => f.dir.clone(),
        None if step == Step::Specify => next_feature_dir(&project_root, &input.user_input),
        None => project_root.clone(),
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
    let req = CompletionRequest {
        model: input.model.clone(),
        system: Some(system),
        messages: vec![Message {
            role: Role::User,
            content: user_msg,
        }],
        tools: vec![],
        max_tokens: 8192,
    };

    let mut stream = provider.complete(req).await.map_err(|e| e.to_string())?;
    let mut collected = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(providers::StreamEvent::TextDelta { text }) => {
                collected.push_str(&text);
                emit(StepStreamPayload::Delta { text });
            }
            Ok(providers::StreamEvent::ToolUse { call }) => {
                emit(StepStreamPayload::ToolUse { name: call.name });
            }
            Ok(providers::StreamEvent::Done) => break,
            Ok(providers::StreamEvent::Error { message }) => {
                emit(StepStreamPayload::Error { message });
                return Ok(());
            }
            Ok(providers::StreamEvent::Usage { .. }) => {}
            Err(e) => {
                emit(StepStreamPayload::Error {
                    message: e.to_string(),
                });
                return Ok(());
            }
        }
    }

    // Write primary artifact through the sanitization boundary.
    if let Some(rel) = StepRunner::primary_artifact(step) {
        if !collected.trim().is_empty() {
            let writes = vec![speckit_core::runner::ArtifactWrite {
                rel_path: rel.to_string(),
                content: strip_code_fence(&collected),
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
