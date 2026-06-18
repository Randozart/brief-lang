use crate::ast::{MacroArgType, Statement};
use std::collections::HashMap;

/// Shared state for macro/template expansion.
pub struct MacroContext {
    pub gensym_counter: u64,
    pub budget: u64,
    pub templates: HashMap<String, TemplateDef>,
    pub macros: HashMap<String, MacroDef>,
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
            templates: HashMap::new(),
            macros: HashMap::new(),
        }
    }

    pub fn next_gensym(&mut self) -> String {
        let n = self.gensym_counter;
        self.gensym_counter += 1;
        format!("__gensym_{}", n)
    }
}
