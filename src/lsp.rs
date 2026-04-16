use crate::ast::{Program, TopLevel};
use crate::errors::{Diagnostic, ErrorMode, Severity, Span};
use crate::parser;
use crate::proof_engine;
use crate::typechecker;
use lsp_server::{Connection, Message, Notification, Request, Response};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};

pub struct LspServer {
    connection: Connection,
    documents: Arc<Mutex<DocumentStore>>,
}

struct DocumentStore {
    docs: HashMap<String, DocumentState>,
}

struct DocumentState {
    text: String,
    version: i32,
    program: Option<Program>,
}

impl LspServer {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let (connection, _) = Connection::stdio();

        Ok(LspServer {
            connection,
            documents: Arc::new(Mutex::new(DocumentStore {
                docs: HashMap::new(),
            })),
        })
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let server_capabilities = serde_json::json!({
            "textDocumentSync": {
                "openClose": true,
                "change": 1, // Full
            },
            "hoverProvider": true,
            "definitionProvider": true,
            "completionProvider": {
                "resolveProvider": false,
                "triggerCharacters": ["."]
            }
        });

        let initialization_params = self.connection.initialize(server_capabilities)?;
        info!("LSP initialized with params: {:?}", initialization_params);

        loop {
            let msg = self.connection.receiver.recv()?;
            match msg {
                Message::Request(req) => {
                    if self.connection.handle_shutdown(&req)? {
                        return Ok(());
                    }
                    self.handle_request(req);
                }
                Message::Response(resp) => self.handle_response(resp),
                Message::Notification(notif) => self.handle_notification(notif),
            }
        }
    }

    fn handle_request(&self, req: Request) {
        match req.method.as_str() {
            "textDocument/hover" => {
                if let Ok(params) = serde_json::from_value(req.params) {
                    self.handle_hover(req.id, params);
                }
            }
            "textDocument/definition" => {
                if let Ok(params) = serde_json::from_value(req.params) {
                    self.handle_definition(req.id, params);
                }
            }
            "textDocument/completion" => {
                if let Ok(params) = serde_json::from_value(req.params) {
                    self.handle_completion(req.id, params);
                }
            }
            _ => {
                warn!("Unknown request method: {}", req.method);
            }
        }
    }

    fn handle_notification(&mut self, notif: Notification) {
        match notif.method.as_str() {
            "textDocument/didOpen" => {
                if let Ok(params) = serde_json::from_value(notif.params) {
                    self.handle_did_open_json(params);
                }
            }
            "textDocument/didChange" => {
                if let Ok(params) = serde_json::from_value(notif.params) {
                    self.handle_did_change_json(params);
                }
            }
            _ => {
                // Ignore unknown notifications
            }
        }
    }

    fn handle_did_open_json(&mut self, params: Value) {
        let uri = params["textDocument"]["uri"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let text = params["textDocument"]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let version = params["textDocument"]["version"].as_i64().unwrap_or(0) as i32;

        {
            let mut docs = self.documents.lock().unwrap();
            docs.docs.insert(
                uri.clone(),
                DocumentState {
                    text: text.clone(),
                    version,
                    program: None,
                },
            );
        }

        self.check_document(&uri, &text);
    }

    fn handle_did_change_json(&mut self, params: Value) {
        let uri = params["textDocument"]["uri"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let version = params["textDocument"]["version"].as_i64().unwrap_or(0) as i32;
        let text = params["contentChanges"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        {
            let mut docs = self.documents.lock().unwrap();
            if let Some(doc) = docs.docs.get_mut(&uri) {
                doc.text = text.clone();
                doc.version = version;
            } else {
                return;
            }
        }

        self.check_document(&uri, &text);
    }

    fn check_document(&self, uri: &str, text: &str) {
        let (diagnostics, program) = self.run_type_check(uri, text);

        {
            let mut docs = self.documents.lock().unwrap();
            if let Some(doc) = docs.docs.get_mut(uri) {
                doc.program = program;
            }
        }

        let params = serde_json::json!({
            "uri": uri,
            "diagnostics": diagnostics
        });

        let notif = Notification::new("textDocument/publishDiagnostics".to_string(), params);
        let _ = self.connection.sender.send(Message::Notification(notif));
    }

    fn run_type_check(&self, uri: &str, text: &str) -> (Vec<Value>, Option<Program>) {
        let is_rbv = uri.ends_with(".rbv");
        let source = self.extract_brief_source(text, is_rbv);

        let mut parser = parser::Parser::new(&source);
        let mut program = match parser.parse() {
            Ok(p) => p,
            Err(e) => {
                return (
                    vec![serde_json::json!({
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 0 }
                        },
                        "severity": 1,
                        "source": "brief-parser",
                        "message": format!("Parse error: {}", e)
                    })],
                    None,
                );
            }
        };

        let mut tc = typechecker::TypeChecker::new();
        tc.check_program(&mut program);
        let type_diagnostics = tc.get_diagnostics();

        let mut pe = proof_engine::ProofEngine::new();
        let proof_errors = pe.verify_program(&program);

        let mut diagnostics = Vec::new();

        for diag in type_diagnostics {
            diagnostics.push(self.diagnostic_to_json(&diag));
        }

        for err in proof_errors {
            diagnostics.push(self.proof_error_to_json(&err));
        }

        (diagnostics, Some(program))
    }

    fn extract_brief_source(&self, source: &str, is_rbv: bool) -> String {
        if !is_rbv {
            return source.to_string();
        }

        let mut output = String::with_capacity(source.len());
        let mut in_script = false;
        let mut current_pos = 0;

        while current_pos < source.len() {
            if !in_script {
                if source[current_pos..].starts_with("<script") {
                    let after_script = &source[current_pos + 7..];
                    let next_char = after_script.chars().next();
                    let is_real_script_tag = next_char.is_none()
                        || next_char == Some('>')
                        || next_char == Some(' ')
                        || next_char == Some('\t')
                        || next_char == Some('\n');

                    if is_real_script_tag {
                        if let Some(tag_end_rel) = source[current_pos..].find('>') {
                            let tag_end = current_pos + tag_end_rel + 1;
                            for c in source[current_pos..tag_end].chars() {
                                if c == '\n' {
                                    output.push('\n');
                                } else {
                                    output.push(' ');
                                }
                            }
                            current_pos = tag_end;
                            in_script = true;
                            continue;
                        }
                    }
                }
                // Not a script start, mask it
                let c = source[current_pos..].chars().next().unwrap();
                if c == '\n' {
                    output.push('\n');
                } else {
                    output.push(' ');
                }
                current_pos += c.len_utf8();
            } else {
                if source[current_pos..].starts_with("</script>") {
                    in_script = false;
                    for c in "</script>".chars() {
                        if c == '\n' {
                            output.push('\n');
                        } else {
                            output.push(' ');
                        }
                        current_pos += c.len_utf8();
                    }
                    continue;
                }
                // Inside script, keep it
                let c = source[current_pos..].chars().next().unwrap();
                output.push(c);
                current_pos += c.len_utf8();
            }
        }
        output
    }

    fn diagnostic_to_json(&self, diag: &Diagnostic) -> Value {
        let severity = match diag.severity {
            Severity::Error => 1,
            Severity::Warning => 2,
            Severity::Info => 3,
            Severity::Note => 4,
        };

        let range = if let Some(span) = diag.span {
            serde_json::json!({
                "start": { "line": span.line.saturating_sub(1), "character": span.column.saturating_sub(1) },
                "end": { "line": span.line.saturating_sub(1), "character": span.column + 1 }
            })
        } else {
            serde_json::json!({
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 1 }
            })
        };

        let mut message = diag.title.clone();
        if !diag.explanation.is_empty() {
            message.push_str("\n\n");
            message.push_str(&diag.explanation.join("\n"));
        }
        if !diag.hints.is_empty() {
            message.push_str("\n\nhint: ");
            message.push_str(&diag.hints.join("\n"));
        }

        serde_json::json!({
            "range": range,
            "severity": severity,
            "code": diag.code,
            "source": "brief",
            "message": message
        })
    }

    fn proof_error_to_json(&self, err: &proof_engine::ProofError) -> Value {
        let range = if let Some(span) = err.span {
            serde_json::json!({
                "start": { "line": span.line.saturating_sub(1), "character": span.column.saturating_sub(1) },
                "end": { "line": span.line.saturating_sub(1), "character": span.column + 1 }
            })
        } else {
            serde_json::json!({
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 1 }
            })
        };

        serde_json::json!({
            "range": range,
            "severity": if err.is_warning { 2 } else { 1 },
            "code": err.code,
            "source": "brief-proof",
            "message": format!("{}: {}", err.title, err.explanation)
        })
    }

    fn handle_completion(&self, id: lsp_server::RequestId, _params: Value) {
        let keywords = vec![
            "txn", "rct", "let", "const", "sig", "defn", "trg", "import", "from", "term", "escape",
            "async", "Int", "UInt", "Float", "String", "Bool", "Data", "Void",
        ];

        let completions: Vec<Value> = keywords
            .into_iter()
            .map(|k| {
                serde_json::json!({
                    "label": k,
                    "kind": 14, // Keyword
                })
            })
            .collect();

        let resp = Response::new_ok(id, completions);
        let _ = self.connection.sender.send(Message::Response(resp));
    }

    fn handle_hover(&self, id: lsp_server::RequestId, params: Value) {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
        let line = params["position"]["line"].as_u64().unwrap_or(0) as usize + 1;
        let character = params["position"]["character"].as_u64().unwrap_or(0) as usize + 1;

        let docs = self.documents.lock().unwrap();
        if let Some(doc) = docs.docs.get(uri) {
            if let Some(program) = &doc.program {
                for item in &program.items {
                    if let Some(span) = item_span(item) {
                        let name = item_name(item);
                        if line == span.line
                            && character >= span.column
                            && character <= span.column + name.len()
                        {
                            let content = format!("**{}**\n\n{}", name, item_description(item));
                            let result = serde_json::json!({
                                "contents": {
                                    "kind": "markdown",
                                    "value": content
                                }
                            });
                            let resp = Response::new_ok(id, result);
                            let _ = self.connection.sender.send(Message::Response(resp));
                            return;
                        }
                    }
                }
            }
        }

        let resp = Response::new_ok(id, serde_json::Value::Null);
        let _ = self.connection.sender.send(Message::Response(resp));
    }

    fn handle_definition(&self, id: lsp_server::RequestId, params: Value) {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
        let line = params["position"]["line"].as_u64().unwrap_or(0) as usize + 1;
        let character = params["position"]["character"].as_u64().unwrap_or(0) as usize + 1;

        let docs = self.documents.lock().unwrap();
        if let Some(doc) = docs.docs.get(uri) {
            if let Some(program) = &doc.program {
                for item in &program.items {
                    if let Some(span) = item_span(item) {
                        let name = item_name(item);
                        if line == span.line
                            && character >= span.column
                            && character <= span.column + name.len()
                        {
                            let result = serde_json::json!({
                                "uri": uri,
                                "range": {
                                    "start": { "line": span.line - 1, "character": span.column - 1 },
                                    "end": { "line": span.line - 1, "character": span.column + name.len() - 1 }
                                }
                            });
                            let resp = Response::new_ok(id, result);
                            let _ = self.connection.sender.send(Message::Response(resp));
                            return;
                        }
                    }
                }
            }
        }

        let resp = Response::new_ok(id, serde_json::Value::Null);
        let _ = self.connection.sender.send(Message::Response(resp));
    }

    fn handle_response(&self, _resp: Response) {}
}

fn item_span(item: &TopLevel) -> Option<Span> {
    match item {
        TopLevel::Transaction(t) => t.span,
        TopLevel::StateDecl(s) => s.span,
        TopLevel::Trigger(t) => t.span,
        TopLevel::Struct(s) => s.span,
        TopLevel::Enum(e) => e.span,
        TopLevel::ForeignBinding { span, .. } => *span,
        TopLevel::Definition(d) => d.contract.span,
        _ => None,
    }
}

fn item_name(item: &TopLevel) -> String {
    match item {
        TopLevel::Signature(s) => s.name.clone(),
        TopLevel::Definition(d) => d.name.clone(),
        TopLevel::Transaction(t) => t.name.clone(),
        TopLevel::StateDecl(s) => s.name.clone(),
        TopLevel::Trigger(t) => t.name.clone(),
        TopLevel::Constant(c) => c.name.clone(),
        TopLevel::Struct(s) => s.name.clone(),
        TopLevel::Enum(e) => e.name.clone(),
        TopLevel::ForeignBinding { name, .. } => name.clone(),
        _ => "unnamed".to_string(),
    }
}

fn item_description(item: &TopLevel) -> String {
    match item {
        TopLevel::Transaction(t) => format!(
            "transaction{}{}",
            if t.is_async { " async" } else { "" },
            if t.is_reactive { " rct" } else { "" }
        ),
        TopLevel::StateDecl(_) => "state variable".to_string(),
        TopLevel::Trigger(_) => "hardware trigger".to_string(),
        TopLevel::Signature(_) => "function signature".to_string(),
        TopLevel::Definition(_) => "function definition".to_string(),
        TopLevel::Constant(_) => "constant".to_string(),
        TopLevel::Struct(_) => "struct".to_string(),
        TopLevel::Enum(_) => "enum".to_string(),
        TopLevel::ForeignBinding { .. } => "foreign binding".to_string(),
        _ => "".to_string(),
    }
}

pub fn run_lsp_server(_mode: ErrorMode) {
    let mut server = LspServer::new().expect("Failed to create LSP server");
    if let Err(e) = server.run() {
        eprintln!("LSP server error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_brief_source_rbv() {
        let lsp = LspServer {
            connection: Connection::stdio().0,
            documents: Arc::new(Mutex::new(DocumentStore {
                docs: HashMap::new(),
            })),
        };

        let rbv_source = r#"
<script type="brief">
let x: Int = 10;
</script>
<view>
  <div>Test</div>
</view>
"#;
        let extracted = lsp.extract_brief_source(rbv_source, true);

        // The script tag should be replaced by spaces/newlines
        assert!(extracted.contains("let x: Int = 10;"));
        assert!(!extracted.contains("<script"));
        assert!(!extracted.contains("<view>"));
        assert!(!extracted.contains("<div>"));

        // Lines should be preserved
        let original_lines: Vec<&str> = rbv_source.lines().collect();
        let extracted_lines: Vec<&str> = extracted.lines().collect();
        assert_eq!(original_lines.len(), extracted_lines.len());

        // Line 3 (1-based) should contain the code
        assert!(extracted_lines[2].contains("let x: Int = 10;"));
    }

    #[test]
    fn test_extract_brief_source_rbv_with_other_tags() {
        let lsp = LspServer {
            connection: Connection::stdio().0,
            documents: Arc::new(Mutex::new(DocumentStore {
                docs: HashMap::new(),
            })),
        };

        let rbv_source = r#"
<p>This is <scripting> test</p>
<script>
let x = 1;
</script>
"#;
        let extracted = lsp.extract_brief_source(rbv_source, true);

        // <scripting> should be masked
        assert!(!extracted.contains("<scripting>"));
        // let x = 1; should be preserved
        assert!(extracted.contains("let x = 1;"));
    }

    #[test]
    fn test_extract_brief_source_bv() {
        let lsp = LspServer {
            connection: Connection::stdio().0,
            documents: Arc::new(Mutex::new(DocumentStore {
                docs: HashMap::new(),
            })),
        };

        let bv_source = "let x: Int = 10;";
        let extracted = lsp.extract_brief_source(bv_source, false);
        assert_eq!(extracted, bv_source);
    }
}
