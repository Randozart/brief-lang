use crate::ast::{MacroArgType, Statement};
use crate::errors::Span;
use std::collections::HashMap;

/// Shared state for macro/template expansion.
pub struct MacroContext {
    pub gensym_counter: u64,
    pub budget: u64,
    pub safe_mode: bool,
    pub call_site_span: Option<Span>,
    pub warnings: Vec<String>,
    pub templates: HashMap<String, TemplateDef>,
    pub macros: HashMap<String, MacroDef>,
    // 2026-07-01: Convergence max iterations for interpreters created during
    // macro/template expansion. Passed through from --txn-convergence-max-iterations.
    pub txn_convergence_max_iterations: u64,
}

#[derive(Debug, Clone)]
pub struct TemplateDef {
    pub name: String,
    pub params: Vec<(String, MacroArgType)>,
    pub return_type: Option<MacroArgType>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct MacroDef {
    pub name: String,
    pub params: Vec<(String, MacroArgType)>,
    pub return_type: Option<MacroArgType>,
    pub body: Vec<Statement>,
}

impl MacroContext {
    pub fn new() -> Self {
        MacroContext {
            gensym_counter: 0,
            budget: 10_000,
            safe_mode: false,
            call_site_span: None,
            warnings: Vec::new(),
            templates: HashMap::new(),
            macros: HashMap::new(),
            txn_convergence_max_iterations: 10_000,
        }
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
        eprintln!("warning: {}", msg);
    }

    pub fn emit_error(&self, msg: &str) -> String {
        if let Some(ref span) = self.call_site_span {
            format!("error at line {}: {}", span.line, msg)
        } else {
            format!("error: {}", msg)
        }
    }

    pub fn next_gensym(&mut self) -> String {
        let n = self.gensym_counter;
        self.gensym_counter += 1;
        format!("__gensym_{}", n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_budget() {
        let ctx = MacroContext::new();
        assert_eq!(ctx.budget, 10_000);
        assert!(!ctx.safe_mode);
    }

    #[test]
    fn test_unlimited_budget() {
        let mut ctx = MacroContext::new();
        ctx.budget = u64::MAX;
        assert_eq!(ctx.budget, u64::MAX);
    }

    #[test]
    fn test_safe_mode() {
        let mut ctx = MacroContext::new();
        ctx.safe_mode = true;
        assert!(ctx.safe_mode);
    }
}
