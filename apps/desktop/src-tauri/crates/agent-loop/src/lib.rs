//! agent-loop: the multi-turn tool-use execution loop, extracted from the Tauri
//! command layer so it is fixture-testable with a mock provider.
//!
//! The loop is deliberately transport-agnostic: it knows nothing about Tauri,
//! approval UIs, or audit logs. Callers supply a ToolExecutor (which owns
//! approval/audit policy) and an event sink.

use async_trait::async_trait;
use futures_util::StreamExt;
use providers::{CompletionRequest, Message, Provider, ProviderError, Role, StreamEvent, ToolCall, ToolDef, ToolResult};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Error)]
pub enum LoopError {
    #[error("provider error: {0}")]
    Provider(#[from] ProviderError),
    #[error("stream ended without completion signal (interrupted?)")]
    StreamInterrupted,
    #[error("exceeded {0} iterations")]
    IterationBound(u32),
    #[error("cancelled by user")]
    Cancelled,
}

/// Events the loop emits; the caller decides how to surface them.
#[derive(Debug, Clone, PartialEq)]
pub enum LoopEvent {
    Delta(String),
    ToolUse { name: String },
    ToolFinished { name: String, is_error: bool },
    Usage { input_tokens: u64, output_tokens: u64 },
}

/// Execution policy for tool calls — approval gating, auditing, scoping all
/// live behind this trait in the caller.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, call: &ToolCall) -> ToolResult;
    /// Short human-readable summary for display/audit. Purely presentational.
    fn describe(&self, call: &ToolCall) -> String;
}

pub struct LoopConfig {
    pub model: String,
    pub system: String,
    pub tools: Vec<ToolDef>,
    pub max_tokens: u32,
    pub max_iterations: u32,
}

/// Run the agent loop to completion. Returns the final assistant text
/// (empty if the model ended on tool results) and the full message transcript.
pub async fn run<E, F>(
    provider: &dyn Provider,
    config: &LoopConfig,
    first_user_message: String,
    executor: &E,
    mut emit: F,
    cancel: CancellationToken,
) -> Result<(String, Vec<Message>), LoopError>
where
    E: ToolExecutor + ?Sized,
    F: FnMut(LoopEvent) + Send,
{
    let mut messages = vec![Message {
        role: Role::User,
        content: first_user_message,
        tool_calls: vec![],
        tool_results: vec![],
    }];

    for iteration in 1..=config.max_iterations {
        let _ = iteration;
        if cancel.is_cancelled() {
            return Err(LoopError::Cancelled);
        }
        let req = CompletionRequest {
            model: config.model.clone(),
            system: Some(config.system.clone()),
            messages: messages.clone(),
            tools: config.tools.clone(),
            max_tokens: config.max_tokens,
        };
        let mut stream = provider.complete(req).await?;

        let mut turn_text = String::new();
        let mut turn_calls: Vec<ToolCall> = Vec::new();
        let mut saw_terminal = false; // Done or Error — anything else is an interruption

        while let Some(item) = stream.next().await {
            if cancel.is_cancelled() {
                return Err(LoopError::Cancelled);
            }
            match item {
                Ok(StreamEvent::TextDelta { text }) => {
                    turn_text.push_str(&text);
                    emit(LoopEvent::Delta(text));
                }
                Ok(StreamEvent::ToolUse { call }) => {
                    emit(LoopEvent::ToolUse {
                        name: call.name.clone(),
                    });
                    turn_calls.push(call);
                }
                Ok(StreamEvent::Usage { input_tokens, output_tokens }) => {
                    emit(LoopEvent::Usage { input_tokens, output_tokens });
                }
                Ok(StreamEvent::Done) => {
                    saw_terminal = true;
                    break;
                }
                Ok(StreamEvent::Error { message }) => {
                    saw_terminal = true;
                    return Err(LoopError::Provider(ProviderError::StreamInterrupted(message)));
                }
                Err(e) => return Err(LoopError::Provider(e)),
            }
        }
        if !saw_terminal {
            return Err(LoopError::StreamInterrupted);
        }

        if turn_calls.is_empty() {
            return Ok((turn_text, messages));
        }

        messages.push(Message {
            role: Role::Assistant,
            content: turn_text,
            tool_calls: turn_calls.clone(),
            tool_results: vec![],
        });

        let mut results = Vec::with_capacity(turn_calls.len());
        for call in &turn_calls {
            let result = executor.execute(call).await;
            emit(LoopEvent::ToolFinished {
                name: call.name.clone(),
                is_error: result.is_error,
            });
            results.push(result);
        }
        messages.push(Message {
            role: Role::User,
            content: String::new(),
            tool_calls: vec![],
            tool_results: results,
        });
    }

    Err(LoopError::IterationBound(config.max_iterations))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use std::sync::Mutex;

    /// Mock provider that replays scripted turns: each turn is a list of events.
    struct ScriptedProvider {
        turns: Mutex<Vec<Vec<StreamEvent>>>,
    }

    impl ScriptedProvider {
        fn new(turns: Vec<Vec<StreamEvent>>) -> Self {
            Self {
                turns: Mutex::new(turns.into_iter().rev().collect()),
            }
        }
    }

    #[async_trait]
    impl Provider for ScriptedProvider {
        fn id(&self) -> &'static str {
            "scripted"
        }
        fn display_name(&self) -> &'static str {
            "Scripted"
        }
        async fn complete(&self, _req: CompletionRequest) -> Result<providers::EventStream, ProviderError> {
            let turn = self.turns.lock().unwrap().pop().unwrap_or_default();
            Ok(Box::pin(stream::iter(turn.into_iter().map(Ok))))
        }
        async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
            Ok(vec![])
        }
    }

    struct RecordingExecutor {
        executed: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl ToolExecutor for RecordingExecutor {
        async fn execute(&self, call: &ToolCall) -> ToolResult {
            self.executed.lock().unwrap().push(call.name.clone());
            ToolResult {
                tool_call_id: call.id.clone(),
                content: format!("{} ok", call.name),
                is_error: false,
            }
        }
        fn describe(&self, call: &ToolCall) -> String {
            call.name.clone()
        }
    }

    fn config() -> LoopConfig {
        LoopConfig {
            model: "m".into(),
            system: "s".into(),
            tools: vec![],
            max_tokens: 1024,
            max_iterations: 10,
        }
    }

    fn tool_call(id: &str, name: &str) -> StreamEvent {
        StreamEvent::ToolUse {
            call: ToolCall {
                id: id.into(),
                name: name.into(),
                input: serde_json::json!({}),
            },
        }
    }

    #[tokio::test]
    async fn single_shot_text_returns_final() {
        let p = ScriptedProvider::new(vec![vec![
            StreamEvent::TextDelta { text: "hello".into() },
            StreamEvent::Done,
        ]]);
        let ex = RecordingExecutor { executed: Mutex::new(vec![]) };
        let (text, _) = run(&p, &config(), "hi".into(), &ex, |_| {}, CancellationToken::new()).await.unwrap();
        assert_eq!(text, "hello");
    }

    #[tokio::test]
    async fn tool_turn_loops_back_with_results() {
        let p = ScriptedProvider::new(vec![
            vec![tool_call("t1", "write_file"), StreamEvent::Done],
            vec![StreamEvent::TextDelta { text: "done writing".into() }, StreamEvent::Done],
        ]);
        let ex = RecordingExecutor { executed: Mutex::new(vec![]) };
        let (text, messages) = run(&p, &config(), "hi".into(), &ex, |_| {}, CancellationToken::new()).await.unwrap();
        assert_eq!(text, "done writing");
        assert_eq!(*ex.executed.lock().unwrap(), vec!["write_file"]);
        // transcript: user, assistant(tool_calls), user(tool_results)
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1].tool_calls.len(), 1);
        assert_eq!(messages[2].tool_results.len(), 1);
        assert_eq!(messages[2].tool_results[0].tool_call_id, "t1");
    }

    #[tokio::test]
    async fn stream_without_done_is_interrupted() {
        // regression guard for truncated-artifact hazard: a stream that just ends
        let p = ScriptedProvider::new(vec![vec![StreamEvent::TextDelta { text: "partial".into() }]]);
        let ex = RecordingExecutor { executed: Mutex::new(vec![]) };
        let err = run(&p, &config(), "hi".into(), &ex, |_| {}, CancellationToken::new()).await.unwrap_err();
        assert!(matches!(err, LoopError::StreamInterrupted));
    }

    #[tokio::test]
    async fn iteration_bound_stops_runaway_loop() {
        let turns = (0..15)
            .map(|i| vec![tool_call(&format!("t{i}"), "write_file"), StreamEvent::Done])
            .collect();
        let p = ScriptedProvider::new(turns);
        let ex = RecordingExecutor { executed: Mutex::new(vec![]) };
        let mut cfg = config();
        cfg.max_iterations = 5;
        let err = run(&p, &cfg, "hi".into(), &ex, |_| {}, CancellationToken::new()).await.unwrap_err();
        assert!(matches!(err, LoopError::IterationBound(5)));
    }

    #[tokio::test]
    async fn events_are_emitted_in_order() {
        let p = ScriptedProvider::new(vec![
            vec![
                StreamEvent::TextDelta { text: "a".into() },
                tool_call("t1", "read_file"),
                StreamEvent::Done,
            ],
            vec![StreamEvent::TextDelta { text: "b".into() }, StreamEvent::Done],
        ]);
        let ex = RecordingExecutor { executed: Mutex::new(vec![]) };
        let mut events = Vec::new();
        run(&p, &config(), "hi".into(), &ex, |e| events.push(e), CancellationToken::new()).await.unwrap();
        assert_eq!(
            events,
            vec![
                LoopEvent::Delta("a".into()),
                LoopEvent::ToolUse { name: "read_file".into() },
                LoopEvent::ToolFinished { name: "read_file".into(), is_error: false },
                LoopEvent::Delta("b".into()),
            ]
        );
    }
    #[tokio::test]
    async fn cancellation_stops_loop() {
        let token = CancellationToken::new();
        token.cancel();
        let p = ScriptedProvider::new(vec![vec![StreamEvent::Done]]);
        let ex = RecordingExecutor { executed: Mutex::new(vec![]) };
        let err = run(&p, &config(), "hi".into(), &ex, |_| {}, token).await.unwrap_err();
        assert!(matches!(err, LoopError::Cancelled));
    }
}
