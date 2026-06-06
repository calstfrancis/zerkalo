use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use serde_json::{json, Value};

// ── Public types ──────────────────────────────────────────────────────────────

pub enum DiagSeverity {
    Error,
    Warning,
    Info,
}

pub struct LspDiagnostic {
    pub file: PathBuf,
    pub line: u32,
    pub col: u32,
    pub message: String,
    pub severity: DiagSeverity,
}

#[derive(Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: u8,
    pub detail: Option<String>,
    pub insert_text: Option<String>,
}

// ── Client ────────────────────────────────────────────────────────────────────

pub struct LspClient {
    child: Child,
    stdin: ChildStdin,
    diag_rx: Receiver<Vec<LspDiagnostic>>,
    comp_rx: Receiver<(u64, Vec<CompletionItem>)>,
    next_id: u64,
    pub root: PathBuf,
}

fn tinymist_command() -> Command {
    let bundled = std::path::Path::new("/usr/lib/zerkalo/tinymist");
    if bundled.exists() {
        Command::new(bundled)
    } else {
        Command::new("tinymist")
    }
}

impl LspClient {
    /// Spawn tinymist, perform the LSP handshake. Returns None if tinymist is
    /// not available or the spawn fails.
    pub fn new(root: &Path) -> Option<Self> {
        let mut child = tinymist_command()
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        let stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;

        let (diag_tx, diag_rx) = mpsc::channel::<Vec<LspDiagnostic>>();
        let (comp_tx, comp_rx) = mpsc::channel::<(u64, Vec<CompletionItem>)>();
        std::thread::spawn(move || reader_thread(BufReader::new(stdout), diag_tx, comp_tx));

        let root_uri = path_to_uri(root);
        let mut client = Self {
            child,
            stdin,
            diag_rx,
            comp_rx,
            next_id: 1,
            root: root.to_path_buf(),
        };

        let id = client.next_id();
        client.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": root_uri,
                "capabilities": {
                    "textDocument": {
                        "publishDiagnostics": {},
                        "completion": {
                            "completionItem": {
                                "snippetSupport": true,
                                "documentationFormat": ["plaintext"]
                            },
                            "completionItemKind": { "valueSet": [1,2,3,4,5,6,7,8,9,10,12,13,14,15] }
                        }
                    }
                }
            }
        }));
        client.send(&json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }));

        tracing::info!("tinymist LSP started for {}", root.display());
        Some(client)
    }

    pub fn did_open(&mut self, path: &Path, text: &str) {
        let uri = path_to_uri(path);
        self.send(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "typst",
                    "version": 1,
                    "text": text
                }
            }
        }));
    }

    pub fn did_change(&mut self, path: &Path, text: &str, version: i64) {
        let uri = path_to_uri(path);
        self.send(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }]
            }
        }));
    }

    /// Request completions at (1-based) line/col. Returns the request id so the
    /// caller can match the response from poll_completion().
    pub fn request_completion(&mut self, path: &Path, line: u32, col: u32) -> u64 {
        let id = self.next_id();
        let uri = path_to_uri(path);
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": uri },
                "position": {
                    "line": line.saturating_sub(1),
                    "character": col.saturating_sub(1)
                },
                "context": { "triggerKind": 1 }
            }
        }));
        id
    }

    /// Drain pending diagnostic notifications.
    pub fn poll(&self) -> Vec<LspDiagnostic> {
        let mut out = Vec::new();
        loop {
            match self.diag_rx.try_recv() {
                Ok(d) => out.extend(d),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        out
    }

    /// Return the most recent completion response if one arrived, discarding
    /// older ones.
    pub fn poll_completion(&self) -> Option<(u64, Vec<CompletionItem>)> {
        let mut latest: Option<(u64, Vec<CompletionItem>)> = None;
        loop {
            match self.comp_rx.try_recv() {
                Ok(pair) => latest = Some(pair),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        latest
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn send(&mut self, msg: &Value) {
        let body = serde_json::to_string(msg).unwrap_or_default();
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let _ = self.stdin.write_all(header.as_bytes());
        let _ = self.stdin.write_all(body.as_bytes());
        let _ = self.stdin.flush();
    }

    /// Returns false if the tinymist process has exited.
    pub fn is_alive(&mut self) -> bool {
        self.child.try_wait().map(|s| s.is_none()).unwrap_or(false)
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let id = self.next_id();
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":"shutdown","params":null}));
        self.send(&json!({"jsonrpc":"2.0","method":"exit","params":null}));
    }
}

// ── Background reader thread ──────────────────────────────────────────────────

fn reader_thread(
    mut reader: BufReader<std::process::ChildStdout>,
    diag_tx: Sender<Vec<LspDiagnostic>>,
    comp_tx: Sender<(u64, Vec<CompletionItem>)>,
) {
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            _ => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let len: usize = match trimmed.strip_prefix("Content-Length:") {
            Some(s) => s.trim().parse().unwrap_or(0),
            None => continue,
        };
        if len == 0 {
            continue;
        }
        let mut blank = String::new();
        if reader.read_line(&mut blank).is_err() {
            break;
        }
        let mut body = vec![0u8; len];
        if reader.read_exact(&mut body).is_err() {
            break;
        }
        let json: Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = json.get("method").and_then(|m| m.as_str());
        let has_id = json.get("id").is_some();
        let has_result = json.get("result").is_some();

        if method == Some("textDocument/publishDiagnostics") {
            if let Some(diags) = parse_diags(&json) {
                if diag_tx.send(diags).is_err() {
                    break;
                }
            }
        } else if method.is_none() && has_id && has_result {
            // This is a response to one of our requests
            if let Some(id) = json.get("id").and_then(|v| v.as_u64()) {
                if let Some(items) = parse_completion_result(&json) {
                    if comp_tx.send((id, items)).is_err() {
                        break;
                    }
                }
            }
        }
    }
}

// ── Parsers ───────────────────────────────────────────────────────────────────

fn parse_diags(json: &Value) -> Option<Vec<LspDiagnostic>> {
    let params = json.get("params")?;
    let uri = params.get("uri")?.as_str()?;
    let file = uri_to_path(uri);
    let raw = params.get("diagnostics")?.as_array()?;

    Some(
        raw.iter()
            .filter_map(|d| {
                let message = d.get("message")?.as_str()?.to_string();
                let start = d.get("range")?.get("start")?;
                let line = start.get("line")?.as_u64().unwrap_or(0) as u32 + 1;
                let col = start.get("character")?.as_u64().unwrap_or(0) as u32 + 1;
                let severity = match d.get("severity").and_then(|s| s.as_u64()).unwrap_or(1) {
                    2 => DiagSeverity::Warning,
                    3 | 4 => DiagSeverity::Info,
                    _ => DiagSeverity::Error,
                };
                Some(LspDiagnostic { file: file.clone(), line, col, message, severity })
            })
            .collect(),
    )
}

fn parse_completion_result(json: &Value) -> Option<Vec<CompletionItem>> {
    let result = json.get("result")?;
    // result can be an array or { isIncomplete, items }
    let raw = if let Some(arr) = result.as_array() {
        arr
    } else if let Some(arr) = result.get("items").and_then(|v| v.as_array()) {
        arr
    } else {
        return Some(Vec::new());
    };

    Some(
        raw.iter()
            .take(40)
            .filter_map(|item| {
                let label = item.get("label")?.as_str()?.to_string();
                let kind = item.get("kind").and_then(|v| v.as_u64()).unwrap_or(1) as u8;
                let detail = item.get("detail").and_then(|v| v.as_str()).map(|s| s.to_string());
                let raw_insert = item
                    .get("insertText")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        item.get("textEdit")
                            .and_then(|te| te.get("newText"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    });
                // Strip LSP snippet markers ($0, ${1:placeholder} → placeholder)
                let insert_text = raw_insert.map(|s| strip_snippet_syntax(&s));
                Some(CompletionItem { label, kind, detail, insert_text })
            })
            .collect(),
    )
}

// ── Snippet syntax stripper ───────────────────────────────────────────────────

/// Convert LSP snippet syntax to plain text:
/// - `${N:placeholder}` → `placeholder`
/// - `$N` / `$0` → removed
/// - `\\$` → `$`
fn strip_snippet_syntax(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        if chars[i] == '\\' && i + 1 < n && chars[i + 1] == '$' {
            out.push('$');
            i += 2;
        } else if chars[i] == '$' {
            i += 1;
            if i >= n { break; }
            if chars[i] == '{' {
                i += 1;
                // skip N:
                while i < n && chars[i] != ':' && chars[i] != '}' { i += 1; }
                if i < n && chars[i] == ':' {
                    i += 1;
                    let mut depth = 1i32;
                    while i < n && depth > 0 {
                        if chars[i] == '{' { depth += 1; }
                        else if chars[i] == '}' { depth -= 1; if depth == 0 { i += 1; break; } }
                        if depth > 0 { out.push(chars[i]); }
                        i += 1;
                    }
                } else if i < n && chars[i] == '}' {
                    i += 1;
                }
            } else {
                while i < n && (chars[i].is_ascii_digit() || chars[i].is_alphanumeric() && i > 0) {
                    if chars[i].is_ascii_digit() { i += 1; } else { break; }
                }
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

// ── URI helpers ───────────────────────────────────────────────────────────────

fn path_to_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}

fn uri_to_path(uri: &str) -> PathBuf {
    PathBuf::from(uri.trim_start_matches("file://"))
}
