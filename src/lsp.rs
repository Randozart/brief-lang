use crate::errors::ErrorMode;
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
        info!("Starting Brief LSP server");

        loop {
            let msg = self.connection.receiver.recv()?;
            match msg {
                Message::Request(req) => self.handle_request(req),
                Message::Response(resp) => self.handle_response(resp),
                Message::Notification(notif) => self.handle_notification(notif),
            }
        }
    }

    fn handle_request(&self, req: Request) {
        info!("Handling request: {:?}", req.method);

        match req.method.as_str() {
            "initialize" => {
                let result = serde_json::json!({
                    "capabilities": {
                        "textDocumentSync": {
                            "kind": "incremental"
                        }
                    },
                    "serverInfo": {
                        "name": "Brief Language Server",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                });
                let resp = Response::new_ok(req.id, result);
                let _ = self.connection.sender.send(Message::Response(resp));
            }
            "shutdown" => {
                let resp = Response::new_ok(req.id, ());
                let _ = self.connection.sender.send(Message::Response(resp));
            }
            _ => {
                error!("Unknown request method: {}", req.method);
            }
        }
    }

    fn handle_response(&self, _resp: Response) {}

    fn handle_notification(&mut self, notif: Notification) {
        info!("Handling notification: {}", notif.method);

        match notif.method.as_str() {
            "initialized" => {
                info!("Client initialized");
            }
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
            "exit" => {
                info!("Received exit notification");
                std::process::exit(0);
            }
            _ => {
                warn!("Unknown notification method: {}", notif.method);
            }
        }
    }

    fn handle_did_open_json(&mut self, params: Value) {
        let uri = params
            .get("textDocument")
            .and_then(|td| td.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();

        let text = params
            .get("textDocument")
            .and_then(|td| td.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();

        let version = params
            .get("textDocument")
            .and_then(|td| td.get("version"))
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as i32;

        info!("Opening document: {}", uri);

        {
            let mut docs = self.documents.lock().unwrap();
            docs.docs.insert(
                uri.clone(),
                DocumentState {
                    text: text.clone(),
                    version,
                },
            );
        }

        self.check_document(&uri, &text);
    }

    fn handle_did_change_json(&mut self, params: Value) {
        let uri = params
            .get("textDocument")
            .and_then(|td| td.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();

        let version = params
            .get("textDocument")
            .and_then(|td| td.get("version"))
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as i32;

        let changes = params
            .get("contentChanges")
            .and_then(|cc| cc.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let new_text = {
            let mut docs = self.documents.lock().unwrap();
            if let Some(doc) = docs.docs.get_mut(&uri) {
                for change in changes {
                    doc.text = change.to_string();
                }
                doc.version = version;
                doc.text.clone()
            } else {
                return;
            }
        };

        self.check_document(&uri, &new_text);
    }

    fn check_document(&self, uri: &str, text: &str) {
        info!("Checking document: {}", uri);

        let diagnostics = self.run_type_check(text);

        let params = serde_json::json!({
            "uri": uri,
            "diagnostics": diagnostics
        });

        let notif = Notification::new("textDocument/publishDiagnostics".to_string(), params);
        let _ = self.connection.sender.send(Message::Notification(notif));
    }

    fn run_type_check(&self, source: &str) -> Vec<Value> {
        let mut parser = parser::Parser::new(source);
        let mut program = match parser.parse() {
            Ok(p) => p,
            Err(e) => {
                return vec![serde_json::json!({
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 0 }
                    },
                    "severity": 1,
                    "message": e
                })];
            }
        };

        let mut tc = typechecker::TypeChecker::new();
        let type_errors = tc.check_program(&mut program);

        let mut pe = proof_engine::ProofEngine::new();
        let proof_errors = pe.verify_program(&program);

        let mut diagnostics = Vec::new();

        for err in type_errors {
            let diag = self.type_error_to_json(&err);
            diagnostics.push(diag);
        }

        for err in proof_errors {
            let diag = self.proof_error_to_json(&err);
            diagnostics.push(diag);
        }

        diagnostics
    }

    fn type_error_to_json(&self, err: &typechecker::TypeError) -> Value {
        use crate::errors::TypeError;

        let message = match err {
            TypeError::UndefinedVariable { name, .. } => {
                format!("undefined variable '{}'", name)
            }
            TypeError::TypeMismatch {
                expected,
                found,
                context,
                ..
            } => {
                format!("expected {} for {}, but found {}", expected, context, found)
            }
            TypeError::UninitializedSignal { name, .. } => {
                format!("signal '{}' has no initial value", name)
            }
            TypeError::OwnershipViolation { var, reason, .. } => {
                format!("ownership violation on '{}': {}", var, reason)
            }
            TypeError::InvalidOperation {
                operation,
                type_name,
                ..
            } => {
                format!("invalid operation '{}' on type {}", operation, type_name)
            }
            TypeError::FFIError { message, .. } => {
                format!("FFI error: {}", message)
            }
        };

        serde_json::json!({
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 0 }
            },
            "severity": 1,
            "message": message
        })
    }

    fn proof_error_to_json(&self, err: &proof_engine::ProofError) -> Value {
        serde_json::json!({
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 0 }
            },
            "severity": 1,
            "message": format!("[{}] {}", err.code, err.title)
        })
    }
}

pub fn run_lsp_server(_mode: ErrorMode) {
    let mut server = LspServer::new().expect("Failed to create LSP server");
    if let Err(e) = server.run() {
        eprintln!("LSP server error: {}", e);
        std::process::exit(1);
    }
}
