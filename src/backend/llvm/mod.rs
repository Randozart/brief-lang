pub mod abi;
pub mod builder;
pub mod context;
pub mod directive;
pub mod dispatch;
pub mod emit_expr;
pub mod emit_stmt;
pub mod emit_toplevel;
pub mod function;
pub mod gpu;
pub mod hazard;
pub mod helpers;
pub mod intrinsics;
pub mod loop_engine;
pub mod normalizer;
pub mod optimizer;
pub mod phi;
pub mod reorder;
pub mod types;
pub mod validate;

#[cfg(test)]
mod tests;

#[cfg(all(feature = "kani", feature = "kani_full"))]
mod kani;

pub use builder::LLVMBuilder;
pub use context::{CompilerContext, FunctionContext, FunctionGuard};

fn escape_llvm_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\22"),
            b'\n' => out.push_str("\\0a"),
            b'\r' => out.push_str("\\0d"),
            b'\t' => out.push_str("\\09"),
            0x20..=0x7e => out.push(byte as char),
            b => { let _ = write!(out, "\\{:02x}", b); }
        }
    }
    out
}

pub(crate) fn float_to_llvm_hex(f: f64) -> String {
    let f32_val = f as f32;
    let bits = f32_val.to_bits();
    format!("{}", bits)
}

// 2026-06-29: For Float64 literals (f64), bitcast directly to i64 bits
// without truncating through f32. Used by Expr::Float64 emission.
pub(crate) fn float64_to_llvm_hex(f: f64) -> String {
    let bits = f.to_bits();
    format!("{}", bits)
}

/// 2026-07-15: Emit a float constant in the correct LLVM format.
/// For `float` (32-bit): `bitcast (i32 <bits> to float)`
/// For `double` (64-bit): `bitcast (i64 <bits> to double)`
pub(crate) fn float_to_llvm_str(f: f64, llvm_ty: &str) -> String {
    match llvm_ty {
        "float" => format!("bitcast (i32 {} to float)", float_to_llvm_hex(f)),
        _ => format!("bitcast (i64 {} to double)", float64_to_llvm_hex(f)),
    }
}

/// Recursively evaluate a constant expression tree to a concrete f64.
/// Used to fold `const m0: Float = 4.0 * pi * pi` into a literal before
/// global emission, avoiding the `constant float 0` bug.
fn try_eval_cfloat(expr: &Expr, constants: &HashMap<String, (Type, Expr)>) -> Option<f64> {
    match expr {
        Expr::Float(f) => Some(*f),
        Expr::Identifier(name) => {
            match constants.get(name) {
                Some((Type::Custom(__t), inner)) if __t == "Float" || __t == "Float64" => try_eval_cfloat(inner, constants),
                _ => None,
            }
        }
        Expr::BinaryOp(kind, l, r) => {
            let lv = try_eval_cfloat(l, constants)?;
            let rv = try_eval_cfloat(r, constants)?;
            match kind {
                crate::ast::BinaryOpKind::Add => Some(lv + rv),
                crate::ast::BinaryOpKind::Sub => Some(lv - rv),
                crate::ast::BinaryOpKind::Mul => Some(lv * rv),
                crate::ast::BinaryOpKind::Div => Some(lv / rv),
                _ => None,
            }
        }
        Expr::UnaryOp(kind, inner) => {
            if matches!(kind, crate::ast::UnaryOpKind::Neg) {
                Some(-try_eval_cfloat(inner, constants)?)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Map an LLVM type string to its byte size. Used by compute_state_size_bytes.
fn llvm_type_byte_size(t: &str) -> i64 {
    match t {
        "i8" | "i1" => 1,
        "i16" => 2,
        "i32" | "float" => 4,
        "i64" | "double" | "i8*" | "ptr" => 8,
        // For aggregate or unknown types, assume max alignment (8 bytes)
        // to err on the side of larger allocation.
        _ => 8,
    }
}

/// Map a type name string to its primitive Type variant.
/// Used by `resolve_bild_type` to resolve aliases and melds.
fn primitive_from_name(name: &str) -> Option<Type> {
    match name {
        "Int" | "UInt" | "Signed" | "Unsigned" => Some(Type::int()),
        "Int8" | "I8" => Some(Type::bits(1)),
        "Int16" | "I16" => Some(Type::bits(2)),
        "Int32" | "I32" => Some(Type::bits(4)),
        "UInt8" | "U8" => Some(Type::bits(1)),
        "UInt16" | "U16" => Some(Type::bits(2)),
        "UInt32" | "U32" => Some(Type::bits(4)),
        "Float" => Some(Type::float()),
        "Float64" | "F64" | "Double" => Some(Type::float64()),
        "Bool" => Some(Type::bool_()),
        "Char" => Some(Type::char_()),
        "String" => Some(Type::string()),
        "Data" | "Bytes" => Some(Type::data()),
        _ => None,
    }
}

/// Detect terminating guard at end of body and hoist it.
/// Returns (body_without_guard, vec_of_(field_name, intrinsic_name)).
pub(crate) fn hoist_terminating_guard(
    body: &[Statement],
    field_index_map: &std::collections::HashMap<String, usize>,
) -> (Vec<Statement>, Vec<Vec<Statement>>) {
    let mut stmts: Vec<&Statement> = body.iter()
        .filter(|s| !matches!(s, Statement::Term(..) | Statement::TermBang(..)))
        .collect();
    // 2026-07-05: Build let-to-state-field mapping from body assignments.
    // When the hoisted swan song references a let binding (like nesc in
    // mandelbrot), the done: block can't use the body's register.  We remap
    // the let binding to the state field that stores its value.
    // Pattern: &field_name = let_name  →  map[let_name] = field_name
    let mut let_to_field: HashMap<String, String> = HashMap::new();
    for s in body {
        if let Statement::Assign(lhs, Expr::Identifier(let_name)) = s {
            if let Some(field_name) = lhs.as_var_name() {
                if field_index_map.contains_key(field_name) {
                    let_to_field.insert(let_name.clone(), field_name.to_string());
                }
            }
        }
    }
    let mut hoist: Vec<Vec<Statement>> = Vec::new();
    while let Some(last_idx) = stmts.len().checked_sub(1) {
        if let Statement::Guarded(_, statements) = &stmts[last_idx] {
            let is_terminating = statements.iter().any(|s| matches!(s, Statement::TermBang(..)));
            if !is_terminating { break; }
            // Hoist the entire guard body (all statements before the term!)
            // into a Vec<Statement> that the post-loop block can re-emit.
            // This handles both simple field-print patterns (original hoisting)
            // and let-binding-based patterns (nbody: energy computation + print).
            let mut body_stmts: Vec<Statement> = statements.iter()
                .filter(|s| !matches!(s, Statement::TermBang(..)))
                .cloned()
                .collect();
            // Remap let binding references to state field names in hoisted body.
            for s in &mut body_stmts {
                remap_stmt_identifiers(s, &let_to_field);
            }
            let swan_song_stmt = statements.iter().find_map(|s| {
                if let Statement::TermBang(Some(ss)) = s {
                    Some(ss.clone())
                } else { None }
            });
            // Remap swan song identifiers too.
            let swan_song_stmt = swan_song_stmt.map(|mut ss| {
                remap_expr_into(&mut ss, &let_to_field);
                ss
            });
            // 2026-07-04: Hoist even when body_stmts is empty — the
            // guard may be just `term! -> print_int#(result)` with no
            // preceding statements.  Previously we only hoisted when
            // body_stmts was non-empty, leaving the swan song in the
            // body and blocking Path A (no-dead-stores) because
            // pending_post_hoist was empty.
            if !body_stmts.is_empty() || swan_song_stmt.is_some() {
                let mut full_body = body_stmts;
                if let Some(sw) = swan_song_stmt {
                    full_body.push(Statement::Expression(sw));
                }
                hoist.push(full_body);
                stmts.pop();
            }
            break;
        } else { break; }
    }
    let body_vec: Vec<Statement> = stmts.into_iter().cloned().collect();
    (body_vec, hoist)
}

/// Recursively remap identifiers in a statement using the let-to-field map.
fn remap_stmt_identifiers(s: &mut Statement, map: &HashMap<String, String>) {
    match s {
        Statement::Assign(_, expr) => {
            remap_expr_into(expr, map);
        }
        Statement::Expression(e) => {
            remap_expr_into(e, map);
        }
        Statement::TermBang(Some(ss)) => {
            remap_expr_into(ss, map);
        }
        Statement::Guarded(condition, statements) => {
            remap_expr_into(condition, map);
            for stmt in statements.iter_mut() {
                remap_stmt_identifiers(stmt, map);
            }
        }
        _ => {}
    }
}

/// Recursively remap identifiers in an expression.
fn remap_expr_into(e: &mut Expr, map: &HashMap<String, String>) {
    match e {
        Expr::Identifier(name) => {
            if let Some(field) = map.get(name) {
                *name = field.clone();
            }
        }
        Expr::Call(_, args, _) => {
            for arg in args.iter_mut() {
                remap_expr_into(arg, map);
            }
        }
        Expr::BinaryOp(_, l, r) => {
            remap_expr_into(l, map);
            remap_expr_into(r, map);
        }
        Expr::UnaryOp(_, inner) | Expr::Cast(inner, _) | Expr::IsType(inner, _) => {
            remap_expr_into(inner, map);
        }
        Expr::Field(target, _) | Expr::Index(target, _) => {
            remap_expr_into(target, map);
        }
        Expr::Block(stmts) => {
            for s in stmts.iter_mut() {
                remap_stmt_identifiers(s, map);
            }
        }
        Expr::If(cond, then_b, else_b) => {
            remap_expr_into(cond, map);
            remap_expr_into(then_b, map);
            if let Some(eb) = else_b {
                remap_expr_into(eb, map);
            }
        }
        Expr::Tuple(elems) | Expr::List(elems) => {
            for e in elems.iter_mut() {
                remap_expr_into(e, map);
            }
        }
        _ => {}
    }
}

use crate::ast::{BinaryOpKind, Expr, Statement, TopLevel, Type};

// 2026-07-18: Allocation strategy — tracks how a pointer was allocated.
// Arena: bump-allocated from per-txn arena (Free# is no-op).
// Malloc: heap-allocated via @malloc (Free# must call @free).
// Alloca: stack-allocated via alloca (Free# is no-op).
#[derive(Debug, Clone, PartialEq)]
pub enum AllocStrategy {
    Arena,
    Malloc,
    Alloca,
}

#[derive(Debug, Clone)]
pub struct TypedRegister {
    pub name: String,
    pub ty: Type,
}

pub struct FoldParam {
    pub counter_idx: usize,
    pub bound_field_idx: Option<usize>,
    pub bound_const_name: Option<String>,
    pub is_decreasing: bool,
    pub bound_literal: Option<i64>,
}

impl std::fmt::Display for TypedRegister {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Map Brief Type to native LLVM type string.
/// This is the single source of truth — eliminates i64 boxing for strings, chars, bools.
impl TypedRegister {
    pub fn llvm(&self) -> &'static str {
        if self.ty == Type::bool_() {
            "i1"
        } else if self.ty == Type::char_() {
            "i32"
        } else if self.ty == Type::int() || self.ty == Type::Custom("UInt".to_string()) {
            "i64"
        } else if self.ty == Type::float() {
            "float"
        } else if self.ty == Type::string() || self.ty == Type::data() {
            "i8*"
        } else {
            "i64"
        }
    }
}

use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Write};

/// Collect all unique string literal values from the program for global emission.
fn collect_strings(items: &[TopLevel]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    // 2026-07-17: Always start with the empty string at index 0.
    // emit_field_init_value references @str.0 as the uninitialized String
    // sentinel (Site B at line 866). Without this, @str.0 is never emitted
    // when no Expr::Quoted appears in the source, producing "use of undefined
    // value '@str.0'" in 14 of 23 benchmarks.
    out.push("".to_string());
    seen.insert("".to_string());
    for item in items {
        collect_strings_tl(item, &mut seen, &mut out);
    }
    out
}
fn collect_strings_tl(tl: &TopLevel, seen: &mut std::collections::HashSet<String>, out: &mut Vec<String>) {
    match tl {
        TopLevel::Transaction(t) => { for s in &t.body { collect_strings_stmt(s, seen, out); } }
        TopLevel::Definition(d) => { for s in &d.body { collect_strings_stmt(s, seen, out); } }
        TopLevel::Cell(c) => {
            // 2026-07-13: Field.default removed in new AST.
            for _ in &c.fields { }
            for txn in &c.transactions { for s in &txn.body { collect_strings_stmt(s, seen, out); } }
            for d in &c.definitions { for s in &d.body { collect_strings_stmt(s, seen, out); } }
            for trg in &c.internal_triggers { collect_strings_expr(&Expr::Identifier(trg.name.clone()), seen, out); }
        }
        _ => {}
    }
}
fn collect_strings_stmt(stmt: &Statement, seen: &mut std::collections::HashSet<String>, out: &mut Vec<String>) {
    match stmt {
        Statement::Let { expr, .. } => { if let Some(e) = expr { collect_strings_expr(e, seen, out); } }
        Statement::Assign(_, expr) => { collect_strings_expr(expr, seen, out); }
        Statement::Expression(e) => { collect_strings_expr(e, seen, out); }
        Statement::Term(Some(e)) | Statement::TermBang(Some(e)) | Statement::Return(Some(e)) => { collect_strings_expr(e, seen, out); }
        Statement::Term(None) | Statement::TermBang(None) | Statement::Return(None) => {}
        Statement::Guarded(condition, statements) => {
            collect_strings_expr(condition, seen, out);
            for s in statements { collect_strings_stmt(s, seen, out); }
        }
        Statement::If(_, then_body, else_body) => {
            for s in then_body { collect_strings_stmt(s, seen, out); }
            for s in else_body { collect_strings_stmt(s, seen, out); }
        }
        Statement::Block(body) | Statement::SyncBlock(body) => {
            for s in body { collect_strings_stmt(s, seen, out); }
        }
        Statement::Escape(Some(e)) => { collect_strings_expr(e, seen, out); }
        Statement::Escape(None) => {}
        Statement::Foreach { list, body, .. } => {
            collect_strings_expr(list, seen, out);
            for s in body { collect_strings_stmt(s, seen, out); }
        }
        Statement::TrgBinding { .. } | Statement::InlineAsm { .. } | Statement::MetadataAssignment(..) => {}
    }
}

fn collect_strings_expr(expr: &Expr, seen: &mut std::collections::HashSet<String>, out: &mut Vec<String>) {
    match expr {
        Expr::Quoted(s) => {
            let s_str = String::from_utf8_lossy(s).into_owned();
            if !seen.contains(&s_str) {
                seen.insert(s_str.clone());
                out.push(s_str);
            }
        }
        Expr::BinaryOp(_, l, r) => {
            collect_strings_expr(l, seen, out);
            collect_strings_expr(r, seen, out);
        }
        Expr::UnaryOp(_, inner) | Expr::Cast(inner, _) | Expr::IsType(inner, _) => {
            collect_strings_expr(inner, seen, out);
        }
        Expr::List(elems) | Expr::Tuple(elems) => {
            for e in elems { collect_strings_expr(e, seen, out); }
        }
        Expr::Field(o, _) | Expr::Index(o, _) => {
            collect_strings_expr(o, seen, out);
        }
        Expr::Call(_, args, _) => {
            for a in args { collect_strings_expr(a, seen, out); }
        }
        Expr::Match(value, arms) => {
            collect_strings_expr(value, seen, out);
            for arm in arms { collect_strings_expr(&arm.body, seen, out); }
        }
        Expr::Block(stmts) => {
            for s in stmts { collect_strings_stmt(s, seen, out); }
        }
        Expr::If(cond, then_b, else_b) => {
            collect_strings_expr(cond, seen, out);
            collect_strings_expr(then_b, seen, out);
            if let Some(eb) = else_b { collect_strings_expr(eb, seen, out); }
        }
        Expr::Lambda(_, body) => {
            collect_strings_expr(body, seen, out);
        }
        Expr::Within(body, fallback) => {
            collect_strings_expr(body, seen, out);
            collect_strings_expr(fallback, seen, out);
        }
        Expr::DerivationBlock(db) => {
            for ex in &db.examples {
                for inp in &ex.inputs { collect_strings_expr(inp, seen, out); }
                collect_strings_expr(&ex.output, seen, out);
            }
        }
        Expr::Deref(inner) => {
            collect_strings_expr(inner, seen, out);
        }
        Expr::AddrOf(inner) => {
            collect_strings_expr(inner, seen, out);
        }
        // Leaves — no sub-expressions
        Expr::Decimal(_) | Expr::Bool(_) | Expr::Float(_) | Expr::Identifier(_)
        | Expr::PropertyGet(_) | Expr::FormattingAnnotation(_) => {}
    }
}

/// LLVM IR backend — the definitive compiler from Brief AST to `.ll`.
///
/// Every lesson from phases 0–5.5 integrated into one coherent pass:
/// - \`noalias nocapture\` on all \`ptr\` — LLVM sees no pointer aliasing
/// - i64-centric expression system — strings/lists become `i64` via `ptrtoint`/`inttoptr`
/// - Bool (i8) fields trunc on store, zext on load; floats via bitcast+zext; char via zext
/// - Unique guard labels, `returns_i64` flag, fused txn terminator filtering
/// - All Expr/Statement/TopLevel variants emit valid IR
/// - Contracts: `!range`, `@llvm.assume` (debug panic / release assume)
/// - Match→switch with phi merge, unification, pattern match
/// - FFI declare+call with C ABI (transparent, no compiler magic)
/// - Transition fusing, trigger sampling by MMIO/linked address
/// - Precondition extraction → internal `i1` functions, dispatch chain
/// - User-provided `frgn __wait_for_event` + `rct txn [true]` for sleep
/// - `@ link` triggers → `external global` + `load volatile`
///
/// Design philosophy: the backend is a single monolithic pass (not a pipeline
/// of small passes) because Brief's contract system provides structural
/// guarantees that LLVM cannot infer from generic IR. By emitting contract-
/// aware IR directly (TBAA, !range, noalias), we avoid the need for an
/// expensive LLVM analysis pass to rediscover what the contracts already state.

/// LLVM storage type for an `@ link` trigger global.
/// The C runtime provides `char` (Bool→i8), `int64_t` (Int→i64),
/// and `char*` (String→i8*).
pub(super) fn trg_llvm_storage_ty(ty: &Type) -> &str {
    match ty {
        Type::Custom(__t) if __t == "Bool" => "i8",
        Type::Custom(__t) if __t == "Int" || __t == "UInt" => "i64",
        Type::Custom(__t) if __t == "Float" => "float",
        Type::Custom(__t) if __t == "Char" => "i32",
        Type::Custom(__t) if __t == "String" || __t == "Data" => "i8*",
        _ => "i8", // fallback for unsupported types
    }
}

/// Map a field's LLVM storage type string to its TBAA metadata node index.
/// Returns the !N index into the TBAA tree emitted at end of module.
/// universe is optional: when available, uses the dynamically-generated
/// TBAA tree (sorted alphabetically, Int first).  When None, falls back
/// to the original hardcoded indices for the 5 built-in types.
pub(super) fn tbaa_node(ty_str: &str, universe: Option<&crate::type_universe::TypeUniverse>) -> i32 {
    // Map string to TBAA group name
    let group = match ty_str {
        "i64" => "Int",
        "i8"  => "Bool",
        "i32" => "Char",
        "i8*" | "ptr" => "String",
        "float" | "double" => "Float",
        _ => "Int",  // fallback
    };
    if let Some(u) = universe {
        // 2026-07-13: Inline sorted groups from type names.
        let mut groups: Vec<String> = u.types.keys().cloned().collect();
        groups.sort();
        if let Some(pos) = groups.iter().position(|g| g == "Int") {
            groups.swap(0, pos);
        }
        groups.iter().position(|g| g == group).map(|i| i as i32 + 1).unwrap_or(1)
    } else {
        // Fallback: hardcoded indices
        match group {
            "Int"    => 1,
            "Bool"   => 2,
            "Char"   => 3,
            "String" => 4,
            "Float"  => 5,
            _ => 1,
        }
    }
}

/// Map a Brief type to its TBAA metadata node index via universe lookup.
/// 2026-07-13: Simplified for new ResolvedType (tbaa_node removed).
/// Uses type name as the TBAA group. Falls back to 1 (Int) when not found.
pub(super) fn tbaa_node_for_type(ty: &Type, universe: &crate::type_universe::TypeUniverse) -> i32 {
    let group = match ty.universe_key() {
        Some(key) if universe.contains(key) => key,
        _ => return 1,
    };
    let mut groups: Vec<String> = universe.types.keys().cloned().collect();
    groups.sort();
    if let Some(pos) = groups.iter().position(|g| g == "Int") {
        groups.swap(0, pos);
    }
    groups.iter().position(|g| g == group).map(|i| i as i32 + 1).unwrap_or(1)
}
    /// at the IR level. Without TBAA, all i64 accesses within %State are
    /// MayAlias, preventing GVN load elimination.

/// Why metadata defs must be deferred: LLVM 18+ rejects metadata definitions
/// (!N = !{...}) inside function bodies. Metadata must be defined at module
/// scope. We collect all pending metadata in a Vec and flush it at the end
/// of the module (after the last function definition). This also lets us
/// deduplicate identical metadata nodes across multiple loop headers.

/// Emit `!llvm.loop` metadata for a backedge branch and the branch itself.
///
/// Follows the `foreach.rs` pattern: emit metadata entries, build the
/// self-referencing loop metadata node, and attach it to the backedge branch.
/// Metadata definitions are written to `pending_metadata` (flushed at module
/// end) because LLVM 18+ rejects metadata definitions inside function bodies.
///
/// 2026-06-20: Phase 0a — retrofitted from foreach.rs to main loop paths.
pub(super) fn emit_loop_metadata(
    out: &mut String,
    indent: &str,
    backedge_label: &str,
    metadata_counter: &mut usize,
    pending_metadata: &mut String,
) {
    let md_idx = emit_loop_metadata_nodes(metadata_counter, pending_metadata);
    writeln!(out, "{0}br label %{1}, !llvm.loop !{2}", indent, backedge_label, md_idx).ok();
}

/// Same as `emit_loop_metadata` but the caller supplies the backedge text.
/// Use when the backedge is a conditional branch (br i1) or when multiple
/// backedges share the same metadata node.
pub(super) fn emit_loop_metadata_nodes(
    metadata_counter: &mut usize,
    pending_metadata: &mut String,
) -> usize {
    let start = *metadata_counter;
    let mut md_count = 1;
    let mut md_entries = Vec::new();
    // Default: vectorize.enable for all counted loops
    let vd_md = start + md_count;
    writeln!(pending_metadata, "!{0} = !{{!\"llvm.loop.vectorize.enable\", i1 true}}", vd_md).ok();
    md_entries.push(format!("!{}", vd_md));
    md_count += 1;

    let entries = md_entries.join(", ");
    writeln!(pending_metadata, "!{0} = !{{!{0}, {1}}}", start, entries).ok();
    *metadata_counter += md_count;
    start
}

fn extract_trigger_keys(pre: &Expr, trigger_names: &std::collections::HashSet<&str>) -> Option<Vec<i64>> {
    let mut keys = Vec::new();
    match pre {
        Expr::BinaryOp(BinaryOpKind::Eq, l, r) => {
            let (ident, val) = if let (Expr::Identifier(name), Expr::Decimal(n)) = (l.as_ref(), r.as_ref()) {
                (name.clone(), *n)
            } else if let (Expr::Decimal(n), Expr::Identifier(name)) = (l.as_ref(), r.as_ref()) {
                (name.clone(), *n)
            } else {
                return None;
            };
            if trigger_names.contains(ident.as_str()) {
                keys.push(val);
            } else {
                return None;
            }
        }
        Expr::BinaryOp(BinaryOpKind::Or, l, r) => {
            keys.extend(extract_trigger_keys(l, trigger_names)?);
            keys.extend(extract_trigger_keys(r, trigger_names)?);
        }
        Expr::BinaryOp(BinaryOpKind::And, l, r) => {
            if let Some(k) = extract_trigger_keys(l, trigger_names) {
                keys.extend(k);
            } else if let Some(k) = extract_trigger_keys(r, trigger_names) {
                keys.extend(k);
            } else {
                return None;
            }
        }
        _ => return None,
    }
    keys.sort_unstable();
    keys.dedup();
    if keys.len() < 2 { None } else { Some(keys) }
}

/// Compute the sparsity ratio of a sorted key set.
/// Dense sets (gap ratio < 4) don't need perfect hashing — standard
/// switch dispatch with consecutive offsets works fine.
fn sparsity_ratio(keys: &[i64]) -> f64 {
    if keys.len() < 2 { return 0.0; }
    let gaps: Vec<u64> = keys.windows(2).map(|w| (w[1] - w[0]) as u64).collect();
    let min_gap = *gaps.iter().min().unwrap_or(&1);
    let max_gap = *gaps.iter().max().unwrap_or(&0);
    if min_gap == 0 { return f64::MAX; }
    max_gap as f64 / min_gap as f64
}

/// Find a multiplicative perfect hash for a set of sparse keys.
/// Returns (multiplier, shift) such that h(k) = (k * M) >> S maps
/// each input key to a unique slot in [0, next_power_of_two(n)).
/// Guaranteed termination: capped at 10,000 iterations.
fn find_perfect_hash(keys: &[i64]) -> Option<(u64, u32)> {
    let n = keys.len();
    let num_slots = n.next_power_of_two();
    let shift = 64 - num_slots.trailing_zeros();
    let mut rng: u64 = 123456789;
    for _ in 0..10000 {
        rng = rng.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
        let multiplier = rng | 1;
        let mut seen = vec![false; num_slots];
        let mut ok = true;
        for &k in keys {
            let hash = (k.wrapping_mul(multiplier as i64) as u64) >> shift;
            if seen[hash as usize] { ok = false; break; }
            seen[hash as usize] = true;
        }
        if ok { return Some((multiplier, shift)); }
    }
    None
}

/// Action when a `@ *ptr` dynamic trigger target resolves to null.
/// 2026-07-15: Phase 7i — --error-unresolved-trg threads this to codegen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrgUnresolvedAction {
    /// Warn on null target (default — currently a no-op at codegen).
    Warn,
    /// Emit null check + unreachable to abort at runtime.
    Error,
}

pub struct LlvmBackend {
    // ── Context Architecture (Phase 0) ─────────────────────
    //
    // 2026-06-29: Three-tier context separation. CompilerContext holds global
    // read-only state; FunctionContext holds per-function mutable state; the
    // remaining fields on LlvmBackend are orchestration/output accumulators.
    // See context.rs for details.
    pub ctx: CompilerContext,
    pub fun: FunctionContext,

    // ── Optimization ───────────────────────────────────────
    pgo_guard_idx: usize,

    // ── Async / Thread Pool ────────────────────────────────
    pub(crate) has_async_txns: bool,
    pub(crate) async_txn_names: Vec<String>,
    pub(crate) async_thread_pool_size: u32,
    pub(crate) is_lightweight_async: bool,

    // ── Program Registry ───────────────────────────────────
    program_txns: Vec<String>,
    pub(crate) fused_to_first: HashMap<String, String>,
    pub(crate) sampled_triggers: HashMap<String, String>,
    pub(crate) txn_write_masks: HashMap<String, u64>,
    cell_thread_names: Vec<String>,

    // ── Reporting & Diagnostics ────────────────────────────
    pub(crate) report_lines: Vec<String>,
    pub(crate) warnings: Vec<String>,
    pub(crate) remarks: Vec<crate::backend::llvm::directive::OptimizationRemark>,
    llvm_extra_flags: Vec<String>,
    pub(crate) pending_async_await_count: usize,

    // ── GPU Offloading ─────────────────────────────────────
    pub(crate) spirv_kernels: Vec<String>,
    pub(crate) spirv_blobs: Vec<Vec<u8>>,

    // ── Dynamic Trigger Safety ─────────────────────────────
    // 2026-07-15: Phase 7i — Controls null-check emission for @ *ptr.
    pub(crate) trg_unresolved_action: TrgUnresolvedAction,

    // ── Allocation Strategy Analysis ─────────────────────────
    // 2026-07-18: Pre-computed strategies from analysis pass.
    // Keyed by analysis_id on Expr::Call("Alloc#", ..., Some(id)).
    pub analysis_alloc_strategies: Option<std::collections::HashMap<usize, AllocStrategy>>,

    // ── SSO String Optimization ──────────────────────────────
    // 2026-07-18: Phase B — When enabled, String is a {i64, i64} struct with
    // inline storage for ≤6 bytes (SSO tag in lower 3 bits) and heap pointer
    // for longer strings. When disabled (default), String is a single i64
    // (ptrtoint of heap/stack pointer, legacy 16-byte header format).
    pub feature_sso_strings: bool,
}

#[derive(Debug, Clone)]
pub struct ChimeraInfo {
    pub is_chimera: bool,
    pub backing_type: String,
}

/// Configuration for embedded (bare-metal) LLVM codegen.
#[derive(Debug, Clone)]
pub struct EmbeddedConfig {
    pub target_triple: String,
    pub linker_script: Option<String>,
    pub cpu: Option<String>,
    pub freestanding: bool,
    pub halt_on_term: bool,
    pub memory_regions: Vec<MemoryRegion>,
    pub interrupts: Vec<InterruptEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchMode {
    Sequential,
    Parallel,
}

#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub name: String,
    pub base: u64,
    pub size: u64,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct InterruptEntry {
    pub name: String,
    pub vector: u32,
    pub trg_name: Option<String>,
}

/// Recursively collect push targets from a statement body.
/// In the new AST, arrow push `queue <- val` parses as
/// `Statement::Assign(Expr::Identifier("queue"), val)`.
/// Over-approximation is safe: preallocating a buffer for a
/// non-push field just allocates unused memory (freed at scope end).
/// 2026-07-18: Fixed — was a stub that never extracted any names.
pub(crate) fn collect_push_targets(body: &[Statement], out: &mut Vec<String>) {
    for stmt in body {
        match stmt {
            Statement::Guarded(_, statements) => {
                collect_push_targets(statements, out);
            }
            Statement::Block(body) | Statement::SyncBlock(body) => {
                collect_push_targets(body, out);
            }
            Statement::Assign(Expr::Identifier(name), _) => {
                out.push(name.clone());
            }
            _ => {}
        }
    }
}

impl LlvmBackend {
    pub fn new() -> Self {
        LlvmBackend {
            ctx: CompilerContext::new(),
            fun: FunctionContext::new(),
            pgo_guard_idx: 0,
            has_async_txns: false,
            async_txn_names: Vec::new(),
            async_thread_pool_size: 0,
            is_lightweight_async: false,
            program_txns: Vec::new(),
            fused_to_first: HashMap::new(),
            sampled_triggers: HashMap::new(),
            txn_write_masks: HashMap::new(),
            cell_thread_names: Vec::new(),
            report_lines: Vec::new(),
            warnings: Vec::new(),
            remarks: Vec::new(),
            llvm_extra_flags: Vec::new(),
            pending_async_await_count: 0,
            spirv_kernels: Vec::new(),
            spirv_blobs: Vec::new(),
            trg_unresolved_action: TrgUnresolvedAction::Warn,
            analysis_alloc_strategies: None,
            feature_sso_strings: false,
        }
    }

    pub fn with_alloc_strategies(mut self, strategies: std::collections::HashMap<usize, AllocStrategy>) -> Self {
        self.analysis_alloc_strategies = Some(strategies);
        self
    }

    // 2026-07-18: Enable SSO (Short String Optimization) for String types.
    // When ON, String is a {i64, i64} struct with inline storage for ≤6 bytes.
    pub fn with_sso_strings(mut self, enabled: bool) -> Self {
        self.feature_sso_strings = enabled;
        self
    }

    pub fn with_spec(mut self, spec: crate::target_spec::TargetSpec) -> Self {
        self.ctx.spec = Some(spec);
        self
    }

    pub fn with_optimize_budget(mut self, budget: u64) -> Self {
        self.ctx.optimize_budget = budget;
        self
    }

    pub fn with_optimize_report(mut self, report: bool) -> Self {
        self.ctx.optimize_report = report;
        self
    }

    // 2026-06-29: Push a field type, recording both the LLVM type string and
    // the original Brief Type. Parallel to field_types/field_brief_types.
    // The Brief Type is needed when reading fields back from %State to
    // distinguish types that share the same LLVM representation (e.g. Char
    // and Int32 both → "i32", Bool and Int8 both → "i8").
    pub(super) fn push_field_type(&mut self, ty: &Type) {
        // 2026-07-17: ALL state fields are stored as i64 in %State, regardless
        // of their Brief type (Float, Float64, Ptr, etc.). The adapt_to_i64 /
        // ensure_typed_value functions handle the conversion between i64 and
        // the field's natural type at load/store time. Override llvm_type(ty)
        // to always return "i64" for state fields — this keeps %State struct
        // layout uniform and avoids type mismatches in codegen paths that
        // assume i64 (load i64, store i64, add i64, icmp i64, etc.).
        // 2026-07-18: SSO String / Utf8View fields occupy 2 consecutive i64 slots
        // (data+tag in slot 0, length in slot 1). The field_index_map entry points
        // to slot 0; slot 1 is implicitly at index+1. State load/store must emit
        // extractvalue/insertvalue on the {i64,i64} struct.
        // Utf8View always gets 2 slots (always {i64,i64}) regardless of SSO flag.
        if matches!(ty, Type::Custom(name) if name == "Utf8View")
            || (self.feature_sso_strings
                && self.ctx.type_universe.as_ref().map_or(false, |u| u.is_string_like(ty)))
        {
            // 2026-07-18: Push 2 slots for SSO string handles ({data, len}).
            self.ctx.field_types.push("i64".to_string());
            self.ctx.field_brief_types.push(ty.clone());
            self.ctx.field_types.push("i64".to_string());
            self.ctx.field_brief_types.push(ty.clone());
            return;
        }
        self.ctx.field_types.push("i64".to_string());
        self.ctx.field_brief_types.push(ty.clone());
    }

    pub fn with_optimize_size(mut self, byte_limit: u64) -> Self {
        self.ctx.optimize_size = Some(byte_limit);
        self.ctx.optimize_report = true;
        self
    }

    pub fn with_dead_info_disabled(mut self, disabled: bool) -> Self {
        self.ctx.dead_info_disabled = disabled;
        self
    }

    pub fn with_explain(mut self, explain: bool) -> Self {
        self.ctx.explain = explain;
        self
    }

    /// Pre-populate MMIO address map from a resolved DBV target binding.
    /// Each alias name maps to a physical u64 address for volatile MMIO access.
    pub fn with_mmio_addresses(mut self, addresses: HashMap<String, u64>) -> Self {
        self.ctx.mmio_fields = addresses;
        self.ctx.mmio_prepopulated = true;
        self
    }

    pub fn with_schema_aliases(mut self, aliases: HashMap<String, crate::dbrief::DbriefType>) -> Self {
        self.ctx.schema_aliases = aliases;
        self
    }

    pub fn with_pgo_profile(mut self, profile: crate::analysis::pgo::PgoProfile) -> Self {
        self.ctx.pgo_profile = Some(profile);
        self
    }

    pub fn with_emit_remarks(mut self, emit: bool) -> Self {
        self.ctx.emit_remarks = emit;
        self
    }

    pub fn with_gpu_offload(mut self, offload: bool) -> Self {
        self.ctx.gpu_offload = offload;
        self
    }

    pub fn with_trg_unresolved_action(mut self, action: TrgUnresolvedAction) -> Self {
        self.trg_unresolved_action = action;
        self
    }

    pub fn with_gpu_backend(mut self, backend: String) -> Self {
        self.ctx.gpu_backend = backend;
        self
    }

    pub fn with_embedded_mode(mut self, enabled: bool) -> Self {
        self.ctx.is_embedded = enabled;
        self
    }

    pub fn with_type_universe(mut self, tu: crate::type_universe::TypeUniverse) -> Self {
        self.ctx.type_universe = Some(tu);
        self
    }

    pub fn with_dump_layout(mut self, v: bool) -> Self {
        self.ctx.dump_layout = v;
        self
    }

    pub fn with_library_mode(mut self, v: bool) -> Self {
        self.ctx.library_mode = v;
        self
    }

    /// Set the LLVM target triple for generated IR.
    /// Also updates the data layout to match.
    /// 2026-07-11: Phase 6 — WASM target support.
    pub fn with_target_triple(mut self, triple: &str) -> Self {
        self.ctx.target_triple = triple.to_string();
        self.ctx.data_layout = match triple {
            "wasm32-unknown-wasi" | "wasm32-unknown-unknown" => {
                Some("e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-n32:64-S128-ni:1:10:20".to_string())
            }
            _ => {
                // Default x86_64 data layout
                Some("e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128".to_string())
            }
        };
        self
    }

    /// Set the LLVM data layout string (overrides the auto-derived layout).
    /// 2026-07-15: Phase 7 — config-driven from targets.toml.
    pub fn with_data_layout(mut self, dl: &str) -> Self {
        self.ctx.data_layout = Some(dl.to_string());
        self
    }

    /// Emit a ptrtoint instruction with the correct pointer-width integer type.
    /// Uses i64 on x86_64, i32 on wasm32.
    /// Uses `&dyn Display` for dest/src so any Display-able type (String, &str,
    /// u64, TypedRegister, etc.) can be passed without manual conversion.
    /// 2026-07-11: Phase 6 — WASM pointer width.
    pub(super) fn emit_ptrtoint(&self, out: &mut String, indent: &str, dest: &dyn Display, src: &dyn Display) {
        let ptr_ty = self.ctx.pointer_llvm_type();
        writeln!(out, "{}{} = ptrtoint {} {} to ptr", indent, dest, ptr_ty, src).ok();
    }

    /// Emit an inttoptr instruction with the correct pointer-width integer type.
    /// Uses i64 on x86_64, i32 on wasm32.
    /// Uses `&dyn Display` for dest/src so any Display-able type (String, &str,
    /// u64, TypedRegister, etc.) can be passed without manual conversion.
    /// 2026-07-11: Phase 6 — WASM pointer width.
    pub(super) fn emit_inttoptr(&self, out: &mut String, indent: &str, dest: &dyn Display, src: &dyn Display) {
        let ptr_ty = self.ctx.pointer_llvm_type();
        writeln!(out, "{}{} = inttoptr {} {} to ptr", indent, dest, ptr_ty, src).ok();
    }

    /// Produce a human-readable layout summary for all state fields.
    pub fn dump_layout_str(&self) -> String {
        let mut out = String::new();
        out.push_str("\n=== Field Layout ===\n");
        let mut field_names: Vec<&String> = self.ctx.field_index_map.keys().collect();
        field_names.sort();
        for name in &field_names {
            let idx = self.ctx.field_index_map.get(*name).copied().unwrap_or(0);
            let ty = self.ctx.field_types.get(idx).map(|s| s.as_str()).unwrap_or("?");
            let mode = self.ctx.field_modes.get(*name)
                .map(|m| format!("{:?}", m))
                .unwrap_or_else(|| "Always".to_string());
            out.push_str(&format!("  {} @[{}]: {} (mode: {})", name, idx, ty, mode));
            if let Some(targets) = self.ctx.cache_slots.get(*name) {
                for (target_name, &(cache_idx, valid_idx)) in targets {
                    out.push_str(&format!(" | cache[{}]: [{}](i64), [{}](i8)", target_name, cache_idx, valid_idx));
                }
            }
            out.push('\n');
        }
        out.push_str("=== End Layout ===\n");
        out
    }

    pub fn gpu_backend(&self) -> &str {
        &self.ctx.gpu_backend
    }

    pub(crate) fn collect_gpu_kernel(
        &mut self,
        txn_name: &str,
        body: &[Statement],
        is_speculative: bool,
    ) {
        let eligibility = gpu::check_eligibility(body);
        eprintln!("[DBG] collect_gpu_kernel: txn={}, body_len={}, eligible={}, reasons={:?}", txn_name, body.len(), eligibility.eligible, eligibility.reasons);
        if !eligibility.eligible {
            if is_speculative {
                let msg = format!("txn '{}' not eligible: {}", txn_name, eligibility.reasons.join(", "));
                self.push_remark(directive::OptimizationRemark::skipped("gpu", msg));
            }
            return;
        }

        // Determine N for cost model: prefer PGO-derived bound, fall back to 0 (runtime).
        let pgo_bound = if self.ctx.pgo_profile.is_some() {
            let max_count = self.ctx.pgo_profile.as_ref().unwrap().branch_counts.values()
                .map(|&(t, f)| t.max(f)).max().unwrap_or(0);
            if max_count > 0 { Some(max_count) } else { None }
        } else { None };
        let cost_n = pgo_bound.unwrap_or(0);

        // Run cost model for speculative directives.
        if is_speculative {
            let est = crate::analysis::gpu_cost::estimate(body, cost_n);
            match est.recommended {
                crate::analysis::gpu_cost::OffloadDecision::Cpu => {
                    self.push_remark(directive::OptimizationRemark::skipped("gpu",
                        format!("txn '{}' kept on CPU — intensity {:.2} ops/byte, crossover N={}",
                            txn_name, est.arithmetic_intensity, est.crossover_point))
                        .with_analysis(vec![
                            format!("ops: {}, bytes: {}, intensity: {:.2}",
                                est.total_ops, est.total_bytes, est.arithmetic_intensity),
                            format!("estimated CPU: {:.0}ns, estimated GPU (incl. PCIe): {:.0}ns",
                                est.estimated_cpu_ns, est.estimated_gpu_ns),
                        ])
                        .with_hints(vec![
                            "Use #gpu (imperative) to force GPU offloading".to_string(),
                        ]));
                    return;
                }
                crate::analysis::gpu_cost::OffloadDecision::Runtime => {
                    // N is runtime-determined — emit dispatch branch.
                    self.push_remark(directive::OptimizationRemark::applied("gpu",
                        format!("txn '{}' will dispatch at runtime — crossover N={}",
                            txn_name, est.crossover_point))
                        .with_analysis(vec![
                            format!("crossing point: {} iterations", est.crossover_point),
                        ]));
                    // Fall through to collect kernel.
                }
                crate::analysis::gpu_cost::OffloadDecision::Gpu => {
                    self.push_remark(directive::OptimizationRemark::applied("gpu",
                        format!("txn '{}' offloaded to GPU — intensity {:.2} ops/byte",
                            txn_name, est.arithmetic_intensity)));
                }
            }
        }

        // Build field type map from the backend's field_index_map + field_types
        let field_types_map: std::collections::HashMap<String, String> = self.ctx.field_index_map.iter()
            .map(|(name, idx)| (name.clone(), self.ctx.field_types[*idx].clone()))
            .collect();
        let kernel = gpu::extract_kernel(txn_name, body, crate::ast::Expr::Decimal(0), &[], field_types_map);
        let spirv_ir = gpu::emit_spirv_module(&kernel);
        self.spirv_kernels.push(spirv_ir.clone());

        if let Ok(binary) = gpu::compile_to_spirv(&spirv_ir) {
            self.spirv_blobs.push(binary);
        } else if self.ctx.emit_remarks {
            // llc not available — emit warning.
            self.warnings.push("info: GPU kernel SPIR-V compilation skipped — llc not found. Install LLVM tools or use --no-gpu to suppress.".to_string());
        }
    }

    /// Append embedded SPIR-V blobs to the output IR string.
    /// Called at the end of `generate()` after all transactions are emitted.
    /// Emit a bump allocation from the active arena, or fall back to @malloc
    /// if no arena is active. Returns the register name holding the allocated
    /// i8* pointer.
    /// Emit preallocation for a single collection field within a bounded loop.
    /// Shared by both emit_prealloc_for_body and emit_prealloc_for_targets.
    fn emit_prealloc_one_field(&mut self, out: &mut String, indent: &str, field_name: &str, bound_reg: &str) {
        if !self.ctx.field_index_map.contains_key(field_name) {
            return;
        }
        let idx = self.ctx.field_index_map[field_name];
        if self.ctx.field_types[idx] != "i64" {
            return;
        }

        let c = self.fun.arena_counter;
        self.fun.arena_counter += 1;
        let cap = format!("%pcap_{}", c);
        writeln!(out, "{}{} = add i64 0, {}", indent, cap, bound_reg).ok();
        let slot_cnt = format!("%psc_{}", c);
        writeln!(out, "{}{} = add i64 {}, 2", indent, slot_cnt, cap).ok();
        let alloc_sz = format!("%psz_{}", c);
        writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_sz, slot_cnt).ok();
        let buf_reg = self.emit_arena_alloc(out, indent, &alloc_sz);
        let buf_i64 = format!("%pbp_{}", c);
        writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, buf_i64, buf_reg).ok();
        let base = format!("%pba_{}", c);
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, base, buf_reg).ok();
        let data_ptr = format!("%pdv_{}", c);
        writeln!(out, "{}{} = add i64 {}, 16", indent, data_ptr, base).ok();
        let s0 = format!("%ps0_{}", c);
        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 0", indent, s0, buf_i64).ok();
        writeln!(out, "{}store i64 {}, ptr {}, align 8, !tbaa !1", indent, data_ptr, s0).ok();
        let s1 = format!("%ps1_{}", c);
        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, s1, buf_i64).ok();
        writeln!(out, "{}store i64 0, ptr {}, align 8, !tbaa !1", indent, s1).ok();
        let ap = format!("%pap_{}", c);
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
        let tn = crate::backend::llvm::tbaa_node(&self.ctx.field_types[idx], self.ctx.type_universe.as_ref());
        writeln!(out, "{}store i64 {}, ptr {}, align 8, !tbaa !{}", indent, base, ap, tn).ok();
        self.fun.field_prealloc_info.insert(field_name.to_string(), (cap, buf_i64));
    }

    /// Emit preallocation for collection fields that receive `<- push` within
    /// a bounded loop. Scans body for push targets via collect_push_targets.
    pub(crate) fn emit_prealloc_for_body(
        &mut self,
        out: &mut String,
        indent: &str,
        body: &[Statement],
        bound_reg: &str,
    ) {
        let mut push_targets: Vec<String> = Vec::new();
        collect_push_targets(body, &mut push_targets);
        push_targets.sort();
        push_targets.dedup();
        for field_name in &push_targets {
            self.emit_prealloc_one_field(out, indent, field_name, bound_reg);
        }
    }

    /// Emit preallocation for a pre-collected list of push target field names.
    pub(crate) fn emit_prealloc_for_targets(
        &mut self,
        out: &mut String,
        indent: &str,
        push_targets: &[String],
        bound_reg: &str,
    ) {
        let mut targets: Vec<String> = push_targets.to_vec();
        targets.sort();
        targets.dedup();
        for field_name in &targets {
            self.emit_prealloc_one_field(out, indent, field_name, bound_reg);
        }
    }

    pub(crate) fn emit_arena_alloc(&mut self, out: &mut String, indent: &str, size_reg: &str) -> String {
        if let Some((ref ptr, ref end, ref base)) = self.fun.arena_slots.clone() {
            let c = self.fun.arena_counter;
            self.fun.arena_counter += 1;
            // 2026-06-26: emit br label %check_l before the check label to
            // terminate whatever block the caller left unterminated (callers
            // emit straight-line code before emit_arena_alloc). Without this,
            // LLVM sees the check label as a new basic block whose predecessor
            // has no terminator — "expected instruction opcode" error.
            // The check label is also used as the PHI predecessor for the
            // "no grow needed" path (aaok_N), avoiding the old self-loop PHI
            // that listed aaok_N as its own predecessor.
            let check_l = format!("aacheck_{}", c);
            writeln!(out, "{}br label %{}", indent, check_l).ok();
            writeln!(out, "{}{}:", indent, check_l).ok();
            let cur = format!("%aacur{}", c);
            writeln!(out, "{}{} = load ptr, ptr {}, align 8", indent, cur, ptr).ok();
            let new_ptr = format!("%aanew{}", c);
            writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, new_ptr, cur, size_reg).ok();
            let end_val = format!("%aaend{}", c);
            writeln!(out, "{}{} = load ptr, ptr {}, align 8", indent, end_val, end).ok();
            let ok = format!("%aaok{}", c);
            writeln!(out, "{}{} = icmp ule i8* {}, {}", indent, ok, new_ptr, end_val).ok();
            let grow_l = format!("aagrow_{}", c);
            let ok_l = format!("aaok_{}", c);
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, ok, ok_l, grow_l).ok();
            writeln!(out, "{}{}:", indent, grow_l).ok();
            let old_base = format!("%aaob{}", c);
            writeln!(out, "{}{} = load ptr, ptr {}, align 8", indent, old_base, base).ok();
            let grow_sz = format!("%aags{}", c);
            writeln!(out, "{}{} = shl i64 {}, 1", indent, grow_sz, size_reg).ok();
            let min_sz = format!("%aams{}", c);
            writeln!(out, "{}{} = add i64 {}, 65536", indent, min_sz, grow_sz).ok();
            let new_base = format!("%aanb{}", c);
            writeln!(out, "{}{} = call ptr @realloc(i8* {}, i64 {})", indent, new_base, old_base, min_sz).ok();
            writeln!(out, "{}store i8* {}, ptr {}, align 8", indent, new_base, ptr).ok();
            writeln!(out, "{}store i8* {}, ptr {}, align 8", indent, new_base, base).ok();
            let new_end = format!("%aane{}", c);
            writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, new_end, new_base, min_sz).ok();
            writeln!(out, "{}store i8* {}, ptr {}, align 8", indent, new_end, end).ok();
            writeln!(out, "{}br label %{}", indent, ok_l).ok();
            writeln!(out, "{}{}:", indent, ok_l).ok();
            let phi = format!("%aaphi{}", c);
            writeln!(out, "{}{} = phi i8* [ {}, %{} ], [ {}, %{} ]",
                indent, phi, cur, check_l, new_base, grow_l).ok();
            // 2026-06-26: compute the new bump from the PHI value (not from
            // the pre-realloc cur), so the grow path uses %aanb + size_reg
            // instead of the dangling old-bump pointer. Without this fix,
            // realloc frees the old buffer but the bump update still points
            // into freed memory — catastrophic corruption on next allocation.
            let new_bump = format!("%aanbp{}", c);
            writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, new_bump, phi, size_reg).ok();
            writeln!(out, "{}store i8* {}, ptr {}, align 8", indent, new_bump, ptr).ok();
            phi
        } else {
            let c = self.fun.arena_counter;
            self.fun.arena_counter += 1;
            let r = format!("%aam{}", c);
            writeln!(out, "{}{} = call noalias ptr @malloc(i64 {})", indent, r, size_reg).ok();
            r
        }
    }

    /// Emit arena initialization at scope entry. Allocates the initial
    /// 64KB arena buffer, sets up ptr/end/base alloca slots.
    pub(crate) fn emit_arena_init(&mut self, out: &mut String, indent: &str) {
        let c = self.fun.arena_counter;
        self.fun.arena_counter += 1;
        let ptr = format!("%arptr{}", c);
        let end = format!("%arend{}", c);
        let base = format!("%arbase{}", c);
        writeln!(out, "{}{} = alloca i8*, align 8", indent, ptr).ok();
        writeln!(out, "{}{} = alloca i8*, align 8", indent, end).ok();
        writeln!(out, "{}{} = alloca i8*, align 8", indent, base).ok();
        let init = format!("%arinit{}", c);
        writeln!(out, "{}{} = call ptr @malloc(i64 65536)", indent, init).ok();
        writeln!(out, "{}store i8* {}, ptr {}, align 8", indent, init, ptr).ok();
        writeln!(out, "{}store i8* {}, ptr {}, align 8", indent, init, base).ok();
        let init_end = format!("%arieu{}", c);
        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 65536", indent, init_end, init).ok();
        writeln!(out, "{}store i8* {}, ptr {}, align 8", indent, init_end, end).ok();
        self.fun.arena_slots = Some((ptr, end, base));
    }

    /// Emit arena reset: rewinds the bump pointer to the base, preserving
    /// the allocated memory for reuse in the next scope iteration.
    /// This is Phase 3 — cross-tick arena pool — keeps pages alive across
    /// loop ticks instead of free+malloc per cycle.
    pub(crate) fn emit_arena_reset(&mut self, out: &mut String, indent: &str) {
        if let Some((ref ptr, _end, ref base)) = self.fun.arena_slots.clone() {
            let r = format!("%arr{}", self.fun.arena_counter);
            self.fun.arena_counter += 1;
            writeln!(out, "{}{} = load ptr, ptr {}, align 8", indent, r, base).ok();
            writeln!(out, "{}store i8* {}, ptr {}, align 8", indent, r, ptr).ok();
            // Slots stay alive (arena is not freed). Memory is reused on next tick.
        }
    }

    /// Emit arena teardown at program exit. Frees the arena buffer and
    /// clears the arena_slots flag. After this, dynamic allocations
    /// fall back to @malloc.
    pub(crate) fn emit_arena_fini(&mut self, out: &mut String, indent: &str) {
        if let Some((_ptr, _end, ref base)) = self.fun.arena_slots.clone() {
            let f = format!("%arf{}", self.fun.arena_counter);
            self.fun.arena_counter += 1;
            writeln!(out, "{}{} = load ptr, ptr {}, align 8", indent, f, base).ok();
            writeln!(out, "{}call void @free(ptr {})", indent, f).ok();
            self.fun.arena_slots = None;
        }
    }

    // 2026-07-18: Check if we're in a scope with a known contract-proven bound.
    // Used by Alloc# to decide between arena bump vs alloca vs @malloc.
    pub(crate) fn is_in_bounded_scope(&self) -> bool {
        self.fun.is_static_bound
    }

    // 2026-07-18: Check if the allocation result escapes the current scope.
    // Simplified: checks if the result register is stored to %State or returned.
    // TEMP: Always returns false (assume no escape) until Phase 4 (Ptr Level 3
    // borrow checker) provides provenance-based escape analysis.
    pub(crate) fn will_escape_current_allocation(&self) -> bool {
        false
    }

    pub(crate) fn emit_spirv_embeds(&self) -> String {
        let mut out = String::new();
        if self.spirv_blobs.is_empty() && self.spirv_kernels.is_empty() {
            return out;
        }
        out.push_str("\n; === GPU Kernel Blobs ===\n");
        for (i, blob) in self.spirv_blobs.iter().enumerate() {
            let name = format!("kernel_{}", i);
            out.push_str(&gpu::embed_spirv_blob(blob, &name));
        }
        out
    }

    pub(crate) fn push_remark(&mut self, remark: crate::backend::llvm::directive::OptimizationRemark) {
        if self.ctx.emit_remarks {
            self.remarks.push(remark);
        }
    }

    pub fn remarks(&self) -> &[crate::backend::llvm::directive::OptimizationRemark] {
        &self.remarks
    }

    pub fn spirv_kernels(&self) -> &[String] {
        &self.spirv_kernels
    }

    /// Scan the typed program for constructs that are forbidden in embedded mode:
    /// dynamic heap allocation (List, String, HashMap) and threading intrinsics.
    /// Also warns about unbounded recursion via the call graph's cycle detection.
    pub(crate) fn check_embedded_restrictions(&mut self, items: &[TopLevel]) {
        let threading_intrinsics: &[&str] = &[
            "ThreadCreate#", "ThreadJoin#", "ThreadExit#",
            "MutexLock#", "MutexUnlock#",
            "CondvarWait#", "CondvarSignal#", "CondvarBroadcast#",
        ];
        for item in items {
            match item {
                TopLevel::StateDecl(decl) => {
                    if self.type_is_heap_allocated(&decl.ty) {
                        self.warnings.push(format!(
                            "TargetError: dynamic allocation not supported on target 'Embedded' — state variable '{}' has type {:?}",
                            decl.name, decl.ty
                        ));
                    }
                }
                TopLevel::Transaction(txn) => {
                    for stmt in &txn.body {
                        self.check_stmt_embedded(stmt, &txn.name, threading_intrinsics);
                    }
                }
                TopLevel::Definition(defn) => {
                    for stmt in &defn.body {
                        self.check_stmt_embedded(stmt, &defn.name, threading_intrinsics);
                    }
                }
                _ => {}
            }
        }
        if self.ctx.has_cycles {
            self.warnings.push(
                "TargetError: unbounded recursion detected — call graph has cycles, which are not supported on target 'Embedded'".to_string()
            );
        }
    }

    fn type_is_heap_allocated(&self, ty: &Type) -> bool {
        // 2026-07-18: Phase A — String check replaced with is_string_like.
        // Phase B (SSO) may make short strings non-heap; is_string_like stays.
        // 2026-07-18: Utf8View, StaticString, SmallString64 are never heap-allocated.
        if matches!(ty, Type::Custom(name) if name == "Utf8View" || name == "StaticString" || name == "SmallString64") {
            return false;
        }
        (self.ctx.type_universe.as_ref().map_or(false, |u| u.is_string_like(ty))
            || matches!(ty, Type::Custom(__t) if __t == "Data"))
            || matches!(ty, Type::Custom(name) if name == "List" || name == "HashMap" || name == "HashSet" || name == "Stack" || name == "Queue" || name == "StringBuilder")
    }

    fn check_stmt_embedded(&mut self, stmt: &Statement, ctx_name: &str, threading_intrinsics: &[&str]) {
        match stmt {
            Statement::Let { ty, expr, .. } => {
                if let Some(t) = ty {
                    if self.type_is_heap_allocated(t) {
                        self.warnings.push(format!(
                            "TargetError: dynamic allocation not supported on target 'Embedded' — variable in '{}' has type {:?}",
                            ctx_name, t
                        ));
                    }
                }
                if let Some(e) = expr {
                    self.check_expr_embedded(e, ctx_name, threading_intrinsics);
                }
            }
            Statement::Assign(_, expr) => {
                self.check_expr_embedded(expr, ctx_name, threading_intrinsics);
            }
            Statement::Expression(e) => {
                self.check_expr_embedded(e, ctx_name, threading_intrinsics);
            }
            Statement::Term(Some(e)) | Statement::TermBang(Some(e)) => {
                self.check_expr_embedded(e, ctx_name, threading_intrinsics);
            }
            Statement::Term(None) | Statement::TermBang(None) => {}
            Statement::Return(Some(e)) => {
                self.check_expr_embedded(e, ctx_name, threading_intrinsics);
            }
            Statement::Return(None) => {}
            Statement::Guarded(condition, statements) => {
                self.check_expr_embedded(condition, ctx_name, threading_intrinsics);
                for s in statements {
                    self.check_stmt_embedded(s, ctx_name, threading_intrinsics);
                }
            }
            Statement::If(_, then_body, else_body) => {
                for s in then_body { self.check_stmt_embedded(s, ctx_name, threading_intrinsics); }
                for s in else_body { self.check_stmt_embedded(s, ctx_name, threading_intrinsics); }
            }
            Statement::Block(body) | Statement::SyncBlock(body) => {
                for s in body { self.check_stmt_embedded(s, ctx_name, threading_intrinsics); }
            }
            Statement::Foreach { list, body, .. } => {
                self.check_expr_embedded(list, ctx_name, threading_intrinsics);
                for s in body {
                    self.check_stmt_embedded(s, ctx_name, threading_intrinsics);
                }
            }
            Statement::Escape(Some(e)) => {
                self.check_expr_embedded(e, ctx_name, threading_intrinsics);
            }
            Statement::Escape(None) => {}
            _ => {}
        }
    }

    fn check_expr_embedded(&mut self, expr: &Expr, ctx_name: &str, threading_intrinsics: &[&str]) {
        match expr {
            Expr::Call(name, args, _) => {
                if threading_intrinsics.contains(&name.as_str()) {
                    self.warnings.push(format!(
                        "TargetError: threading intrinsic not supported on target 'Embedded' — '{}' in '{}'",
                        name, ctx_name
                    ));
                }
                for arg in args {
                    self.check_expr_embedded(arg, ctx_name, threading_intrinsics);
                }
            }
            Expr::BinaryOp(_, l, r) => {
                self.check_expr_embedded(l, ctx_name, threading_intrinsics);
                self.check_expr_embedded(r, ctx_name, threading_intrinsics);
            }
            Expr::UnaryOp(_, inner) | Expr::Cast(inner, _) | Expr::IsType(inner, _) => {
                self.check_expr_embedded(inner, ctx_name, threading_intrinsics);
            }
            Expr::Field(target, _) | Expr::Index(target, _) => {
                self.check_expr_embedded(target, ctx_name, threading_intrinsics);
            }
            Expr::Block(stmts) => {
                for s in stmts {
                    self.check_stmt_embedded(s, ctx_name, threading_intrinsics);
                }
            }
            Expr::If(cond, then_b, else_b) => {
                self.check_expr_embedded(cond, ctx_name, threading_intrinsics);
                self.check_expr_embedded(then_b, ctx_name, threading_intrinsics);
                if let Some(eb) = else_b {
                    self.check_expr_embedded(eb, ctx_name, threading_intrinsics);
                }
            }
            Expr::Match(value, arms) => {
                self.check_expr_embedded(value, ctx_name, threading_intrinsics);
                for arm in arms {
                    self.check_expr_embedded(&arm.body, ctx_name, threading_intrinsics);
                }
            }
            Expr::Tuple(elems) | Expr::List(elems) => {
                for e in elems {
                    self.check_expr_embedded(e, ctx_name, threading_intrinsics);
                }
            }
            Expr::Lambda(_, body) => {
                self.check_expr_embedded(body, ctx_name, threading_intrinsics);
            }
            Expr::Within(body, fallback) => {
                self.check_expr_embedded(body, ctx_name, threading_intrinsics);
                self.check_expr_embedded(fallback, ctx_name, threading_intrinsics);
            }
            Expr::DerivationBlock(db) => {
                for ex in &db.examples {
                    for inp in &ex.inputs {
                        self.check_expr_embedded(inp, ctx_name, threading_intrinsics);
                    }
                    self.check_expr_embedded(&ex.output, ctx_name, threading_intrinsics);
                }
            }
            _ => {}
        }
    }

    pub fn generate(&mut self, items: &[TopLevel], exit_condition: Option<Box<Expr>>) -> String {
        let mut analysis = crate::backend::analyze_program(items, false);
        self.ctx.dep_graph = analysis.dependency_graph.clone();

        analysis.region_analyzer.compose_chains();
        analysis.region_analyzer.build_budget_plan(self.ctx.optimize_budget);

        // ── Precomputation check (EmitPureCounterFold) ──────────────────────────────
        //
        // Before emitting any runtime loop, check if the entire program can be
        // precomputed at compile time. If all inputs are const and the body
        // converges within --optimize-budget, we emit O(1) final-value stores.
        // If budget is exceeded but no FFI exists, warn and fall through to
        // runtime loop. If FFI exists, warn that compile-time eval is blocked.

        let precomputed_final_values = if analysis.region_analyzer.is_fully_precomputable(self.ctx.optimize_budget) {
            analysis.region_analyzer.collect_final_values(items)
        } else if !analysis.region_analyzer.composed_chains.is_empty() {
            // A002 already covers empty-chains case: no precomputable program.
            // Only emit A001 when chains exist but budget/FFI prevents evaluation.
            let has_ffi = analysis.region_analyzer.composed_chains.iter().any(|cc|
                crate::analysis::region::has_ffi_or_trigger_stmt_in_chain(&cc.composed_body));
            if has_ffi {
                self.warnings.push("info: program not fully precomputed — FFI calls in transaction body prevent compile-time evaluation".into());
            } else {
                self.warnings.push(format!(
                    "info: program not fully precomputed — budget {} exceeded by composed chain product. Emitting runtime loop.",
                    self.ctx.optimize_budget));
            }
            None
        } else {
            None
        };

        let cg = &analysis.call_graph;
        self.ctx.has_cycles = cg.has_cycle();

        if self.ctx.is_embedded {
            self.check_embedded_restrictions(items);
        }

        self.ctx.exit_condition = exit_condition;
        // 2026-07-13: normalize_to_old_recursive removed in new AST.
        // BinaryOp/UnaryOp exit conditions are already the canonical form.
        self.build_field_index(items);

        // Scan for cell-to-cell wires from TrgBinding statements
        self.scan_cell_wires(items);

        // Inject synthetic __trg_epfd field if program has built-in triggers
        let has_builtin_trg = false;
        // 2026-07-13: New AST triggers don't have address field.
        // Built-in trigger detection (stdin/timer/signal) is TODO.
        // For now, has_builtin_trg is always false.
        if has_builtin_trg && !self.ctx.field_index_map.contains_key("__trg_epfd") {
            let idx = self.ctx.field_index_map.len();
            self.ctx.field_index_map.insert("__trg_epfd".to_string(), idx);
            self.ctx.field_types.push("i32".to_string());
            self.ctx.field_brief_types.push(Type::int());
            self.ctx.field_initializers.insert("__trg_epfd".to_string(), None);
        }
        // Inject synthetic cycle_count field for watchdog timing
        if !self.ctx.field_index_map.contains_key("cycle_count") {
            let idx = self.ctx.field_index_map.len();
            self.ctx.field_index_map.insert("cycle_count".to_string(), idx);
            self.ctx.field_types.push("i64".to_string());
            self.ctx.field_brief_types.push(Type::int());
            self.ctx.field_initializers.insert("cycle_count".to_string(), Some(Expr::Decimal(0)));
        }
        self.validate_schema_types();
        self.ctx.triggers.clear();
        self.ctx.trigger_names.clear();
        self.program_txns.clear();
        self.ctx.defn_params.clear();
        self.ctx.defn_return_types.clear();
        self.ctx.constants.clear();
        self.ctx.string_constants = collect_strings(items);

        let mut txns: Vec<(String, &crate::ast::Transaction)> = Vec::new();
        for item in items {
            match item {
                TopLevel::Constant(c) => {
                    self.ctx.constants.insert(c.name.clone(), (c.ty.clone(), c.expr.clone()));
                }
                TopLevel::Transaction(t) => {
                    txns.push((t.name.clone(), t));
                    self.program_txns.push(t.name.clone());
                    // Register callable txn param types for Expr::Call marshaling
                    let has_output = t.output_type.is_some() || !t.outputs.is_empty();
                    if !t.is_reactive && (!t.parameters.is_empty() || has_output) {
                        let tys: Vec<Type> = t.parameters.iter().map(|(_, ty)| ty.clone()).collect();
                        self.ctx.defn_params.insert(t.name.clone(), tys);
                        self.ctx.defn_return_types.insert(t.name.clone(), t.outputs.clone());
                    }
                }
                TopLevel::Trigger(trg) => {
                    // 2026-07-14: Convert new AST Trigger to TriggerDeclaration.
                    // The new Trigger struct has name/instance/port/span fields.
                    // 2026-07-15: Support @ *ptr dynamic triggers — map Expr::Deref
                    // to LinkRef::Deref so emit_trg_load emits a load from the pointer.
                    let address = match &trg.instance {
                        Expr::Deref(ptr_expr) => {
                            crate::ast::LinkRef::Deref(ptr_expr.clone())
                        }
                        _ => crate::ast::LinkRef::Explicit(0),
                    };
                    let trg_decl = crate::ast::TriggerDeclaration {
                        name: trg.name.clone(),
                        ty: crate::ast::Type::string(),
                        address,
                        bit_range: None,
                        stages: vec![],
                        condition: None,
                        // 2026-07-14: Triggers whose name starts with __wake are wake triggers
                        is_wake: trg.name.starts_with("__wake") || trg.port == "__wake",
                        is_const: false,
                        span: trg.span.clone(),
                        annotations: vec![],
                        modifiers: vec![],
                    };
                    self.ctx.triggers.insert(trg.name.clone(), trg_decl);
                    self.ctx.trigger_names.push(trg.name.clone());
                }
                TopLevel::Definition(d) => {
                    let tys: Vec<Type> = d.parameters.iter().map(|(_, t)| t.clone()).collect();
                    self.ctx.defn_params.insert(d.name.clone(), tys);
                    self.ctx.defn_return_types.insert(d.name.clone(), d.outputs.clone());
                }
                TopLevel::ForeignBinding(fb) => {
                    let sig = crate::ast::ForeignSignature {
                        name: fb.name.clone(),
                        from: fb.from.clone(),
                        inputs: fb.inputs.clone(),
                        result_type: crate::ast::ResultType::Projection(fb.success_output.iter().map(|(_, t)| t.clone()).collect()),
                        wasm_impl: fb.wasm_impl.clone(),
                        wasm_setup: fb.wasm_setup.clone(),
                        span: fb.span,
                    };
                    self.ctx.frgn_map.insert(fb.name.clone(), sig);
                }
                TopLevel::Inop(inop) => {
                    self.ctx.inop_decls.insert(inop.name.clone(), inop.clone());
                }
                TopLevel::Struct(s) => {
                    let fields: Vec<(String, Type)> = s.fields.iter()
                        .map(|f| (f.name.clone(), f.ty.clone()))
                        .collect();
                    self.ctx.struct_types.insert(s.name.clone(), fields.clone());

                    // 2026-07-14: Struct auto-registration in TypeUniverse with
                    // dynamic byte size computed from field types.
                    if let Some(ref mut universe) = self.ctx.type_universe {
                        if !universe.types.contains_key(&s.name) {
                            let bytes: u64 = fields.iter().map(|(_, ty)| {
                                match ty {
                                    Type::Custom(n) if n == "Bool" => 1,
                                    _ => 8,
                                }
                            }).sum();
                            let rt = crate::type_universe::ResolvedType {
                                name: s.name.clone(),
                                base: "Bits".to_string(),
                                bytes,
                                alignment: 8,
                                properties: std::collections::HashMap::new(),
                                fields: vec![],
                            };
                            universe.types.insert(s.name.clone(), rt);
                        }
                    }
                }
                // 2026-07-14: Register TypeDef slots as struct types so
                // test_type_with_slots_populates_struct_types passes.
                TopLevel::TypeDef(td) => {
                    let fields: Vec<(String, Type)> = td.body.slots.iter()
                        .map(|s| (s.name.clone(), s.ty.clone()))
                        .collect();
                    self.ctx.struct_types.insert(td.name.clone(), fields);
                }
                TopLevel::Enum(e) => {
                    self.ctx.enum_types.insert(e.name.clone(), e.clone());
                }
                TopLevel::Cell(c) => {
                    self.ctx.cell_defs.insert(c.name.clone(), c.clone());
                }
                TopLevel::TriggerBinding { name, instance, port, ty, modifiers: _ } => {
                    // Register a cell binding trigger: trg name @ CellName!.port
                    if let Expr::Identifier(cell_name) = instance {
                        let resolved_port = if port.is_empty() {
                            // Auto-detect single output port: use the first named output
                            if let Some(cell_def) = self.ctx.cell_defs.get(cell_name.as_str()) {
                                "line".to_string() // Console's first output port
                            } else { String::new() }
                        } else { port.clone() };
                        if !resolved_port.is_empty() {
                            self.ctx.cell_trigger_bindings.push((
                                name.clone(), cell_name.clone(), resolved_port.clone()
                            ));
                            // Register the trigger so its storage is allocated in %State
                            let trig_ty = ty.clone().unwrap_or(crate::ast::Type::string());
                            self.ctx.trigger_names.push(name.clone());
                            let trg_decl = crate::ast::TriggerDeclaration {
                                name: name.clone(),
                                ty: trig_ty,
                                address: crate::ast::LinkRef::Explicit(0),
                                bit_range: None, stages: vec![], condition: None,
                                is_wake: false, is_const: false, span: None,
                                annotations: vec![],
                                modifiers: vec![],
                            };
                            self.ctx.triggers.insert(name.clone(), trg_decl);
                        }
                    }
                }
                _ => {}
            }
        }

        // 2026-07-13: struct_layout removed from ResolvedType in new AST.
        // TypeUniverse-based struct population is a no-op until slot syntax is reintroduced.

        // Verify all #!exit identifiers exist as state fields or constants.
        // Run BEFORE field elimination so we see the full field set.
        if let Some(ref cond) = self.ctx.exit_condition {
            let errors = self.check_exit_condition_idents(cond);
            if !errors.is_empty() {
                for err in &errors {
                    eprintln!("{}", err);
                }
                std::process::exit(1);
            }
        }

        // Phase 1: Apply adaptive layout — eliminate Never fields, append cache slots.
        // Must run AFTER trigger_names is populated (above) to prevent trigger field elimination.
        {
            let projection_usage = crate::analysis::transition_graph::compute_projection_usage(items);
            self.apply_field_modes(items, &analysis.transition_graph.live_fields, &projection_usage);
        }

        // W001: Detect dead cache slots — allocated but no loop context (one-shot program).
        if !self.ctx.cache_slots.is_empty() {
            // Check if ANY transaction has bounded convergence (loop context)
            let has_loop_context = analysis.transition_graph.nodes.iter().any(|n| {
                n.bounded_pre.is_some()
                    && n.increments.as_ref().map_or(false, |i| i.delta > 0)
            });
            if !has_loop_context {
                let field_list: Vec<String> = self.ctx.cache_slots.keys().cloned().collect();
                self.warnings.push(format!(
                    "W001: dead cache slot(s) — `{}` allocated cache slots but the program has no loop context.\n\
                      note: cache slots are only useful when a projection appears multiple times in a loop body.\n\
                      help: remove the meld routes that trigger dual-lens access, or add a loop.",
                    field_list.join(", "),
                ));
            }
        }

        // Build variant → (enum_name, discriminant, field_count) mapping.
        for (enum_name, edef) in &self.ctx.enum_types {
            let mut next_disc: u64 = 0;
            for v in &edef.variants {
                let (vname, field_count) = match v {
                    crate::ast::EnumVariant::Unit(n) => (n.clone(), 0),
                    crate::ast::EnumVariant::Tuple(n, fields) => (n.clone(), fields.len()),
                    crate::ast::EnumVariant::Struct(n, fields) => (n.clone(), fields.len()),
                };
                let disc = next_disc;
                next_disc += 1;
                self.ctx.variant_disc.insert(vname, (enum_name.clone(), disc, field_count));
            }
        }

        // Fold complex float constant expressions (e.g. const m0: Float = 4.0 * pi * pi)
        // into simple Expr::Float(f64) literals so the global emission path
        // produces valid LLVM IR instead of `constant float 0`.
        let consts_snapshot: Vec<(String, (Type, Expr))> = self.ctx.constants.iter()
            .map(|(k, v)| (k.clone(), v.clone())).collect();
        for (name, (ty, expr)) in consts_snapshot {
            // 2026-07-13: Expr::Float64 removed — all floats use Expr::Float.
            if ty == Type::float() || ty == Type::float64() {
                if let Some(val) = try_eval_cfloat(&expr, &self.ctx.constants) {
                    self.ctx.constants.insert(name, (ty.clone(), Expr::Float(val)));
                }
            }
        }

        // Select optimization strategy via extracted decision tree
        let strategy = self.select_optimization_strategy(items, &analysis, &txns);
        let dispatch_mode = strategy.dispatch_mode;
        let has_wake_triggers = strategy.has_wake_triggers;
        let enumerable = strategy.enumerable;
        let enum_keys = strategy.enum_keys;
        let enum_txn_names = strategy.enum_txn_names;

        let mut out = String::new();
        self.emit_header(&mut out);
        // 2026-07-10: Phase 1 — emit struct type declarations before
        // function definitions so foreign callers see named types.
        self.declare_struct_types(&mut out);
        self.emit_declares(&mut out);

        // Emit foreign declares inline (frgn_map is populated from the scan above)
        // Skip names that are also linked triggers — they'll be emitted as global variables below.
        let trigger_linked_symbols: std::collections::HashSet<&str> = self.ctx.triggers.iter()
            .filter_map(|(_, t)| match &t.address {
                crate::ast::LinkRef::Linked(sym) => Some(sym.as_str()),
                _ => None,
            })
            .collect();
        for (name, sig) in &self.ctx.frgn_map {
            if trigger_linked_symbols.contains(name.as_str()) { continue; }
            let ret_ty = match sig.result_type {
                crate::ast::ResultType::VoidType | crate::ast::ResultType::TrueAssertion => "void",
                crate::ast::ResultType::Projection(ref ts) => {
                    if ts.is_empty() || ts.iter().any(|t| matches!(t, Type::Void)) { "void" }
                    else if ts.iter().any(|t| matches!(t, Type::Custom(__t) if __t == "Float")) { "float" }
                    else { "i64" }
                }
            };
            let param_tys: Vec<&str> = sig.inputs.iter().map(|(_, t)| match t {
                Type::Custom(__t) if __t == "Int" || __t == "UInt" => "i64",
                Type::Custom(__t) if __t == "Bool" => "i32",
                Type::Custom(__t) if __t == "Char" => "i32",
                Type::Custom(__t) if __t == "Float" => "float",
        Type::Custom(__t) if __t == "String" || __t == "Data" => "i8*",
                _ => "i64",
            }).collect();
            write!(out, "declare {} @{}(", ret_ty, name).ok();
            for (pi, pt) in param_tys.iter().enumerate() {
                if pi > 0 { write!(out, ", ").ok(); }
                write!(out, "{}", pt).ok();
            }
            writeln!(out, ") #6").ok();
        }
        // 2026-07-15: POSIX declares removed — they conflict with defn wrappers
        // (getpid, sigprocmask, close, nanosleep, sched_yield, getuid, etc.)

        // Declare cast helper functions
        writeln!(out, "declare i8* @__chr_to_str(i32) #1").ok();
        writeln!(out, "declare i64 @__int_to_str__(i64) #1").ok();
        writeln!(out, "declare i64 @__str_bytes__(i64) #1").ok();
        writeln!(out, "declare i64 @__str_to_int(i8*) #1").ok();

        // 2026-07-08: Phase 3 — brief_rt.c wrapper function declarations
        // These are called by inop declarations in lib/std/os/*.bv.
        // All take/return i64 (boxed value) matching Brief's ABI.
        writeln!(out, "declare i64 @brief_open(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_close(i64) #1").ok();
        writeln!(out, "declare i64 @brief_read(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_write(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_lseek(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_pread(i64, i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_pwrite(i64, i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_stat(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_fstat(i64) #1").ok();
        writeln!(out, "declare i64 @brief_truncate(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_ftruncate(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_fsync(i64) #1").ok();
        writeln!(out, "declare i64 @brief_dup(i64) #1").ok();
        writeln!(out, "declare i64 @brief_dup2(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_fcntl(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_socket(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_bind(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_listen(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_accept(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_connect(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_send(i64, i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_recv(i64, i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_sendto(i64, i64, i64, i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_recvfrom(i64, i64, i64, i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_setsockopt(i64, i64, i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_getsockopt(i64, i64, i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_shutdown(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_mkdir(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_rmdir(i64) #1").ok();
        writeln!(out, "declare i64 @brief_unlink(i64) #1").ok();
        writeln!(out, "declare i64 @brief_rename(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_symlink(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_link(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_chdir(i64) #1").ok();
        writeln!(out, "declare i64 @brief_chmod(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_chown(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_umask(i64) #1").ok();
        writeln!(out, "declare i64 @brief_access(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_mmap(i64, i64, i64, i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_munmap(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_mprotect(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_brk(i64) #1").ok();
        writeln!(out, "declare i64 @brief_mlock(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_pipe(i64) #1").ok();
        writeln!(out, "declare i64 @brief_shm_open(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_shm_unlink(i64) #1").ok();
        writeln!(out, "declare i64 @brief_sem_open(i64, i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_sem_wait(i64) #1").ok();
        writeln!(out, "declare i64 @brief_sem_post(i64) #1").ok();
        writeln!(out, "declare i64 @brief_getpid() #1").ok();
        writeln!(out, "declare i64 @brief_getppid() #1").ok();
        writeln!(out, "declare i64 @brief_clock_gettime(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_nanosleep(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_getenv(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_setenv(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_unsetenv(i64) #1").ok();
        writeln!(out, "declare i64 @brief_futex(i64, i64, i64, i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @__ioctl__(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @__isatty__(i64) #1").ok();
        writeln!(out, "declare i64 @__print(i64) #1").ok();
        writeln!(out, "declare i64 @brief_getuid() #1").ok();
        writeln!(out, "declare i64 @brief_geteuid() #1").ok();
        writeln!(out, "declare i64 @brief_getgid() #1").ok();
        writeln!(out, "declare i64 @brief_getegid() #1").ok();
        writeln!(out, "declare i64 @brief_sched_yield() #1").ok();
        writeln!(out, "declare i64 @brief_getpriority(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_setpriority(i64, i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_getrlimit(i64) #1").ok();
        writeln!(out, "declare i64 @brief_setrlimit(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_pagesize() #1").ok();
        writeln!(out, "declare i64 @brief_cpu_count() #1").ok();
        writeln!(out, "declare i64 @brief_ttyname(i64) #1").ok();
        writeln!(out, "declare i64 @brief_ring_push(i64, i64) #1").ok();
        writeln!(out, "declare i64 @brief_ring_pop(i64) #1").ok();
        writeln!(out, "declare i64 @__tty_read_key__(i64) #1").ok();
        writeln!(out, "declare i64 @__tty_size__() #1").ok();
        writeln!(out, "declare i64 @cpu_count() #1").ok();
        writeln!(out, "declare i64 @pagesize() #1").ok();

        // Format string constants for benchmark intrinsics (print_int#, print_float#)
        writeln!(out, "@FMT_INT = private unnamed_addr constant [5 x i8] c\"%ld\\0A\\00\"").ok();
        writeln!(out, "@FMT_FLOAT = private unnamed_addr constant [6 x i8] c\"%.9f\\0A\\00\"").ok();
        writeln!(out, "@FMT_STR = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\"").ok();
        // Error message for read_file# — returned as Err's String payload
        writeln!(out, "@STR_READFILE_ERR = private unnamed_addr constant [15 x i8] c\"file not found\\00\"").ok();
        // Declare libc functions used by direct-libc intrinsics
        // 2026-07-05: dso_local prevents LLVM globalopt from treating
        // @stdout as null (LLVM 18 assumes external globals without
        // initializer are zero = null). Without dso_local, LLVM's
        // function-attributor deduces fprintf(stdout) has a null pointer
        // argument → UB → entire body is dead → knucleotide prints nothing.
        writeln!(out, "@stdout = external dso_local global ptr").ok();
        writeln!(out, "declare i32 @fprintf(ptr, ptr, ...) #1").ok();
        writeln!(out, "declare i32 @printf(ptr, ...) #1").ok();
        writeln!(out, "declare i32 @fputc(i32, ptr) #1").ok();
        writeln!(out, "declare i32 @fflush(ptr) #1").ok();
        // 2026-07-15: atol used by getenv — kept, no conflict with defn wrappers
        writeln!(out, "declare i64 @atol(ptr) #1").ok();
        // 2026-07-15: getenv — used by emit_get_env (GetEnv# intrinsic)
        writeln!(out, "declare ptr @getenv(ptr) #1").ok();
        // 2026-07-15: Async dispatch runtime functions
        writeln!(out, "declare void @__wait_for_trigger__() #1").ok();
        // 2026-07-15: Removed conflicting POSIX declares (getuid, sched_yield,
        // nanosleep, exit, etc.) — replaced by Brief defn wrappers using SysCall#.

        // Emit external global declarations for linked triggers (fixes bug 4B)
        for (name, trg) in &self.ctx.triggers {
            if let crate::ast::LinkRef::Linked(sym) = &trg.address {
                let store_ty = trg_llvm_storage_ty(&trg.ty);
                let align = if store_ty == "i64" { 8 } else if store_ty == "i32" { 4 } else { 1 };
                writeln!(out, "@{} = external global {}, align {}", sym, store_ty, align).ok();
                // Warn if a linked trigger symbol is also declared as a frgn function
                if self.ctx.frgn_map.contains_key(sym.as_str()) {
                    eprintln!("warning: '{}' is declared as a frgn function but used as a @ link trigger. \
                               Use a volatile C variable for triggers, or built-in sources like @stdin#.", sym);
                }
                // Warn on unsupported trigger types
                match &trg.ty {
                    Type::Custom(__t) if __t == "Bool" || __t == "Int" || __t == "UInt" || __t == "Char" || __t == "String" || __t == "Data" => {}
                    _ => {
                        eprintln!("warning:{}:{}: trigger '{}' has type {:?} which the LLVM runtime does not fully support; using i8 storage",
                            trg.span.as_ref().map(|s| s.line).unwrap_or(0),
                            trg.span.as_ref().map(|s| s.column).unwrap_or(0),
                            name, trg.ty);
                    }
                }
            }
        }
        if self.ctx.triggers.iter().any(|(_, t)| matches!(t.address, crate::ast::LinkRef::Linked(_))) {
            writeln!(out).ok();
        }

        // Emit constant globals for TopLevel::Constant declarations.
        // Deduplicate identical constants to avoid redundant cache lines.
        // LLVM `@alias` maps multiple names to the same global without
        // allocating separate storage.
        let mut dedup_map: HashMap<String, String> = HashMap::new(); // key → canonical_name
        let mut alias_map: HashMap<String, String> = HashMap::new(); // name → canonical_name
        for (name, (ty, expr)) in &self.ctx.constants {
            let llvm_ty = match ty {
                // 2026-06-29: Updated for fixed-width types
                Type::Custom(__t) if __t == "Float64" => "double",
                Type::Custom(__t) if __t == "Float" => "float",
                Type::Custom(__t) if __t == "Int" || __t == "UInt" => "i64",
                Type::Custom(__t) if __t == "Int8" || __t == "UInt8" => "i8",
                Type::Custom(__t) if __t == "Int16" || __t == "UInt16" => "i16",
                Type::Custom(__t) if __t == "Int32" || __t == "UInt32" => "i32",
                Type::Custom(__t) if __t == "Bool" => "i1",
                _ => "i64",
            };
            let key = match expr {
                Expr::Float(f) => format!("{}:{}", llvm_ty, float_to_llvm_str(*f, llvm_ty)),
                Expr::Decimal(n) => format!("{}:{}", llvm_ty, n),
                Expr::Bool(b) => format!("{}:{}", llvm_ty, if *b { "true" } else { "false" }),
                Expr::UnaryOp(crate::ast::UnaryOpKind::Neg, inner) => match inner.as_ref() {
                    Expr::Float(f) => format!("{}:{}", llvm_ty, float_to_llvm_str(-*f, llvm_ty)),
                    Expr::Decimal(n) => format!("{}:-{}", llvm_ty, n),
                    _ => format!("{}:neg:{}", llvm_ty, name),
                },
                Expr::Quoted(_) => format!("{}:null", llvm_ty),
                _ => format!("{}:unresolved:{}", llvm_ty, name),
            };
            if let Some(canonical) = dedup_map.get(&key) {
                alias_map.insert(name.clone(), canonical.clone());
            } else {
                dedup_map.insert(key, name.clone());
                alias_map.insert(name.clone(), name.clone());
            }
        }
        // Emit declaration for canonical names only; emit alias for duplicates
        for (name, (ty, expr)) in &self.ctx.constants {
            let canonical = alias_map.get(name).cloned().unwrap_or_else(|| name.clone());
            if canonical != *name {
                let llvm_ty = match ty {
                    Type::Custom(__t) if __t == "Float64" => "double",
                    Type::Custom(__t) if __t == "Float" => "float",
                    Type::Custom(__t) if __t == "Int" || __t == "UInt" => "i64",
                    Type::Custom(__t) if __t == "Int8" || __t == "UInt8" => "i8",
                    Type::Custom(__t) if __t == "Int16" || __t == "UInt16" => "i16",
                    Type::Custom(__t) if __t == "Int32" || __t == "UInt32" => "i32",
                    Type::Custom(__t) if __t == "Bool" => "i1",
                    _ => "i64",
                };
                writeln!(out, "@{} = alias {}, {}* @{}", name, llvm_ty, llvm_ty, canonical).ok();
                continue;
            }
            let llvm_ty = match ty {
                Type::Custom(__t) if __t == "Float64" => "double",
                Type::Custom(__t) if __t == "Float" => "float",
                Type::Custom(__t) if __t == "Int" || __t == "UInt" => "i64",
                Type::Custom(__t) if __t == "Int8" || __t == "UInt8" => "i8",
                Type::Custom(__t) if __t == "Int16" || __t == "UInt16" => "i16",
                Type::Custom(__t) if __t == "Int32" || __t == "UInt32" => "i32",
                Type::Custom(__t) if __t == "Bool" => "i1",
                _ => "i64",
            };
            let val_str = match expr {
                Expr::Float(f) => float_to_llvm_str(*f, llvm_ty),
                Expr::Decimal(n) => n.to_string(),
                Expr::Bool(b) => (if *b { "true" } else { "false" }).to_string(),
                Expr::UnaryOp(crate::ast::UnaryOpKind::Neg, inner) => match inner.as_ref() {
                    Expr::Float(f) => float_to_llvm_str(-*f, llvm_ty),
                    Expr::Decimal(n) => format!("-{}", n),
                    _ => if *ty == Type::float() { "0.0".to_string() } else { "0".to_string() },
                },
                Expr::Quoted(_) => "null".to_string(),
                _ => {
                    if *ty == Type::float() {
                        "0.0".to_string()
                    } else {
                        "0".to_string()
                    }
                },
            };
            writeln!(out, "@{} = constant {} {}", name, llvm_ty, val_str).ok();
        }
        if !self.ctx.constants.is_empty() { writeln!(out).ok(); }

        self.declare_state_type(&mut out);
        // %State no longer has a module-level global. Instead, main()
        // allocates it on the stack as an alloca and passes it to all
        // internal functions as a noalias nocapture parameter. This
        // guarantees SROA promotes all fields to scalar registers.
        writeln!(out, "; %State is allocated on the stack in main() as %state = alloca %State").ok();
        writeln!(out).ok();

        // Emit string constants as global Brief headers
        // Each is a 2-slot header: { data_ptr (ptrtoint of slot 2), length, [chars] }
        // This makes ALL string values in the IR uniform — same format as heap-allocated strings.
        for (si, s) in self.ctx.string_constants.iter().enumerate() {
            let escaped = escape_llvm_string(s);
            let len = s.len();
            writeln!(out, "@str.{} = private unnamed_addr constant <{{ i64, i64, [{} x i8] }}> <{{", si, len + 1).ok();
            writeln!(out, "  i64 ptrtoint (i8* getelementptr inbounds (<{{ i64, i64, [{} x i8] }}>, <{{ i64, i64, [{} x i8] }}>* @str.{}, i64 0, i32 2) to i64),", len + 1, len + 1, si).ok();
            writeln!(out, "  i64 {},", len).ok();
            writeln!(out, "  [{} x i8] c\"{}\\00\"", len + 1, escaped).ok();
            writeln!(out, "}}>, align 8").ok();
        }
        if !self.ctx.string_constants.is_empty() { writeln!(out).ok(); }

        // 2026-06-29: Global sentinel for all empty list literals `[]`.
        // LLVM eliminates stack-allocated empty lists (dead alloca elimination)
        // because ptrtoint/inttoptr round-trip is invisible to SROA. A single
        // rodata constant { data_ptr=0, length=0 } handles all [] instances
        // with zero runtime cost and zero allocation. See docs/plans/2026-06-29-list-allocation-fix.md.
        writeln!(out, "@ll_empty_list = private unnamed_addr constant {{ i64, i64 }} {{ i64 0, i64 0 }}").ok();
        writeln!(out).ok();

        // Run SLP hazard analysis before emitting function definitions and attributes.
        // This populates slp_hazard_fns so that slp_attr() returns the correct attribute
        // group (#4/#5) for hazardous functions, and the attributes section emits #4/#5.
        self.estimate_slp_hazard(&txns);

        let mut range_meta: Vec<String> = Vec::new();

        // Definitions
        for item in items {
            if let TopLevel::Definition(d) = item {
                self.emit_definition(&mut out, d);
                writeln!(out).ok();
            }
        }
        // User-defined inop# intrinsics
        for item in items {
            if let TopLevel::Inop(inop) = item {
                self.emit_inop(&mut out, inop);
                writeln!(out).ok();
            }
        }
        // Transactions
        for (name, txn) in &txns {
            self.emit_transaction(&mut out, txn, name, &mut range_meta);
            writeln!(out).ok();
        }
        // Precondition functions (skip callable txns — no ptr)
        for (name, txn) in &txns {
            let has_output = txn.output_type.is_some() || !txn.outputs.is_empty();
            if !txn.is_reactive && (!txn.parameters.is_empty() || has_output) { continue; }
            self.emit_pre_function(&mut out, txn, name);
        }
        // Async body functions — simple pre→fire wrapper for worker threads
        for (name, txn) in &txns {
            if self.async_txn_names.iter().any(|n| n.as_str() == name.as_str()) && !self.is_lightweight_async {
                self.emit_async_body(&mut out, txn, name);
                writeln!(out).ok();
            }
        }
        // Fused transactions
        let fusable = self.resolve_fusable_pairs(&txns);
        for (a, b) in &fusable {
            if let (Some(ta), Some(tb)) = (
                txns.iter().find(|(n, _)| n == a).map(|(_, t)| t),
                txns.iter().find(|(n, _)| n == b).map(|(_, t)| t),
            ) {
                self.emit_fused(&mut out, ta, tb, &format!("{}_{}_fused", a, b));
                writeln!(out).ok();
            }
        }
        // Composed chain functions
        // Extract all-internal counter info before taking composed_chains.
        //
        // ── Composed chain extraction ─────────────────────────────────
        //
        // Chains of reactive transactions that are all-internal (no FFI, no
        // external triggers) have their final counter values stored directly
        // as O(1) stores inside enum dispatch case arms. Non-all-internal
        // chains emit a fused composed function.
        let all_internal_counter: HashMap<String, (usize, i64)> = analysis.region_analyzer.composed_chains
            .iter()
            .filter(|cc| cc.all_internal)
            .filter_map(|cc| {
                let cv = cc.counter_var.as_ref()?;
                let ci = *self.ctx.field_index_map.get(cv)?;
                let bound = analysis.region_analyzer.iteration_bound_of(&cc.chain[0])? as i64;
                let base = format!("{}_fused_txn", cc.chain.join("_"));
                let fn_name = if let Some(ref tv) = cc.trigger_values {
                    let suffix: String = tv.iter().map(|(_, v)| v.to_string()).collect::<Vec<_>>().join("_");
                    format!("{}_trg_{}", base, suffix)
                } else { base };
                Some((fn_name, (ci, bound)))
            })
            .collect();

        let composed_chains = std::mem::take(&mut analysis.region_analyzer.composed_chains);
        let mut composed_fn_map: HashMap<String, String> = HashMap::new();
        let mut composed_by_trig: HashMap<String, Vec<(i64, String)>> = HashMap::new();
        for cc in &composed_chains {
            let base = format!("{}_fused_txn", cc.chain.join("_"));
            let fused_name = if let Some(ref tv) = cc.trigger_values {
                let variant_suffix: String = tv.iter()
                    .map(|(_, v)| v.to_string())
                    .collect::<Vec<_>>().join("_");
                format!("{}_trg_{}", base, variant_suffix)
            } else {
                base.clone()
            };
            composed_fn_map.insert(cc.chain[0].clone(), fused_name.clone());
            if let Some(ref tv) = cc.trigger_values {
                for (_, val) in tv {
                    composed_by_trig.entry(cc.chain[0].clone()).or_default().push((*val, fused_name.clone()));
                }
            }
            // Skip emitting fused function for all-internal chains — the
            // per-case arms in emit_enum_main will store the final counter
            // value directly instead of calling the folded loop.
            if !cc.all_internal {
                self.emit_fused_composed(&mut out, &cc.composed_body, &fused_name);
                writeln!(out).ok();
            }
        }
        // Init
        self.fun.txn_counter = 0;
        self.fun.within_counter = 0;
        self.emit_init_state(&mut out);
        self.emit_persistent_cell_ticks(&mut out);
        writeln!(out).ok();

        // Emit cell channel globals and persistent cell thread functions
        let persistent_cells: Vec<crate::ast::CellDef> = self.ctx.cell_defs.values()
            .filter(|c| c.is_persistent)
            .cloned()
            .collect();
        for cell in &persistent_cells {
            if self.cell_thread_names.contains(&cell.name) {
                self.emit_cell_channel_globals(&mut out, cell);
            }
        }
        for cell in &persistent_cells {
            if self.cell_thread_names.contains(&cell.name) {
                self.emit_cell_thread(&mut out, cell);
            }
        }
        writeln!(out).ok();

        // Reactor — sequential or parallel
        // Enumeration and wake trigger detection were computed above
        // in the auto-categorization step (lines ~312-380).

        // Reactor tick — use folded path when a single bounded-counter txn
        // with no triggers can be collapsed into a canonical while loop.
        let graph = &analysis.transition_graph;

        // Natural death: auto-exit for wake-triggered programs where ALL reactive
        // transactions have proven bounded convergence (bounded_pre + increments).
        // When every reactive txn is foldable, the program is guaranteed to converge
        // and can safely exit without an explicit #!exit pragma.
        //
        // We build a synthetic exit condition: for each foldable bounded-counter txn,
        // check `counter >= bound`. When ALL counters reach their bounds, no txn can
        // fire again, and main() returns 0.
        self.ctx.has_natural_exit = false;
        // 2026-07-18: Build synthetic exit condition for ALL programs, not just
        // wake-triggered ones. One-shot txns (no triggers, no async) should also
        // auto-exit when all bounded counters reach their bounds.
        if self.ctx.exit_condition.is_none() {
            let has_persistent_txn = txns.iter().any(|(name, t)| {
                t.is_reactive && !graph.nodes.iter()
                    .filter(|n| n.name == *name)
                    .any(|n| n.bounded_pre.is_some() && n.increments.is_some())
            });
            if !has_persistent_txn {
                let mut checks: Vec<Expr> = Vec::new();
                for (name, t) in &txns {
                    if !t.is_reactive { continue; }
                    if let Some(node) = graph.nodes.iter().find(|n| n.name == *name) {
                        if let Some(ref bp) = node.bounded_pre {
                            if let Some(ref inc) = node.increments {
                                if bp.var == inc.var {
                                    checks.push(Expr::BinaryOp(
                                        crate::ast::BinaryOpKind::Ge,
                                        Box::new(Expr::Identifier(bp.var.clone())),
                                        Box::new(Expr::Identifier(bp.bound_var.clone())),
                                    ));
                                }
                            }
                        }
                    }
                }
                if !checks.is_empty() {
                    let combined = checks.into_iter()
                        .reduce(|a, b| Expr::BinaryOp(crate::ast::BinaryOpKind::And, Box::new(a), Box::new(b)))
                        .unwrap();
                    self.ctx.exit_condition = Some(Box::new(combined));
                    self.ctx.has_natural_exit = true;
                }
            }
        }

        // ── Loop emission strategy selection ──────────────────────
        //
        // This is the core decision tree that maps a reactive transaction's
        // structure to the optimal LLVM IR emission strategy:
        //
        //   Single txn + bounded counter:
        //     → check = pure counter fold (EmitPerFieldPhi), folded SSA (EmitInlineSsa), or
        //        folded memory (EmitMemoryCounter)
        //   All-const inputs:
        //     → precompute (EmitPureCounterFold) — no runtime loop
        //   Multi-txn all-pure:
        //     → multi-txn pure fold (O(1) per counter)
        //   Sequential bounded multi-txn:
        //     → SSA register pipeline (EmitSequentialSsa)
        //   Enumerable triggers:
        //     → switch-dispatch per-key folded loops
        //   Reactive with triggers:
        //     → reactor tick loop (sequential or parallel)
        //
        // The decision is driven by the transition graph (analysis), not by
        // runtime profiling data. Contracts provide the bound/liveness info.

        // 2026-07-10: Count distinct fields written by the txn via &field = value.
        // EmitPerFieldPhi per-field phi loop is optimal for 1-4 fields. Beyond that, the
        // GEP+load+store per tick overhead exceeds the phi register benefit,
        // and EmitSequentialSsa (direct SSA loop with full phi state) produces better code.
        let active_writes: usize = txns.first().map_or(0, |(_, txn)| {
            let mut seen = std::collections::HashSet::new();
            for stmt in &txn.body {
                if let Statement::Assign(lhs, _) = stmt {
                    if let Some(name) = lhs.as_var_name() {
                        seen.insert(name.to_string());
                    }
                }
            }
            seen.len()
        });
        let foldable = graph.nodes.len() == 1
            && !graph.has_triggers
            && txns.len() == 1
            && graph.nodes[0].bounded_pre.is_some()
            && graph.nodes[0].increments.is_some()
            && graph.nodes[0].is_reactive;
        let folded = if foldable {
            let node = &graph.nodes[0];
            let bp = node.bounded_pre.as_ref().unwrap();
            let inc = node.increments.as_ref().unwrap();
            if bp.var == inc.var {
                if let Some(&counter_idx) = self.ctx.field_index_map.get(&bp.var) {
                    let total_idx = self.ctx.field_index_map.get(&bp.bound_var).copied();
                    let total_const_name: Option<&str> = if total_idx.is_none() {
                        if self.ctx.constants.contains_key(&bp.bound_var) {
                            Some(bp.bound_var.as_str())
                        } else { None }
                    } else { None };
                    if total_idx.is_some() || total_const_name.is_some() || bp.bound_literal.is_some() {
                        // 2026-07-03: Swan song check — terminating guards with FFI
                        // (e.g. term! -> print_int#) are hoisted by hoist_terminating_guard.
                        // If a swan song exists, the body is treated as non-pure.
                        let has_swan_song = txns[0].1.body.iter().any(|s| {
                            match s {
                                Statement::Term(Some(_)) | Statement::TermBang(Some(_)) => true,
                Statement::Term(None) | Statement::TermBang(None) => false,
                                Statement::Guarded(Expr::Bool(true), statements) => {
                                    statements.iter().any(|gs| matches!(gs, Statement::TermBang(Some(_))))
                                }
                                _ => false,
                            }
                        });
                        let raw_body = &txns[0].1.body;
                        let (body_stmts, post_hoist) = hoist_terminating_guard(raw_body, &self.ctx.field_index_map);
                        // ── Dispatch: pure counter vs per-field phi loop ───────
                        //
                        // For bodies proven pure (or effectively pure), the compiler
                        // can emit an O(1) counter-only fold or a runtime phi pipeline.
                        // For ALL other bodies, emit a per-field phi loop (EmitPerFieldPhi).
                        //
                        // Why per-field phis instead of the old EmitInlineSsa/EmitMemoryCounter paths:
                        //   EmitInlineSsa (inline SSA) used a %slot_case alloca round-trip:
                        //     load %State → extractvalue×N → insertvalue×N → store
                        //     — 33-field struct load/store per iteration hid fields
                        //       from LLVM's induction variable analysis.
                        //   EmitMemoryCounter (memory) kept the counter in %State via GEP+load+store:
                        //     — 3 extra memory uops per tick for the counter alone.
                        //
                        //   EmitPerFieldPhi creates per-field phi nodes at the loop header
                        //   so LLVM sees a canonical loop structure (phi + icmp slt
                        //   + add) that enables induction variable analysis, SROA,
                        //   and loop vectorization. Guard branches in the body are
                        //   handled naturally: every path stores to the same GEP
                        //   addresses, and the latch reloads from them (GVN eliminates
                        //   the redundant load-via-store round trip).
                        //
                        // 2026-07-04: Removed memory-loop variant. EmitPerFieldPhi (per-field
                        // phi loop) is now used for ALL field counts. The original
                        // concern was that 31+ phi nodes would choke SROA, but chunk
                        // allocas (≤15 fields per chunk, MAX_FIELDS_PER_ALLLOCA=15)
                        // decompose the state into SROA-friendly chunks. With Path A
                        // (needs_state_stores_in_body=false), EmitPerFieldPhi emits zero memory
                        // traffic regardless of field count — strictly better than
                        // Removed memory-loop variant's GEP+load+store per iteration.
                        if !has_swan_song && (node.is_pure_body || node.is_effectively_pure) {
                            let total_val = self.ctx.field_initializers
                                .get(&bp.bound_var)
                                .and_then(|e| e.as_ref())
                                .and_then(|e| {
                                    if let Expr::Decimal(n) = e { Some(*n) } else { None }
                                })
                                .or_else(|| {
                                    self.ctx.constants.get(&bp.bound_var).and_then(|(_, e)| {
                                        if let Expr::Decimal(n) = e { Some(*n) } else { None }
                                    })
                                });
                            if let Some(tv) = total_val {
                                // EmitPureCounterFold: pure counter fold (O(1) — single store, no loop)
                                // 2026-07-14: Wrap in define i32 @main() so emitted IR is valid
                                self.warnings.push(format!("info: txn '{}' dispatched via pure counter fold ({} iterations, O(1) store)", node.name, tv));
                                writeln!(out, "define i32 @main() local_unnamed_addr #9 {{").ok();
                                writeln!(out, "entry:").ok();
                                writeln!(out, "  %state = alloca %State, align 8").ok();
                                self.emit_inline_init_stores(&mut out, "%state");
                                self.emit_folded_pure_counter(&mut out, counter_idx, tv);
                                if self.ctx.exit_condition.is_some() {
                                    self.emit_exit_check(&mut out);
                                    writeln!(out, ".end:").ok();
                                }
                                writeln!(out, "  ret i32 0").ok();
                                writeln!(out, "}}").ok();
                                true
                        } else {
                            // Adaptive dispatch: EmitInlineSsa (inline SSA) vs EmitPerFieldPhi (per-field phi).
                            // 2026-07-05: EmitInlineSsa is selected for dense-write, small-field
                            // bodies — the single %State phi + insertvalue chain lets
                            // LLVM optimize the entire state as one SSA unit.  Guards are
                            // handled via phi merge (emit_stmt.rs:983-992).  EmitPerFieldPhi is
                            // selected for sparse-write, large-field bodies — per-field
                            // phis avoid the long insertvalue chain.
                            // 2026-07-05: When the body has FFI calls (print_int#, etc.),
                            // use EmitPerFieldPhi instead of EmitInlineSsa.  EmitInlineSsa's insertvalue chain makes
                            // it easier for LLVM's ipsccp/globalopt to prove the loop is
                            // pure (by analyzing @stdout as null/undef), which causes
                            // LLVM to eliminate the entire loop including all fprintf
                            // calls — producing empty output (knucleotide, fasta bug).
                            let total_fields = self.ctx.field_index_map.len();
                            let write_count = node.write_set.len();
                            let write_density = if total_fields > 0 { write_count as f64 / total_fields as f64 } else { 1.0 };
                            let has_body_ffi = raw_body.iter().any(|s| {
                                crate::analysis::transition_graph::statement_contains_ffi(s)
                            });
                            if write_density >= 0.5 && total_fields < 8 && !has_body_ffi {
                                // EmitInlineSsa: inline SSA with insertvalue chain.
                                // Best for dense writes (knucleotide: 4 fields all written,
                                // mandelbrot: 5 fields all written).
                                self.fun.pending_post_hoist = post_hoist;
                                self.warnings.push(format!("info: txn '{}' dispatched via inline SSA (EmitInlineSsa, {}/{} fields written)", &node.name, write_count, total_fields));
                                self.emit_folded_main(&mut out, &node.name, counter_idx, total_idx, total_const_name, false, Some(&body_stmts));
                                true
                            } else {
                                // EmitPerFieldPhi: per-field phi loop with Path A + dead-field
                                // elimination + commit block.
                                // 2026-07-10: Cap write_set to avoid register spilling.
                                // Too many phi registers (>=8) causes LLVM to spill to
                                // stack, which is slower than GEP+load+store for the
                                // non-tracked fields. Priority: counter, bound, vec groups.
                                let mut capped_set: HashSet<String> = HashSet::new();
                                capped_set.insert(bp.var.clone());
                                if let Some(ref tv) = total_idx {
                                    if let Some(name) = self.ctx.field_index_map.iter()
                                        .find(|&(_, v)| *v == *tv).map(|(k, _)| k.clone())
                                    {
                                        capped_set.insert(name);
                                    }
                                }
                                for f in &node.write_set {
                                    if capped_set.len() >= 6 { break; }
                                    capped_set.insert(f.clone());
                                }
                                self.fun.pending_post_hoist = post_hoist;
                                let num_fields = capped_set.len().max(2);
                                self.warnings.push(format!("info: txn '{}' dispatched via per-field phi loop (EmitPerFieldPhi, {} fields)", &node.name, num_fields));
                                let is_decreasing = bp.direction == crate::analysis::transition_graph::ConvergeDirection::Decreasing;
                                self.emit_countable_main(&mut out, &node.name, counter_idx, total_idx, total_const_name, &body_stmts, &capped_set, is_decreasing);
                                true
                            }
                            }
                        } else {
                            // Adaptive dispatch for non-pure bodies: EmitInlineSsa vs EmitPerFieldPhi.
                            // 2026-07-05: Same criteria as pure path — dense writes,
                            // small fields favors EmitInlineSsa insertvalue chain.  The SSA mode
                            // guard handler (emit_stmt.rs:983-992) handles guards with
                            // phi merge, so guards don't disqualify EmitInlineSsa.
                            // 2026-07-05: has_body_ffi check — when the body has FFI
                            // calls, use EmitPerFieldPhi to prevent LLVM from eliminating the
                            // loop+fprintf chain via globalopt (knucleotide bug).
                            let total_fields = self.ctx.field_index_map.len();
                            let write_count = node.write_set.len();
                            let write_density = if total_fields > 0 { write_count as f64 / total_fields as f64 } else { 1.0 };
                            let has_body_ffi = raw_body.iter().any(|s| {
                                crate::analysis::transition_graph::statement_contains_ffi(s)
                            });
                            if write_density >= 0.5 && total_fields < 8 && !has_body_ffi {
                                self.fun.pending_post_hoist = post_hoist;
                                self.warnings.push(format!("info: txn '{}' dispatched via inline SSA (EmitInlineSsa, {}/{} fields written)", &node.name, write_count, total_fields));
                                self.emit_folded_main(&mut out, &node.name, counter_idx, total_idx, total_const_name, false, Some(&body_stmts));
                                true
                            } else {
                                let mut capped_set: HashSet<String> = HashSet::new();
                                capped_set.insert(bp.var.clone());
                                if let Some(ref tv) = total_idx {
                                    if let Some(name) = self.ctx.field_index_map.iter()
                                        .find(|&(_, v)| *v == *tv).map(|(k, _)| k.clone())
                                    {
                                        capped_set.insert(name);
                                    }
                                }
                                for f in &node.write_set {
                                    if capped_set.len() >= 6 { break; }
                                    capped_set.insert(f.clone());
                                }
                                self.fun.pending_post_hoist = post_hoist;
                                let num_fields = capped_set.len().max(2);
                                self.warnings.push(format!("info: txn '{}' dispatched via per-field phi loop (EmitPerFieldPhi, {} fields)", &node.name, num_fields));
                                let is_decreasing = bp.direction == crate::analysis::transition_graph::ConvergeDirection::Decreasing;
                                self.emit_countable_main(&mut out, &node.name, counter_idx, total_idx, total_const_name, &body_stmts, &capped_set, is_decreasing);
                                true
                            }
                        }
                    } else { false }
                } else { false }
            } else { false }
        } else { false };

        // Emit the trg step() function if the program has trigger declarations.
        // The step() function recomputes dependent variables in topological order
        // when trigger inputs change. It is called from the event loop.
        if !self.ctx.trigger_names.is_empty() {
            let trg_names = self.ctx.trigger_names.clone();
            self.emit_trg_step(&mut out, &analysis.dependency_graph, &trg_names);
        }

        if !folded {
            let precomputed = if let Some(ref final_values) = precomputed_final_values {
                // EmitPureCounterFold: fully precomputed — no runtime loop emitted
                self.warnings.push("info: program fully precomputed — no runtime loop emitted. If this is unexpected, increase --optimize-budget or add frgn calls for observability.".into());
                self.emit_precomputed_main(&mut out, final_values);
                true
            } else { false };

            if !precomputed {
                // A004: warn when a runtime loop has zero observability
                if !txns.is_empty() {
                    let any_has_ffi = txns.iter().any(|(_, t)|
                        t.body.iter().any(|s| crate::analysis::transition_graph::statement_contains_ffi(s)));
                    if !any_has_ffi {
                        self.warnings.push(
                            "warning: emitted runtime loop has no observable side effects — \
                             LLVM may eliminate it entirely. Add frgn calls for output, \
                             or this program may run without producing results.".into());
                    }
                }
                // Multi-txn all-pure folding: when NO triggers exist and ALL
                // reactive async txns have bounded_pre + increments with pure
                // bodies, fold them into a single register-pipeline main loop.
                let multi_foldable = enumerable.is_none()
                    && !has_wake_triggers
                    && !self.async_txn_names.is_empty()
                    && self.async_txn_names.iter().all(|name| {
                        graph.nodes.iter().find(|n| n.name == *name).map_or(false, |node| {
                            (node.is_pure_body || node.is_effectively_pure)
                            && node.bounded_pre.is_some()
                            && node.increments.is_some()
                        })
                    });
                let mut multi_fold_params: HashMap<String, FoldParam> = HashMap::new();
                if multi_foldable {
                    for txn_name in &self.async_txn_names {
                        if let Some(node) = graph.nodes.iter().find(|n| n.name == *txn_name) {
                            if let Some(ref bp) = node.bounded_pre {
                                if let Some(&cidx) = self.ctx.field_index_map.get(&bp.var) {
                                    let tidx = self.ctx.field_index_map.get(&bp.bound_var).copied();
                                    let tcname = if tidx.is_none() {
                                        if self.ctx.constants.contains_key(&bp.bound_var) {
                                            Some(bp.bound_var.clone())
                                        } else { None }
                                    } else { None };
                                    multi_fold_params.insert(txn_name.clone(), FoldParam {
                                        counter_idx: cidx,
                                        bound_field_idx: tidx,
                                        bound_const_name: tcname,
                                        is_decreasing: bp.direction == crate::analysis::transition_graph::ConvergeDirection::Decreasing,
                                        bound_literal: bp.bound_literal,
                                    });
                                }
                            }
                        }
                    }
                }
                if !multi_fold_params.is_empty() {
                    // EmitAdaptive: multi-txn pure fold
                    let txn_list: Vec<&str> = multi_fold_params.keys().map(|s| s.as_str()).collect();
                    self.warnings.push(format!("info: txns [{}] dispatched via multi-txn pure fold (all-internal, async)", txn_list.join(", ")));
                    self.emit_folded_multi_main(&mut out, &txns, &[], &HashMap::new(), &multi_fold_params,
                        &HashMap::new(), 0, None, None, None, None, None, false);
                    self.emit_thread_pool_metadata(&mut out);
                } else if dispatch_mode == DispatchMode::Sequential && !txns.is_empty()
                    && enumerable.is_none() && !has_wake_triggers
                {
                    // EmitAdaptive: SSA register pipeline (or modulo-switch dispatch)
                    // 2026-07-09: Removed bounded_pre + increments requirement —
                    // emit_ssa_main correctly handles txns without bounded_pre via
                    // emit_ssa_txn_with_precond (per-tick pre check + any_fired).
                    // The EmitAdaptive fold path is independently guarded by multi_fold_params.
                    // The EmitSequentialSsa fallback at line 2632 handles the same codegen path,
                    // so entering here vs EmitSequentialSsa produces identical IR for non-foldable
                    // programs. This change allows mixed bounded/unbounded reactive txns
                    // to share the SSA pipeline instead of falling through to EmitSequentialSsa.
                    self.emit_ssa_main(&mut out, &txns, false);
                } else if let Some(ref enum_sizes) = enumerable {
                // Enumerable triggers — emit switch-dispatch main
                // This path handles triggers with small compile-time-known value sets.
                // We emit a single @main that samples triggers once, then switch
                // dispatches to per-value folded loops.

                // Build per-txn folding params for all enum-candidate txns.
                // Each trigger-gated bounded-counter txn gets its own folded
                // loop in the case arm.  Multi-txn programs (e.g. async_counters)
                // need this to converge in O(1) ticks instead of one increment
                // per tick via reactor_tick.
                let enum_fold_params: HashMap<String, FoldParam> = {
                    let mut m = HashMap::new();
                    for txn_name in &enum_txn_names {
                        if let Some(node) = graph.nodes.iter().find(|n| n.name == *txn_name) {
                            if let Some(ref bp) = node.bounded_pre {
                                if let Some(&cidx) = self.ctx.field_index_map.get(&bp.var) {
                                    let tidx = self.ctx.field_index_map.get(&bp.bound_var).copied();
                                    let tcname = if tidx.is_none() {
                                        if self.ctx.constants.contains_key(&bp.bound_var) {
                                            Some(bp.bound_var.clone())
                                        } else { None }
                                    } else { None };
                                    let inc = node.increments.as_ref();
                                    if inc.map_or(false, |i| i.var == bp.var && i.delta > 0) {
                                        m.insert(txn_name.clone(), FoldParam {
                                            counter_idx: cidx,
                                            bound_field_idx: tidx,
                                            bound_const_name: tcname,
                                            is_decreasing: bp.direction == crate::analysis::transition_graph::ConvergeDirection::Decreasing,
                                            bound_literal: bp.bound_literal,
                                        });
                }
            }
        }
    }
                    }
                    m
                };
                // Companion map: pure-body flag + total value for each foldable txn.
                // When a txn is pure and its bound is a compile-time constant, the
                // case arm can store the total directly instead of looping 50M times.
                let enum_fold_pure: HashMap<String, (bool, Option<i64>)> = {
                    let mut m = HashMap::new();
                    for txn_name in &enum_txn_names {
                        if let Some(node) = graph.nodes.iter().find(|n| n.name == *txn_name) {
                            let total_val = node.bounded_pre.as_ref().and_then(|bp| {
                                self.ctx.field_initializers.get(&bp.bound_var)
                                    .and_then(|e| e.as_ref())
                                    .and_then(|e| if let Expr::Decimal(n) = e { Some(*n) } else { None })
                                    .or_else(|| {
                                        self.ctx.constants.get(&bp.bound_var).and_then(|(_, e)| {
                                            if let Expr::Decimal(n) = e { Some(*n) } else { None }
                                        })
                                    })
                            });
                            m.insert(txn_name.clone(), (node.is_pure_body, total_val));
                        }
                    }
                    m
                };
                // Legacy single-txn params for chain composition (unchanged)
                let (enum_ci, enum_ti, enum_tcn): (usize, Option<usize>, Option<String>) = if graph.nodes.len() == 1 && txns.len() == 1 {
                    if let Some(bp) = graph.nodes[0].bounded_pre.as_ref() {
                        if let Some(&cidx) = self.ctx.field_index_map.get(&bp.var) {
                            let tidx = self.ctx.field_index_map.get(&bp.bound_var).copied();
                            let tcname = if tidx.is_none() {
                                if self.ctx.constants.contains_key(&bp.bound_var) {
                                    Some(bp.bound_var.clone())
                                } else { None }
                            } else { None };
                            (cidx, tidx, tcname)
                        } else { (0, None, None) }
                    } else { (0, None, None) }
                } else { (0, None, None) };

                // Emit the reactor_tick function (needed for residual fallback path)
                match dispatch_mode {
                    DispatchMode::Parallel => {
                        self.build_write_masks(items);
                        self.emit_parallel_reactor(&mut out, &txns, &fusable);
                    }
                    DispatchMode::Sequential => {
                        self.emit_reactor(&mut out, &txns, &fusable);
                    }
                }

                // Emit enum main with switch dispatch
                let enum_tcn_ref = enum_tcn.as_deref();
                let composed_fn = txns.first()
                    .and_then(|(n, _)| composed_fn_map.get(n))
                    .map(|s| s.as_str());
                let composed_trig_ref: Option<&HashMap<String, Vec<(i64, String)>>> = if !composed_by_trig.is_empty() {
                    Some(&composed_by_trig)
                } else { None };
                let all_int_ref: Option<&HashMap<String, (usize, i64)>> = if all_internal_counter.is_empty() {
                    None
                } else {
                    Some(&all_internal_counter)
                };
                // Declare __rt_wait for wake-triggered programs.
                // trg owns its runtime — the compiler emits the declare implicitly.
                // EmitAdaptive: enum dispatch
                self.warnings.push(format!("info: program dispatched via enum trigger dispatch ({} trigger keys)", enum_keys.len()));
                if has_wake_triggers {
                    writeln!(out, "declare void @__rt_wait() local_unnamed_addr").ok();
                }
                self.emit_folded_multi_main(
                    &mut out,
                    &txns,
                    enum_sizes,
                    &enum_keys,
                    &enum_fold_params,
                    &enum_fold_pure,
                    enum_ci,
                    enum_ti,
                    enum_tcn_ref,
                    composed_fn,
                    composed_trig_ref,
                    all_int_ref,
                    has_wake_triggers,
                );

                if has_wake_triggers {
                    self.emit_wake_metadata(&mut out);
                }
                self.emit_thread_pool_metadata(&mut out);
            } else if self.ctx.library_mode {
                self.emit_library_shim(&mut out, &txns);
            } else if !txns.is_empty()
                && self.async_txn_names.is_empty()
                && self.ctx.mmio_fields.is_empty()
            {
                // EmitSequentialSsa: Direct phi-based loop — no async, no MMIO.
                // Inline all txn bodies directly in main() instead of reactor_tick.
                // Triggers are sampled inline via lazy emit_trg_load, wake path uses
                // __rt_wait between ticks. LLVM promotes %State fields to phi nodes.
                if has_wake_triggers {
                    writeln!(out, "declare void @__rt_wait() local_unnamed_addr").ok();
                }
                self.warnings.push(
                    "info: program dispatched via direct SSA loop".into()
                );
                self.fun.txn_counter = 0;
                self.fun.within_counter = 0;
                self.emit_ssa_main(&mut out, &txns, has_wake_triggers);
            } else if !txns.is_empty() {
                // reactor loop fallback — only reached for async dispatch or MMIO
                // (all other programs go through EmitSequentialSsa direct SSA loop above)
                self.warnings.push(format!("info: program dispatched via reactor loop ({})", match dispatch_mode {
                    DispatchMode::Parallel => "parallel thread pool",
                    DispatchMode::Sequential => "sequential tick loop",
                }));
                match dispatch_mode {
                    DispatchMode::Parallel => {
                        self.build_write_masks(items);
                        self.emit_parallel_reactor(&mut out, &txns, &fusable);
                    }
                    DispatchMode::Sequential => {
                        self.emit_reactor(&mut out, &txns, &fusable);
                    }
                }
                // Main
                if has_wake_triggers {
                    writeln!(out, "declare void @__rt_wait() local_unnamed_addr").ok();
                }
                self.fun.txn_counter = 0;
                self.fun.within_counter = 0;
                self.emit_main(&mut out, has_wake_triggers);
                // Wake trigger metadata
                if has_wake_triggers {
                    self.emit_wake_metadata(&mut out);
                }
                self.emit_thread_pool_metadata(&mut out);
            } else {
        writeln!(out, "define void @reactor_tick(ptr noalias nocapture %state) local_unnamed_addr #2 {{").ok();
                writeln!(out, "  entry:").ok();
                writeln!(out, "  ret void").ok();
                writeln!(out, "}}").ok();
                writeln!(out).ok();
                // Main
                self.fun.txn_counter = 0;
                self.fun.within_counter = 0;
                self.emit_main(&mut out, false);
            }
            }
        }

        // ── DEAD-FIELD INFO DIAGNOSTICS (A002/A003) ─────────
        if !self.ctx.dead_info_disabled {
            for node in &graph.nodes {
                let dead_fields: Vec<&String> = node.write_set.iter()
                    .filter(|f| !graph.live_fields.contains(*f))
                    .collect();

                if !dead_fields.is_empty() {
                    let dead_list: Vec<String> = dead_fields.iter()
                        .map(|f| format!("'{}'", f))
                        .collect();
                    if node.is_effectively_pure {
                        // Folded txn — these stores are genuinely eliminated
                        self.warnings.push(format!(
                            "info: field(s) {} written by txn '{}' are never read — stores eliminated\n\
                              note: not referenced by any precondition or #!exit condition",
                            dead_list.join(", "),
                            node.name,
                        ));
                    } else {
                        // Non-folded txn — stores still execute but value is wasted
                        self.warnings.push(format!(
                            "info: field(s) {} written by txn '{}' are never read — wasted work\n\
                              note: not referenced by any precondition or #!exit; values computed but have no effect",
                            dead_list.join(", "),
                            node.name,
                        ));
                    }
                }

                // A003: pure-counter fold info
                if node.is_effectively_pure {
                    let inc = node.increments.as_ref().unwrap();
                    let bp = node.bounded_pre.as_ref().unwrap();
                    let total_str = self.ctx.constants.get(&bp.bound_var)
                        .and_then(|(_, e)| if let Expr::Decimal(n) = e { Some(n.to_string()) } else { None })
                        .or_else(|| self.ctx.field_initializers.get(&bp.bound_var)
                            .and_then(|e| e.as_ref())
                            .and_then(|e| if let Expr::Decimal(n) = e { Some(n.to_string()) } else { None }));
                    let iterations_msg = match &total_str {
                        Some(s) => format!(" — {} iterations replaced by single store", s),
                        None => String::new(),
                    };
                    let dead_list: Vec<String> = dead_fields.iter()
                        .map(|f| format!("'{}'", f))
                        .collect();
                    let mut msg = format!(
                        "info: txn '{}' folded to O(1){}",
                        node.name, iterations_msg,
                    );
                    msg.push_str(&format!(
                        "\n  info: counter '{}' retains its store (the only live write)",
                        inc.var,
                    ));
                    if !dead_list.is_empty() {
                        msg.push_str(&format!("\n  info: dead fields: {}", dead_list.join(", ")));
                    }
                    self.warnings.push(msg);
                }
            }
        }

        // ── EXIT CONDITION DIAGNOSTICS ───────────────────────

        // Warning: #!exit on a one-shot program that never checks it.
        // Folded and precomputed paths exit without a tick loop; enum dispatch without
        // wake has no exit_check label. The standard reactor path (emit_main) always
        // checks, so we warn only when we know the check is unreachable.
        if self.ctx.exit_condition.is_some() {
            let is_one_shot = folded
                || precomputed_final_values.is_some()
                || (enumerable.is_some() && !has_wake_triggers);
            if is_one_shot {
                self.warnings.push(format!(
                    "warning: #!exit declared but program has no tick loop\n\
                      note: the exit condition will never be checked\n\
                      help: add an @link trigger to make the program reactive, or remove #!exit"
                ));
            }
        }

        // Warning: wake-triggered program without any exit path.
        // Without #!exit or natural death, the program will idle forever after all
        // reactive transactions converge. Natural death (auto-exit) is automatically
        // applied when all reactive txns have bounded convergence, so this warning
        // only fires for programs with persistent (non-foldable) reactive txns.
        if has_wake_triggers && self.ctx.exit_condition.is_none() {
            self.warnings.push(format!(
                "warning: program has wake triggers but no exit path\n\
                  note: after all transactions converge, the program will spin forever\n\
                  help: add `#!exit <condition>;` at the top of the file"
            ));
        }

        // Attributes
        if !self.fun.pending_metadata.is_empty() {
            writeln!(out, "; Loop metadata").ok();
            out.push_str(&self.fun.pending_metadata);
        }
        writeln!(out).ok();
        writeln!(out, "attributes #0 = {{").ok();
        writeln!(out, "    mustprogress nofree norecurse nosync nounwind memory(readwrite)").ok();
        writeln!(out, "}}").ok();
        writeln!(out, "attributes #1 = {{ nocallback nofree nosync nounwind willreturn memory(readwrite) }}").ok();
        writeln!(out, "attributes #2 = {{ mustprogress nofree norecurse nosync nounwind memory(readwrite) }}").ok();
        writeln!(out, "attributes #3 = {{ nofree norecurse nosync nounwind memory(readwrite) }}").ok();
        // SLP-safe attribute variants: #4 = #0 + disable-slp, #5 = #3 + disable-slp.
        // Dual attributes (disable-slp-vectorize + no-vectorize-slp) ensure LLVM
        // compatibility across versions 15–22+. Emitted only when needed.
        if !self.ctx.slp_hazard_fns.is_empty() {
            writeln!(out, "attributes #4 = {{").ok();
            writeln!(out, "    mustprogress nofree norecurse nosync nounwind memory(readwrite)").ok();
            writeln!(out, "    \"disable-slp-vectorize\"=\"true\" \"no-vectorize-slp\"=\"true\"").ok();
            writeln!(out, "}}").ok();
            writeln!(out, "attributes #5 = {{").ok();
            writeln!(out, "    nofree norecurse nosync nounwind memory(readwrite)").ok();
            writeln!(out, "    \"disable-slp-vectorize\"=\"true\" \"no-vectorize-slp\"=\"true\"").ok();
            writeln!(out, "}}").ok();
        }
        writeln!(out, "attributes #6 = {{ nounwind }}").ok();
        // 2026-07-04: #7 = readonly for @pre_* functions.
        // Precondition expressions never write to %State — they only read
        // state fields via GEP+load. readonly tells LLVM the function has
        // no memory writes, enabling CSE of redundant pre_ calls and load
        // hoisting past precondition checks.
        // Other paths: #0 for definitions and callable txns (they may read
        // and write through %state), #2 for reactor_tick (always writes
        // the state copy), #3 for @main (writes through reactor tick loop).
        // 2026-07-04: #7 = memory(read) for @pre_* functions.
        // LLVM 18+ uses lowercase access kinds: memory(read) not memory(readonly).
        writeln!(out, "attributes #7 = {{").ok();
        writeln!(out, "    mustprogress nofree norecurse nosync nounwind willreturn memory(read)").ok();
        writeln!(out, "}}").ok();
        // 2026-07-04: #8 = argmemonly variant of #0 for definitions/callable txns.
        // Brief functions only access memory through pointer arguments (%state).
        // No globals, no heap (beyond arena which is stack-allocated), no trigger
        // globals for defns. argmemonly lets LLVM promote allocas and eliminate
        // redundant loads across call boundaries.
        // Not used for reactive txns (they may read @link trigger globals).
        writeln!(out, "attributes #8 = {{").ok();
        writeln!(out, "    mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite)").ok();
        writeln!(out, "}}").ok();
        // 2026-07-04: #9 = argmem:readwrite variant of #3 for @main.
        // 2026-07-05: #9 = memory(readwrite) for main functions.
        // Main accesses @stdout (a global, not argmem) through fprintf
        // calls. Using memory(readwrite) prevents LLVM from eliminating
        // side-effectful I/O calls during opt's ipsccp/globalopt passes.
        // This attribute is deliberately HIGH-numbered (#9) to avoid
        // collision with clang-generated bitcode attributes (#0-#8)
        // during LTO merging (llvm-link renumbers but keeps #9).
        writeln!(out, "attributes #9 = {{").ok();
        writeln!(out, "    nofree norecurse nosync nounwind memory(readwrite)").ok();
        writeln!(out, "}}").ok();
        // 2026-07-04: #10 = argmem:read + willreturn for @pre_* functions.
        // Precondition functions only read state through %state and never
        // access trigger globals. Combines the benefits of #7 (memory(read))
        // and #8 (memory(argmem: readwrite)).
        writeln!(out, "attributes #10 = {{").ok();
        writeln!(out, "    mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: read)").ok();
        writeln!(out, "}}").ok();
        // Range metadata
        if !range_meta.is_empty() {
            writeln!(out).ok();
            for m in &range_meta {
                writeln!(out, "{}", m).ok();
            }
        }

        // TBAA metadata tree for type-based alias analysis
        // 2026-06-29: Dynamic generation from TypeUniverse. Each unique
        // tbaa_node group name gets its own TBAA node. "Int" is always
        // first (index 1) since it's the fallback for unmatched types.
        writeln!(out).ok();
        writeln!(out, "!0 = !{{!\"Brief\"}}").ok();
        if let Some(ref universe) = self.ctx.type_universe {
            let mut groups: Vec<String> = universe.types.keys().cloned().collect();
            groups.sort();
            if let Some(pos) = groups.iter().position(|g| g == "Int") {
                groups.swap(0, pos);
            }
            for (i, group) in groups.iter().enumerate() {
                writeln!(out, "!{} = !{{!\"{}\", !0}}", i + 1, group).ok();
            }
        } else {
            // Fallback: hardcoded nodes for built-in types
            writeln!(out, "!1 = !{{!\"Int\", !0}}").ok();
            writeln!(out, "!2 = !{{!\"Bool\", !0}}").ok();
            writeln!(out, "!3 = !{{!\"Char\", !0}}").ok();
            writeln!(out, "!4 = !{{!\"String\", !0}}").ok();
                writeln!(out, "!5 = !{{!\"Float\", !0}}").ok();
        }
        // 2026-07-04: StateAliasScope — a distinct !{} node used by !noalias
        // metadata on Ptr<T> volatile load/store instructions.  This scope
        // represents "accesses to %State memory."  By annotating volatile
        // accesses through Ptr<T> with !noalias !{!StateScope}, we tell LLVM
        // that the Ptr<T> access does NOT alias with any %State field access.
        // The scope ID is fixed at 99 to avoid conflicts with TBAA (!0..!N),
        // range metadata (!{i}), and function-scoped metadata (starting at 100).
        self.ctx.state_alias_scope_md = 99;
        writeln!(out, "!99 = distinct !{{}} ; StateAliasScope").ok();

        // Build optimization report if requested
        if self.ctx.optimize_report {
            self.report_lines.push("=== Optimization Report ===".to_string());
            self.report_lines.push(format!("Optimize budget: {}", self.ctx.optimize_budget));
            let enum_count = enumerable.as_ref().map(|e| e.len()).unwrap_or(0);
            self.report_lines.push(format!("Triggers found: {} (enumerable: {})", self.ctx.trigger_names.len(), enum_count));
            if let Some(ref sizes) = enumerable {
                let mut total_combos: u64 = 1;
                self.report_lines.push("".to_string());
                self.report_lines.push("Trigger variable value sets:".to_string());
                for (tn, sz) in sizes {
                    let s = sz.unwrap_or(0);
                    self.report_lines.push(format!("  {}: {} values", tn, s));
                    total_combos = total_combos.saturating_mul(s);
                }
                self.report_lines.push(format!("Total combinations: {}", total_combos));
                if total_combos <= self.ctx.optimize_budget {
                    self.report_lines.push(format!("  ✅ Within budget ({} ≤ {})", total_combos, self.ctx.optimize_budget));
                    self.report_lines.push("  → Switch-dispatch enumeration enabled".to_string());
                } else {
                    self.report_lines.push(format!("  ❌ Exceeds budget ({} > {})", total_combos, self.ctx.optimize_budget));
                    self.report_lines.push("  → Standard reactor path used".to_string());
                }

                // Size estimation when --optimize-size is set
                if let Some(byte_limit) = self.ctx.optimize_size {
                    self.report_lines.push("".to_string());
                    self.report_lines.push("Size estimation:".to_string());
                    let bytes_per_combo: u64 = 80; // approximate bytes per switch case + folded loop
                    let base_estimate: u64 = 5000;
                    let enum_estimate = base_estimate + total_combos.saturating_mul(bytes_per_combo);
                    self.report_lines.push(format!("  Base binary (standard reactor): ~{} KB", base_estimate / 1024));
                    self.report_lines.push(format!("  With {} enumerated combos: ~{} KB", total_combos, enum_estimate / 1024));
                    if enum_estimate <= byte_limit {
                        self.report_lines.push(format!("  ✅ Enumeration fits within {} KB limit", byte_limit / 1024));
                        self.report_lines.push(format!("  Recommended budget: {}", total_combos));
                    } else {
                        self.report_lines.push(format!("  ❌ Enumeration exceeds {} KB limit", byte_limit / 1024));
                        let max_fit: u64 = if bytes_per_combo > 0 {
                            (byte_limit.saturating_sub(base_estimate)) / bytes_per_combo
                        } else { 0 };
                        self.report_lines.push(format!("  Max combos within limit: {}", max_fit));
                        self.report_lines.push(format!("  Recommended budget: {} (partial enumeration)", max_fit));
                    }
                }
            }
            if let Some(ref graph) = Some(&analysis.transition_graph) {
                self.report_lines.push("".to_string());
                self.report_lines.push("Transaction graph:".to_string());
                self.report_lines.push(format!("  Nodes: {}", graph.nodes.len()));
                self.report_lines.push(format!("  Has triggers: {}", graph.has_triggers));
                if let Some(node) = graph.nodes.first() {
                    if let Some(ref bp) = node.bounded_pre {
                        self.report_lines.push(format!("  Bounded pre: {} < {}", bp.var, bp.bound_var));
                    }
                    self.report_lines.push(format!("  Is reactive: {}", node.is_reactive));
                    self.report_lines.push(format!("  Pure body: {}", node.is_pure_body));
                    if let Some(ref inc) = node.increments {
                        self.report_lines.push(format!("  Increment: {} += {}", inc.var, inc.delta));
                    }
                }
            }
            let chains = &analysis.region_analyzer.linear_chains;
            if !chains.is_empty() {
                self.report_lines.push("".to_string());
                self.report_lines.push("Linear transaction chains detected:".to_string());
                for (i, chain) in chains.iter().enumerate() {
                    let chain_str: String = chain.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" -> ");
                    self.report_lines.push(format!("  Chain {}: {}", i + 1, chain_str));
                }
            }
            let scores = &analysis.region_analyzer.region_scores;
            if !scores.is_empty() {
                self.report_lines.push("".to_string());
                self.report_lines.push("Optimization priority ranking:".to_string());
                for (rank, score) in scores.iter().enumerate() {
                    let class_str = format!("{:?}", score.complexity);
                    let gpu = if score.gpu_eligible { " GPU" } else { "" };
                    let chain_tag = if score.chain_composed { " Chain" } else { "" };
                    let score_str = if score.optimization_score.is_infinite() || score.optimization_score <= 0.0 {
                        "—".to_string()
                    } else {
                        format!("{:.1}", score.optimization_score)
                    };
                    let vs_size = match score.value_set_size {
                        Some(n) => format!("{}", n),
                        None => "∞".to_string(),
                    };
                    let txn_list = score.txn_names.join(",");
                    self.report_lines.push(format!(
                        "  #{:<3} R{:<4} {:<20} {:<9} {:<7} {:<8} {:<8} {:<8} {}{}",
                        rank + 1, score.region_id, txn_list, class_str,
                        score.body_weight, score.iteration_count, vs_size, score_str,
                        chain_tag, gpu
                    ));
                }
            }
            if let Some(ref plan) = analysis.region_analyzer.budget_plan {
                self.report_lines.push("".to_string());
                self.report_lines.push(format!("Budget plan (budget={}):", plan.total_budget));
                let spent: u64 = plan.total_budget - plan.residual_budget;
                self.report_lines.push(format!("  Allocated: {} regions, spent {}/{}", plan.allocated.len(), spent, plan.total_budget));
                for (rid, cls, cost, score) in &plan.allocated {
                    self.report_lines.push(format!("    R{}: {:?}, cost={}, score={:.1}", rid, cls, cost, score));
                }
                self.report_lines.push(format!("  Residual: {} budget units", plan.residual_budget));
                if !plan.skipped.is_empty() {
                    self.report_lines.push(format!("  Skipped: {} regions (unbounded or exceeds budget)", plan.skipped.len()));
                    for (rid, cls, _) in &plan.skipped {
                        self.report_lines.push(format!("    R{}: {:?}", rid, cls));
                    }
                }
            }
            if !composed_chains.is_empty() {
                self.report_lines.push("".to_string());
                self.report_lines.push("Composed chains:".to_string());
                for cc in &composed_chains {
                    let chain_str: String = cc.chain.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" → ");
                    let triggers = if cc.root_triggers.is_empty() {
                        "none".to_string()
                    } else {
                        cc.root_triggers.join(", ")
                    };
                    let tv_str = match &cc.trigger_values {
                        Some(tv) => tv.iter().map(|(n, v)| format!("{}={}", n, v)).collect::<Vec<_>>().join(", "),
                        None => if cc.root_triggers.is_empty() { "—".to_string() } else { "unbranched".to_string() },
                    };
                    let internal = if cc.all_internal { " all-internal" } else { "" };
                    self.report_lines.push(format!(
                        "  {} (link vars: {}, triggers: {}, values: [{}], fused weight: {}{})",
                        chain_str, cc.link_vars.join(","), triggers, tv_str, cc.fused_weight, internal
                    ));
                }
            }
            if precomputed_final_values.is_some() {
                self.report_lines.push("".to_string());
                self.report_lines.push("Precomputed (compile-time evaluation):".to_string());
                self.report_lines.push("  All state values determined at compile time — O(1) runtime.".to_string());
                if let Some(ref fv) = precomputed_final_values {
                    self.report_lines.push(format!("  {} chains precomputed.", fv.len()));
                    for (chain, bindings) in fv {
                        let chain_str: String = chain.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" → ");
                        let vars: Vec<String> = bindings.iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect();
                        self.report_lines.push(format!("    {} → final state: [{}]", chain_str, vars.join(", ")));
                    }
                }
            }
        }

        // Append any compiled SPIR-V kernel blobs to the output for embedding.
        if self.ctx.gpu_offload || !self.spirv_blobs.is_empty() {
            out.push_str(&self.emit_spirv_embeds());
        }

        // Phase 4: --layout diagnostic flag — print field layout after generation
        if self.ctx.dump_layout {
            eprintln!("{}", self.dump_layout_str());
        }

        // 2026-06-29: %tddup post-processing pass removed — all register
        // allocation now goes through FunctionContext::next_reg() which
        // guarantees unique %t{N} names by construction. The old pass
        // redefined duplicate %t{N} registers as %tddup{N} but did NOT
        // rename subsequent uses, creating SSA violations.
        // See docs/plans/2026-06-29-llvm-backend-refactoring.md#p2.

        out
    }

    /// Return the optimization report lines collected during `generate()`.
    pub fn report(&self) -> &[String] {
        &self.report_lines
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Return any extra flags needed for `opt`. Currently emits
    /// `-slp-vectorize-hor=false` when SLP hazards exist, since LLVM 18's
    /// per-function `"disable-slp-vectorize"` attribute is not always respected
    /// by the new pass manager. This is a safeguard: the per-function attribute
    /// works on LLVM 15-17 and 22+; the global flag covers LLVM 18-21.
    pub fn llvm_extra_flags(&self) -> Vec<String> {
        let mut flags = Vec::new();
        if !self.ctx.slp_hazard_fns.is_empty() {
            flags.push("-slp-vectorize-hor=false".to_string());
        }
        flags
    }

    /// Select the LLVM attribute group for a function, adjusting for SLP hazard.
    /// Non-hazardous functions use #0 or #3; hazardous functions use #4 or #5
    /// (which disable SLP vectorization). See hazard.rs for the analysis.
    /// Check if an expression produces a `Ptr<T>` value.
    /// Used by `ListIndex` to decide between direct pointer GEP vs 2-slot header load.
    /// Resolve a Brief type to its underlying LLVM type for BILD purposes.
    /// Walks through type aliases and melds to find the concrete primitive.
    pub(crate) fn resolve_bild_type(&self, ty: &Type) -> Type {
        match ty {
            Type::Custom(name) => {
                // Check type universe for type definition (alias like `type UserId = Int`)
                if let Some(tu) = &self.ctx.type_universe {
                    if let Some(resolved) = tu.types.get(name) {
                        if let Some(primitive) = primitive_from_name(&resolved.base) {
                            return primitive;
                        }
                    }
                    // Check melds: if this type has a meld with a primitive partner,
                    // resolve to the primitive (e.g. `meld Meters(f: Float)` → Float)
                    for ((a, b), _) in &tu.melds {
                        let partner = if a == name { Some(b) } else if b == name { Some(a) } else { None };
                        if let Some(pname) = partner {
                            if let Some(primitive) = primitive_from_name(pname) {
                                return primitive;
                            }
                        }
                    }
                }
                ty.clone()
            }
            _ => ty.clone(),
        }
    }

    fn is_ptr_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Field(_, target) => target == "Ptr",
            Expr::Identifier(name) => {
                self.fun.let_binding_types.get(name)
                    .map(|t| matches!(t, Type::Applied(n, _) if n == "Ptr")
                        || matches!(t, Type::LayoutPtr(_)))
                    .unwrap_or(false)
            }
            _ => false,
        }
    }

    fn validate_schema_types(&mut self) {
        if self.ctx.schema_aliases.is_empty() {
            return;
        }
        for (name, schema_type) in &self.ctx.schema_aliases.clone() {
            let brief_type = match schema_type.to_brief_type_name() {
                Ok(t) => t,
                Err(e) => {
                    self.warnings.push(format!(
                        "warning: schema type incompatibility for '{}': {}. Field will NOT be treated as MMIO.",
                        name, e
                    ));
                    continue;
                }
            };
            let name_clone = name.clone();
            for item_name in self.ctx.field_index_map.keys().chain(self.ctx.mmio_initializers.keys()) {
                if item_name == &name_clone {
                    if brief_type == "Int" {
                        if brief_type == "Int" && schema_type.is_unsigned_int() {
                            self.warnings.push(format!(
                                "warning: schema declares '{}' as unsigned but Brief uses Int. 64-bit target makes this safe.",
                                name
                            ));
                        }
                    }
                    break;
                }
            }
        }
    }

    // ── Field index ───────────────────────────────────────────
    fn build_field_index(&mut self, items: &[TopLevel]) {
        self.ctx.field_index_map.clear();
        self.ctx.field_types.clear();
        self.ctx.field_initializers.clear();
        if !self.ctx.mmio_prepopulated {
            self.ctx.mmio_fields.clear();
            self.ctx.mmio_initializers.clear();
        }
        for item in items {
            if let TopLevel::StateDecl(s) = item {
                // 2026-07-14: Preserve prepopulated MMIO addresses from with_mmio_addresses()
                let addr = if self.ctx.mmio_prepopulated && self.ctx.mmio_fields.contains_key(&s.name) {
                    *self.ctx.mmio_fields.get(&s.name).unwrap()
                } else {
                    0u64
                };
                self.ctx.mmio_fields.insert(s.name.clone(), addr);
                self.ctx.mmio_initializers.insert(s.name.clone(), None);
                if self.ctx.mmio_prepopulated && self.ctx.mmio_fields.contains_key(&s.name) {
                    if self.ctx.schema_aliases.is_empty() || self.ctx.schema_aliases.contains_key(&s.name) {
                        self.ctx.mmio_initializers.insert(s.name.clone(), None);
                    } else {
                        // Not in any imported schema — remove from mmio_fields to prevent
                        // accidental MMIO routing in reads/writes.
                        self.ctx.mmio_fields.remove(&s.name);
                        self.ctx.field_index_map
                            .insert(s.name.clone(), self.ctx.field_types.len());
                        self.push_field_type(&s.ty);
                        self.ctx.field_initializers.insert(s.name.clone(), None);
                    }
                } else {
                    self.ctx.field_index_map
                        .insert(s.name.clone(), self.ctx.field_types.len());
                    self.push_field_type(&s.ty);
                    self.ctx.field_initializers.insert(s.name.clone(), None);
                    if let Some(tu) = &self.ctx.type_universe {
                        let type_name = match &s.ty {
                            crate::ast::Type::Custom(n) => n.as_str(),
                            crate::ast::Type::Applied(n, _) => n.as_str(),
                            _ => "",
                        };
                        // 2026-07-18: The parser stores InsertAt/ExtractFrom as
                        // "op.InsertAt" / "op.ExtractFrom" with PropertyValue::Identifier.
                        // Check both Identifier and String for backward compat.
                        if tu.get(type_name).and_then(|rt| rt.properties.get("op.InsertAt"))
                            .map_or(false, |strat| {
                                *strat == crate::ast::PropertyValue::Identifier("ring_push".to_string())
                                || *strat == crate::ast::PropertyValue::String("ring_push".to_string())
                            })
                        {
                            let data_idx = self.ctx.field_types.len();
                            self.ctx.field_index_map.insert(format!("{}_data", s.name), data_idx);
                            self.ctx.field_types.push("i64".to_string());
                            self.ctx.field_brief_types.push(Type::int());
                            self.ctx.field_initializers.insert(format!("{}_data", s.name), None);
                            let head_idx = self.ctx.field_types.len();
                            self.ctx.field_index_map.insert(format!("{}_head", s.name), head_idx);
                            self.ctx.field_types.push("i64".to_string());
                            self.ctx.field_brief_types.push(Type::int());
                            self.ctx.field_initializers.insert(format!("{}_head", s.name), None);
                            let tail_idx = self.ctx.field_types.len();
                            self.ctx.field_index_map.insert(format!("{}_tail", s.name), tail_idx);
                            self.ctx.field_types.push("i64".to_string());
                            self.ctx.field_brief_types.push(Type::int());
                            self.ctx.field_initializers.insert(format!("{}_tail", s.name), None);
                            let mask_idx = self.ctx.field_types.len();
                            self.ctx.field_index_map.insert(format!("{}_mask", s.name), mask_idx);
                            self.ctx.field_types.push("i64".to_string());
                            self.ctx.field_brief_types.push(Type::int());
                            self.ctx.field_initializers.insert(format!("{}_mask", s.name), None);
                            self.ctx.ringbuf_inline.insert(s.name.clone(),
                                crate::backend::llvm::context::RingbufInlineFields {
                                    data_idx, head_idx, tail_idx, mask_idx,
                                });
                        }
                    }
                }
            } else if let TopLevel::Trigger(trg) = item {
                // 2026-07-14: Trigger.ty removed - use string as default type.
                self.ctx.field_index_map
                    .insert(trg.name.clone(), self.ctx.field_types.len());
                self.push_field_type(&Type::string());
                self.ctx.field_initializers.insert(trg.name.clone(), None);
            } else if let TopLevel::TriggerBinding { name, ty, .. } = item {
                // Trigger bindings (trg name: Type @ Console!) get a state slot
                // like regular triggers, so emit_expr can load their value.
                let trig_ty = ty.clone().unwrap_or(crate::ast::Type::string());
                self.ctx.field_index_map
                    .insert(name.clone(), self.ctx.field_types.len());
                self.push_field_type(&trig_ty);
                self.ctx.field_initializers.insert(name.clone(), None);
            // 2026-07-14: Top-level `let name: Type = expr;` — register as state field.
            // The initializer expr is stored in field_initializers so emit_init_state
            // can evaluate and store the runtime value at startup.
            } else if let TopLevel::Statement(stmt) = item {
                if let crate::ast::Statement::Let { name, ty, expr, .. } = stmt.as_ref() {
                    let field_ty = ty.clone().unwrap_or(crate::ast::Type::int());
                    self.ctx.field_index_map
                        .insert(name.clone(), self.ctx.field_types.len());
                    self.push_field_type(&field_ty);
                    self.ctx.field_initializers.insert(name.clone(), expr.clone());
                }
            } else if let TopLevel::Cell(c) = item {
                // Cell fields are handled differently depending on whether the
                // cell is persistent (threaded) or sync (non-threaded).
                //
                // For threaded cells: fields go into a separate %CellState.<name>
                // type so the thread function can operate on its own struct without
                // sharing %State. The %CellState.* definitions are built here and
                // emitted in declare_state_type.
                //
                // For non-threaded persistent cells (used by @cell_persistent_ticks):
                // fields stay in %State as prefixed slots, accessed via the same
                // GEP path as program state fields.
                //
                // Sync CellCall codegen always allocates %CellState.* locally and
                // accesses fields through the cell_state_types map.
                if c.is_persistent {
                    // Build cell-state type info for this persistent cell
                    let mut cs_imap: HashMap<String, usize> = HashMap::new();
                    let mut cs_tys: Vec<String> = Vec::new();
                    for field in &c.fields {
                        let prefixed = format!("cell${}${}", c.name, field.name);
                        cs_imap.insert(prefixed.clone(), cs_tys.len());
                        cs_tys.push(self.llvm_type(&field.ty).to_string());
                        // Also register in %State for cell_persistent_ticks access
                        self.ctx.field_index_map.insert(prefixed.clone(), self.ctx.field_types.len());
                        self.push_field_type(&field.ty);
                        self.ctx.field_initializers.insert(prefixed, None);
                    }
                    for (param_name, param_ty) in &c.parameters {
                        let prefixed = format!("cell${}${}", c.name, param_name);
                        cs_imap.insert(prefixed.clone(), cs_tys.len());
                        cs_tys.push(self.llvm_type(param_ty).to_string());
                        // Also register in %State
                        self.ctx.field_index_map.insert(prefixed.clone(), self.ctx.field_types.len());
                        self.push_field_type(param_ty);
                        self.ctx.field_initializers.insert(prefixed, None);
                    }
                    // Register internal trigger fields in %State and %CellState.*
                    // 2026-07-14: Trigger.ty removed - use string as default type.
                    for trg in &c.internal_triggers {
                        let prefixed = format!("cell${}${}", c.name, trg.name);
                        cs_imap.insert(prefixed.clone(), cs_tys.len());
                        cs_tys.push(self.llvm_type(&Type::string()).to_string());
                        self.ctx.field_index_map.insert(prefixed.clone(), self.ctx.field_types.len());
                        self.push_field_type(&Type::string());
                        self.ctx.field_initializers.insert(prefixed, None);
                    }
                    self.ctx.cell_state_types.insert(c.name.clone(), (cs_imap, cs_tys));
                    // 2026-07-14: reactor_speed removed from Transaction. Thread speed not available.
                    let has_thread_speed = false;
                    if has_thread_speed {
                        self.cell_thread_names.push(c.name.clone());
                    }
                } else {
                    // Non-persistent (sync) cell: add to %State as prefixed slots
                    for field in &c.fields {
                        let prefixed = format!("cell${}${}", c.name, field.name);
                        self.ctx.field_index_map.insert(prefixed.clone(), self.ctx.field_types.len());
                        self.push_field_type(&field.ty);
                        self.ctx.field_initializers.insert(prefixed, None);
                    }
                    for (param_name, param_ty) in &c.parameters {
                        let prefixed = format!("cell${}${}", c.name, param_name);
                        self.ctx.field_index_map.insert(prefixed.clone(), self.ctx.field_types.len());
                        self.push_field_type(param_ty);
                        self.ctx.field_initializers.insert(prefixed, None);
                    }
                }
            }
        }
    }

    // ── Chimera tracking (Phase 3) ──────────────────────────
    /// Check if a register holds a chimera value.
    pub(crate) fn is_chimera(&self, reg_name: &str) -> bool {
        self.fun.chimera_map.get(reg_name).map_or(false, |c| c.is_chimera)
    }

    /// Get the backing type of a chimera value, if any.
    pub(crate) fn chimera_backing(&self, reg_name: &str) -> Option<&str> {
        self.fun.chimera_map.get(reg_name)
            .filter(|c| c.is_chimera)
            .map(|c| c.backing_type.as_str())
    }

    /// Mark a register as a chimera value with the given backing type.
    pub(crate) fn mark_chimera(&mut self, reg_name: &str, backing_type: &str) {
        if let Some(c) = self.fun.chimera_map.get_mut(reg_name) {
            c.is_chimera = true;
            c.backing_type = backing_type.to_string();
        } else {
            self.fun.chimera_map.insert(reg_name.to_string(), ChimeraInfo { is_chimera: true, backing_type: backing_type.to_string() });
        }
    }

    /// Compute the total size of the %State struct in bytes from field_types.
    /// Used by the memcpy round-trip SROA optimization in emit_main.
    pub(crate) fn compute_state_size_bytes(&self) -> i64 {
        self.ctx.field_types.iter().map(|t| llvm_type_byte_size(t)).sum()
    }

    // ── Adaptive Layout — apply field modes ──────────────────
    /// Apply field liveness analysis to eliminate dead fields and add
    /// cache slots for lazily-computed projections.
    ///
    /// Why this runs after the transition graph is built: the transition
    /// graph's live_fields analysis can only be accurate after the full
    /// region analysis (which determines which txns fire under which
    /// conditions). Running dead-field elimination earlier would conservatively
    /// keep all fields, missing elimination opportunities.
    pub(crate) fn apply_field_modes(&mut self, items: &[TopLevel],
        live_fields: &std::collections::HashSet<String>,
        projection_usage: &std::collections::HashMap<String, std::collections::HashSet<String>>)
    {
        let all_state_fields: std::collections::HashSet<String> = self.ctx.field_index_map.keys().cloned().collect();
        let referenced_fields = crate::analysis::transition_graph::compute_referenced_fields(items);
        // Union with live_fields to prevent elimination of precondition-only,
        // postcondition-only, or exit-condition-only fields. live_fields includes
        // fields referenced in contracts and #!exit expressions.
        let referenced_fields: std::collections::HashSet<String> = referenced_fields.union(live_fields).cloned().collect();
        self.ctx.field_modes = crate::analysis::transition_graph::assign_field_modes(
            &all_state_fields, &referenced_fields, projection_usage);
        // Triggers must never be eliminated — they're accessed by the event loop,
        // not by txn body identifiers.
        for name in &self.ctx.trigger_names {
            self.ctx.field_modes.insert(name.clone(), crate::analysis::FieldMode::Always);
        }
        // Cell fields and parameters must never be eliminated — they're accessed
        // by Expr::CellCall codegen with cellular$name prefixed names.
        for name in self.ctx.field_index_map.keys() {
            if name.starts_with("cell$") {
                self.ctx.field_modes.insert(name.clone(), crate::analysis::FieldMode::Always);
            }
        }
        // Synthetic cycle_count field must never be eliminated — it's maintained
        // by the tick loop, not by txn body code.
        if let Some(idx) = self.ctx.field_index_map.get("cycle_count") {
            self.ctx.field_modes.insert("cycle_count".to_string(), crate::analysis::FieldMode::Always);
        }
        // 2026-07-02: RingBuffer inline fields must never be eliminated.
        // Even though they appear dead (not in exit condition), they're used
        // by the arrow dispatch (emit_arrow_push/discard) for inline RingBuffer
        // operations. Without them, LLVM can't SROA the RingBuf struct into
        // registers, and the inttoptr bottleneck remains.
        for (base, rbf) in &self.ctx.ringbuf_inline {
            for (suffix, idx) in &[("_data", rbf.data_idx), ("_head", rbf.head_idx),
                ("_tail", rbf.tail_idx), ("_mask", rbf.mask_idx)]
            {
                let name = format!("{}{}", base, suffix);
                self.ctx.field_modes.insert(name, crate::analysis::FieldMode::Always);
            }
        }
        self.ctx.cache_slots.clear();

        // Phase 1: Remove Never fields from field_index_map and field_types.
        // Rebuild both from scratch to handle index shifting correctly.
        // IMPORTANT: sort by original index to preserve deterministic field ordering.
        let mut old_pairs: Vec<(String, usize)> = self.ctx.field_index_map.drain().collect();
        let old_types = std::mem::take(&mut self.ctx.field_types);
        let old_brief_types = std::mem::take(&mut self.ctx.field_brief_types);
        self.ctx.field_index_map.reserve(old_pairs.len());
        self.ctx.field_types.reserve(old_types.len());
        self.ctx.field_brief_types.reserve(old_brief_types.len());
        old_pairs.sort_by_key(|(_, idx)| *idx);

        for (name, _old_idx) in &old_pairs {
            // 2026-07-16: Default to Always for fields without an explicit mode.
            // Fields referenced only in defn bodies (not txn bodies) don't get a
            // field_modes entry — changing Never→Always prevents their accidental
            // elimination while still allowing explicitly-marked fields to be pruned.
            let mode = self.ctx.field_modes.get(name).copied().unwrap_or(crate::analysis::FieldMode::Always);
            match mode {
                crate::analysis::FieldMode::Never => {
                    // Eliminate this field from %State entirely
                }
                crate::analysis::FieldMode::Always | crate::analysis::FieldMode::LazyCached { .. } => {
                    let new_idx = self.ctx.field_types.len();
                    let orig_type_idx = old_pairs.iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, i)| *i)
                        .unwrap_or(0);
                    self.ctx.field_index_map.insert(name.clone(), new_idx);
                    self.ctx.field_types.push(old_types[orig_type_idx].clone());
                    // 2026-06-29: Preserve the original Brief type alongside LLVM type
                    self.ctx.field_brief_types.push(
                        old_brief_types.get(orig_type_idx).cloned().unwrap_or(Type::int())
                    );
                }
            }
        }

        // Phase 2: Append cache slots — one per (field, projection_target) pair.
        for (name, mode) in &self.ctx.field_modes {
            if let crate::analysis::FieldMode::LazyCached { cache_index: _ } = mode {
                let targets = projection_usage.get(name);
                let mut target_map: HashMap<String, (usize, usize)> = HashMap::new();
                if let Some(targets) = targets {
                    for target_name in targets {
                        let cache_idx = self.ctx.field_types.len();
                        self.ctx.field_types.push("i64".to_string());
            self.ctx.field_brief_types.push(Type::int());
                        let valid_idx = self.ctx.field_types.len();
                        self.ctx.field_types.push("i8".to_string());
                        self.ctx.field_brief_types.push(Type::bool_());
                        target_map.insert(target_name.clone(), (cache_idx, valid_idx));
                    }
                }
                // Fallback: at least one cache slot even if projection_usage is empty
                if target_map.is_empty() {
                    let cache_idx = self.ctx.field_types.len();
                    self.ctx.field_types.push("i64".to_string());
            self.ctx.field_brief_types.push(Type::int());
                    let valid_idx = self.ctx.field_types.len();
                    self.ctx.field_types.push("i8".to_string());
                        self.ctx.field_brief_types.push(Type::bool_());
                    target_map.insert("_".to_string(), (cache_idx, valid_idx));
                }
                self.ctx.cache_slots.insert(name.clone(), target_map);
            }
        }
    }

    /// Scan the program for cell-to-cell wires from TrgBinding statements.
    fn scan_cell_wires(&mut self, items: &[TopLevel]) {
        for item in items {
            self.scan_item_for_wires(item);
        }
    }

    fn scan_top_level_body(&mut self, body: &[Statement]) {
        for stmt in body {
            self.scan_trg_stmt(stmt);
        }
    }

    fn scan_trg_stmt(&mut self, stmt: &Statement) {
        if let Statement::TrgBinding { instance, .. } = stmt {
            if let Expr::Call(callee, args, _) = instance {
                if !self.ctx.cell_defs.contains_key(callee) { return; }
                let cell = &self.ctx.cell_defs[callee];
                for (i, arg) in args.iter().enumerate() {
                    if let Expr::Field(inner, port_name) = arg {
                        if let Expr::Identifier(src_cell) = inner.as_ref() {
                            if self.ctx.cell_defs.contains_key(src_cell) {
                                if let Some(param_name) = cell.parameters.get(i) {
                                    self.ctx.cell_wires.push((
                                        src_cell.clone(),
                                        port_name.clone(),
                                        callee.clone(),
                                        param_name.0.clone(),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn scan_item_for_wires(&mut self, item: &TopLevel) {
        match item {
            TopLevel::Statement(stmt) => self.scan_trg_stmt(stmt.as_ref()),
            TopLevel::Transaction(t) => self.scan_top_level_body(&t.body),
            TopLevel::Definition(d) => self.scan_top_level_body(&d.body),
            TopLevel::Cell(c) => {
                for txn in &c.transactions {
                    self.scan_top_level_body(&txn.body);
                }
            }
            TopLevel::SyncGroup { item: inner, .. } => {
                self.scan_item_for_wires(inner);
            }
            _ => {}
        }
    }
}

