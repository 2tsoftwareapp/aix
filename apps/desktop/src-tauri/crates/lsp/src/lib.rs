//! Lightweight LSP client over stdio.
//!
//! One client owns one language-server child process scoped to a project root.
//! The protocol implementation is intentionally small: initialize/shutdown,
//! didOpen/didChange, definition, references, and document symbols.

use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdout, Command};
use tokio::sync::{oneshot, Mutex};
use url::Url;

const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Debug, Error)]
pub enum LspError {
    #[error("unsupported language for {0}")]
    UnsupportedLanguage(String),
    #[error("failed to spawn language server {command}: {source}")]
    Spawn {
        command: String,
        source: std::io::Error,
    },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("lsp protocol error: {0}")]
    Protocol(String),
    #[error("lsp request timed out: {0}")]
    Timeout(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    TypeScript,
    Python,
}

impl Language {
    pub fn for_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
            "rs" => Some(Self::Rust),
            "ts" | "tsx" | "js" | "jsx" | "mts" | "cts" | "mjs" | "cjs" => Some(Self::TypeScript),
            "py" | "pyi" => Some(Self::Python),
            _ => None,
        }
    }

    fn server_command(self) -> (&'static str, Vec<&'static str>) {
        match self {
            Self::Rust => ("rust-analyzer", vec![]),
            Self::TypeScript => ("typescript-language-server", vec!["--stdio"]),
            Self::Python => ("pylsp", vec![]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspLocation {
    pub path: PathBuf,
    /// One-based line number for display/tool output.
    pub line: u32,
    /// One-based character number for display/tool output.
    pub character: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspSymbol {
    pub name: String,
    pub kind: u32,
    pub line: u32,
    pub character: u32,
    pub container: Option<String>,
}

type PendingResult = Result<Value, String>;

#[derive(Default)]
struct SharedState {
    pending: Mutex<HashMap<u64, oneshot::Sender<PendingResult>>>,
}

pub struct LspClient {
    root: PathBuf,
    stdin: Mutex<tokio::process::ChildStdin>,
    child: Mutex<Child>,
    state: Arc<SharedState>,
    next_id: AtomicU64,
    documents: Mutex<HashMap<PathBuf, u64>>,
}

impl LspClient {
    pub async fn spawn(root: &Path, language: Language) -> Result<Self, LspError> {
        let (program, args) = language.server_command();
        Self::spawn_command(root, language, program, &args).await
    }

    async fn spawn_command(
        root: &Path,
        _language: Language,
        program: &str,
        args: &[&str],
    ) -> Result<Self, LspError> {
        let mut child = Command::new(program)
            .args(args)
            .current_dir(root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|source| LspError::Spawn {
                command: program.to_string(),
                source,
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LspError::Protocol("language server stdin unavailable".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LspError::Protocol("language server stdout unavailable".into()))?;
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                while reader.read_line(&mut line).await.is_ok_and(|n| n > 0) {
                    line.clear();
                }
            });
        }

        let state = Arc::new(SharedState::default());
        tokio::spawn(reader_loop(stdout, state.clone()));
        let client = Self {
            root: root.to_path_buf(),
            stdin: Mutex::new(stdin),
            child: Mutex::new(child),
            state,
            next_id: AtomicU64::new(1),
            documents: Mutex::new(HashMap::new()),
        };
        client.initialize().await?;
        Ok(client)
    }

    async fn initialize(&self) -> Result<(), LspError> {
        let root_uri = file_uri(&self.root)?;
        self.request(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": root_uri,
                "capabilities": {
                    "textDocument": {
                        "definition": { "linkSupport": true },
                        "references": {},
                        "documentSymbol": { "hierarchicalDocumentSymbolSupport": true }
                    }
                },
                "workspaceFolders": [{
                    "uri": root_uri,
                    "name": self.root.file_name().and_then(|n| n.to_str()).unwrap_or("workspace")
                }],
                "clientInfo": { "name": "aix", "version": env!("CARGO_PKG_VERSION") }
            }),
        )
        .await?;
        self.notify("initialized", json!({})).await
    }

    async fn request(&self, method: &'static str, params: Value) -> Result<Value, LspError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.state.pending.lock().await.insert(id, tx);
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        if let Err(e) = self.write_message(&body).await {
            self.state.pending.lock().await.remove(&id);
            return Err(e);
        }
        match tokio::time::timeout(REQUEST_TIMEOUT, rx).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(message))) => Err(LspError::Protocol(message)),
            Ok(Err(_)) => Err(LspError::Protocol(
                "language server response channel closed".into(),
            )),
            Err(_) => {
                self.state.pending.lock().await.remove(&id);
                Err(LspError::Timeout(method))
            }
        }
    }

    async fn notify(&self, method: &str, params: Value) -> Result<(), LspError> {
        let body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(&body).await
    }

    async fn write_message(&self, body: &Value) -> Result<(), LspError> {
        let bytes = serde_json::to_vec(body)?;
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(format!("Content-Length: {}\r\n\r\n", bytes.len()).as_bytes())
            .await?;
        stdin.write_all(&bytes).await?;
        stdin.flush().await?;
        Ok(())
    }

    fn absolute_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        }
    }

    async fn sync_document(&self, path: &Path) -> Result<(PathBuf, String), LspError> {
        let abs = self.absolute_path(path);
        let content = tokio::fs::read_to_string(&abs).await?;
        let uri = file_uri(&abs)?;
        let mut docs = self.documents.lock().await;
        match docs.get_mut(&abs) {
            Some(version) => {
                *version += 1;
                self.notify(
                    "textDocument/didChange",
                    json!({
                        "textDocument": { "uri": uri, "version": *version },
                        "contentChanges": [{ "text": content }]
                    }),
                )
                .await?;
            }
            None => {
                docs.insert(abs.clone(), 1);
                self.notify(
                    "textDocument/didOpen",
                    json!({
                        "textDocument": {
                            "uri": uri,
                            "languageId": language_id(&abs),
                            "version": 1,
                            "text": content
                        }
                    }),
                )
                .await?;
            }
        }
        Ok((abs, uri))
    }

    /// line and character are one-based at the public tool boundary; LSP uses
    /// zero-based positions internally.
    pub async fn goto_definition(
        &self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<LspLocation>, LspError> {
        let (_abs, uri) = self.sync_document(path).await?;
        let result = self
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": uri },
                    "position": position(line, character)
                }),
            )
            .await?;
        Ok(parse_locations(&result))
    }

    pub async fn find_references(
        &self,
        path: &Path,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Result<Vec<LspLocation>, LspError> {
        let (_abs, uri) = self.sync_document(path).await?;
        let result = self
            .request(
                "textDocument/references",
                json!({
                    "textDocument": { "uri": uri },
                    "position": position(line, character),
                    "context": { "includeDeclaration": include_declaration }
                }),
            )
            .await?;
        Ok(parse_locations(&result))
    }

    pub async fn document_symbols(&self, path: &Path) -> Result<Vec<LspSymbol>, LspError> {
        let (_abs, uri) = self.sync_document(path).await?;
        let result = self
            .request(
                "textDocument/documentSymbol",
                json!({ "textDocument": { "uri": uri } }),
            )
            .await?;
        Ok(parse_symbols(&result))
    }

    async fn is_alive(&self) -> bool {
        match self.child.lock().await.try_wait() {
            Ok(None) => true,
            _ => false,
        }
    }

    pub async fn shutdown(&self) {
        let _ = self.request("shutdown", json!({})).await;
        let _ = self.notify("exit", json!({})).await;
        let _ = self.child.lock().await.kill().await;
    }
}

#[derive(Default)]
pub struct LspManager {
    clients: Mutex<HashMap<String, Arc<LspClient>>>,
}

impl LspManager {
    pub async fn client_for_path(
        &self,
        root: &Path,
        path: &Path,
    ) -> Result<Arc<LspClient>, LspError> {
        let language = Language::for_path(path)
            .ok_or_else(|| LspError::UnsupportedLanguage(path.display().to_string()))?;
        let key = client_key(root, language);
        if let Some(client) = self.clients.lock().await.get(&key).cloned() {
            if client.is_alive().await {
                return Ok(client);
            }
        }
        let client = Arc::new(LspClient::spawn(root, language).await?);
        self.clients.lock().await.insert(key, client.clone());
        Ok(client)
    }

    pub async fn shutdown_project(&self, root: &Path) {
        let prefix = format!("{}|", root_key(root));
        let clients = {
            let mut guard = self.clients.lock().await;
            let keys = guard
                .keys()
                .filter(|key| key.starts_with(&prefix))
                .cloned()
                .collect::<Vec<_>>();
            keys.into_iter()
                .filter_map(|key| guard.remove(&key))
                .collect::<Vec<_>>()
        };
        for client in clients {
            client.shutdown().await;
        }
    }

    pub async fn shutdown_all(&self) {
        let clients = {
            let mut guard = self.clients.lock().await;
            guard.drain().map(|(_, client)| client).collect::<Vec<_>>()
        };
        for client in clients {
            client.shutdown().await;
        }
    }
}

fn root_key(root: &Path) -> String {
    root.canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn client_key(root: &Path, language: Language) -> String {
    format!("{}|{:?}", root_key(root), language)
}

async fn reader_loop(stdout: ChildStdout, state: Arc<SharedState>) {
    let mut reader = BufReader::new(stdout);
    loop {
        match read_lsp_message(&mut reader).await {
            Ok(Some(message)) => {
                if let Some(id) = message.get("id").and_then(Value::as_u64) {
                    let tx = state.pending.lock().await.remove(&id);
                    if let Some(tx) = tx {
                        if let Some(error) = message.get("error") {
                            let text = error
                                .get("message")
                                .and_then(Value::as_str)
                                .map(str::to_string)
                                .unwrap_or_else(|| error.to_string());
                            let _ = tx.send(Err(text));
                        } else {
                            let _ =
                                tx.send(Ok(message.get("result").cloned().unwrap_or(Value::Null)));
                        }
                    }
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
    let pending = std::mem::take(&mut *state.pending.lock().await);
    for (_, tx) in pending {
        let _ = tx.send(Err("language server exited".into()));
    }
}

async fn read_lsp_message(
    reader: &mut BufReader<ChildStdout>,
) -> Result<Option<Value>, std::io::Error> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().ok();
            }
        }
    }
    let len = content_length.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "missing Content-Length")
    })?;
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;
    let message = serde_json::from_slice(&body)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(Some(message))
}

fn file_uri(path: &Path) -> Result<String, LspError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Url::from_file_path(&absolute)
        .map(|u| u.to_string())
        .map_err(|_| LspError::Protocol(format!("invalid file path: {}", absolute.display())))
}

fn uri_path(uri: &str) -> Option<PathBuf> {
    Url::parse(uri).ok()?.to_file_path().ok()
}

fn position(line: u32, character: u32) -> Value {
    json!({
        "line": line.saturating_sub(1),
        "character": character.saturating_sub(1),
    })
}

fn language_id(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "rs" => "rust",
        "tsx" => "typescriptreact",
        "jsx" => "javascriptreact",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "py" | "pyi" => "python",
        _ => "plaintext",
    }
}

fn parse_locations(result: &Value) -> Vec<LspLocation> {
    let mut out = Vec::new();
    match result {
        Value::Null => {}
        Value::Array(items) => {
            for item in items {
                if let Some(location) = parse_location(item) {
                    out.push(location);
                }
            }
        }
        item => {
            if let Some(location) = parse_location(item) {
                out.push(location);
            }
        }
    }
    out
}

fn parse_location(item: &Value) -> Option<LspLocation> {
    let (uri, range) = if let Some(target_uri) = item.get("targetUri").and_then(Value::as_str) {
        let range = item
            .get("targetSelectionRange")
            .or_else(|| item.get("targetRange"))?;
        (target_uri, range)
    } else {
        (item.get("uri").and_then(Value::as_str)?, item.get("range")?)
    };
    let start = range.get("start")?;
    Some(LspLocation {
        path: uri_path(uri)?,
        line: start.get("line").and_then(Value::as_u64).unwrap_or(0) as u32 + 1,
        character: start.get("character").and_then(Value::as_u64).unwrap_or(0) as u32 + 1,
    })
}

fn parse_symbols(result: &Value) -> Vec<LspSymbol> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    if let Some(items) = result.as_array() {
        for item in items {
            parse_symbol_item(item, None, &mut out, &mut seen);
        }
    }
    out
}

fn parse_symbol_item(
    item: &Value,
    container: Option<&str>,
    out: &mut Vec<LspSymbol>,
    seen: &mut HashSet<(String, u32, u32)>,
) {
    let name = item.get("name").and_then(Value::as_str).unwrap_or("");
    if name.is_empty() {
        return;
    }
    let kind = item.get("kind").and_then(Value::as_u64).unwrap_or(0) as u32;
    let range = if item.get("selectionRange").is_some() {
        item.get("selectionRange")
    } else {
        item.pointer("/location/range")
            .or_else(|| item.get("range"))
    };
    let Some(start) = range.and_then(|r| r.get("start")) else {
        return;
    };
    let line = start.get("line").and_then(Value::as_u64).unwrap_or(0) as u32 + 1;
    let character = start.get("character").and_then(Value::as_u64).unwrap_or(0) as u32 + 1;
    let key = (name.to_string(), line, character);
    if seen.insert(key) {
        out.push(LspSymbol {
            name: name.to_string(),
            kind,
            line,
            character,
            container: container.map(str::to_string),
        });
    }
    if let Some(children) = item.get("children").and_then(Value::as_array) {
        for child in children {
            parse_symbol_item(child, Some(name), out, seen);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn language_is_detected_from_file_extension() {
        assert_eq!(
            Language::for_path(std::path::Path::new("src/main.rs")),
            Some(Language::Rust)
        );
        assert_eq!(
            Language::for_path(std::path::Path::new("src/App.tsx")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            Language::for_path(std::path::Path::new("src/main.py")),
            Some(Language::Python)
        );
        assert_eq!(Language::for_path(std::path::Path::new("README.md")), None);
    }

    async fn fake_client(root: &std::path::Path, script: &std::path::Path) -> LspClient {
        LspClient::spawn_command(
            root,
            Language::Rust,
            "python3",
            &[script.to_string_lossy().as_ref()],
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn client_queries_fake_lsp_server() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("proj");
        fs::create_dir_all(root.join("src")).unwrap();
        let source = root.join("src/main.rs");
        fs::write(&source, "fn main() {}\n").unwrap();
        let server = tmp.path().join("fake_lsp.py");
        fs::write(&server, r#"
import sys, json

def read_message():
    length = 0
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        if line in (b"\r\n", b"\n"):
            break
        if line.lower().startswith(b"content-length:"):
            length = int(line.split(b":", 1)[1].strip())
    return json.loads(sys.stdin.buffer.read(length).decode())

def send(message):
    body = json.dumps(message, separators=(",", ":")).encode()
    sys.stdout.buffer.write(b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body)
    sys.stdout.buffer.flush()

while True:
    msg = read_message()
    if msg is None:
        break
    method = msg.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {"capabilities": {"definitionProvider": True, "referencesProvider": True, "documentSymbolProvider": True}}})
    elif method == "textDocument/definition":
        uri = msg["params"]["textDocument"]["uri"]
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {"uri": uri, "range": {"start": {"line": 4, "character": 2}, "end": {"line": 4, "character": 8}}}})
    elif method == "textDocument/references":
        uri = msg["params"]["textDocument"]["uri"]
        send({"jsonrpc": "2.0", "id": msg["id"], "result": [
            {"uri": uri, "range": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 8}}},
            {"uri": uri, "range": {"start": {"line": 4, "character": 2}, "end": {"line": 4, "character": 8}}}
        ]})
    elif method == "textDocument/documentSymbol":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": [
            {"name": "outer", "kind": 12, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 5, "character": 1}}, "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 8}}, "children": [
                {"name": "inner", "kind": 6, "range": {"start": {"line": 2, "character": 1}, "end": {"line": 3, "character": 1}}, "selectionRange": {"start": {"line": 2, "character": 5}, "end": {"line": 2, "character": 10}}, "children": []}
            ]}
        ]})
    elif method == "shutdown":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": None})
    elif method == "exit":
        break
"#).unwrap();
        let client = fake_client(&root, &server).await;

        let definitions = client.goto_definition(&source, 1, 1).await.unwrap();
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].line, 5, "public locations are one-based");
        assert_eq!(definitions[0].character, 3);

        let references = client.find_references(&source, 1, 1, true).await.unwrap();
        assert_eq!(references.len(), 2);
        assert_eq!(references[0].line, 1);

        let symbols = client.document_symbols(&source).await.unwrap();
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "outer");
        assert_eq!(symbols[1].name, "inner");
        assert_eq!(symbols[1].container.as_deref(), Some("outer"));
        assert_eq!(symbols[1].line, 3);
    }

    #[tokio::test]
    async fn real_rust_analyzer_smoke_test_when_available() {
        if std::process::Command::new("rust-analyzer")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| !status.success())
            .unwrap_or(true)
        {
            return;
        }

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("proj");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"lsp-smoke\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        let source = root.join("src/main.rs");
        fs::write(
            &source,
            "fn target() -> i32 {\n    42\n}\n\nfn main() {\n    let value = target();\n    println!(\"{value}\");\n}\n",
        )
        .unwrap();

        let client = LspClient::spawn(&root, Language::Rust).await.unwrap();
        let mut definitions = Vec::new();
        for _ in 0..20 {
            definitions = client
                .goto_definition(&source, 6, 18)
                .await
                .unwrap_or_default();
            if !definitions.is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        client.shutdown().await;
        assert!(
            definitions.iter().any(|loc| loc.line == 1),
            "rust-analyzer should resolve the target function on line 1: {definitions:?}"
        );
    }
}
