use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};

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
    child: Option<Child>,
    stdin: ChildStdin,
    diag_rx: Receiver<Vec<LspDiagnostic>>,
    comp_rx: Receiver<(u64, Vec<CompletionItem>)>,
    next_id: u64,
    pub root: PathBuf,
}

fn tinymist_command() -> Command {
    for candidate in &["/app/lib/zerkalo/tinymist", "/usr/lib/zerkalo/tinymist"] {
        let p = std::path::Path::new(candidate);
        if p.exists() {
            return Command::new(p);
        }
    }
    Command::new("tinymist")
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
            child: Some(child),
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
                    // Explicit about the position encoding we actually send —
                    // request_completion()'s column is a UTF-16 code-unit
                    // count (see its caller in editor_pane.rs), matching the
                    // LSP spec's default, so this is documentation as much as
                    // negotiation. Without this, some servers assume the
                    // client speaks the spec's default anyway, but relying on
                    // an unstated default is fragile if that ever changes.
                    "general": {
                        "positionEncodings": ["utf-16"]
                    },
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
        while let Ok(d) = self.diag_rx.try_recv() {
            out.extend(d);
        }
        out
    }

    /// Return the most recent completion response if one arrived, discarding
    /// older ones.
    pub fn poll_completion(&self) -> Option<(u64, Vec<CompletionItem>)> {
        let mut latest: Option<(u64, Vec<CompletionItem>)> = None;
        while let Ok(pair) = self.comp_rx.try_recv() {
            latest = Some(pair);
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
        self.child
            .as_mut()
            .map(|c| c.try_wait().map(|s| s.is_none()).unwrap_or(false))
            .unwrap_or(false)
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let id = self.next_id();
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":"shutdown","params":null}));
        self.send(&json!({"jsonrpc":"2.0","method":"exit","params":null}));
        // Reap the child on a background thread so repeated LSP restarts
        // (e.g. switching projects) don't accumulate zombie tinymist
        // processes — without blocking the caller (likely the GTK main
        // thread) on however long tinymist takes to actually exit. `exit`
        // above should make it prompt; kill() is a fallback if it doesn't.
        if let Some(mut child) = self.child.take() {
            std::thread::spawn(move || {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(300));
                if matches!(child.try_wait(), Ok(None)) {
                    let _ = child.kill();
                }
                let _ = child.wait();
            });
        }
    }
}

// ── Background reader thread ──────────────────────────────────────────────────

fn reader_thread(
    mut reader: BufReader<std::process::ChildStdout>,
    diag_tx: Sender<Vec<LspDiagnostic>>,
    comp_tx: Sender<(u64, Vec<CompletionItem>)>,
) {
    'msg: loop {
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
        // Consume all remaining headers until the blank separator line.
        // The LSP spec allows multiple headers (e.g. Content-Type after Content-Length);
        // reading only one line would consume a real header as the separator, corrupting
        // the body offset for every subsequent message.
        loop {
            let mut hdr = String::new();
            match reader.read_line(&mut hdr) {
                Ok(0) | Err(_) => break 'msg,
                _ => {}
            }
            if hdr.trim().is_empty() {
                break;
            }
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

// Uses the `url` crate rather than manual string concatenation so paths with
// spaces or non-ASCII characters (e.g. a home directory with a Unicode
// username) round-trip correctly — tinymist percent-encodes/decodes URIs per
// RFC 3986, so a hand-built `file://{path}` without encoding wouldn't match
// what it sends back in `publishDiagnostics`, silently dropping or
// misattributing diagnostics for such paths.
fn path_to_uri(path: &Path) -> String {
    url::Url::from_file_path(path)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| format!("file://{}", path.to_string_lossy()))
}

fn uri_to_path(uri: &str) -> PathBuf {
    url::Url::parse(uri)
        .ok()
        .and_then(|u| u.to_file_path().ok())
        .unwrap_or_else(|| PathBuf::from(uri.trim_start_matches("file://")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_to_uri_percent_encodes_spaces() {
        let uri = path_to_uri(Path::new("/home/user/My Docs/main.typ"));
        assert!(uri.contains("%20"), "spaces should be percent-encoded: {uri}");
        assert!(!uri.contains(' '), "no literal spaces should remain: {uri}");
    }

    #[test]
    fn uri_round_trips_path_with_spaces_and_unicode() {
        let original = Path::new("/home/user/Café Notes/thèse.typ");
        let uri = path_to_uri(original);
        let round_tripped = uri_to_path(&uri);
        assert_eq!(round_tripped, original);
    }

    #[test]
    fn uri_round_trips_plain_ascii_path() {
        let original = Path::new("/home/user/project/main.typ");
        let uri = path_to_uri(original);
        assert_eq!(uri_to_path(&uri), original);
    }

    #[test]
    fn strip_snippet_syntax_removes_placeholders_and_tabstops() {
        assert_eq!(strip_snippet_syntax("#heading(${1:level})$0"), "#heading(level)");
        assert_eq!(strip_snippet_syntax(r"\$5 total"), "$5 total");
    }
}
