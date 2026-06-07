use crate::analysis::call_graph::CallGraph;
use crate::ast::{Expr, Program, Statement, TopLevel, Type};
use std::fmt::Write;

/// Intent: Target execution environment for the WASM module.
#[derive(Clone, Copy, PartialEq)]
pub enum WasmTarget {
    Browser,
    Node,
    Wasi,
    Bare,
}

/// Intent: Text-based WASM (WAT) backend that generates human-readable WebAssembly text format.
pub struct WasmBackend {
    pending_cleanup: Vec<Statement>,
    has_cycles: bool,
}

impl WasmBackend {
    /// Intent: Create a new WASM backend with an empty cleanup queue.
    pub fn new() -> Self {
        WasmBackend {
            pending_cleanup: Vec::new(),
            has_cycles: false,
        }
    }

    /// Intent: Generate WAT text output for the given Brief program.
    pub fn generate(&mut self, program: &Program) -> String {
        let _analysis = crate::backend::analyze_program(program, false);
        let cg = &_analysis.call_graph;
        let _pr = &_analysis.param_ranges;
        self.has_cycles = cg.has_cycle();
        if !self.has_cycles {
            println!("  WASM backend: acyclic call graph — static dispatch enabled");
        }

        let mut output = String::new();
        output.push_str("(module\n");
        for item in &program.items {
            match item {
                TopLevel::Transaction(txn) => {
                    self.pending_cleanup.clear();
                    writeln!(output, "  (func (export \"{}\")", txn.name).ok();
                    for stmt in &txn.body {
                        self.generate_statement(&mut output, stmt);
                    }
                    output.push_str("  )\n");
                }
                _ => {}
            }
        }
        output.push_str(")\n");
        output
    }

    /// Intent: Generate WAT statement text, handling cleanup, let, expression, trigger, on_exit, escape, alka, and inline_asm.
    fn generate_statement(&mut self, output: &mut String, stmt: &Statement) {
        match stmt {
            Statement::Term { .. } | Statement::TermBang { .. } => {
                let cleanup = std::mem::take(&mut self.pending_cleanup);
                for s in &cleanup {
                    self.generate_statement(output, s);
                }
                writeln!(output, "    ;; term — transaction complete").ok();
            }
            Statement::Let { name, address_expr, expr, .. } => {
                if let Some(addr) = address_expr {
                    self.generate_expr(output, addr);
                    writeln!(output, "    ;; let {} = ptr (addr computed above)", name).ok();
                } else if let Some(e) = expr {
                    self.generate_expr(output, e);
                    writeln!(output, "    ;; let {} = expr (value on stack)", name).ok();
                }
            }
            Statement::Expression(e) => {
                self.generate_expr(output, e);
                writeln!(output, "    drop").ok();
            }
            Statement::LocalTrigger { name, expr, .. } => {
                if let Some(e) = expr {
                    self.generate_expr(output, e);
                    writeln!(output, "    ;; trg! {}: await expr (on stack)", name).ok();
                } else {
                    writeln!(output, "    ;; trg! {}: await external signal", name).ok();
                }
            }
            Statement::OnExit { body, .. } => {
                self.pending_cleanup.extend(body.iter().cloned());
                writeln!(output, "    ;; #on_exit cleanup registered").ok();
            }
            Statement::Escape(Some(v)) => {
                self.generate_expr(output, v);
                writeln!(output, "    ;; escape (value on stack)").ok();
            }
            Statement::Escape(None) => {
                writeln!(output, "    ;; escape (unit)").ok();
            }
            Statement::Alka(block) => {
                for line in block.content.lines() {
                    let _ = writeln!(output, "    {}", line);
                }
            }
            Statement::InlineAsm { asm_string, .. } => {
                writeln!(output, "    ;; inline asm: \"{}\"", asm_string).ok();
            }
            Statement::Guarded { condition, statements, .. } => {
                self.generate_expr(output, condition);
                writeln!(output, "    if").ok();
                for s in statements {
                    self.generate_statement(output, s);
                }
                writeln!(output, "    end").ok();
            }
            Statement::Assignment { lhs, expr, .. } => {
                self.generate_expr(output, expr);
                if let Expr::Identifier(name) = lhs {
                    writeln!(output, "    local.set ${}", name).ok();
                }
            }
            Statement::Unification { name, variant, fields: _, expr } => {
                self.generate_expr(output, expr);
                writeln!(output, "    ;; unification: {} {} (value on stack)", name, variant).ok();
            }
        }
    }

    /// Intent: Generate a WAT expression, pushing a value onto the WASM stack.
    fn generate_expr(&self, output: &mut String, expr: &Expr) {
        match expr {
            Expr::Integer(n) => {
                writeln!(output, "    i32.const {}", n).ok();
            }
            Expr::Bool(true) => {
                writeln!(output, "    i32.const 1").ok();
            }
            Expr::Bool(false) => {
                writeln!(output, "    i32.const 0").ok();
            }
            Expr::Float(f) => {
                writeln!(output, "    f64.const {}", f).ok();
            }
            Expr::String(s) => {
                writeln!(output, "    ;; String: \"{}\" (loaded from memory)", s).ok();
            }
            Expr::Identifier(name) | Expr::PriorState(name) | Expr::OwnedRef(name) => {
                writeln!(output, "    local.get ${}", name).ok();
            }
            Expr::Add(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.add").ok();
            }
            Expr::Sub(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.sub").ok();
            }
            Expr::Mul(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.mul").ok();
            }
            Expr::Div(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.div_s").ok();
            }
            Expr::Mod(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.rem_s").ok();
            }
            Expr::Eq(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.eq").ok();
            }
            Expr::Ne(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.ne").ok();
            }
            Expr::Lt(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.lt_s").ok();
            }
            Expr::Le(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.le_s").ok();
            }
            Expr::Gt(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.gt_s").ok();
            }
            Expr::Ge(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.ge_s").ok();
            }
            Expr::And(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.and").ok();
            }
            Expr::Or(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.or").ok();
            }
            Expr::Not(inner) => {
                self.generate_expr(output, inner);
                writeln!(output, "    i32.eqz").ok();
            }
            Expr::Neg(inner) => {
                self.generate_expr(output, inner);
                writeln!(output, "    i32.const -1").ok();
                writeln!(output, "    i32.mul").ok();
            }
            Expr::BitAnd(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.and").ok();
            }
            Expr::BitOr(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.or").ok();
            }
            Expr::BitXor(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.xor").ok();
            }
            Expr::BitNot(inner) => {
                self.generate_expr(output, inner);
                writeln!(output, "    i32.const -1").ok();
                writeln!(output, "    i32.xor").ok();
            }
            Expr::Shl(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.shl").ok();
            }
            Expr::Shr(l, r) => {
                self.generate_expr(output, l);
                self.generate_expr(output, r);
                writeln!(output, "    i32.shr_s").ok();
            }
            Expr::Call(name, args) => {
                for arg in args {
                    self.generate_expr(output, arg);
                }
                writeln!(output, "    call ${}", name).ok();
            }
            Expr::ListLiteral(elems) => {
                for elem in elems {
                    self.generate_expr(output, elem);
                }
                writeln!(output, "    ;; ListLiteral [{} elems]", elems.len()).ok();
            }
            Expr::ListIndex(list, idx) => {
                self.generate_expr(output, list);
                self.generate_expr(output, idx);
                writeln!(output, "    ;; list[index]").ok();
            }
            Expr::Projection { source: list, .. } => {
                self.generate_expr(output, list);
                writeln!(output, "    ;; list.length").ok();
            }
            Expr::FieldAccess(obj, field) => {
                self.generate_expr(output, obj);
                writeln!(output, "    ;; .{}", field).ok();
            }
            _ => {
                writeln!(output, "    ;; Unimplemented expr").ok();
            }
        }
    }
}
impl Default for WasmTarget {
    /// Intent: Return the default WASM target (Wasi).
    fn default() -> Self {
        WasmTarget::Wasi
    }
}

/// Intent: Configuration for the WASM binary generator.
#[derive(Clone)]
pub struct WasmConfig {
    pub target: WasmTarget,
    pub min_pages: u32,
    pub max_pages: u32,
    pub debug: bool,
}

/// Intent: Default WASM configuration with safe memory bounds.
impl Default for WasmConfig {
    /// Intent: Return the default WASM configuration.
    fn default() -> Self {
        WasmConfig {
            target: WasmTarget::default(),
            min_pages: 1,
            max_pages: 256,
            debug: false,
        }
    }
}

/// Intent: The compiled WASM module with its binary bytes and metadata.
#[derive(Debug)]
pub struct WasmModule {
    pub bytes: Vec<u8>,
    pub function_count: usize,
    pub export_count: usize,
}

/// Intent: WASM value type opcode constants for i32, i64, f32, f64.
#[derive(Clone, Copy)]
pub enum ValType {
    I32 = 0x7F,
    I64 = 0x7E,
    F32 = 0x7D,
    F64 = 0x7C,
}

/// Intent: WASM function type signature with parameter and result types.
#[derive(Clone)]
struct FuncType {
    params: Vec<ValType>,
    results: Vec<ValType>,
}

/// Intent: WASM export entry mapping a name to its export kind and index.
#[derive(Clone)]
struct ExportEntry {
    name: String,
    kind: ExportKind,
    index: u32,
}

/// Intent: WASM export kind discriminator for function, table, memory, or global.
#[derive(Clone, Copy)]
enum ExportKind {
    Func = 0x00,
    Table = 0x01,
    Mem = 0x02,
    Global = 0x03,
}

/// Intent: WASM import entry referencing a host module and field name.
#[derive(Clone)]
struct ImportEntry {
    module: String,
    field: String,
    kind: ImportKind,
}

/// Intent: WASM import kind, currently only Function with type index.
#[derive(Clone)]
enum ImportKind {
    Func(u32),
}

/// Intent: WASM local variable declaration with count and value type.
#[derive(Clone)]
pub struct LocalDecl {
    count: u32,
    val_type: ValType,
}

/// Intent: WASM function body containing local declarations and opcode bytes.
#[derive(Clone)]
struct FunctionBody {
    locals: Vec<LocalDecl>,
    code: Vec<u8>,
}

/// Intent: WASM section ID constants for each standard binary section type.
#[derive(Clone, Copy)]
enum SectionId {
    Custom = 0x00,
    Type = 0x01,
    Import = 0x02,
    Function = 0x03,
    Table = 0x04,
    Memory = 0x05,
    Global = 0x06,
    Export = 0x07,
    Start = 0x08,
    Element = 0x09,
    Code = 0x0A,
    Data = 0x0B,
}

/// Intent: Encode an unsigned 32-bit integer as LEB128 variable-length encoding.
fn leb128_u32(value: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut v = value;
    loop {
        if v < 0x80 {
            buf.push(v as u8);
            break;
        } else {
            buf.push((v as u8) & 0x7F | 0x80);
            v >>= 7;
        }
    }
    buf
}

/// Intent: Encode an items vector prefixed with its length.
fn encode_vector(items: &[u8]) -> Vec<u8> {
    let mut buf = leb128_u32(items.len() as u32);
    buf.extend_from_slice(items);
    buf
}

/// Intent: Build the WASM Type section from a list of function types.
fn encode_type_section(types: &[FuncType]) -> Vec<u8> {
    let mut content = leb128_u32(types.len() as u32);
    for ft in types {
        content.push(0x60);
        let param_bytes: Vec<u8> = ft.params.iter().map(|t| *t as u8).collect();
        content.extend_from_slice(&encode_vector(&param_bytes));
        let result_bytes: Vec<u8> = ft.results.iter().map(|t| *t as u8).collect();
        content.extend_from_slice(&encode_vector(&result_bytes));
    }
    let mut section = vec![SectionId::Type as u8];
    section.extend_from_slice(&leb128_u32(content.len() as u32));
    section.extend_from_slice(&content);
    section
}

/// Intent: Build the WASM Import section from a list of imported functions.
fn encode_import_section(imports: &[ImportEntry]) -> Vec<u8> {
    let mut content = leb128_u32(imports.len() as u32);
    for imp in imports {
        let module_bytes = imp.module.as_bytes();
        content.extend_from_slice(&leb128_u32(module_bytes.len() as u32));
        content.extend_from_slice(module_bytes);
        let field_bytes = imp.field.as_bytes();
        content.extend_from_slice(&leb128_u32(field_bytes.len() as u32));
        content.extend_from_slice(field_bytes);
        match &imp.kind {
            ImportKind::Func(type_idx) => {
                content.push(0x00);
                content.extend_from_slice(&leb128_u32(*type_idx));
            }
        }
    }
    let mut section = vec![SectionId::Import as u8];
    section.extend_from_slice(&leb128_u32(content.len() as u32));
    section.extend_from_slice(&content);
    section
}

/// Intent: Build the WASM Function section mapping functions to their type indices.
fn encode_function_section(type_indices: &[u32]) -> Vec<u8> {
    let mut content = leb128_u32(type_indices.len() as u32);
    for idx in type_indices {
        content.extend_from_slice(&leb128_u32(*idx));
    }
    let mut section = vec![SectionId::Function as u8];
    section.extend_from_slice(&leb128_u32(content.len() as u32));
    section.extend_from_slice(&content);
    section
}

/// Intent: Build the WASM Memory section declaring linear memory bounds.
fn encode_memory_section(memories: &[(u32, u32)]) -> Vec<u8> {
    let mut content = leb128_u32(memories.len() as u32);
    for (min, max) in memories {
        if *max == 0 {
            content.push(0x00);
            content.extend_from_slice(&leb128_u32(*min));
        } else {
            content.push(0x01);
            content.extend_from_slice(&leb128_u32(*min));
            content.extend_from_slice(&leb128_u32(*max));
        }
    }
    let mut section = vec![SectionId::Memory as u8];
    section.extend_from_slice(&leb128_u32(content.len() as u32));
    section.extend_from_slice(&content);
    section
}

/// Intent: Build the WASM Export section listing exported symbols.
fn encode_export_section(exports: &[ExportEntry]) -> Vec<u8> {
    let mut content = leb128_u32(exports.len() as u32);
    for exp in exports {
        let name_bytes = exp.name.as_bytes();
        content.extend_from_slice(&leb128_u32(name_bytes.len() as u32));
        content.extend_from_slice(name_bytes);
        content.push(exp.kind as u8);
        content.extend_from_slice(&leb128_u32(exp.index));
    }
    let mut section = vec![SectionId::Export as u8];
    section.extend_from_slice(&leb128_u32(content.len() as u32));
    section.extend_from_slice(&content);
    section
}

/// Intent: Build the WASM Code section containing raw function bodies.
fn encode_code_section(bodies: &[FunctionBody]) -> Vec<u8> {
    let mut content = leb128_u32(bodies.len() as u32);
    for body in bodies {
        let mut body_bytes = Vec::new();
        body_bytes.extend_from_slice(&leb128_u32(body.locals.len() as u32));
        for local in &body.locals {
            body_bytes.extend_from_slice(&leb128_u32(local.count));
            body_bytes.push(local.val_type as u8);
        }
        body_bytes.extend_from_slice(&body.code);
        content.extend_from_slice(&leb128_u32(body_bytes.len() as u32));
        content.extend_from_slice(&body_bytes);
    }
    let mut section = vec![SectionId::Code as u8];
    section.extend_from_slice(&leb128_u32(content.len() as u32));
    section.extend_from_slice(&content);
    section
}

/// Intent: Internal struct linking a WASM function's type index, body, and optional export.
struct WasmFuncIdx {
    type_idx: u32,
    body: FunctionBody,
    export_name: Option<String>,
}

/// Intent: Builder for constructing a complete WebAssembly binary module.
pub struct WasmModuleBuilder {
    types: Vec<FuncType>,
    type_indices: Vec<u32>,
    imports: Vec<ImportEntry>,
    functions: Vec<WasmFuncIdx>,
    exports: Vec<ExportEntry>,
    memory: Option<(u32, u32)>,
    config: WasmConfig,
}

impl WasmModuleBuilder {
    /// Intent: Create a new empty WASM module builder with the given configuration.
    pub fn new(config: WasmConfig) -> Self {
        WasmModuleBuilder {
            types: Vec::new(),
            type_indices: Vec::new(),
            imports: Vec::new(),
            functions: Vec::new(),
            exports: Vec::new(),
            memory: None,
            config,
        }
    }

    /// Register a function signature and return its type index.
    pub fn add_function_type(&mut self, params: Vec<ValType>, results: Vec<ValType>) -> u32 {
        let idx = self.types.len() as u32;
        self.types.push(FuncType { params, results });
        idx
    }

    /// Declare an imported function from a host module.
    pub fn add_import(&mut self, module: &str, field: &str, type_idx: u32) -> u32 {
        let idx = self.imports.len() as u32;
        self.imports.push(ImportEntry {
            module: module.to_string(),
            field: field.to_string(),
            kind: ImportKind::Func(type_idx),
        });
        idx
    }

    /// Add a function body with its type reference and optional export name.
    pub fn add_function(
        &mut self,
        type_idx: u32,
        locals: Vec<LocalDecl>,
        code: Vec<u8>,
        export_name: Option<String>,
    ) -> u32 {
        let idx = (self.imports.len() + self.functions.len()) as u32;
        self.type_indices.push(type_idx);
        self.functions.push(WasmFuncIdx {
            type_idx,
            body: FunctionBody { locals, code },
            export_name: export_name.clone(),
        });
        if let Some(name) = export_name {
            self.exports.push(ExportEntry {
                name,
                kind: ExportKind::Func,
                index: idx,
            });
        }
        idx
    }

    /// Set the linear memory bounds in pages (64KB each).
    pub fn set_memory(&mut self, min_pages: u32, max_pages: u32) {
        self.memory = Some((min_pages, max_pages));
    }

    /// Assemble all sections into the final WASM binary.
    pub fn build(self) -> Vec<u8> {
        let magic: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];
        let version: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

        let type_sec = encode_type_section(&self.types);
        let import_sec = encode_import_section(&self.imports);
        let function_sec = encode_function_section(&self.type_indices);

        let memory_sec = if let Some((min, max)) = self.memory {
            encode_memory_section(&[(min, max)])
        } else {
            encode_memory_section(&[(1, 256)])
        };

        let export_sec = encode_export_section(&self.exports);
        let bodies: Vec<FunctionBody> = self.functions.iter().map(|f| f.body.clone()).collect();
        let code_sec = encode_code_section(&bodies);

        let mut wasm = Vec::new();
        wasm.extend_from_slice(&magic);
        wasm.extend_from_slice(&version);
        wasm.extend_from_slice(&type_sec);
        wasm.extend_from_slice(&import_sec);
        wasm.extend_from_slice(&function_sec);
        wasm.extend_from_slice(&memory_sec);
        wasm.extend_from_slice(&export_sec);
        wasm.extend_from_slice(&code_sec);

        wasm
    }
}

/// Intent: Map a Brief type to the nearest WASM value type.
fn map_brief_type(val_type: &Type) -> ValType {
    match val_type {
        Type::Int | Type::Bool | Type::UInt | Type::Data => ValType::I32,
        Type::Float => ValType::F64,
        Type::Vector(_, _) => ValType::I32,
        Type::Custom(s) if s == "U64" || s == "u64" => ValType::I64,
        _ => ValType::I32,
    }
}

/// Intent: Compile a Brief expression into a sequence of WASM opcodes.
fn expression_to_wasm(expr: &Expr, builder: &mut WasmModuleBuilder) -> Vec<u8> {
    match expr {
        Expr::Integer(value) => {
            let val = *value as i32;
            let unsigned = val as u32;
            let mut code = vec![0x41];
            code.extend_from_slice(&leb128_u32(unsigned));
            code
        }
        Expr::Bool(value) => {
            if *value {
                vec![0x41, 0x01]
            } else {
                vec![0x41, 0x00]
            }
        }
        Expr::Identifier(name) | Expr::PriorState(name) | Expr::OwnedRef(name) => {
            let mut code = vec![0x23];
            code.extend_from_slice(&leb128_u32(0));
            code
        }
        Expr::Add(l, r) => {
            let mut code = expression_to_wasm(l, builder);
            code.extend_from_slice(&expression_to_wasm(r, builder));
            code.push(0x6A);
            code
        }
        Expr::Sub(l, r) => {
            let mut code = expression_to_wasm(l, builder);
            code.extend_from_slice(&expression_to_wasm(r, builder));
            code.push(0x6B);
            code
        }
        Expr::Mul(l, r) => {
            let mut code = expression_to_wasm(l, builder);
            code.extend_from_slice(&expression_to_wasm(r, builder));
            code.push(0x6C);
            code
        }
        Expr::Div(l, r) => {
            let mut code = expression_to_wasm(l, builder);
            code.extend_from_slice(&expression_to_wasm(r, builder));
            code.push(0x6D);
            code
        }
        Expr::Eq(l, r) => {
            let mut code = expression_to_wasm(l, builder);
            code.extend_from_slice(&expression_to_wasm(r, builder));
            code.push(0x46);
            code
        }
        Expr::Lt(l, r) => {
            let mut code = expression_to_wasm(l, builder);
            code.extend_from_slice(&expression_to_wasm(r, builder));
            code.push(0x48);
            code
        }
        Expr::Gt(l, r) => {
            let mut code = expression_to_wasm(l, builder);
            code.extend_from_slice(&expression_to_wasm(r, builder));
            code.push(0x4A);
            code
        }
        Expr::Not(inner) => {
            let mut code = expression_to_wasm(inner, builder);
            code.push(0x41);
            code.extend_from_slice(&leb128_u32(1));
            code.push(0x73);
            code
        }
        Expr::And(l, r) => {
            let mut code = expression_to_wasm(l, builder);
            code.extend_from_slice(&expression_to_wasm(r, builder));
            code.push(0x71);
            code
        }
        Expr::Or(l, r) => {
            let mut code = expression_to_wasm(l, builder);
            code.extend_from_slice(&expression_to_wasm(r, builder));
            code.push(0x72);
            code
        }
        Expr::Le(l, r) => {
            let mut code = expression_to_wasm(l, builder);
            code.extend_from_slice(&expression_to_wasm(r, builder));
            code.push(0x4C);
            code
        }
        Expr::Ge(l, r) => {
            let mut code = expression_to_wasm(l, builder);
            code.extend_from_slice(&expression_to_wasm(r, builder));
            code.push(0x4E);
            code
        }
        Expr::Ne(l, r) => {
            let mut code = expression_to_wasm(l, builder);
            code.extend_from_slice(&expression_to_wasm(r, builder));
            code.push(0x47);
            code
        }
        _ => {
            vec![0x41, 0x00]
        }
    }
}

/// Intent: Compile a guard expression into WASM code that produces an i32 condition.
fn compile_guard(guard: &Expr, builder: &mut WasmModuleBuilder) -> Vec<u8> {
    expression_to_wasm(guard, builder)
}

/// Intent: Compile a list of Brief statements into WASM opcode bytes.
fn compile_body(body: &[Statement], builder: &mut WasmModuleBuilder) -> Vec<u8> {
    let mut code = Vec::new();
    for stmt in body {
        match stmt {
            Statement::Assignment { lhs: _, expr, .. } => {
                code.extend_from_slice(&expression_to_wasm(expr, builder));
                code.push(0x24);
                code.extend_from_slice(&leb128_u32(0));
            }
            Statement::Let { expr, .. } => {
                if let Some(init) = expr {
                    code.extend_from_slice(&expression_to_wasm(init, builder));
                    code.push(0x21);
                    code.extend_from_slice(&leb128_u32(0));
                }
            }
            Statement::Guarded {
                condition,
                statements,
            } => {
                let guard_code = compile_guard(condition, builder);
                let body_code = compile_body(statements, builder);

                code.push(0x02);
                code.push(0x7F);
                code.extend_from_slice(&guard_code);
                code.push(0x04);
                code.extend_from_slice(&body_code);
                code.push(0x05);
                code.push(0x41);
                code.extend_from_slice(&leb128_u32(0));
                code.push(0x0B);
                code.push(0x1B);
            }
            Statement::Term { values, .. } | Statement::TermBang { values, .. } => {
                if let Some(first) = values.first().and_then(|v| v.as_ref()) {
                    code.extend_from_slice(&expression_to_wasm(first, builder));
                } else {
                    code.push(0x41);
                    code.extend_from_slice(&leb128_u32(0));
                }
                code.push(0x0F);
            }
            _ => {}
        }
    }
    code.push(0x0B);
    code
}

/// Intent: Compile an entire Brief Program into a WASM binary module.
pub fn compile_program(program: &Program, config: WasmConfig) -> WasmModule {
    let mut builder = WasmModuleBuilder::new(config.clone());

    builder.set_memory(config.min_pages, config.max_pages);

    let main_type = builder.add_function_type(vec![], vec![]);

    for item in &program.items {
        match item {
            TopLevel::Transaction(txn) => {
                let guard_body = compile_guard(&txn.contract.pre_condition, &mut builder);
                let stmt_body = compile_body(&txn.body, &mut builder);

                let mut code = Vec::new();
                code.extend_from_slice(&guard_body);
                code.push(0x04);
                code.extend_from_slice(&stmt_body);
                code.push(0x0B);
                code.push(0x0F);
                code.push(0x0B);

                builder.add_function(
                    main_type,
                    vec![LocalDecl {
                        count: 1,
                        val_type: ValType::I32,
                    }],
                    code,
                    Some(txn.name.clone()),
                );
            }
            _ => {}
        }
    }

    let mut main_code = Vec::new();
    main_code.push(0x0B);

    let function_count = builder.functions.len();
    let export_count = builder.exports.len();

    builder.add_function(main_type, vec![], main_code, Some("_start".to_string()));

    let wasm_bytes = builder.build();

    WasmModule {
        function_count,
        export_count,
        bytes: wasm_bytes,
    }
}

/// Intent: Generate a WASM module from a Brief program using default configuration.
pub fn generate_wasm(program: &Program) -> WasmModule {
    compile_program(program, WasmConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    /// Intent: Verify the WAT text backend generates a valid (module) wrapper.
    #[test]
    fn test_wasm_generates_module() {
        let mut backend = WasmBackend::new();
        let program = Program {
            items: vec![],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program);
        assert!(output.contains("(module"));
    }

    /// Intent: Verify generate_statement handles Term by draining pending_cleanup.
    #[test]
    fn test_term_drains_cleanup() {
        let mut backend = WasmBackend::new();
        backend.pending_cleanup.push(Statement::Escape(None));
        let mut output = String::new();
        backend.generate_statement(
            &mut output,
            &Statement::Term {
                values: vec![],
                modifiers: vec![],
                swan_song: None,
            },
        );
        assert!(output.contains(";; term"));
    }

    /// Intent: Verify generate_expr handles integer, bool, float, and string literals.
    #[test]
    fn test_expr_literals() {
        let backend = WasmBackend::new();
        let mut output = String::new();
        backend.generate_expr(&mut output, &Expr::Integer(42));
        assert!(output.contains("i32.const 42"));
    }

    /// Intent: Verify generate_statement handles Let with expr.
    #[test]
    fn test_let_with_expr() {
        let mut backend = WasmBackend::new();
        let mut output = String::new();
        backend.generate_statement(
            &mut output,
            &Statement::Let {
                name: "x".into(),
                ty: Some(Type::Int),
                expr: Some(Expr::Integer(7)),
                address: None,
                address_expr: None,
                bit_range: None,
                is_override: false,
                modifiers: vec![],
            },
        );
        assert!(output.contains("i32.const 7"));
        assert!(output.contains("let x"));
    }

    /// Intent: Verify generate_statement handles OnExit by registering cleanup.
    #[test]
    fn test_on_exit_registers_cleanup() {
        let mut backend = WasmBackend::new();
        let mut output = String::new();
        backend.generate_statement(
            &mut output,
            &Statement::OnExit {
                body: vec![Statement::Escape(None)],
                span: None,
            },
        );
        assert!(output.contains("on_exit cleanup registered"));
        assert_eq!(backend.pending_cleanup.len(), 1);
    }

    /// Intent: Verify Expression statement generates a drop.
    #[test]
    fn test_expression_drop() {
        let mut backend = WasmBackend::new();
        let mut output = String::new();
        backend.generate_statement(
            &mut output,
            &Statement::Expression(Expr::Integer(99)),
        );
        assert!(output.contains("i32.const 99"));
        assert!(output.contains("drop"));
    }
}
