#!/usr/bin/env python3

path = "/home/randozart/Desktop/Projects/briev-compiler/src/lsp.rs"
with open(path) as f:
    content = f.read()

# 1. Add import_resolver
content = content.replace(
    "use crate::parser;",
    "use crate::import_resolver;\nuse crate::parser;",
    1
)

# 2. Add PathBuf
content = content.replace(
    "use std::sync::{Arc, Mutex};",
    "use std::path::PathBuf;\nuse std::sync::{Arc, Mutex};",
    1
)

# 3. Add SymbolEntry and SymbolTable structs after import block
old = "use tracing::{error, info, warn};"
lt = chr(60)
gt = chr(62)
sym_entry = "SymbolEntry"

insert = old + "\n\n"
insert += "/// A symbol table entry built from the AST.\n"
insert += "#[derive(Debug, Clone)]\n"
insert += "struct SymbolEntry {\n"
insert += "    name: String,\n"
insert += "    kind: String,\n"
insert += "    span: Span,\n"
insert += "    description: String,\n"
insert += "}\n\n"
insert += "/// Index of program symbols by name for fast lookup.\n"
insert += "struct SymbolTable {\n"
insert += "    symbols: Vec" + lt + sym_entry + gt + ",\n"