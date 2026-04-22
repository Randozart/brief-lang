// Copyright 2026 Randy Smits-Schreuder Goedheijt
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Runtime Exception for Use as a Language:
// When the Work or any Derivative Work thereof is used to generate code
// ("generated code"), such generated code shall not be subject to the
// terms of this License, provided that the generated code itself is not
// a Derivative Work of the Work. This exception does not apply to code
// that is itself a compiler, interpreter, or similar tool that incorporates
// or embeds the Work.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Span {
            start,
            end,
            line,
            column,
        }
    }

    pub fn dummy() -> Self {
        Span {
            start: 0,
            end: 0,
            line: 0,
            column: 0,
        }
    }

    pub fn format(&self, source: &str) -> String {
        let lines: Vec<&str> = source.lines().collect();
        if self.line > 0 && self.line <= lines.len() {
            let line_content = lines[self.line - 1];
            let pointer = " ".repeat(self.column.saturating_sub(1)) + "^";
            format!(
                " --> {}:{}:{}\n  |\n{} | {}\n{} | {}",
                "file", self.line, self.column, self.line, line_content, self.line, pointer
            )
        } else {
            format!(" --> {}:{}:{}", "file", self.line, self.column)
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub title: String,
    pub span: Option<Span>,
    pub source_snippet: Option<String>,
    pub explanation: Vec<String>,
    pub proof_chain: Vec<String>,
    pub examples: Vec<String>,
    pub hints: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorMode {
    Verbose,
    Whisper,
}

impl Diagnostic {
    pub fn new(code: &str, severity: Severity, title: &str) -> Self {
        Diagnostic {
            code: code.to_string(),
            severity,
            title: title.to_string(),
            span: None,
            source_snippet: None,
            explanation: Vec::new(),
            proof_chain: Vec::new(),
            examples: Vec::new(),
            hints: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_explanation(mut self, text: &str) -> Self {
        self.explanation.push(text.to_string());
        self
    }

    pub fn with_proof_step(mut self, step: &str) -> Self {
        self.proof_chain.push(step.to_string());
        self
    }

    pub fn with_example(mut self, example: &str) -> Self {
        self.examples.push(example.to_string());
        self
    }

    pub fn with_hint(mut self, hint: &str) -> Self {
        self.hints.push(hint.to_string());
        self
    }

    pub fn with_note(mut self, note: &str) -> Self {
        self.notes.push(note.to_string());
        self
    }

    pub fn format(&self, source: &str, file_name: &str) -> String {
        self.format_with_mode(source, file_name, ErrorMode::Verbose)
    }

    pub fn format_with_mode(&self, source: &str, file_name: &str, mode: ErrorMode) -> String {
        match mode {
            ErrorMode::Verbose => self.format_verbose(source, file_name),
            ErrorMode::Whisper => self.format_whisper(source, file_name),
        }
    }

    fn format_verbose(&self, source: &str, file_name: &str) -> String {
        let mut output = String::new();

        let severity_str = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
            Severity::Info => "info",
        };

        if let Some(span) = &self.span {
            output.push_str(&format!(
                "{}: {} [{}{}]\n --> {}:{}\n",
                severity_str,
                self.title,
                self.code,
                if self.code.is_empty() {
                    "".to_string()
                } else {
                    ", ".to_string()
                },
                file_name,
                span
            ));
        } else {
            output.push_str(&format!(
                "{}: {} [{}{}]\n",
                severity_str,
                self.title,
                self.code,
                if self.code.is_empty() {
                    "".to_string()
                } else {
                    ", ".to_string()
                },
            ));
        }

        if let Some(span) = &self.span {
            if self.severity == Severity::Error {
                let lines: Vec<&str> = source.lines().collect();
                if span.line > 0 && span.line <= lines.len() {
                    let line_content = lines[span.line - 1];
                    let line_str = format!("{}", span.line);
                    let padding = " ".repeat(line_str.len());

                    output.push_str(&format!("  |\n"));
                    output.push_str(&format!("{} | {}\n", line_str, line_content));
                    output.push_str(&format!(
                        "{} | {}\n",
                        padding,
                        " ".repeat(span.column.saturating_sub(1)) + "^"
                    ));
                }
            }
        }

        for line in &self.explanation {
            output.push_str(&format!("  |\n  = {}\n", line));
        }

        if !self.proof_chain.is_empty() {
            output.push_str("  |\n  = proof:\n");
            for (i, step) in self.proof_chain.iter().enumerate() {
                let prefix = if i == 0 {
                    "  =   ".to_string()
                } else {
                    "  =     ".to_string()
                };
                output.push_str(&format!("{}• {}\n", prefix, step));
            }
        }

        if !self.examples.is_empty() {
            output.push_str("  |\n  = example failure:\n");
            for example in &self.examples {
                output.push_str(&format!("  =   {}\n", example));
            }
        }

        if !self.hints.is_empty() {
            output.push_str("  |\n  = hint:");
            for hint in &self.hints {
                output.push_str(&format!(" {}\n", hint));
            }
        }

        if !self.notes.is_empty() {
            output.push_str("  |\n");
            for note in &self.notes {
                output.push_str(&format!("  = note: {}\n", note));
            }
        }

        output
    }

    fn format_whisper(&self, source: &str, file_name: &str) -> String {
        let severity_str = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
            Severity::Info => "info",
        };

        let mut parts = Vec::new();

        if let Some(span) = &self.span {
            parts.push(format!("{}:{}:{}", file_name, span.line, span.column));
        }

        parts.push(format!("[{}]", self.code));
        parts.push(self.title.clone());

        if !self.explanation.is_empty() {
            let hint = self
                .explanation
                .first()
                .map(|s| if s.len() > 50 { &s[..50] } else { s.as_str() })
                .unwrap_or("");
            if !hint.is_empty() {
                parts.push(format!("({})", hint));
            }
        }

        if !self.hints.is_empty() {
            if let Some(first_hint) = self.hints.first() {
                if first_hint.starts_with("did you mean") {
                    parts.push(
                        first_hint
                            .replace("did you mean ", "try: ")
                            .replace("'", "")
                            .replace("?", ""),
                    );
                } else {
                    parts.push(first_hint.chars().take(40).collect::<String>());
                }
            }
        }

        format!("{} {}\n", severity_str, parts.join(" "))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
    Info,
}

#[derive(Debug, Clone)]
pub enum TypeError {
    UndefinedVariable {
        name: String,
        available: Vec<String>,
    },
    TypeMismatch {
        expected: String,
        found: String,
        context: String,
    },
    UninitializedSignal {
        name: String,
    },
    OwnershipViolation {
        var: String,
        reason: String,
    },
    InvalidOperation {
        operation: String,
        type_name: String,
    },
    FFIError {
        message: String,
    },
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeError::UndefinedVariable { name, .. } => {
                write!(f, "undefined variable '{}'", name)
            }
            TypeError::TypeMismatch {
                expected,
                found,
                context,
                ..
            } => {
                write!(
                    f,
                    "type mismatch: expected {} for {}, found {}",
                    expected, context, found
                )
            }
            TypeError::UninitializedSignal { name, .. } => {
                write!(f, "signal '{}' has no initial value", name)
            }
            TypeError::OwnershipViolation { var, reason, .. } => {
                write!(f, "ownership violation on '{}': {}", var, reason)
            }
            TypeError::InvalidOperation {
                operation,
                type_name,
                ..
            } => {
                write!(f, "invalid operation '{}' on type {}", operation, type_name)
            }
            TypeError::FFIError { message, .. } => {
                write!(f, "FFI error: {}", message)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ProofError {
    UnreachableState {
        transaction: String,
        precondition: String,
        reason: String,
        proof_trace: Vec<String>,
        span: Span,
    },
    PostconditionUnsatisfiable {
        transaction: String,
        postcondition: String,
        reason: String,
        example_values: Vec<String>,
        suggestion: String,
        span: Span,
    },
    NoAcceptingPath {
        transaction: String,
        reason: String,
        rollback_count: usize,
        span: Span,
    },
    MutualExclusionViolation {
        txn1: String,
        txn2: String,
        shared_vars: Vec<String>,
        conflict_description: String,
        span: Span,
    },
    UnhandledOutcome {
        signature: String,
        union_type: String,
        missing_variants: Vec<String>,
        span: Span,
    },
    TrueAssertionFailure {
        signature: String,
        reason: String,
        proof_steps: Vec<String>,
        span: Span,
    },
    CircularDependency {
        transactions: Vec<String>,
        call_chain: Vec<String>,
        span: Span,
    },
    ImpossiblePrecondition {
        condition: String,
        contradiction: String,
        span: Span,
    },
    PostconditionMutationViolation {
        transaction: String,
        postcondition: String,
        mutation: String,
        explanation: String,
        span: Span,
    },
    TrivialPrecondition {
        item_name: String,
        item_type: String,
        span: Span,
    },
    TrivialPostcondition {
        item_name: String,
        item_type: String,
        span: Span,
    },
}

impl fmt::Display for ProofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProofError::UnreachableState { transaction, .. } => {
                write!(
                    f,
                    "transaction '{}' has unreachable precondition",
                    transaction
                )
            }
            ProofError::PostconditionUnsatisfiable { transaction, .. } => {
                write!(
                    f,
                    "transaction '{}' postcondition cannot be satisfied",
                    transaction
                )
            }
            ProofError::NoAcceptingPath { transaction, .. } => {
                write!(f, "transaction '{}' has no valid termination", transaction)
            }
            ProofError::MutualExclusionViolation { txn1, txn2, .. } => {
                write!(
                    f,
                    "transactions '{}' and '{}' have unsafe concurrent access",
                    txn1, txn2
                )
            }
            ProofError::UnhandledOutcome { signature, .. } => {
                write!(f, "unhandled outcome for signature '{}'", signature)
            }
            ProofError::TrueAssertionFailure { signature, .. } => {
                write!(f, "true assertion failed for signature '{}'", signature)
            }
            ProofError::CircularDependency { .. } => {
                write!(f, "circular transaction dependency detected")
            }
            ProofError::ImpossiblePrecondition { condition, .. } => {
                write!(f, "precondition '{}' is impossible to satisfy", condition)
            }
            ProofError::PostconditionMutationViolation { transaction, .. } => {
                write!(
                    f,
                    "transaction '{}' postcondition references mutated state incorrectly",
                    transaction
                )
            }
            ProofError::TrivialPrecondition {
                item_name,
                item_type,
                ..
            } => {
                write!(
                    f,
                    "{} '{}' has a trivial precondition '[true]'",
                    item_type, item_name
                )
            }
            ProofError::TrivialPostcondition {
                item_name,
                item_type,
                ..
            } => {
                write!(
                    f,
                    "{} '{}' has a trivial postcondition '[true]'",
                    item_type, item_name
                )
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum SyntaxError {
    UnexpectedToken {
        expected: String,
        found: String,
        span: Span,
    },
    UnexpectedEOF {
        expected: String,
        span: Span,
    },
    InvalidExpression {
        reason: String,
        span: Span,
    },
    InvalidStatement {
        reason: String,
        span: Span,
    },
    InvalidType {
        type_name: String,
        span: Span,
    },
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyntaxError::UnexpectedToken {
                expected, found, ..
            } => {
                write!(f, "expected {}, found '{}'", expected, found)
            }
            SyntaxError::UnexpectedEOF { expected, .. } => {
                write!(f, "expected {}, found end of file", expected)
            }
            SyntaxError::InvalidExpression { reason, .. } => {
                write!(f, "invalid expression: {}", reason)
            }
            SyntaxError::InvalidStatement { reason, .. } => {
                write!(f, "invalid statement: {}", reason)
            }
            SyntaxError::InvalidType { type_name, .. } => {
                write!(f, "invalid type: '{}'", type_name)
            }
        }
    }
}

impl From<String> for SyntaxError {
    fn from(s: String) -> Self {
        SyntaxError::InvalidStatement {
            reason: s,
            span: Span::dummy(),
        }
    }
}

impl std::error::Error for SyntaxError {}

#[derive(Debug, Clone)]
pub enum ImportError {
    ModuleNotFound {
        module: String,
        search_paths: Vec<String>,
        span: Span,
    },
    CircularImport {
        module: String,
        import_chain: Vec<String>,
        span: Span,
    },
    InvalidImport {
        reason: String,
        span: Span,
    },
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::ModuleNotFound { module, .. } => {
                write!(f, "module '{}' not found", module)
            }
            ImportError::CircularImport { module, .. } => {
                write!(f, "circular import detected for module '{}'", module)
            }
            ImportError::InvalidImport { reason, .. } => {
                write!(f, "invalid import: {}", reason)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ContractError {
    PreconditionUnsatisfiable {
        condition: String,
        proof: String,
        span: Span,
    },
    PostconditionUnsatisfiable {
        condition: String,
        proof: String,
        span: Span,
    },
    GuardViolation {
        guard: String,
        explanation: String,
        span: Span,
    },
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContractError::PreconditionUnsatisfiable { condition, .. } => {
                write!(f, "precondition '{}' can never be true", condition)
            }
            ContractError::PostconditionUnsatisfiable { condition, .. } => {
                write!(f, "postcondition '{}' can never be true", condition)
            }
            ContractError::GuardViolation { guard, .. } => {
                write!(f, "guard '{}' violation", guard)
            }
        }
    }
}
