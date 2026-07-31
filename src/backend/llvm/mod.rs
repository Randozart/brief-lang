pub mod abi;
pub mod builder;
pub mod context;
pub mod directive;
pub mod dispatch;
pub mod emit_expr;
pub mod emit_stmt;
pub mod emit_toplevel;
pub mod gpu;
pub mod helpers;
pub mod intrinsics;
pub mod loop_engine;
pub mod normalizer;
pub mod types;
pub mod strategy;
pub mod vector_phi;

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

// ── Swan-song hoist: moved to frontend analysis ─────────────────────
//
// 2026-07-31: hoist_terminating_guard / remap_stmt_identifiers /
// remap_expr_into were removed from this file. The terminating-guard hoist
// and its let-to-field remap now live in src/analysis/swan_song.rs
// (hoist_swan_song), computed once per transaction in analyze_program and
// consumed here via analysis.swan_songs. See
// docs/plans/2026-07-31-frontend-driven-dispatch.md §6.
//
// Preserved rationale from the removed functions:
//   - 2026-07-05: The let-to-state-field mapping (`&field = let_name` →
//     map[let_name] = field_name) exists because a hoisted swan song may
//     reference a let binding (e.g. `nesc` in mandelbrot) whose register is
//     only valid inside the loop body; the value lives in a state field, so
//     identifiers are rewritten to the field name.
//   - 2026-07-04: The hoist fires even when the guard body is empty — the
//     guard may be just `term! -> print_int#(result)` with no preceding
//     statements. Hoisting it empties pending_post_hoist correctly, which
//     unblocks Path A (no-dead-stores) emission in emit_countable_main.

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
    /// 2026-07-18: Inline storage in parent struct (SSO/SVO for ≤64B).
    Inline,
    /// 2026-07-18: Circular buffer, overwrite-oldest. Free# is no-op.
    RingBuffer,
    /// 2026-07-18: Named strategy from config/alloc-strategies.toml.
    Config(String),
    /// 2026-07-18: User-provided Brief function as allocator.
    Custom(String),
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

// TypedRegister has no llvm() method — use LlvmBackend::llvm_type() instead.
// See emit_toplevel.rs:298 for the canonical type mapping.

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
        TopLevel::Export(e) => collect_strings_tl(&e.inner, seen, out),
        TopLevel::Cell(c) => {
            // 2026-07-13: Field.default removed in new AST.
            for _ in &c.fields { }
            for txn in &c.transactions { for s in &txn.body { collect_strings_stmt(s, seen, out); } }
            for d in &c.definitions { for s in &d.body { collect_strings_stmt(s, seen, out); } }
            for trg in &c.internal_triggers { collect_strings_expr(&Expr::Identifier(trg.name.clone()), seen, out); }
        }
        // 2026-07-26: TopLevel::Statement wraps top-level let bindings
        // that contain string literals (e.g. GetEnvInt!("BOUND")).
        // Without this arm, the string is never collected and @str.N is
        // referenced but undefined, causing clang to fail.
        TopLevel::Statement(stmt) => {
            collect_strings_stmt(stmt, seen, out);
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
        Statement::Gate(cond) => { collect_strings_expr(cond, seen, out); }
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
        Statement::InlineAsm { .. } | Statement::TrgBinding { .. } | Statement::MetadataAssignment(..) | Statement::InlineDefn(_) | Statement::InlineTxn(_) | Statement::Match { .. } => {}
    }
}

fn collect_strings_expr(expr: &Expr, seen: &mut std::collections::HashSet<String>, out: &mut Vec<String>) {
    match expr {
        Expr::Quoted(s) | Expr::TaggedQuotedLiteral(s, _) => {
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
        Expr::PluginIntercept { args, .. } => {
            for a in args { collect_strings_expr(a, seen, out); }
        }
        // Leaves — no sub-expressions
        Expr::Decimal(_) | Expr::TaggedLiteral(_, _) | Expr::Bool(_) | Expr::Float(_) | Expr::Identifier(_)
        | Expr::PropertyGet(_) | Expr::FormattingAnnotation(_) | Expr::StructLiteral { .. } => {}
        Expr::Exists(_) => { unreachable!("fn? only in stage eval") },
            Expr::Slice { array, start, end, stride } => {
                collect_strings_expr(array, seen, out);
                if let Some(e) = start.as_deref() { collect_strings_expr(e, seen, out); }
                if let Some(e) = end.as_deref() { collect_strings_expr(e, seen, out); }
                if let Some(e) = stride.as_deref() { collect_strings_expr(e, seen, out); }
            }

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
/// - User-provided `frgn __wait_for_event` + `node [true]` for sleep
/// - `@ link` triggers → `external global` + `load volatile`
///
/// Design philosophy: the backend is a single monolithic pass (not a pipeline
/// of small passes) because Brief's contract system provides structural
/// guarantees that LLVM cannot infer from generic IR. By emitting contract-
/// aware IR directly (TBAA, !range, noalias), we avoid the need for an
/// expensive LLVM analysis pass to rediscover what the contracts already state.

/// 2026-07-26: Protocol-driven LLVM type. Reads llvm_type from the
/// type's ResolvedType in the universe. No name matching — the primordial
/// llvm_type property is the single source of truth. Returns "i64" if the
/// universe is unavailable or the type is unknown (safe default).
/// Derive LLVM type string from a Type, using protocol membership + bytes.
/// 2026-07-30: No longer reads llvm_type from universe properties.
/// Protocol-based resolution is handled by CastingGraph::resolve_llvm_type()
/// (accessible via LlvmBackend::llvm_type() in codegen contexts).
/// Ptr<T> and Vector<T,N> are compiler constructs, not universe types.
pub fn protocol_llvm_type(ty: &Type, universe: Option<&crate::type_universe::TypeUniverse>) -> String {
    if matches!(ty, Type::Ptr(_) | Type::Vector(_, _)) {
        return "ptr".to_string();
    }
    // 2026-07-30: String/UTF8View use { i64, i64 } fat-pointer representation.
    // Check by name BEFORE the bytes-based fallback to avoid i128 (16 bytes
    // → i128) which changes FFI ABI and triggers clang 18.1.3 LICM crashes.
    if matches!(ty, Type::Custom(name) if name == "String" || name == "UTF8View") {
        return "{ i64, i64 }".to_string();
    }
    if let Some(ref u) = universe {
        if let Some(rt) = ty.universe_key().and_then(|k| u.get(k)) {
            // Check protocol membership first — float types get native float/double
            if rt.properties.contains_key("Cast.#Float") {
                return if rt.max_bits <= 32 { "float".to_string() }
                       else if rt.max_bits <= 64 { "double".to_string() }
                       else { "i64".to_string() };
            }
            // Use bytes as fallback for non-protocol types
            if rt.bytes > 0 {
                return format!("i{}", rt.bytes * 8);
            }
        }
    }
    "i64".to_string()
}
/// 2026-07-26: Storage type for trigger fields. Calls protocol_llvm_type
/// with the universe available during codegen.
pub(super) fn trg_llvm_storage_ty(ty: &Type, universe: Option<&crate::type_universe::TypeUniverse>) -> String {
    protocol_llvm_type(ty, universe)
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

    // 2026-07-21: Loop alignment — prevents DSB/MITE penalty from instructions
    // crossing 32-byte fetch window boundaries (ring_buffer mul instruction).
    let align_md = start + md_count;
    writeln!(pending_metadata, "!{0} = !{{!\"llvm.loop.align\", i32 32}}", align_md).ok();
    md_entries.push(format!("!{}", align_md));
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
    // 2026-07-31: Phase 3 (§8.3) — write masks are u128 so the 65th+ field is
    // no longer silently dropped; the EMITTED width is i128 when the program
    // has >64 state fields (write_mask_type), else i64.
    pub(crate) txn_write_masks: HashMap<String, u128>,
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

    // ── Arena Allocator (cross-function) ────────────────────
    // 2026-07-19: Arena system field indices in %State. Set during generate(),
    // used by emit_arena_init/emit_arena_alloc to access arena state.
    pub(crate) arena_ptr_idx: Option<usize>,
    pub(crate) arena_end_idx: Option<usize>,
    pub(crate) arena_base_idx: Option<usize>,

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

    // ── SVO List Optimization ────────────────────────────────
    // 2026-07-18: Small Vector Optimization — List<T> becomes a
    // multi-slot struct with inline storage for ≤N elements (N from
    // svo <~ metadata). Tag bit 0 distinguishes inline vs heap.
    pub feature_svo: bool,

    // ── Frgn Dispatch Resolution ──────────────────────────────
    // 2026-07-22: Pre-resolved frgn dispatch strategies computed
    // during the main compilation pass. The backend uses these to
    // decide whether to inline a foreign call or emit a bridge call.
    pub(crate) resolved_frgns: Option<std::collections::HashMap<String, crate::analysis::frgn_dispatch::ResolvedFrgn>>,
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
            Statement::Assign(Expr::AddrOf(inner), _) => {
                if let Expr::Identifier(name) = inner.as_ref() {
                    out.push(name.clone());
                }
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
            arena_ptr_idx: None,
            arena_end_idx: None,
            arena_base_idx: None,
            analysis_alloc_strategies: None,
            feature_sso_strings: false,
            feature_svo: false,
            resolved_frgns: None,
        }
    }

    pub fn with_alloc_strategies(mut self, strategies: std::collections::HashMap<usize, AllocStrategy>) -> Self {
        self.analysis_alloc_strategies = Some(strategies);
        self
    }

    /// 2026-07-27: Set the set of function names that need arena initialization.
    /// Populated by analyze_arena_need before codegen. When empty, arena fields
    /// in %State and all arena init/fini calls are skipped.
    pub fn with_needs_arena(mut self, needs_arena: std::collections::HashSet<String>) -> Self {
        self.ctx.needs_arena = needs_arena;
        self
    }

    // 2026-07-18: Enable SVO (Small Vector Optimization) for List types.
    pub fn with_svo(mut self, enabled: bool) -> Self {
        self.feature_svo = enabled;
        self
    }

    // 2026-07-18: Enable SSO (Short String Optimization) for String types.
    // When ON, String is a {i64, i64} struct with inline storage for ≤6 bytes.
    pub fn with_sso_strings(mut self, enabled: bool) -> Self {
        self.feature_sso_strings = enabled;
        self
    }

    // 2026-07-18: Set the stack allocation threshold for runtime fallback.
    pub fn with_stack_threshold(mut self, threshold: u64) -> Self {
        self.ctx.stack_threshold = threshold;
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
        // 2026-07-18: SSO String / String-like fields occupy 2 consecutive i64 slots.
        if self.feature_sso_strings
            && self.ctx.type_universe.as_ref().map_or(false, |u| u.is_string_like(ty))
        {
            self.ctx.field_types.push("i64".to_string());
            self.ctx.field_brief_types.push(ty.clone());
            self.ctx.field_types.push("i64".to_string());
            self.ctx.field_brief_types.push(ty.clone());
            return;
        }
        // 2026-07-18: SVO List — push N+1 slots (N inline data + 1 len+cap).
        if self.feature_svo
            && self.ctx.type_universe.as_ref().map_or(false, |u| u.is_vector_like(ty))
        {
            let cap = self.ctx.type_universe.as_ref()
                .map(|u| u.svo_capacity(ty)).unwrap_or(0);
            if cap > 0 {
                for _ in 0..=cap {  // cap + 1 slots
                    self.ctx.field_types.push("i64".to_string());
                    self.ctx.field_brief_types.push(ty.clone());
                }
                return;
            }
        }
        // 2026-07-25: Fixed-size array: Int[1024] → [1024 x i64].
        // Emitted as a single LLVM array field. Index accesses become GEPs.
        if let Type::Vector(inner, dims) = ty {
            if dims.len() == 1 {
                if let crate::ast::Dimension::Anonymous(n) = dims[0] {
                    let inner_llvm = if **inner == Type::float64() { "double".to_string() }
                        else if **inner == Type::float() { "float".to_string() }
                        else { "i64".to_string() };
                    let arr_ty = format!("[{} x {}]", n, inner_llvm);
                    self.ctx.field_types.push(arr_ty);
                    self.ctx.field_brief_types.push(ty.clone());
                    return;
                }
            }
        }
        // 2026-07-26: Derive %State field type from protocol + maxbits.
        // Float types get native float/double. Exact integer types (Int8..Int128)
        // get native iN width. Everything else (flexible Int, Bool, Ptr, String)
        // stores as i64 — adapt_to_i64/ensure_typed_value handle conversion.
        let llvm_ty = if let Some(ref universe) = self.ctx.type_universe {
            if let Some(rt) = ty.universe_key().and_then(|k| universe.get(k)) {
                let is_float = rt.properties.contains_key("Cast.#Float");
                if is_float {
                    if rt.max_bits <= 32 { "float".to_string() }
                    else if rt.max_bits <= 64 { "double".to_string() }
                    else { "i64".to_string() }
                } else if rt.min_bits == rt.max_bits && rt.max_bits > 0 {
                    // Exact integer types get native iN width.
                    let bits = if rt.max_bits <= 8 { 8 }
                        else if rt.max_bits <= 16 { 16 }
                        else if rt.max_bits <= 32 { 32 }
                        else if rt.max_bits <= 64 { 64 }
                        else { 128 };
                    format!("i{}", bits)
                } else {
                    "i64".to_string()
                }
            } else {
                "i64".to_string()
            }
        } else {
            "i64".to_string()
        };
        self.ctx.field_types.push(llvm_ty);
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

    /// 2026-07-26: Enable webstack mode (WASM-first rendering).
    /// When enabled, the backend emits __web_flush_state calls at term
    /// and exports state_layout() for the JS runtime shim.
    /// Only meaningful for .rbv files compiled with BackendKind::Webstack.
    pub fn with_webstack(mut self, enabled: bool) -> Self {
        self.ctx.webstack_enabled = enabled;
        self
    }

    /// Pre-populate MMIO address map from a resolved DBV target binding.
    /// Each alias name maps to a physical u64 address for volatile MMIO access.
    pub fn with_mmio_addresses(mut self, addresses: HashMap<String, u64>) -> Self {
        self.ctx.mmio_fields = addresses;
        self.ctx.mmio_prepopulated = true;
        self
    }

    pub fn with_schema_aliases(mut self, aliases: HashSet<String>) -> Self {
        self.ctx.schema_alias_names = aliases;
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

    /// 2026-07-25: Set the native integer width for #Int protocol.
    /// WASM should use 32 to emit i32 instead of i64 (avoid BigInt).
    pub fn with_int_bits(mut self, bits: u64) -> Self {
        self.ctx.int_bits = bits;
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

    /// 2026-07-20: Set operator definitions for <- operator dispatch.
    pub fn with_operator_defs(mut self, defs: HashMap<String, Vec<crate::ast::top::OperatorDef>>) -> Self {
        self.ctx.operator_defs = defs;
        self
    }

    /// 2026-07-30: Register CastFrom(#Bit) overrides on the casting graph.
    /// Maps type_name → constructor_function_name for constructing a type
    /// from raw memory bits. This is the sole user-extensible cast edge.
    pub fn with_cast_from_bit_overrides(mut self, overrides: HashMap<String, String>) -> Self {
        if let Some(ref mut graph) = self.ctx.casting_graph {
            for (type_name, fn_name) in overrides {
                graph.register_cast_from_bit(&type_name, &fn_name);
            }
        }
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

    /// 2026-07-18: Build a shared library (.so). When true, no main loop
    /// is emitted; only exported wrappers and reactive convergence entry.
    pub fn with_shared_lib(mut self, v: bool) -> Self {
        self.ctx.is_shared_lib = v;
        self
    }

    /// 2026-07-23: Emit Python C extension module init metadata.
    /// When true, generate() emits PyMethodDef[], PyModuleDef, and PyInit_.
    pub fn with_module_init(mut self, v: bool) -> Self {
        self.ctx.module_init = v;
        self
    }

    /// 2026-07-23: Skip emitting the default main() entry point.
    /// The caller provides its own main (e.g., protocol bridge shim).
    pub fn with_no_main(mut self, v: bool) -> Self {
        self.ctx.no_main = v;
        self
    }

    /// 2026-07-22: Provide pre-resolved frgn dispatch strategies.
    /// The backend uses these to decide how to emit foreign calls.
    pub fn with_resolved_frgns(
        mut self,
        map: std::collections::HashMap<String, crate::analysis::frgn_dispatch::ResolvedFrgn>,
    ) -> Self {
        self.resolved_frgns = Some(map);
        self
    }

    /// Set the LLVM target triple for generated IR.
    /// Also updates the data layout to match and derives int_bits from it.
    /// 2026-07-11: Phase 6 — WASM target support.
    /// 2026-07-29: Wire parse_pointer_width to auto-derive int_bits from data layout.
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
        if let Some(ref dl) = self.ctx.data_layout {
            self.ctx.int_bits = CompilerContext::parse_pointer_width(dl);
        }
        self
    }

    /// Set the LLVM data layout string (overrides the auto-derived layout).
    /// Also derives int_bits from the data layout's pointer width.
    /// 2026-07-15: Phase 7 — config-driven from targets.toml.
    /// 2026-07-29: Wire parse_pointer_width to auto-derive int_bits from data layout.
    pub fn with_data_layout(mut self, dl: &str) -> Self {
        self.ctx.data_layout = Some(dl.to_string());
        self.ctx.int_bits = CompilerContext::parse_pointer_width(dl);
        self
    }

    /// Emit an inttoptr instruction with the correct pointer-width integer type.
    /// Uses int_bits (from data layout or CLI --int-bits) for the integer side.
    /// 2026-07-27: DataLayout-driven int_bits replaces pointer_llvm_type().
    pub(super) fn emit_inttoptr(&self, out: &mut String, indent: &str, dest: &dyn Display, src: &dyn Display) {
        let int_ty = format!("i{}", self.ctx.int_bits);
        writeln!(out, "{}{} = inttoptr {} {} to ptr", indent, dest, int_ty, src).ok();
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
        // 2026-07-19: emit_arena_alloc returns i64 — inttoptr to get ptr.
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, buf_i64, buf_reg).ok();
        let base = format!("%pba_{}", c);
        // 2026-07-19: buf_reg is already i64 from emit_arena_alloc.
        writeln!(out, "{}{} = add i64 0, {}", indent, base, buf_reg).ok();
        let data_ptr = format!("%pdv_{}", c);
        writeln!(out, "{}{} = add i64 {}, 16", indent, data_ptr, base).ok();
        let s0 = format!("%ps0_{}", c);
        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 0", indent, s0, buf_i64).ok();
        writeln!(out, "{}store i64 {}, ptr {}, align 8, !tbaa !1", indent, data_ptr, s0).ok();
        let s1 = format!("%ps1_{}", c);
        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, s1, buf_i64).ok();
        writeln!(out, "{}store i64 0, ptr {}, align 8, !tbaa !1", indent, s1).ok();
        let ap = self.emit_state_gep(out, indent, "pap", "%state", idx);
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
        // 2026-07-19: Bump-pointer arena allocation via %State fields.
        // Uses next_reg_with_prefix (no closures — avoids borrow conflicts).

        // 2026-07-22: Low budget → direct malloc (simpler IR, faster compile).
        // The --optimize-budget flag (default 256) controls simulation depth;
        // below the config arena_min_budget, skip the bump arena entirely and
        // use heap allocation.
        // 2026-07-31: Phase 3 (§8.2) — threshold from config/ir-lowering.toml.
        if (self.ctx.optimize_budget as u32) < crate::config_tuning::ir_lowering().arena_min_budget {
            let r = self.fun.next_reg_with_prefix("aam");
            writeln!(out, "{}{} = call noalias ptr @malloc(i64 {})", indent, r, size_reg).ok();
            let ri = self.fun.next_reg_with_prefix("aami");
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, ri, r).ok();
            return ri;
        }

        let Some(aptr_idx) = self.arena_ptr_idx else {
            let r = self.fun.next_reg_with_prefix("aam");
            writeln!(out, "{}{} = call noalias ptr @malloc(i64 {})", indent, r, size_reg).ok();
            let ri = self.fun.next_reg_with_prefix("aami");
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, ri, r).ok();
            return ri;
        };
        let aend_idx = self.arena_end_idx.unwrap();
        let abase_idx = self.arena_base_idx.unwrap();

        // Helper: emit GEP+load+inttoptr for an arena %State field → returns ptr reg.
        macro_rules! load_state_ptr { ($idx:expr, $pfx:expr) => {{
            let _gep = self.fun.next_reg_with_prefix(concat!($pfx, "g"));
            writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                indent, _gep, $idx).ok();
            let _vi = self.fun.next_reg_with_prefix(concat!($pfx, "i"));
            writeln!(out, "{}{} = load i64, ptr {}", indent, _vi, _gep).ok();
            let _vp = self.fun.next_reg_with_prefix(concat!($pfx, "p"));
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, _vp, _vi).ok();
            _vp
        }}; }
        // Helper: emit GEP+ptrtoint+store for an arena %State field.
        macro_rules! store_state_ptr { ($idx:expr, $pfx:expr, $ptr:expr) => {{
            let _gep = self.fun.next_reg_with_prefix(concat!($pfx, "g"));
            writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                indent, _gep, $idx).ok();
            let _pi = self.fun.next_reg_with_prefix(concat!($pfx, "pi"));
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, _pi, $ptr).ok();
            writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, _pi, _gep).ok();
        }}; }

        let ok_l_n = self.fun.txn_counter;
        self.fun.txn_counter += 1;
        let check_l = format!("aacheck_{}", ok_l_n);
        let grow_l = format!("aagrow_{}", ok_l_n);

        // Load current ptr and end from %State
        let cur = load_state_ptr!(aptr_idx, "aac");
        let end = load_state_ptr!(aend_idx, "aae");

        writeln!(out, "{}br label %{}", indent, check_l).ok();
        writeln!(out, "{}{}:", indent, check_l).ok();
        let new_ptr = self.fun.next_reg_with_prefix("aanew");
        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, new_ptr, cur, size_reg).ok();
        let ok_val = self.fun.next_reg_with_prefix("aaok");
        writeln!(out, "{}{} = icmp ule ptr {}, {}", indent, ok_val, new_ptr, end).ok();
        writeln!(out, "{}br i1 {}, label %aaok_{}, label %{}", indent, ok_val, ok_l_n, grow_l).ok();

        // Grow path
        writeln!(out, "{}{}:", indent, grow_l).ok();
        let base = load_state_ptr!(abase_idx, "aaob");
        let grow_sz = self.fun.next_reg_with_prefix("aags");
        writeln!(out, "{}{} = shl i64 {}, 1", indent, grow_sz, size_reg).ok();
        let min_sz = self.fun.next_reg_with_prefix("aams");
        writeln!(out, "{}{} = add i64 {}, {}", indent, min_sz, grow_sz, self.ctx.arena_initial_size).ok();
        let new_base = self.fun.next_reg_with_prefix("aanb");
        writeln!(out, "{}{} = call ptr @realloc(ptr {}, i64 {})", indent, new_base, base, min_sz).ok();
        store_state_ptr!(aptr_idx, "aaps", &new_base);
        store_state_ptr!(abase_idx, "aabs", &new_base);
        let new_end = self.fun.next_reg_with_prefix("aane");
        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, new_end, new_base, min_sz).ok();
        store_state_ptr!(aend_idx, "aaes", &new_end);
        writeln!(out, "{}br label %aaok_{}", indent, ok_l_n).ok();

        // OK path
        writeln!(out, "aaok_{}:", ok_l_n).ok();
        let phi = self.fun.next_reg_with_prefix("aaphi");
        writeln!(out, "{}{} = phi ptr [ {}, %{} ], [ {}, %{} ]",
            indent, phi, cur, check_l, new_base, grow_l).ok();
        let new_bump = self.fun.next_reg_with_prefix("aanbp");
        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, new_bump, phi, size_reg).ok();
        store_state_ptr!(aptr_idx, "aaps2", &new_bump);
        let result = self.fun.next_reg_with_prefix("aapi");
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, result, phi).ok();
        result
    }

    /// Emit arena initialization at scope entry. Allocates the initial
    /// 64KB arena buffer, sets up ptr/end/base alloca slots.
    pub(crate) fn emit_arena_init(&mut self, out: &mut String, indent: &str) {
        // 2026-07-27: Global gate — if no function in the program needs arena,
        // skip emission entirely. The field-injection guard ensures arena_ptr_idx
        // is None when needs_arena is empty, but this early return avoids even
        // entering the function dispatch logic.
        if self.ctx.needs_arena.is_empty() {
            return;
        }
        let Some(aptr_idx) = self.arena_ptr_idx else { return; };
        let Some(aend_idx) = self.arena_end_idx else { return; };
        let Some(abase_idx) = self.arena_base_idx else { return; };
        // 2026-07-19: Uses next_reg_with_prefix (txn_counter) for unique names.
        // Eliminates arena_counter — consistent with emit_arena_alloc pattern.
        if self.has_async_txns {
            writeln!(out, "{}%arena_mutex = alloca i64, align 8", indent).ok();
            writeln!(out, "{}store i64 0, ptr %arena_mutex, align 8", indent).ok();
        }
        let init = self.fun.next_reg_with_prefix("arinit");
        writeln!(out, "{}{} = call ptr @malloc(i64 {})", indent, init, self.ctx.arena_initial_size).ok();
        let init_i64 = self.fun.next_reg_with_prefix("arii");
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, init_i64, init).ok();
        // Store arena_ptr, arena_base, arena_end
        self.emit_state_store_i64_by_idx(out, indent, aptr_idx, &init_i64);
        self.emit_state_store_i64_by_idx(out, indent, abase_idx, &init_i64);
        let init_end = self.fun.next_reg_with_prefix("arieu");
        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, init_end, init, self.ctx.arena_initial_size).ok();
        let end_i64 = self.fun.next_reg_with_prefix("arie");
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, end_i64, init_end).ok();
        self.emit_state_store_i64_by_idx(out, indent, aend_idx, &end_i64);
    }

    /// Emit arena reset: rewinds the bump pointer to the base, preserving
    /// the allocated memory for reuse in the next scope iteration.
    /// 2026-07-19: Arena state is in %State — reload base from there.
    pub(crate) fn emit_arena_reset(&mut self, out: &mut String, indent: &str) {
        // 2026-07-27: Global gate — skip if no function needs arena.
        if self.ctx.needs_arena.is_empty() {
            return;
        }
        let Some(aptr_idx) = self.arena_ptr_idx else { return; };
        let Some(abase_idx) = self.arena_base_idx else { return; };
        let (base_val, _) = self.emit_state_load_i64_by_idx(out, indent, abase_idx);
        let base_ptr = self.fun.next_reg_with_prefix("arbp");
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, base_ptr, base_val).ok();
        self.emit_state_store_i64_by_idx(out, indent, aptr_idx, &base_val);
    }

    /// Emit arena teardown at program exit. Frees the arena buffer.
    pub(crate) fn emit_arena_fini(&mut self, out: &mut String, indent: &str) {
        // 2026-07-27: Global gate — skip if no function needs arena.
        if self.ctx.needs_arena.is_empty() {
            return;
        }
        let Some(abase_idx) = self.arena_base_idx else { return; };
        let (base_val, _) = self.emit_state_load_i64_by_idx(out, indent, abase_idx);
        let base_ptr = self.fun.next_reg_with_prefix("afp");
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, base_ptr, base_val).ok();
        writeln!(out, "{}call void @free(ptr {})", indent, base_ptr).ok();
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

    /// Emit Python C extension module init metadata: PyMethodDef[],
    /// PyModuleDef, and PyInit_<name>. The struct layouts are read from
    /// the type universe (injected by InjectTypeLayout$ at compile time).
    /// 2026-07-23: Config-driven, zero Rust per-language code.
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
        // 2026-07-26: Protocol-driven. String-like types (SSO/non-SSO) are tracked
        // by is_string_like in the universe. Heap-allocated types declare
        // Cast.#HeapAllocated in their type properties. UTF8View, StaticString,
        // SmallString64 are stack-allocated (no Cast.#HeapAllocated).
        if self.ctx.type_universe.as_ref().map_or(false, |u| u.is_string_like(ty)) {
            return true;
        }
        if self.is_protocol_member(ty, "#Data") {
            return true;
        }
        ty.universe_key()
            .and_then(|k| self.ctx.type_universe.as_ref().and_then(|u| u.get(k)))
            .map(|rt| rt.properties.contains_key("Cast.#HeapAllocated"))
            .unwrap_or(false)
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
        // 2026-07-31: Phase 3 (§8.1) — warn once when the target triple's prefix
        // is unknown to config/targets.toml, so the x86_64 tuning fallback is
        // never applied silently to a foreign target.
        if !crate::config_tuning::known_target_triple(&self.ctx.target_triple) {
            self.warnings.push(format!(
                "warning: target triple '{}' has no [target.<prefix>] entry in \
                 config/targets.toml — using x86_64 tuning defaults",
                self.ctx.target_triple
            ));
        }
        let mut analysis = crate::backend::analyze_program(
            items,
            false,
            // 2026-07-31: Phase 3 (§8.1) — vector-phi promotion gate from
            // config/targets.toml `vector_min_width` for this target.
            crate::config_tuning::target_settings_for(&self.ctx.target_triple).vector_min_width,
        );
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
            analysis.region_analyzer.collect_final_values(&items)
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

        // 2026-07-28: Persist transition graph for !prof computation in emit_toplevel.
        // iter_bounds populated later (at txn processing loop) — txns not in scope yet.
        self.ctx.transition_graph = Some(analysis.transition_graph.clone());
        // 2026-07-31: Phase 2 measurement passes (plan §7.5) — persist so the
        // emission consumers (density downgrade, modulo dispatch, auto-inline)
        // read frontend analysis instead of re-walking bodies.
        self.ctx.density = analysis.density.clone();
        self.ctx.inline_decisions = analysis.inline_decisions.clone();
        self.ctx.modulo_partition = analysis.modulo_partition.clone();

        let cg = &analysis.call_graph;
        self.ctx.has_cycles = cg.has_cycle();
        let sb = self.compute_state_size_bytes() as u64;
        self.ctx.state_size_bytes = sb;
        self.ctx.state_ptr_param = if sb > 0 {
            format!("ptr noundef dereferenceable({}) noalias nocapture align 8 %state", sb)
        } else {
            "ptr noundef noalias nocapture align 8 %state".to_string()
        };

        if self.ctx.is_embedded {
            self.check_embedded_restrictions(items);
        }

        self.ctx.exit_condition = exit_condition;
        // 2026-07-13: normalize_to_old_recursive removed in new AST.
        // BinaryOp/UnaryOp exit conditions are already the canonical form.
        // 2026-07-29: Reorder state field declarations to SoA layout before
        // index assignment. This makes same-component fields (bx0..bx4) have
        // consecutive indices, enabling the backend to form <N x float> vector
        // phi groups. See analysis/soa_reorder.rs for safety verification.
        let reordered_items = crate::analysis::soa_reorder::reorder_fields(items);
        self.build_field_index(&reordered_items);

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
        // 2026-07-27: Arena system fields — injected only when program needs arena.
        // Skipping these saves 24 bytes in %State and eliminates the 64KB malloc
        // for benchmarks with no Alloc# calls. When needs_arena is empty, no function
        // code references these fields so they are safely omitted.
        if !self.ctx.needs_arena.is_empty() {
            let aptr = self.ctx.field_index_map.len();
            self.ctx.field_index_map.insert("__arena_ptr".to_string(), aptr);
            self.ctx.field_types.push("i64".to_string());
            self.ctx.field_brief_types.push(Type::int());
            self.arena_ptr_idx = Some(aptr);

            let aend = self.ctx.field_index_map.len();
            self.ctx.field_index_map.insert("__arena_end".to_string(), aend);
            self.ctx.field_types.push("i64".to_string());
            self.ctx.field_brief_types.push(Type::int());
            self.arena_end_idx = Some(aend);

            let abase = self.ctx.field_index_map.len();
            self.ctx.field_index_map.insert("__arena_base".to_string(), abase);
            self.ctx.field_types.push("i64".to_string());
            self.ctx.field_brief_types.push(Type::int());
            self.arena_base_idx = Some(abase);
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
                        // 2026-07-18: Populate from output_type as well.
                        let ret_tys = if !t.outputs.is_empty() {
                            t.outputs.clone()
                        } else if let Some(ref ot) = t.output_type {
                            ot.all_types()
                        } else {
                            vec![]
                        };
                        self.ctx.defn_return_types.insert(t.name.clone(), ret_tys);
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
                    // 2026-07-18: Populate return types from output_type (-> Type) as
                    // well as legacy outputs. output_type is the primary metadata source.
                    let ret_tys = if !d.outputs.is_empty() {
                        d.outputs.clone()
                    } else if let Some(ref ot) = d.output_type {
                        ot.all_types()
                    } else {
                        vec![]
                    };
                    self.ctx.defn_return_types.insert(d.name.clone(), ret_tys);
                }
                TopLevel::AsmFn(asm_fn) => {
                    let tys: Vec<Type> = asm_fn.params.iter().map(|(_, t)| t.clone()).collect();
                    self.ctx.defn_params.insert(asm_fn.name.clone(), tys);
                    let ret_tys = vec![asm_fn.ret_type.clone()];
                    self.ctx.defn_return_types.insert(asm_fn.name.clone(), ret_tys);
                }
                TopLevel::ForeignBinding(fb) => {
                    let sig = crate::ast::ForeignSignature {
                        name: fb.foreign_name.clone(),
                        from: fb.from.clone(),
                        inputs: fb.inputs.clone(),
                        result_type: crate::ast::ResultType::Projection(fb.success_output.iter().map(|(_, t)| t.clone()).collect()),
                        wasm_impl: fb.wasm_impl.clone(),
                        wasm_setup: fb.wasm_setup.clone(),
                        span: fb.span,
                    };
                    self.ctx.frgn_map.insert(fb.foreign_name.clone(), sig.clone());
                    // 2026-07-22: Also index by Brief name so call resolution
                    // (Expr::Call uses the Brief name, e.g. "frgn__getenv_brief")
                    // finds the frgn entry. The declare loop emits only for the
                    // foreign_name key to avoid duplicate declarations.
                    let brief_name = fb.effective_brief_name();
                    if brief_name != fb.foreign_name {
                        self.ctx.frgn_map.insert(brief_name.to_string(), sig);
                    }
                }
                TopLevel::Obj(s) => {
                    let fields: Vec<(String, Type)> = s.fields.iter()
                        .map(|f| (f.name.clone(), f.ty.clone()))
                        .collect();
                    self.ctx.struct_types.insert(s.name.clone(), fields.clone());
                    if let Some(ref mut universe) = self.ctx.type_universe {
                        if !universe.types.contains_key(&s.name) {
                            let bytes: u64 = fields.iter().map(|(_, ty)| {
                                crate::backend::llvm::types::type_size(ty, Some(universe))
                            }).sum();
                            let rt = crate::type_universe::ResolvedType {
                                name: s.name.clone(),
                                base: "Bit".to_string(),
                                bytes,
                                min_bits: bytes * 8,
                                max_bits: bytes * 8,
                                alignment: 8,
                                properties: std::collections::HashMap::new(),
                                fields: fields.clone(),
                            };
                            universe.types.insert(s.name.clone(), rt);
                        }
                    }
                }
                // 2026-07-24: StaticStruct (C-compatible struct) registration.
                // These use `struct Name { ... }` syntax.
                TopLevel::StaticStruct(s) => {
                    let fields: Vec<(String, Type)> = s.fields.iter()
                        .map(|(n, t)| (n.clone(), t.clone()))
                        .collect();
                    self.ctx.struct_types.insert(s.name.clone(), fields.clone());
                    if let Some(ref mut universe) = self.ctx.type_universe {
                        if !universe.types.contains_key(&s.name) {
                            let bytes: u64 = fields.iter().map(|(_, ty)| {
                                crate::backend::llvm::types::type_size(ty, Some(universe))
                            }).sum();
                            let rt = crate::type_universe::ResolvedType {
                                name: s.name.clone(),
                                base: "Bit".to_string(),
                                bytes,
                                min_bits: bytes * 8,
                                max_bits: bytes * 8,
                                alignment: 8,
                                properties: std::collections::HashMap::new(),
                                fields: fields.clone(),
                            };
                            universe.types.insert(s.name.clone(), rt);
                        }
                    }
                }
                // 2026-07-24: Handle TopLevel::TypeDef with slots as struct types.
                // This handles `obj` declarations (which parse to TypeDef) and
                // other type declarations with field slots.
                TopLevel::TypeDef(td) if !td.body.slots.is_empty() => {
                    let fields: Vec<(String, Type)> = td.body.slots.iter()
                        .map(|s| (s.name.clone(), s.ty.clone()))
                        .collect();
                    // 2026-07-24: Register struct type in both struct_types and universe
                    self.ctx.struct_types.insert(td.name.clone(), fields.clone());
                    if let Some(ref mut universe) = self.ctx.type_universe {
                        if !universe.types.contains_key(&td.name) {
                            let bytes: u64 = fields.iter().map(|(_, ty)| {
                                crate::backend::llvm::types::type_size(ty, Some(universe))
                            }).sum();
                            let rt = crate::type_universe::ResolvedType {
                                name: td.name.clone(),
                                base: "Bit".to_string(),
                                bytes,
                                min_bits: bytes * 8,
                                max_bits: bytes * 8,
                                alignment: 8,
                                properties: std::collections::HashMap::new(),
                                fields: fields.clone(),
                            };
                            universe.types.insert(td.name.clone(), rt);
                        }
                    }
                }
                TopLevel::Enum(e) => {
                    self.ctx.enum_types.insert(e.name.clone(), e.clone());
                }
                // 2026-07-30: Unwrap Export to register defn/txn/asm_fn params
                // from exported definitions. Without this, emit_user_call can't
                // find the parameter types and falls through to the non-defn path
                // which doesn't insert inttoptr for Ptr args.
                TopLevel::Export(e) => {
                    match e.inner.as_ref() {
                        TopLevel::Definition(d) => {
                            let tys: Vec<Type> = d.parameters.iter().map(|(_, t)| t.clone()).collect();
                            self.ctx.defn_params.insert(d.name.clone(), tys);
                            let ret_tys = if !d.outputs.is_empty() {
                                d.outputs.clone()
                            } else if let Some(ref ot) = d.output_type {
                                ot.all_types()
                            } else {
                                vec![]
                            };
                            self.ctx.defn_return_types.insert(d.name.clone(), ret_tys);
                        }
                        TopLevel::Transaction(t) => {
                            let tys: Vec<Type> = t.parameters.iter().map(|(_, ty)| ty.clone()).collect();
                            self.ctx.defn_params.insert(t.name.clone(), tys);
                            let ret_tys = if !t.outputs.is_empty() {
                                t.outputs.clone()
                            } else if let Some(ref ot) = t.output_type {
                                ot.all_types()
                            } else {
                                vec![]
                            };
                            self.ctx.defn_return_types.insert(t.name.clone(), ret_tys);
                        }
                        TopLevel::AsmFn(af) => {
                            let tys: Vec<Type> = af.params.iter().map(|(_, t)| t.clone()).collect();
                            self.ctx.defn_params.insert(af.name.clone(), tys);
                            self.ctx.defn_return_types.insert(af.name.clone(), vec![af.ret_type.clone()]);
                        }
                        _ => {}
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
        // 2026-07-22: Deduplicate by foreign_name — frgn_map may have dual
        // keys (foreign_name + effective_brief_name) but declares use only the
        // C linker symbol name (sig.name = foreign_name).
        // 2026-07-31: Sort by key before emitting — frgn_map is a HashMap with
        // a per-process SipHash seed; unsorted iteration produced run-to-run
        // nondeterministic declare ORDER in the IR (Coding Standard 7).
        let mut declared: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut frgn_sorted: Vec<(&String, &crate::ast::ForeignSignature)> =
            self.ctx.frgn_map.iter().collect();
        frgn_sorted.sort_by_key(|(name, _)| (*name).clone());
        for (name, sig) in frgn_sorted {
            if trigger_linked_symbols.contains(name.as_str()) { continue; }
            // Dedup: skip if we already emitted a declare for this foreign_name
            if !declared.insert(&sig.name) { continue; }
            let ret_ty: String = match sig.result_type {
                crate::ast::ResultType::VoidType | crate::ast::ResultType::TrueAssertion => "void".into(),
                crate::ast::ResultType::Projection(ref ts) => {
                    if ts.is_empty() || ts.iter().any(|t| matches!(t, Type::Void)) { "void".into() }
                    else {
                        // 2026-07-26: Use protocol-driven LLVM type.
                        let first_ret = &ts[0];
                        protocol_llvm_type(first_ret, self.ctx.type_universe.as_ref())
                    }
                }
            };
            let param_tys: Vec<String> = sig.inputs.iter().map(|(_, t)| {
                // 2026-07-26: Use protocol-driven LLVM type.
                protocol_llvm_type(t, self.ctx.type_universe.as_ref())
            }).collect();
            write!(out, "declare {} @{}(", ret_ty, sig.name).ok();
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

        // Error message for read_file# — returned as Err's String payload
        writeln!(out, "@STR_READFILE_ERR = private unnamed_addr constant [15 x i8] c\"file not found\\00\"").ok();
        // Declare libc functions used by direct-libc intrinsics
        // 2026-07-05: dso_local prevents LLVM globalopt from treating
        // @stdout as null (LLVM 18 assumes external globals without
        // initializer are zero = null). Without dso_local, LLVM's
        // function-attributor deduces fprintf(stdout) has a null pointer
        // argument → UB → entire body is dead → knucleotide prints nothing.
        writeln!(out, "@stdout = external dso_local global ptr").ok();


        // 2026-07-15: Async dispatch runtime functions
        writeln!(out, "declare void @__wait_for_trigger__() #1").ok();
        // 2026-07-15: Removed conflicting POSIX declares (getuid, sched_yield,
        // nanosleep, exit, etc.) — replaced by Brief defn wrappers using SysCall#.

        // Emit external global declarations for linked triggers (fixes bug 4B)
        for (name, trg) in &self.ctx.triggers {
            if let crate::ast::LinkRef::Linked(sym) = &trg.address {
                let store_ty = trg_llvm_storage_ty(&trg.ty, self.ctx.type_universe.as_ref());
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
        // 2026-07-19: Sorted for deterministic IR order.
        let mut sorted_constants: Vec<String> = self.ctx.constants.keys().cloned().collect();
        sorted_constants.sort();
        for name in &sorted_constants {
            let (ty, expr) = &self.ctx.constants[name];
            let llvm_ty = protocol_llvm_type(ty, self.ctx.type_universe.as_ref());
            let key = match expr {
                Expr::Float(f) => format!("{}:{}", llvm_ty, float_to_llvm_str(*f, &llvm_ty)),
                Expr::Decimal(n) => format!("{}:{}", llvm_ty, n),
                Expr::Bool(b) => format!("{}:{}", llvm_ty, if *b { "true" } else { "false" }),
                Expr::UnaryOp(crate::ast::UnaryOpKind::Neg, inner) => match inner.as_ref() {
                    Expr::Float(f) => format!("{}:{}", llvm_ty, float_to_llvm_str(-*f, &llvm_ty)),
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
        // 2026-07-19: Sorted for deterministic IR order.
        let mut sorted_constants2: Vec<String> = self.ctx.constants.keys().cloned().collect();
        sorted_constants2.sort();
        for name in &sorted_constants2 {
            let (ty, expr) = &self.ctx.constants[name];
            let canonical = alias_map.get(name).cloned().unwrap_or_else(|| name.clone());
            if canonical != *name {
                let llvm_ty = protocol_llvm_type(ty, self.ctx.type_universe.as_ref());
                writeln!(out, "@{} = alias {}, {}* @{}", name, llvm_ty, llvm_ty, canonical).ok();
                continue;
            }
            let llvm_ty = protocol_llvm_type(ty, self.ctx.type_universe.as_ref());
            let val_str = match expr {
                Expr::Float(f) => float_to_llvm_str(*f, &llvm_ty),
                Expr::Decimal(n) => n.to_string(),
                Expr::Bool(b) => (if *b { "true" } else { "false" }).to_string(),
                Expr::UnaryOp(crate::ast::UnaryOpKind::Neg, inner) => match inner.as_ref() {
                    Expr::Float(f) => float_to_llvm_str(-*f, &llvm_ty),
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

        // Emit string constants in C-compatible format: [i64 length][chars\0]
        // The handle points to the start, so handle[0] is the length.
        for (si, s) in self.ctx.string_constants.iter().enumerate() {
            let escaped = escape_llvm_string(s);
            let len = s.len();
            writeln!(out, "@str.{} = private unnamed_addr constant <{{ i64, [{} x i8] }}> <{{ i64 {}, [{} x i8] c\"{}\\00\" }}>, align 8",
                si, len + 1, len, len + 1, escaped).ok();
        }
        if !self.ctx.string_constants.is_empty() { writeln!(out).ok(); }

        // 2026-06-29: Global sentinel for all empty list literals `[]`.
        // LLVM eliminates stack-allocated empty lists (dead alloca elimination)
        // because ptrtoint/inttoptr round-trip is invisible to SROA. A single
        // rodata constant { data_ptr=0, length=0 } handles all [] instances
        // with zero runtime cost and zero allocation. See docs/plans/2026-06-29-list-allocation-fix.md.
        writeln!(out, "@ll_empty_list = private unnamed_addr constant {{ i64, i64 }} {{ i64 0, i64 0 }}").ok();
        writeln!(out).ok();

        // 2026-07-28: Populate iter_bounds for !prof computation (txns is now in scope).
        // 2026-07-29: SLP hazard and isomorphism analysis removed — proven counterproductive.
        // LLVM's SLP vectorizer has its own cost model. See §7 of recovery plan.
        self.ctx.iter_bounds.clear();
        for (name, _) in &txns {
            if let Some(bound) = analysis.region_analyzer.iteration_bound_of(name) {
                self.ctx.iter_bounds.insert(name.clone(), bound);
            }
        }

        let mut range_meta: Vec<String> = Vec::new();

        // Definitions
        for item in items {
            match item {
                TopLevel::Definition(d) => {
                    self.emit_definition(&mut out, d, true);
                    writeln!(out).ok();
                }
                TopLevel::Export(e) => {
                    if let TopLevel::Definition(d) = e.inner.as_ref() {
                        let needs_state = self.definition_needs_state(d);
                        self.emit_definition(&mut out, d, needs_state);
                        writeln!(out).ok();
                    }
                }
                // 2026-07-29: Emit asm function bodies.
                // Each AsmFn is emitted as a define function with
                // call asm sideeffect body.
                TopLevel::AsmFn(asm_fn) => {
                    self.emit_asm_fn(&mut out, asm_fn);
                    writeln!(out).ok();
                }
                _ => {}
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
        // 2026-07-19: Sorted for deterministic IR.
        let mut persistent_cells: Vec<crate::ast::CellDef> = self.ctx.cell_defs.values()
            .filter(|c| c.is_persistent)
            .cloned()
            .collect();
        persistent_cells.sort_by_key(|c| c.name.clone());
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
        //
        // 2026-07-31: The derivation moved to the frontend pass
        // loop_shape::program_convergence — the backend consumes the derived
        // (counter, bound_var) pairs instead of re-walking the txn list. The
        // 2026-07-18 behavior is preserved exactly: the synthetic exit is built
        // for ALL programs (not just wake-triggered ones) whenever every reactive
        // txn is foldable and no explicit exit condition exists.
        let explicit_exit = self.ctx.exit_condition.clone();
        let program_conv = crate::analysis::loop_shape::program_convergence(
            graph, items, explicit_exit.is_some(),
        );
        self.ctx.has_natural_exit = program_conv.has_natural_exit;
        if explicit_exit.is_none() && !program_conv.counter_ge_bounds.is_empty() {
            let combined = program_conv.counter_ge_bounds.into_iter()
                .map(|(counter, bound)| Expr::BinaryOp(
                    crate::ast::BinaryOpKind::Ge,
                    Box::new(Expr::Identifier(counter)),
                    Box::new(Expr::Identifier(bound)),
                ))
                .reduce(|a, b| Expr::BinaryOp(crate::ast::BinaryOpKind::And, Box::new(a), Box::new(b)))
                .unwrap();
            self.ctx.exit_condition = Some(Box::new(combined));
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

        // 2026-07-10: EmitPerFieldPhi per-field phi loop is optimal for 1-4
        // fields; beyond that the GEP+load+store per tick overhead exceeds the
        // phi register benefit. (The old `active_writes` body re-walk that fed
        // this heuristic was removed 2026-07-31 as dead — the write set comes
        // from the transition graph's node.write_set, and the dispatch decision
        // now comes from the LoopShape, not a field count.)
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
            if bp.var != inc.var {
                false
            } else {
                match self.ctx.field_index_map.get(&bp.var) {
                    None => false,
                    Some(&counter_idx) => {
                        // 2026-07-31: Dispatch now switches on the frontend-computed
                        // LoopShape (analysis.loop_shapes) instead of re-deriving
                        // decisions from write_density/total_fields body re-walks.
                        // Every foldable bounded-counter reactive node has a shape
                        // (build_loop_shapes mirrors this gate); a missing shape
                        // falls through to the conservative reactor path.
                        // See docs/plans/2026-07-31-frontend-driven-dispatch.md §6.5.
                        match analysis.loop_shapes.get(&node.name) {
                            None => false,
                            Some(shape) => {
                                // 2026-07-31: Swan-song hoist consumed from the
                                // frontend analysis (swan_song.rs) — the stripped
                                // body + post-loop hoist pair replaces the backend
                                // hoist_terminating_guard body re-walk.
                                let (txn_body, post_hoist) = match analysis.swan_songs.get(&node.name) {
                                    Some((stripped, hoisted)) => (stripped.clone(), hoisted.clone()),
                                    None => (txns[0].1.body.clone(), Vec::new()),
                                };
                                self.emit_folded_loop_shape(
                                    &mut out, &analysis, node, counter_idx, shape, &txn_body, post_hoist,
                                )
                            }
                        }
                    }
                }
            }
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
                // 2026-07-19: Removed is_pure_body requirement. The multi-txn
                // fold calls @txn_<name> which handles guard checks including
                // observable effects. We only need deterministic convergence
                // (bounded_pre + increments) so the loop bounds are known.
                let multi_foldable = enumerable.is_none()
                    && !has_wake_triggers
                    && !self.async_txn_names.is_empty()
                    && self.async_txn_names.iter().all(|name| {
                        graph.nodes.iter().find(|n| n.name == *name).map_or(false, |node| {
                            node.bounded_pre.is_some()
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
            } else if self.ctx.is_shared_lib {
                // 2026-07-19: Shared library mode — emit export wrappers and
                // reactive entry points. No main loop, no reactor tick.
                self.emit_shared_lib_exports(&mut out, items);
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
                if !self.ctx.no_main {
                    self.emit_main(&mut out, has_wake_triggers);
                }
                // Wake trigger metadata
                if has_wake_triggers {
                    self.emit_wake_metadata(&mut out);
                }
                self.emit_thread_pool_metadata(&mut out);
            } else {
        writeln!(out, "define void @reactor_tick({}) local_unnamed_addr #2 {{", self.ctx.state_ptr_param).ok();
                writeln!(out, "  entry:").ok();
                writeln!(out, "  ret void").ok();
                writeln!(out, "}}").ok();
                writeln!(out).ok();
                // Main
                self.fun.txn_counter = 0;
                self.fun.within_counter = 0;
                if !self.ctx.no_main {
                    self.emit_main(&mut out, false);
                }
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
        // 2026-07-21: Fast-math attributes (tried for nbody vrcpps conversion, but
        // vrcpps only works on vector types — all nbody divisions are scalar fdiv).
        // Reverted — function-level attrs alone don't enable vrcpps for scalar ops.
        // A future optimization could emit vector <4 x float> divisions and use
        // SLP vectorization to pack scalar ops into vector vrcpps calls.
        writeln!(out, "attributes #0 = {{").ok();
        writeln!(out, "    mustprogress nofree norecurse nosync nounwind memory(readwrite)").ok();
        writeln!(out, "}}").ok();
        writeln!(out, "attributes #1 = {{ nocallback nofree nosync nounwind willreturn memory(readwrite) }}").ok();
        writeln!(out, "attributes #2 = {{ mustprogress nofree norecurse nosync nounwind memory(readwrite) }}").ok();
        writeln!(out, "attributes #3 = {{ nofree norecurse nosync nounwind memory(readwrite) }}").ok();
        // 2026-07-27: SLP hazard attribute variants #4/#5 removed — manual SLP
        // vector emission is disabled (counter.rs), so there's no conflict with
        // LLVM's auto-vectorizer. All functions use #0 or #3 without disable-slp.
        // The hazard analysis code in hazard.rs is retained for future re-evaluation.
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
        // 2026-07-27: #9 = memory(readwrite) for @main.
        // Main calls FFI functions (__print_int, __print_float, __print_str) that
        // access @stdout (a global). memory(argmem: readwrite) would be a lie —
        // telling LLVM main only accesses %state causes incorrect alias analysis
        // and spurious register spill across FFI calls. Per-txn functions (#8)
        // can safely use argmem:readwrite since they only access %state.
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
        // 2026-07-27: #11 = argmem:readwrite for reactive txns after cold-path
        // outlining. When FFI-containing guard blocks are outlined into separate
        // cold functions, the remaining hot body is pure-Brief (only accesses %state).
        // Unlike #8, does NOT include willreturn — reactive txns may loop forever
        // if their convergence condition is never met (though all benchmarks converge).
        writeln!(out, "attributes #11 = {{").ok();
        writeln!(out, "    mustprogress nofree norecurse nosync nounwind memory(argmem: readwrite)").ok();
        writeln!(out, "}}").ok();
        // 2026-07-27: #12 = argmem:readwrite for reactor_tick when all txns are
        // FFI-free after outlining. Includes willreturn because the reactor loop
        // always converges (all txns converge).
        writeln!(out, "attributes #12 = {{").ok();
        writeln!(out, "    mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite)").ok();
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

        // 2026-07-26: Phase 4 — Webstack exports (state_layout + generation)
        // and __web_flush_state import. Emitted when webstack_enabled is true.
        if self.ctx.webstack_enabled {
            // Declare the __web_flush_state import — called at each term;
            writeln!(out, "declare void @__web_flush_state(i32, i32)").ok();
            // Generation counter — incremented after each txn commit
            writeln!(out, "@__web_generation = global i32 0").ok();
            // Export generation counter (WASM global export)
            writeln!(out, "@__web_generation_export = hidden global ptr @__web_generation").ok();
            // State layout function — returns ptr to layout struct
            // For Phase 4, emit a stub with 0 fields. Phase 6 will wire
            // the actual field layout table.
            writeln!(out, "@__web_layout = private constant {{ i32, i32, i32, i32 }} {{ i32 0, i32 0, i32 64, i32 16 }}").ok();
            writeln!(out, "define i32 @state_layout() {{").ok();
            writeln!(out, "  ret i32 ptrtoint ({{ i32, i32, i32, i32 }}* @__web_layout to i32)").ok();
            writeln!(out, "}}").ok();
        }

        out
    }

    /// Emit the folded single-bounded-counter `main()` for a node, selecting the
    /// emission strategy from its frontend-computed `LoopShape`.
    ///
    /// Returns `true` when a folded `main()` was emitted; `false` when the shape
    /// says the node cannot be folded (unresolvable bound), so the caller falls
    /// through to the conservative reactor path.
    ///
    /// # Dispatch
    ///
    /// 2026-07-31: The four-way dispatch replaces the previous body of
    /// heuristics (write_density, total_fields thresholds). The decision is now
    /// structural and backend-agnostic:
    ///
    /// ```text
    /// Pure && const bound            → EmitPureCounterFold (O(1) store)
    /// single runtime guard           → emit_version_dag_main (self-deciding)
    /// counter_only && !swan_song     → emit_folded_main (InlineSsa)
    /// vector groups && carried > regs→ emit_countable_main (VectorPhiGroup label)
    /// _                              → emit_countable_main (PerFieldPhi)
    /// ```
    ///
    /// The InlineSsa guardrail (2026-07-29, dispatch-bug-analysis.md) is encoded
    /// structurally: `counter_only_writes` is true exactly when the write set is
    /// `{counter}`, so emit_folded_loop's empty write_set never silently discards
    /// a non-counter state write. version-DAG is tried before InlineSsa/PerFieldPhi
    /// so a body with a single runtime `when` guard (e.g. print_loop) keeps its
    /// guard-absent/guard-present split — see
    /// docs/plans/2026-07-30-flat-node-decomposition.md §11.
    ///
    /// Why per-field phis instead of the old EmitInlineSsa/EmitMemoryCounter paths:
    ///   EmitInlineSsa (inline SSA) used a %slot_case alloca round-trip:
    ///     load %State → extractvalue×N → insertvalue×N → store
    ///     — 33-field struct load/store per iteration hid fields
    ///       from LLVM's induction variable analysis.
    ///   EmitMemoryCounter (memory) kept the counter in %State via GEP+load+store:
    ///     — 3 extra memory uops per tick for the counter alone.
    ///   EmitPerFieldPhi creates per-field phi nodes at the loop header so LLVM
    ///   sees a canonical loop structure (phi + icmp slt + add) that enables
    ///   induction variable analysis, SROA, and loop vectorization. With Path A
    ///   (needs_state_stores_in_body=false) it emits zero memory traffic
    ///   regardless of field count (2026-07-04, MAX_FIELDS_PER_ALLLOCA=15 chunk
    ///   allocas keep SROA happy past 15 phis).
    ///
    /// # Arguments
    ///
    /// * `txn_body` — the swan-song-stripped transaction body (from
    ///   `analysis.swan_songs`); LICM hoisting runs inside.
    /// * `post_hoist` — the hoisted post-loop swan-song tail, assigned to
    ///   `pending_post_hoist` for Path B emission.
    fn emit_folded_loop_shape(
        &mut self,
        out: &mut String,
        analysis: &crate::backend::AnalysisResults,
        node: &crate::analysis::transition_graph::ReactorNode,
        counter_idx: usize,
        shape: &crate::analysis::loop_shape::LoopShape,
        txn_body: &[Statement],
        post_hoist: Vec<Vec<Statement>>,
    ) -> bool {
        let bp = node.bounded_pre.as_ref().unwrap();
        // 2026-07-31: Bound resolution maps the structured Bound to the backend's
        // own index/const tables (field first, then const; literal/unknown →
        // neither) — mirroring the old total_idx / total_const_name lookup so
        // literal-bound txns reach emit_countable_main with both None exactly as
        // before (emit_countable_load_bound falls back to `add i64 0, 1`).
        let (total_idx, total_const_name) = match &shape.bound {
            crate::analysis::loop_shape::Bound::Field(name) => {
                (self.ctx.field_index_map.get(name.as_str()).copied(), None)
            }
            crate::analysis::loop_shape::Bound::Const(name) => (None, Some(name.as_str())),
            crate::analysis::loop_shape::Bound::Literal(_) | crate::analysis::loop_shape::Bound::Unknown(_) => {
                (None, None)
            }
        };
        // 2026-07-31: Only fold when the bound resolves to something the emitters
        // can load. Reproduces the old
        // `total_idx.is_some() || total_const_name.is_some() || bound_literal.is_some()`
        // gate exactly: a Field bound missing from field_index_map, or an Unknown
        // bound, falls through to the reactor path.
        let bound_resolvable = total_idx.is_some()
            || total_const_name.is_some()
            || matches!(shape.bound, crate::analysis::loop_shape::Bound::Literal(_));
        if !bound_resolvable {
            return false;
        }
        // 2026-07-29: Brief-level LICM — hoist loop-invariant let-bindings.
        // The hoisted bindings are prepended to the body so LLVM's LICM
        // can hoist them to the preheader. See analysis/licm.rs.
        let state_fields: HashSet<String> = self.ctx.field_index_map.keys().cloned().collect();
        let (hoisted_reordered, body_stmts) = crate::analysis::licm::hoist_loop_invariants(
            txn_body, &node.write_set, &state_fields,
        );
        // Prepend hoisted bindings to body (they appear at loop entry).
        let body_stmts: Vec<Statement> = hoisted_reordered.into_iter()
            .chain(body_stmts).collect();

        // ── EmitPureCounterFold ─────────────────────────────────────
        // Pure body + constant bound → O(1) counter-only store, no loop.
        // 2026-07-03: A swan song makes the body non-pure — the hoisted print
        // would be silently dropped by the fold, so the fold is blocked exactly
        // when the swan-song hoist fires (frontend has_swan_song).
        if !shape.has_swan_song && shape.is_pure {
            // 2026-07-31: `total_val` mirrors the old field_initializers-then-
            // constants lookup exactly — a Literal bound never folds (the old
            // total_val was None for it), so literal-bound pure txns still reach
            // the version-DAG / InlineSsa / PerFieldPhi emitters below. This keeps
            // Phase 1b strictly behavior-preserving ("pure fold stays as-is",
            // plan §6.2).
            let total_val = match &shape.bound {
                crate::analysis::loop_shape::Bound::Field(name) => self.ctx.field_initializers
                    .get(name.as_str())
                    .and_then(|e| e.as_ref())
                    .and_then(|e| if let Expr::Decimal(n) = e { Some(*n) } else { None }),
                crate::analysis::loop_shape::Bound::Const(name) => self.ctx.constants
                    .get(name.as_str())
                    .and_then(|(_, e)| if let Expr::Decimal(n) = e { Some(*n) } else { None }),
                crate::analysis::loop_shape::Bound::Literal(_) | crate::analysis::loop_shape::Bound::Unknown(_) => None,
            };
            if let Some(tv) = total_val {
                // 2026-07-14: Wrap in define i32 @main() so emitted IR is valid.
                self.warnings.push(format!("info: txn '{}' dispatched via pure counter fold ({} iterations, O(1) store)", node.name, tv));
                writeln!(out, "define i32 @main() local_unnamed_addr #9 {{").ok();
                writeln!(out, "entry:").ok();
                writeln!(out, "  %state = alloca %State, align 8").ok();
                self.emit_inline_init_stores(out, "%state");
                self.emit_folded_pure_counter(out, counter_idx, tv);
                if self.ctx.exit_condition.is_some() {
                    self.emit_exit_check(out);
                    writeln!(out, ".end:").ok();
                }
                writeln!(out, "  ret i32 0").ok();
                writeln!(out, "}}").ok();
                return true;
            }
        }

        // 2026-07-31: Composite-node decomposition (version-DAG). Tried before the
        // batch-loop strategies — a body with a single runtime `when` guard is
        // handled by the guard-absent/guard-present emission and supersedes them.
        // See docs/plans/2026-07-30-flat-node-decomposition.md §11.
        self.fun.pending_post_hoist = post_hoist.clone();
        let is_decreasing_vd = bp.direction == crate::analysis::transition_graph::ConvergeDirection::Decreasing;
        if self.emit_version_dag_main(
            out, counter_idx, total_idx, total_const_name,
            &body_stmts, &node.write_set, is_decreasing_vd, Some(&bp.var),
        ) {
            return true;
        }
        // 2026-07-31: version-DAG did not handle the body — remaining paths use the
        // FULL body (no guard stripping) with plain PerFieldPhi (batch_info = None).
        let inner_body = body_stmts.clone();

        // ── InlineSsa ───────────────────────────────────────────────
        // counter-only write sets are the ONLY safe input (emit_folded_loop passes
        // an empty write_set to emit_countable_body and would silently discard any
        // non-counter state write). `counter_only_writes` encodes that structurally.
        if shape.counter_only_writes && !shape.has_swan_song {
            let total_fields = self.ctx.field_index_map.len();
            self.fun.pending_post_hoist = post_hoist;
            self.warnings.push(format!("info: txn '{}' dispatched via inline SSA ({} fields)", node.name, total_fields));
            self.emit_folded_main(out, &node.name, counter_idx, total_idx, total_const_name, false, Some(&body_stmts));
            return true;
        }

        // ── VectorPhiGroup vs PerFieldPhi ───────────────────────────
        // Isomorphic groups (with the backend's same-type gate applied) on a wide
        // carried set select the vector-phi label. Emission is identical today —
        // emit_countable_main clears active_vector_groups (counter.rs:241) — so the
        // split only records the intended strategy for future vector-phi emission.
        let vg = self.shape_vector_groups(&shape.vector_groups, &node.write_set);
        let carried_len = shape.carried_fields.len();
        let regs = self.ctx.float_register_count();
        let total_fields = self.ctx.field_index_map.len();
        let write_count = node.write_set.len();
        if !vg.is_empty() && carried_len > regs {
            let field_count: usize = vg.iter().map(|g| g.width).sum();
            self.warnings.push(format!("info: txn '{}' dispatched via vector phi ({}/{} fields in {} groups)", node.name, field_count, total_fields, vg.len()));
        } else {
            self.warnings.push(format!("info: txn '{}' dispatched via per-field phi ({}/{} fields written)", node.name, write_count, total_fields));
        }
        self.fun.pending_post_hoist = post_hoist;
        let is_decreasing = bp.direction == crate::analysis::transition_graph::ConvergeDirection::Decreasing;
        self.emit_countable_main(
            out, &node.name, counter_idx, total_idx, total_const_name,
            &inner_body, &node.write_set, is_decreasing, Some(&bp.var),
        );
        true
    }

    /// Convert frontend structural vector groups to backend `VectorPhiGroup`s,
    /// applying the LLVM same-type gate the structural pass cannot express.
    ///
    /// 2026-07-31: The frontend `LoopShape` carries isomorphic groups already
    /// filtered for write-set membership, power-of-2 width, duplicate fields, and
    /// overlap (loop_shape::detect_vector_groups_structural). The backend re-applies
    /// the same-LLVM-type check it used in `detect_vector_groups` so a mixed-type
    /// group never influences dispatch. See
    /// docs/plans/2026-07-31-frontend-driven-dispatch.md §6.2.
    fn shape_vector_groups(
        &self,
        groups: &[crate::analysis::loop_shape::VectorGroup],
        write_set: &HashSet<String>,
    ) -> Vec<crate::backend::llvm::vector_phi::VectorPhiGroup> {
        let mut accepted: HashSet<String> = HashSet::new();
        let mut out_groups: Vec<crate::backend::llvm::vector_phi::VectorPhiGroup> = Vec::new();
        for g in groups {
            // All fields must be unconditionally written (in write_set).
            if !g.fields.iter().all(|f| write_set.contains(f)) {
                continue;
            }
            let element_ty = match g.fields.first() {
                Some(first) => {
                    let Some(&idx) = self.ctx.field_index_map.get(first.as_str()) else { continue; };
                    self.ctx.field_types.get(idx).cloned().unwrap_or_else(|| "i64".to_string())
                }
                None => continue,
            };
            // All fields must have the same LLVM type.
            if !g.fields.iter().all(|f| {
                self.ctx.field_index_map.get(f.as_str())
                    .and_then(|idx| self.ctx.field_types.get(*idx))
                    .map_or(false, |t| t == &element_ty)
            }) {
                continue;
            }
            // Skip if ANY field is already in an accepted group (no overlap).
            if g.fields.iter().any(|f| accepted.contains(f)) {
                continue;
            }
            for f in &g.fields {
                accepted.insert(f.clone());
            }
            out_groups.push(crate::backend::llvm::vector_phi::VectorPhiGroup {
                name: g.name.clone(),
                element_ty,
                width: g.width,
                fields: g.fields.clone(),
                phi_reg: String::new(),
                backedge_reg: String::new(),
            });
        }
        out_groups
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
    /// 2026-07-29: SLP hazard gating removed — proven counterproductive.
    /// No global SLP-disable flags needed. LLVM's auto-vectorizer runs freely.
    pub fn llvm_extra_flags(&self) -> Vec<String> {
        Vec::new()
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

    /// 2026-07-26: Removed type-level validation — schema types were from .dbvs era.
    /// The V2 parser's FieldType handles all type compatibility at parse time.
    fn validate_schema_types(&mut self) {
        // No-op: alias validation is now name-only via schema_alias_names.
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
                    if self.ctx.schema_alias_names.is_empty() || self.ctx.schema_alias_names.contains(&s.name) {
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
                    // 2026-07-20: Check operator_defs for InsertAt strategy.
                    // Any type with op InsertAt(...) gets inline ring buffer fields.
                    let has_insert_strategy = {
                        let type_name = match &s.ty {
                            crate::ast::Type::Custom(n) => n.as_str(),
                            crate::ast::Type::Applied(n, _) => n.as_str(),
                            _ => "",
                        };
                        self.ctx.operator_defs.get(type_name)
                            .map_or(false, |defs| defs.iter().any(|d| d.op == "InsertAt"))
                    };
                    if has_insert_strategy {
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
        // 2026-07-19: Arena system fields must never be eliminated — they're
        // accessed by emit_arena_alloc via %State field indices, not by identifiers.
        for arena_name in &["__arena_ptr", "__arena_end", "__arena_base"] {
            if let Some(idx) = self.ctx.field_index_map.get(*arena_name) {
                self.ctx.field_modes.insert(arena_name.to_string(), crate::analysis::FieldMode::Always);
            }
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

        // 2026-07-21: Update stale arena/ringbuf indices after field elimination.
        // The rebuild above may have shifted field indices; re-read them from
        // the new field_index_map to prevent out-of-bounds GEPs.
        self.arena_ptr_idx = self.ctx.field_index_map.get("__arena_ptr").copied();
        self.arena_end_idx = self.ctx.field_index_map.get("__arena_end").copied();
        self.arena_base_idx = self.ctx.field_index_map.get("__arena_base").copied();
        for (_base, rbf) in &mut self.ctx.ringbuf_inline {
            rbf.data_idx = self.ctx.field_index_map
                .get(&format!("{}_data", _base)).copied().unwrap_or(rbf.data_idx);
            rbf.head_idx = self.ctx.field_index_map
                .get(&format!("{}_head", _base)).copied().unwrap_or(rbf.head_idx);
            rbf.tail_idx = self.ctx.field_index_map
                .get(&format!("{}_tail", _base)).copied().unwrap_or(rbf.tail_idx);
            rbf.mask_idx = self.ctx.field_index_map
                .get(&format!("{}_mask", _base)).copied().unwrap_or(rbf.mask_idx);
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
            // 2026-07-23: Export wraps an inner item (defn, txn, etc.).
            TopLevel::Export(e) => self.scan_item_for_wires(&e.inner),
            _ => {}
        }
    }
}

