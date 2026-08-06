//! Accel analysis — GPU-deferral eligibility, cost, and decision (SPEC §9.7).
//!
//! Frontend-driven dispatch: computed once in `analyze_program`, stored in
//! `AnalysisResults.accel`, consumed by the LLVM backend as a deterministic
//! switch. The backend never re-derives these decisions (it has no `accel`
//! knowledge by design — see docs/plans/2026-08-06-accel-gpu-offload.md).
//!
//! Policy (module-level `!> accel: <value>`):
//!   - absent            → TryKeyword: only `accel`-keyword bodies, try mode
//!   - `try_all`         → every eligible body is a candidate, try mode
//!   - `force`           → keyword bodies MUST offload (strict)
//!   - `try_all_force`   → every body tried; keyword bodies forced
//!
//! Try mode: speedup must be verified (static crossover for known N, else a
//! runtime probe). Any miss is a silent CPU fallback with a remark. Force mode:
//! eligibility must prove (compile error otherwise), the speedup gate is
//! skipped, and a missing GPU at runtime is an error — never a silent fallback.
//!
//! Eligibility is a proof obligation, not a heuristic. The proof covers:
//! bound (`[i < N]`), write disjointness (array writes affine in `i`), flat
//! value types (resolved through the TypeUniverse, never by name — rules 14/18),
//! and purity (no observable/FFI side effects in kernel statements).

use crate::ast::*;
use crate::type_universe::TypeUniverse;
use crate::type_universe::operators::protocol_category;
use std::collections::{HashMap, HashSet};

/// Module-level policy resolved from `!> accel: <value>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelMode {
    /// Absent: only `accel`-keyword bodies, try mode.
    TryKeyword,
    /// `try_all`: every body, try mode.
    TryAll,
    /// `force`: keyword bodies, force mode.
    Force,
    /// `try_all_force`: every body tried; keyword bodies forced.
    TryAllForce,
}

impl AccelMode {
    /// Whether a keyword-marked body is a candidate under this mode.
    fn targets_marked(self) -> bool {
        matches!(self, AccelMode::TryKeyword | AccelMode::Force | AccelMode::TryAllForce)
    }

    /// Whether ALL bodies are candidates under this mode.
    fn targets_all(self) -> bool {
        matches!(self, AccelMode::TryAll | AccelMode::TryAllForce)
    }

    /// Whether a keyword-marked body is forced (not merely tried).
    fn forces_marked(self) -> bool {
        matches!(self, AccelMode::Force | AccelMode::TryAllForce)
    }
}

/// Resolve the module policy from `AnalysisResults.module_metadata`.
/// Unknown values fall back to the conservative default (`TryKeyword`) —
/// never an error, matching the "absent is default" rule.
pub fn resolve_mode(module_metadata: &HashMap<String, PropertyValue>) -> AccelMode {
    let value = match module_metadata.get("accel") {
        Some(PropertyValue::Identifier(s)) | Some(PropertyValue::String(s)) => s.as_str(),
        _ => return AccelMode::TryKeyword,
    };
    match value {
        "try_all" => AccelMode::TryAll,
        "force" => AccelMode::Force,
        "try_all_force" => AccelMode::TryAllForce,
        _ => AccelMode::TryKeyword,
    }
}

/// Per-body offload decision the backend consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelDecision {
    /// Dispatch to GPU unconditionally (static crossover passed, or force).
    Gpu,
    /// Runtime N — emit both paths + the auto-tuning probe.
    Probe,
    /// Try-mode miss: ineligible or unverifiable speedup → CPU + remark.
    Cpu,
}

/// The proven kernel structure of one candidate body.
#[derive(Debug, Clone)]
pub struct KernelShape {
    /// Virtual work-item index bound by `[i < N]`.
    pub index_var: String,
    /// The work-item count expression `N` (may be runtime-determined).
    pub count_expr: Option<Expr>,
    /// Statements proven safe to offload.
    pub kernel_stmts: Vec<Statement>,
    /// Statements that stay on the CPU path (loop control, observables).
    pub host_stmts: Vec<Statement>,
    /// Array state fields read by the kernel (shared, coalesced-friendly).
    pub read_buffers: Vec<String>,
    /// Array state fields written per work-item slot.
    pub write_buffers: Vec<String>,
    /// Read-only scalar state/const fields consumed by the kernel.
    pub scalar_ins: Vec<String>,
    /// Eligibility proof result.
    pub eligible: bool,
    /// Ineligibility evidence (optimization remarks).
    pub reasons: Vec<String>,
}

/// One analyzed candidate body, keyed by transaction name.
#[derive(Debug, Clone)]
pub struct AccelEntry {
    pub mode: AccelMode,
    /// True when this body is force mode (keyword-marked under force policy).
    pub forced: bool,
    pub shape: KernelShape,
    pub decision: AccelDecision,
}

/// State-field facts extracted from the program.
struct ProgramInfo {
    /// All state field names (scalar + array).
    state_fields: HashSet<String>,
    /// Array state field name → its full `Type::Vector` type.
    array_types: HashMap<String, Type>,
    /// `const Name = expr;` values (compile-time).
    consts: HashMap<String, Expr>,
    /// `const Name: Type = ...;` declared types.
    const_types: HashMap<String, Type>,
}

impl ProgramInfo {
    fn build(items: &[TopLevel]) -> ProgramInfo {
        let mut state_fields = HashSet::new();
        let mut array_types = HashMap::new();
        let mut consts = HashMap::new();
        let mut const_types = HashMap::new();
        for item in items {
            match item {
                TopLevel::StateDecl(s) => {
                    state_fields.insert(s.name.clone());
                    if let Type::Vector(_, _) = &s.ty {
                        array_types.insert(s.name.clone(), s.ty.clone());
                    }
                }
                TopLevel::Statement(stmt) => {
                    if let Statement::Let { name, ty, expr, .. } = stmt.as_ref() {
                        state_fields.insert(name.clone());
                        if let Some(Type::Vector(_, _)) = ty {
                            if let Some(ty) = ty {
                                array_types.insert(name.clone(), ty.clone());
                            }
                        }
                    }
                }
                TopLevel::Constant(c) => {
                    consts.insert(c.name.clone(), c.expr.clone());
                    const_types.insert(c.name.clone(), c.ty.clone());
                }
                _ => {}
            }
        }
        ProgramInfo { state_fields, array_types, consts, const_types }
    }

    /// Flatness of a referenced array: look up its declared type (state or
    /// const) and prove the element type is a flat scalar via the universe.
    fn array_is_flat(&self, name: &str, universe: &TypeUniverse) -> bool {
        let ty = self.array_types.get(name).or_else(|| self.const_types.get(name));
        match ty {
            Some(Type::Vector(inner, _)) => is_flat_scalar(universe, inner),
            Some(ty) => is_flat_scalar(universe, ty),
            None => false,
        }
    }
}

/// Flat scalar protocol categories: `#Int`, `#UInt`, `#Float`, `#Bool`,
/// `#Char`. `#String`/`#Data` and pointers are not flat and reject the kernel.
/// Resolved through the TypeUniverse (`protocol_category`), never by matching
/// type names (rules 14/18). `Bits(n)` is the sole physical primitive.
fn is_flat_scalar(universe: &TypeUniverse, ty: &Type) -> bool {
    match ty {
        Type::Bits(_) => true,
        Type::Vector(inner, _) => is_flat_scalar(universe, inner),
        _ => protocol_category(universe, ty)
            .map_or(false, |cat| matches!(cat.as_str(), "Int" | "UInt" | "Float" | "Bool" | "Char")),
    }
}

/// True when `expr` mentions `var` anywhere (used to require array-write
/// indices to depend on the work-item index).
fn expr_contains(expr: &Expr, var: &str) -> bool {
    match expr {
        Expr::Identifier(name) => name == var,
        Expr::BinaryOp(_, l, r) => expr_contains(l, var) || expr_contains(r, var),
        Expr::UnaryOp(_, e) => expr_contains(e, var),
        Expr::Index(a, i) => expr_contains(a, var) || expr_contains(i, var),
        Expr::Cast(e, _) => expr_contains(e, var),
        Expr::Field(o, _) => expr_contains(o, var),
        Expr::Call(_, args, _) => args.iter().any(|a| expr_contains(a, var)),
        Expr::Tuple(items) => items.iter().any(|i| expr_contains(i, var)),
        Expr::List(items) => items.iter().any(|i| expr_contains(i, var)),
        _ => false,
    }
}

/// True when `expr` is linear in `var`: `a*var + b` with constant a/b
/// (no var*var, no division by var). An identifier is always linear: either
/// `var` itself (coefficient 1) or a constant. Disjointness additionally
/// requires the index to CONTAIN `var` (see `stmt_is_kernel`).
fn is_linear_in(expr: &Expr, var: &str) -> bool {
    match expr {
        Expr::Identifier(_) | Expr::Decimal(_) | Expr::Char(_) | Expr::Bool(_) | Expr::Float(_) => true,
        Expr::BinaryOp(BinaryOpKind::Add | BinaryOpKind::Sub, l, r) => {
            is_linear_in(l, var) && is_linear_in(r, var)
        }
        Expr::BinaryOp(BinaryOpKind::Mul, l, r) => {
            // Exactly one side depends on var; the other is constant.
            (expr_contains(l, var) && !expr_contains(r, var)
                && is_linear_in(l, var) && is_linear_in(r, var))
                || (!expr_contains(l, var) && expr_contains(r, var)
                    && is_linear_in(l, var) && is_linear_in(r, var))
        }
        _ => false,
    }
}

/// Purity: kernel expressions must be pure arithmetic/data. Reject calls,
/// reflection, pointers, control flow, and any observable/FFI surface.
fn expr_is_pure(expr: &Expr) -> bool {
    match expr {
        Expr::Decimal(_) | Expr::Char(_) | Expr::Bool(_) | Expr::Float(_)
        | Expr::Quoted(_) | Expr::TaggedLiteral(_, _) | Expr::TaggedQuotedLiteral(_, _)
        | Expr::Identifier(_) => true,
        Expr::BinaryOp(_, l, r) => expr_is_pure(l) && expr_is_pure(r),
        Expr::UnaryOp(_, e) => expr_is_pure(e),
        Expr::Index(a, i) => expr_is_pure(a) && expr_is_pure(i),
        Expr::Cast(e, _) => expr_is_pure(e),
        Expr::Tuple(items) => items.iter().all(expr_is_pure),
        Expr::List(items) => items.iter().all(expr_is_pure),
        _ => false,
    }
}

/// Shared context for the kernel proof: the work-item index, the program's
/// state-field facts, and the ineligibility-evidence sink (remarks).
struct KernelCtx<'a> {
    index_var: &'a str,
    info: &'a ProgramInfo,
    reasons: &'a mut Vec<String>,
}

/// Outputs of the kernel/host partition.
struct PartitionOut {
    kernel: Vec<Statement>,
    host: Vec<Statement>,
    locals: HashSet<String>,
}

/// Partition a body into kernel statements (proven offloadable) and host
/// statements (CPU: loop control, observables). Records the disjoint-write
/// proof and rejects cross-work-item array writes.
fn partition(body: &[Statement], ctx: &mut KernelCtx<'_>) -> PartitionOut {
    let mut out = PartitionOut {
        kernel: Vec::new(),
        host: Vec::new(),
        locals: HashSet::new(),
    };
    for stmt in body {
        if stmt_is_kernel(stmt, ctx, &mut out.locals) {
            out.kernel.push(stmt.clone());
        } else {
            out.host.push(stmt.clone());
        }
    }
    out
}

/// Whether a single statement is offloadable. Host statements (loop counters,
/// term/exit/rollback, when/gate, observables, non-affine scalar writes) fall
/// through to `host`. A cross-work-item array write is NOT host material — it
/// is an ineligibility, recorded in `ctx.reasons`.
fn stmt_is_kernel(
    stmt: &Statement,
    ctx: &mut KernelCtx<'_>,
    locals: &mut HashSet<String>,
) -> bool {
    match stmt {
        Statement::Let { name, expr, .. } => {
            if let Some(e) = expr {
                if expr_is_pure(e) {
                    locals.insert(name.clone());
                    return true;
                }
            }
            false
        }
        Statement::Assign(lhs, rhs) => assign_is_kernel(lhs, rhs, ctx, locals),
        Statement::Guarded(cond, body) => {
            if !expr_is_pure(cond) {
                return false;
            }
            !body.is_empty()
                && body.iter().all(|s| stmt_is_kernel(s, ctx, locals))
        }
        Statement::Block(b) => {
            !b.is_empty() && b.iter().all(|s| stmt_is_kernel(s, ctx, locals))
        }
        _ => false,
    }
}

/// One assignment's eligibility. Array writes must target a state array at a
/// slot linear in the work-item index (disjointness). Scalar writes are kernel
/// only when the scalar is a body-local temporary; state-scalar writes (loop
/// counters, bookkeeping) belong to the CPU host.
fn assign_is_kernel(
    lhs: &Expr,
    rhs: &Expr,
    ctx: &mut KernelCtx<'_>,
    locals: &mut HashSet<String>,
) -> bool {
    match lhs {
        Expr::Index(arr, idx) => index_write_is_kernel(arr, idx, rhs, ctx),
        Expr::Identifier(name) => {
            !ctx.info.state_fields.contains(name)
                && locals.contains(name)
                && expr_is_pure(rhs)
        }
        _ => false,
    }
}

/// Eligibility of one array-slot write. The slot index must depend on the
/// work-item id (`contains`) and be linear in it (no `i*i`, no `i/j`), so no
/// two work-items write the same slot.
fn index_write_is_kernel(
    arr: &Expr,
    idx: &Expr,
    rhs: &Expr,
    ctx: &mut KernelCtx<'_>,
) -> bool {
    if !expr_is_pure(rhs) {
        return false;
    }
    let arr_name = match arr {
        Expr::Identifier(n) => n.clone(),
        _ => return false,
    };
    if !ctx.info.state_fields.contains(&arr_name) {
        ctx.reasons.push(format!("write to non-state array '{}'", arr_name));
        return false;
    }
    if expr_contains(idx, ctx.index_var) && is_linear_in(idx, ctx.index_var) {
        true
    } else {
        ctx.reasons.push(format!(
            "cross-work-item array write: '{}[...]' index must be linear in work-item id '{}'",
            arr_name, ctx.index_var
        ));
        false
    }
}

/// Prove the whole kernel and collect its buffer contracts.
fn prove_kernel(
    name: &str,
    body: &[Statement],
    contract: &Contract,
    info: &ProgramInfo,
    universe: &TypeUniverse,
) -> KernelShape {
    let mut reasons = Vec::new();
    let mut shape = KernelShape {
        index_var: String::new(),
        count_expr: None,
        kernel_stmts: Vec::new(),
        host_stmts: Vec::new(),
        read_buffers: Vec::new(),
        write_buffers: Vec::new(),
        scalar_ins: Vec::new(),
        eligible: false,
        reasons: Vec::new(),
    };

    // 1. Bound: the contract precondition must be `[i < N]`.
    let (index_var, count_expr) = match &contract.pre_condition {
        Expr::BinaryOp(BinaryOpKind::Lt, left, right)
            if matches!(left.as_ref(), Expr::Identifier(_)) =>
        {
            let i = match left.as_ref() {
                Expr::Identifier(s) => s.clone(),
                _ => unreachable!(),
            };
            (i, Some(right.as_ref().clone()))
        }
        _ => {
            reasons.push(format!(
                "accel '{}' requires a work-item bound precondition '[i < N]'",
                name
            ));
            shape.reasons = reasons;
            return shape;
        }
    };
    shape.index_var = index_var.clone();
    shape.count_expr = count_expr;

    // 2/4. Partition + purity + disjoint writes.
    let PartitionOut { kernel, host, .. } = {
        let mut ctx = KernelCtx {
            index_var: &index_var,
            info,
            reasons: &mut reasons,
        };
        partition(body, &mut ctx)
    };
    shape.kernel_stmts = kernel;
    shape.host_stmts = host;
    if shape.kernel_stmts.is_empty() {
        reasons.push(format!(
            "accel '{}' has no offloadable statements (pure, disjoint, work-item-affine)",
            name
        ));
        shape.reasons = reasons;
        return shape;
    }

    // 3. Buffer contracts: array reads (shared) + array writes (disjoint) +
    //    read-only scalars. Rejected types are collected for flatness.
    let (reads, writes, scalars) = collect_buffers(&shape.kernel_stmts, info);
    shape.read_buffers = reads;
    shape.write_buffers = writes;
    shape.scalar_ins = scalars;

    // 5. Flat types via the TypeUniverse.
    for buf in shape.read_buffers.iter().chain(shape.write_buffers.iter()) {
        if !info.array_is_flat(buf, universe) {
            reasons.push(format!(
                "accel '{}' array '{}' is not a flat scalar type (needs #Int/#UInt/#Float/#Bool/#Char)",
                name, buf
            ));
        }
    }

    shape.eligible = reasons.is_empty();
    shape.reasons = reasons;
    shape
}

/// Walk kernel statements collecting the buffer contract:
/// array reads → read_buffers, array writes → write_buffers, scalar state/const
/// reads → scalar_ins (read-only inputs). Returns sorted lists.
fn collect_buffers(
    stmts: &[Statement],
    info: &ProgramInfo,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut reads: HashSet<String> = HashSet::new();
    let mut writes: HashSet<String> = HashSet::new();
    let mut scalars: HashSet<String> = HashSet::new();
    for stmt in stmts {
        collect_stmt_buffers(stmt, info, &mut reads, &mut writes, &mut scalars);
    }
    let mut reads: Vec<String> = reads.into_iter().collect();
    let mut writes: Vec<String> = writes.into_iter().collect();
    let mut scalars: Vec<String> = scalars.into_iter().collect();
    reads.sort();
    writes.sort();
    scalars.sort();
    (reads, writes, scalars)
}

fn collect_stmt_buffers(
    stmt: &Statement,
    info: &ProgramInfo,
    reads: &mut HashSet<String>,
    writes: &mut HashSet<String>,
    scalars: &mut HashSet<String>,
) {
    match stmt {
        Statement::Let { expr, .. } => {
            if let Some(e) = expr {
                collect_expr_buffers(e, info, reads, writes, scalars);
            }
        }
        Statement::Assign(lhs, rhs) => {
            collect_expr_buffers(rhs, info, reads, writes, scalars);
            if let Expr::Index(arr, _) = lhs {
                if let Expr::Identifier(n) = arr.as_ref() {
                    writes.insert(n.clone());
                }
            }
        }
        Statement::Guarded(cond, body) => {
            collect_expr_buffers(cond, info, reads, writes, scalars);
            for s in body {
                collect_stmt_buffers(s, info, reads, writes, scalars);
            }
        }
        Statement::Block(b) => {
            for s in b {
                collect_stmt_buffers(s, info, reads, writes, scalars);
            }
        }
        _ => {}
    }
}

fn collect_expr_buffers(
    expr: &Expr,
    info: &ProgramInfo,
    reads: &mut HashSet<String>,
    writes: &mut HashSet<String>,
    scalars: &mut HashSet<String>,
) {
    match expr {
        Expr::Identifier(name) => {
            if info.state_fields.contains(name) {
                scalars.insert(name.clone());
            } else if info.consts.contains_key(name) {
                scalars.insert(name.clone());
            }
        }
        Expr::Index(arr, idx) => {
            collect_expr_buffers(arr, info, reads, writes, scalars);
            collect_expr_buffers(idx, info, reads, writes, scalars);
            if let Expr::Identifier(n) = arr.as_ref() {
                reads.insert(n.clone());
            }
        }
        Expr::BinaryOp(_, l, r) => {
            collect_expr_buffers(l, info, reads, writes, scalars);
            collect_expr_buffers(r, info, reads, writes, scalars);
        }
        Expr::UnaryOp(_, e) => collect_expr_buffers(e, info, reads, writes, scalars),
        Expr::Cast(e, _) => collect_expr_buffers(e, info, reads, writes, scalars),
        Expr::Tuple(items) | Expr::List(items) => {
            for i in items {
                collect_expr_buffers(i, info, reads, writes, scalars);
            }
        }
        _ => {}
    }
}

/// Resolve the work-item count to a compile-time constant when possible.
/// `const N = 100;` and literals are static; `get_env_int!` etc. are runtime.
fn constant_n(count_expr: &Option<Expr>, consts: &HashMap<String, Expr>) -> Option<u64> {
    match count_expr {
        Some(Expr::Decimal(n)) if *n > 0 => Some(*n as u64),
        Some(Expr::Identifier(name)) => match consts.get(name) {
            Some(Expr::Decimal(n)) if *n > 0 => Some(*n as u64),
            _ => None,
        },
        _ => None,
    }
}

/// The per-body decision: force skips the speedup gate; try mode verifies
/// statically (known N) or defers to the runtime probe (runtime N).
fn decide(shape: &KernelShape, forced: bool, info: &ProgramInfo) -> AccelDecision {
    if !shape.eligible {
        return AccelDecision::Cpu;
    }
    if forced {
        return AccelDecision::Gpu;
    }
    match constant_n(&shape.count_expr, &info.consts) {
        Some(n) => {
            // Reuse the arithmetic-intensity cost model for the crossover.
            let est = crate::analysis::gpu_cost::estimate(&shape.kernel_stmts, n);
            if n >= est.crossover_point {
                AccelDecision::Gpu
            } else {
                AccelDecision::Cpu
            }
        }
        None => AccelDecision::Probe,
    }
}

/// Analyze every candidate body. Requires a populated TypeUniverse for the
/// flat-type proof; without one, no accel decisions are produced (the LLVM
/// backend always supplies it).
pub fn analyze(
    items: &[TopLevel],
    module_metadata: &HashMap<String, PropertyValue>,
    universe: Option<&TypeUniverse>,
) -> HashMap<String, AccelEntry> {
    let mut out = HashMap::new();
    let Some(universe) = universe else {
        return out;
    };
    let mode = resolve_mode(module_metadata);
    let info = ProgramInfo::build(items);
    for item in items {
        let txn = match item {
            TopLevel::Transaction(t) => t,
            _ => continue,
        };
        let marked = txn.modifiers.iter().any(|m| m.name == "accel");
        if !(mode.targets_all() || (mode.targets_marked() && marked)) {
            continue;
        }
        let forced = mode.forces_marked() && marked;
        let shape = prove_kernel(&txn.name, &txn.body, &txn.contract, &info, universe);
        let decision = decide(&shape, forced, &info);
        out.insert(
            txn.name.clone(),
            AccelEntry { mode, forced, shape, decision },
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn txn_with(name: &str, pre: Expr, body: Vec<Statement>) -> TopLevel {
        TopLevel::Transaction(Transaction {
            name: name.to_string(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: Contract {
                pre_condition: pre,
                post_condition: Expr::Bool(true),
                watchdog: None,
                span: None,
                explicit: true,
            },
            body,
            metadata: HashMap::new(),
            derivation: None,
            modifiers: vec![Annotation { name: "accel".into(), value: None }],
            span: None,
            doc: None,
        })
    }

    fn pre_lt(var: &str, bound: Expr) -> Expr {
        Expr::BinaryOp(
            BinaryOpKind::Lt,
            Box::new(Expr::Identifier(var.to_string())),
            Box::new(bound),
        )
    }

    fn state(items: &mut Vec<TopLevel>) {
        items.push(TopLevel::StateDecl(StateDecl {
            name: "px".into(),
            ty: Type::Vector(Box::new(Type::Custom("Float".into())), vec![]),
            span: None,
        }));
        items.push(TopLevel::StateDecl(StateDecl {
            name: "dv".into(),
            ty: Type::Vector(Box::new(Type::Custom("Float".into())), vec![]),
            span: None,
        }));
        items.push(TopLevel::StateDecl(StateDecl {
            name: "nb".into(),
            ty: Type::int(),
            span: None,
        }));
    }

    fn universe() -> TypeUniverse {
        // Primordials (Int/Float/Bool/Char/...) carry Cast.#<Category> props,
        // so protocol_category resolves flatness without stdlib.
        TypeUniverse::new()
    }

    fn entry<'a>(map: &'a HashMap<String, AccelEntry>, name: &str) -> &'a AccelEntry {
        map.get(name).unwrap_or_else(|| panic!("no entry for {name}"))
    }

    // ── resolve_mode ─────────────────────────────────────────────

    #[test]
    fn mode_absent_is_try_keyword() {
        let meta = HashMap::new();
        assert_eq!(resolve_mode(&meta), AccelMode::TryKeyword);
    }

    #[test]
    fn mode_values() {
        let mut meta = HashMap::new();
        meta.insert("accel".into(), PropertyValue::Identifier("try_all".into()));
        assert_eq!(resolve_mode(&meta), AccelMode::TryAll);
        meta.insert("accel".into(), PropertyValue::Identifier("force".into()));
        assert_eq!(resolve_mode(&meta), AccelMode::Force);
        meta.insert("accel".into(), PropertyValue::Identifier("try_all_force".into()));
        assert_eq!(resolve_mode(&meta), AccelMode::TryAllForce);
        meta.insert("accel".into(), PropertyValue::Identifier("bogus".into()));
        assert_eq!(resolve_mode(&meta), AccelMode::TryKeyword);
    }

    // ── eligibility: bound ────────────────────────────────────────

    #[test]
    fn bound_requires_lt_precondition() {
        let mut items = vec![];
        state(&mut items);
        let body = vec![Statement::Assign(
            Expr::Index(Box::new(Expr::Identifier("dv".into())), Box::new(Expr::Identifier("i".into()))),
            Expr::Decimal(1),
        )];
        items.push(txn_with("ok", pre_lt("i", Expr::Identifier("nb".into())), body.clone()));
        // Non-`[i < N]` precondition → ineligible.
        items.push(txn_with("bad", Expr::Bool(true), body));
        let map = analyze(&items, &HashMap::new(), Some(&universe()));
        assert!(entry(&map, "ok").shape.eligible);
        assert!(!entry(&map, "bad").shape.eligible);
        assert!(entry(&map, "bad").shape.reasons.iter().any(|r| r.contains("[i < N]")));
    }

    // ── eligibility: write disjointness ───────────────────────────

    #[test]
    fn array_write_must_be_affine_in_index() {
        let mut items = vec![];
        state(&mut items);
        let ok_body = vec![Statement::Assign(
            Expr::Index(Box::new(Expr::Identifier("dv".into())), Box::new(Expr::Identifier("i".into()))),
            Expr::Decimal(1),
        )];
        // a[0] — constant slot written by every work-item → cross-work-item.
        let cross_body = vec![Statement::Assign(
            Expr::Index(Box::new(Expr::Identifier("dv".into())), Box::new(Expr::Decimal(0))),
            Expr::Decimal(1),
        )];
        // a[j] with free j → not affine in i → cross-work-item.
        let free_j = vec![Statement::Assign(
            Expr::Index(Box::new(Expr::Identifier("dv".into())), Box::new(Expr::Identifier("j".into()))),
            Expr::Decimal(1),
        )];
        let pre = pre_lt("i", Expr::Identifier("nb".into()));
        items.push(txn_with("affine", pre.clone(), ok_body));
        items.push(txn_with("const_slot", pre.clone(), cross_body));
        items.push(txn_with("free_j", pre, free_j));
        let map = analyze(&items, &HashMap::new(), Some(&universe()));
        assert!(entry(&map, "affine").shape.eligible);
        assert!(!entry(&map, "const_slot").shape.eligible);
        assert!(!entry(&map, "free_j").shape.eligible);
    }

    #[test]
    fn scalar_state_write_is_host_not_kernel() {
        let mut items = vec![];
        state(&mut items);
        // count = count + 1 is loop bookkeeping → host; dv[i] = 1 is kernel.
        let body = vec![
            Statement::Assign(
                Expr::Index(Box::new(Expr::Identifier("dv".into())), Box::new(Expr::Identifier("i".into()))),
                Expr::Decimal(1),
            ),
            Statement::Assign(
                Expr::Identifier("nb".into()),
                Expr::BinaryOp(BinaryOpKind::Add, Box::new(Expr::Identifier("nb".into())), Box::new(Expr::Decimal(1))),
            ),
        ];
        items.push(txn_with("t", pre_lt("i", Expr::Identifier("nb".into())), body));
        let map = analyze(&items, &HashMap::new(), Some(&universe()));
        let e = entry(&map, "t");
        assert!(e.shape.eligible);
        assert_eq!(e.shape.kernel_stmts.len(), 1);
        assert_eq!(e.shape.host_stmts.len(), 1);
    }

    // ── eligibility: purity ───────────────────────────────────────

    #[test]
    fn call_rejects_kernel() {
        let mut items = vec![];
        state(&mut items);
        let body = vec![Statement::Assign(
            Expr::Index(Box::new(Expr::Identifier("dv".into())), Box::new(Expr::Identifier("i".into()))),
            Expr::Call("println!".into(), vec![Expr::Decimal(1)], None),
        )];
        items.push(txn_with("t", pre_lt("i", Expr::Identifier("nb".into())), body));
        let map = analyze(&items, &HashMap::new(), Some(&universe()));
        assert!(!entry(&map, "t").shape.eligible);
        assert!(entry(&map, "t").shape.reasons.iter().any(|r| r.contains("no offloadable statements")));
    }

    // ── eligibility: flat types ───────────────────────────────────

    #[test]
    fn string_array_rejects_kernel() {
        let mut items = vec![];
        state(&mut items);
        // Add a String[] state array written by the kernel → not flat.
        items.push(TopLevel::StateDecl(StateDecl {
            name: "ss".into(),
            ty: Type::Vector(Box::new(Type::string()), vec![]),
            span: None,
        }));
        let body = vec![Statement::Assign(
            Expr::Index(Box::new(Expr::Identifier("ss".into())), Box::new(Expr::Identifier("i".into()))),
            Expr::Identifier("x".into()),
        )];
        items.push(txn_with("t", pre_lt("i", Expr::Identifier("nb".into())), body));
        let map = analyze(&items, &HashMap::new(), Some(&universe()));
        let e = entry(&map, "t");
        assert!(!e.shape.eligible);
        assert!(e.shape.reasons.iter().any(|r| r.contains("not a flat scalar")));
    }

    // ── decisions ─────────────────────────────────────────────────

    #[test]
    fn runtime_bound_is_probe() {
        let mut items = vec![];
        state(&mut items);
        let body = vec![Statement::Assign(
            Expr::Index(Box::new(Expr::Identifier("dv".into())), Box::new(Expr::Identifier("i".into()))),
            Expr::Decimal(1),
        )];
        // `[i < nb]` with nb a runtime state scalar → Probe (try mode).
        items.push(txn_with("t", pre_lt("i", Expr::Identifier("nb".into())), body));
        let map = analyze(&items, &HashMap::new(), Some(&universe()));
        let e = entry(&map, "t");
        assert!(e.shape.eligible);
        assert_eq!(e.decision, AccelDecision::Probe);
    }

    #[test]
    fn const_bound_below_crossover_is_cpu() {
        let mut items = vec![];
        state(&mut items);
        items.push(TopLevel::Constant(Constant {
            name: "N".into(),
            ty: Type::int(),
            expr: Expr::Decimal(4),
        }));
        let body = vec![Statement::Assign(
            Expr::Index(Box::new(Expr::Identifier("dv".into())), Box::new(Expr::Identifier("i".into()))),
            Expr::Decimal(1),
        )];
        items.push(txn_with("t", pre_lt("i", Expr::Identifier("N".into())), body));
        let map = analyze(&items, &HashMap::new(), Some(&universe()));
        // N=4 below the PCIe crossover → CPU.
        assert_eq!(entry(&map, "t").decision, AccelDecision::Cpu);
    }

    #[test]
    fn forced_body_skips_speedup_gate() {
        let mut items = vec![];
        state(&mut items);
        // const bound below crossover (would be Cpu in try mode)…
        items.push(TopLevel::Constant(Constant { name: "nb".into(), ty: Type::int(), expr: Expr::Decimal(4) }));
        // …but the body is accel-keyword-marked and the policy is force → Gpu.
        let body = vec![Statement::Assign(
            Expr::Index(Box::new(Expr::Identifier("dv".into())), Box::new(Expr::Identifier("i".into()))),
            Expr::Decimal(1),
        )];
        items.push(txn_with("t", pre_lt("i", Expr::Identifier("nb".into())), body));
        let mut meta = HashMap::new();
        meta.insert("accel".into(), PropertyValue::Identifier("force".into()));
        let map = analyze(&items, &meta, Some(&universe()));
        let e = entry(&map, "t");
        assert!(e.forced);
        assert_eq!(e.decision, AccelDecision::Gpu);
    }

    #[test]
    fn try_all_targets_unmarked_body() {
        let mut items = vec![];
        state(&mut items);
        let body = vec![Statement::Assign(
            Expr::Index(Box::new(Expr::Identifier("dv".into())), Box::new(Expr::Identifier("i".into()))),
            Expr::Decimal(1),
        )];
        // No accel modifier on the txn itself.
        let mut t = match txn_with("t", pre_lt("i", Expr::Identifier("nb".into())), body) {
            TopLevel::Transaction(t) => t,
            _ => unreachable!(),
        };
        t.modifiers.clear();
        items.push(TopLevel::Transaction(t));
        let mut meta = HashMap::new();
        meta.insert("accel".into(), PropertyValue::Identifier("try_all".into()));
        let map = analyze(&items, &meta, Some(&universe()));
        assert!(map.contains_key("t"), "try_all must target unmarked bodies");
        assert_eq!(entry(&map, "t").decision, AccelDecision::Probe);
    }

    #[test]
    fn absent_mode_targets_only_marked() {
        let mut items = vec![];
        state(&mut items);
        let body = vec![Statement::Assign(
            Expr::Index(Box::new(Expr::Identifier("dv".into())), Box::new(Expr::Identifier("i".into()))),
            Expr::Decimal(1),
        )];
        let mut t = match txn_with("t", pre_lt("i", Expr::Identifier("nb".into())), body) {
            TopLevel::Transaction(t) => t,
            _ => unreachable!(),
        };
        t.modifiers.clear();
        items.push(TopLevel::Transaction(t));
        // No module key → absent → unmarked body is not a candidate.
        let map = analyze(&items, &HashMap::new(), Some(&universe()));
        assert!(!map.contains_key("t"));
    }
}
