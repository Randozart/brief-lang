#!/usr/bin/env python3
import re

path = '/home/randozart/Desktop/Projects/brief-compiler/src/lsp.rs'
content = open(path, 'r').read()

# Add import_resolver and PathBuf to imports
content = content.replace(
    'use crate::parser;',
    'use crate::import_resolver;\nuse crate::parser;'
)
content = content.replace(
    'use std::sync::{Arc, Mutex};',
    'use std::path::PathBuf;\nuse std::sync::{Arc, Mutex};'
)

# Find the line after the tracing import and insert structs
old = 'use tracing::{error, info, warn};'

insert = '''/// A symbol table entry built from the AST.
#[derive(Debug, Clone)]
struct SymbolEntry {
    name: String,
    kind: String,
    span: Span,
    description: String,
}

/// Index of program symbols by name for fast lookup.
struct SymbolTable {
    symbols: Vec