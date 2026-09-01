//! Tiled GEMM synthesis (plan 2026-09-01-m2-gemm M2.1).
//!
//! The .abv author writes the NAIVE matmul — one output element per item,
//! a plain foreach dot product. When the accel analysis's shape matches the
//! canonical flattened-2D form, this module synthesizes what a hand-tuner
//! would: a workgroup per TILE_M×TILE_N block of the output, A/B k-panels
//! staged through WORKGROUP SHARED MEMORY, register-blocked 4×4 FMAs per
//! invocation. The metadata that makes this derivable rather than
//! handwritten: static shapes (M/N/K consts), the proven reduction shape,
//! and the single-SSBO layout (no aliasing to disprove).
//!
//! Grid contract (this driver flattens Y — see the handoff trap list): the
//! grid is X-only; `gl_WorkGroupID.x` linearizes (tile_m, tile_n) as
//! `tile_m * tiles_x + tile_n`. LocalSize is 16×16×1 = 256 invocations,
//! each computing a 4×4 register tile → a 64×64 output tile per workgroup.
//!
//! Everything falls back: a body that does not match the canonical form
//! (or shapes not divisible by the tile) lowers to the flat naive kernel,
//! which is correct (M2.0) and merely slow. Tiling is a strategy the
//! compiler picks — never a keyword the user writes.

use super::kernel::{begin_structured_loop, end_structured_loop, CoopLoopSig};
use super::lower::fold_consts;
use crate::ast::{Expr, Statement, TopLevel};
use rspirv::dr::{Instruction, Operand};
use rspirv::spirv::{self, StorageClass, Word};
use std::collections::HashMap;

/// Tile configuration (v1): 64×64 output tile, 16×16 invocations, 4×4
/// register tile per invocation, 64-deep k-panels (16 KB shared per panel,
/// 32 KB total — fits every Vulkan-class GPU's minimum workgroup shared mem).
pub(crate) const TILE: u64 = 64;
pub(crate) const THREADS: u64 = 16;
pub(crate) const REG: u64 = 4;

/// The recognized naive-GEMM shape, with every literal the tiled synthesis
/// needs. Field names stay opaque (no Briev-type knowledge).
pub(crate) struct GemmPlan {
    pub m: i64,
    pub n: i64,
    pub k: i64,
    /// State field names (a: row-major M×K, b: row-major K×N, y: M×N).
    pub a_field: String,
    pub b_field: String,
    pub y_field: String,
}

/// Resolve a literal expression: Decimal, a const identifier, or a pure
/// arithmetic combination of those (`M * N` bounds, `0..K` ends). The
/// module-level consts are the .abv metadata that makes the tiled shape
/// statically checkable at all.
fn lit(e: &Expr, consts: &HashMap<String, i64>) -> Option<i64> {
    match e {
        Expr::Decimal(d) => Some(*d),
        Expr::Identifier(name) => consts.get(name).copied(),
        Expr::BinaryOp(kind, l, r) => {
            let (a, b) = (lit(l, consts)?, lit(r, consts)?);
            match kind {
                crate::ast::BinaryOpKind::Add => Some(a.checked_add(b)?),
                crate::ast::BinaryOpKind::Sub => Some(a.checked_sub(b)?),
                crate::ast::BinaryOpKind::Mul => Some(a.checked_mul(b)?),
                crate::ast::BinaryOpKind::Div if b != 0 => Some(a.checked_div(b)?),
                _ => None,
            }
        }
        _ => None,
    }
}

/// `mul(ident, literal)` — either operand order.
fn linear_term_of(e: &Expr, name: &str) -> Option<i64> {
    match e {
        Expr::BinaryOp(crate::ast::BinaryOpKind::Mul, l, r) => {
            if let Expr::Identifier(n) = l.as_ref() {
                if n == name {
                    if let Some(v) = lit(r, &HashMap::new()) {
                        return Some(v);
                    }
                }
            }
            if let Expr::Identifier(n) = r.as_ref() {
                if n == name {
                    if let Some(v) = lit(l, &HashMap::new()) {
                        return Some(v);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// `a_idx` must be `m*K + k` or `k + m*K` (row-major, k coefficient 1).
/// Returns K. Callers fold the expr with the const map first so `K` (an
/// identifier const in the .abv) arrives as a Decimal.
fn match_a_index(e: &Expr, m: &str, k: &str) -> Option<i64> {
    match e {
        Expr::BinaryOp(crate::ast::BinaryOpKind::Add, l, r) => {
            if let Some(v) = linear_term_of(l, m) {
                if let Expr::Identifier(k1) = r.as_ref() {
                    if k1 == k {
                        return Some(v);
                    }
                }
            }
            if let Some(v) = linear_term_of(r, m) {
                if let Expr::Identifier(k1) = l.as_ref() {
                    if k1 == k {
                        return Some(v);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// `b_idx` must be `k*N + n` or `n + k*N`. Returns N.
fn match_b_index(e: &Expr, k: &str, n: &str) -> Option<i64> {
    match e {
        Expr::BinaryOp(crate::ast::BinaryOpKind::Add, l, r) => {
            if let Some(v) = linear_term_of(l, k) {
                if let Expr::Identifier(n1) = r.as_ref() {
                    if n1 == n {
                        return Some(v);
                    }
                }
            }
            if let Some(v) = linear_term_of(r, k) {
                if let Expr::Identifier(n1) = l.as_ref() {
                    if n1 == n {
                        return Some(v);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

impl GemmPlan {
    /// Recognize the canonical naive-GEMM body in the analyzed shape.
    /// Anything that does not match exactly returns None — the flat naive
    /// kernel (correct since M2.0) remains the fallback.
    pub(crate) fn match_stmts(
        shape: &crate::analysis::accel::KernelShape,
        items: &[TopLevel],
    ) -> Option<GemmPlan> {
        let consts = module_const_map(items);
        let iv = shape.index_var.clone();

        let bound_v = lit(&fold_consts(shape.count_expr.as_ref()?, &consts), &consts)?;
        let lets = collect_let_table(&shape.kernel_stmts, &consts);
        let (m_name, n_name, n) = match_decomposition(&lets, &consts, &iv)?;
        let m = bound_v.checked_div(n)?;

        let (k_item, k, fbody) = match_foreach(&shape.kernel_stmts, &consts)?;
        let decomp = Decomp {
            m_name: &m_name,
            n_name: &n_name,
            k_item: &k_item,
            k,
            n,
        };
        let (acc, a_field, b_field) = match_reduction(&fbody, &decomp, &consts)?;
        let y_field = match_y_store(&shape.kernel_stmts, &iv, &acc)?;

        if m <= 0 || n <= 0 || k <= 0 {
            return None;
        }
        if m % TILE as i64 != 0 || n % TILE as i64 != 0 || k % TILE as i64 != 0 {
            return None;
        }
        Some(GemmPlan {
            m,
            n,
            k,
            a_field,
            b_field,
            y_field,
        })
    }
}

/// Module-level `const` literals — the .abv metadata that makes the tiled
/// shape statically checkable at all.
fn module_const_map(items: &[TopLevel]) -> HashMap<String, i64> {
    let mut consts = HashMap::new();
    for item in items {
        if let TopLevel::Constant(c) = item {
            if let Expr::Decimal(d) = &c.expr {
                consts.insert(c.name.clone(), *d);
            }
        }
    }
    consts
}

/// The node's let table (name → const-folded initializer).
fn collect_let_table(stmts: &[Statement], consts: &HashMap<String, i64>) -> HashMap<String, Expr> {
    let mut lets = HashMap::new();
    for stmt in stmts {
        if let Statement::Let { name, expr: Some(e), .. } = stmt {
            lets.insert(name.clone(), fold_consts(e, consts));
        }
    }
    lets
}

/// The flattened-2D decomposition: m = i / DN, n = i % DN (same divisor).
fn match_decomposition(
    lets: &HashMap<String, Expr>,
    consts: &HashMap<String, i64>,
    iv: &str,
) -> Option<(String, String, i64)> {
    let mut m_name: Option<String> = None;
    let mut n_name: Option<String> = None;
    let mut div: Option<i64> = None;
    for (name, e) in lets {
        if let Expr::BinaryOp(kind, l, r) = e {
            if !matches!(l.as_ref(), Expr::Identifier(x) if x == iv) {
                continue;
            }
            let d = lit(r, consts)?;
            match kind {
                crate::ast::BinaryOpKind::Div => {
                    m_name = Some(name.clone());
                    div = Some(d);
                }
                crate::ast::BinaryOpKind::Mod => {
                    n_name = Some(name.clone());
                }
                _ => {}
            }
        }
    }
    let n = div?;
    Some((m_name?, n_name?, n))
}

/// The reduction foreach: item name, literal trip count K, body clone.
fn match_foreach(
    stmts: &[Statement],
    consts: &HashMap<String, i64>,
) -> Option<(String, i64, Vec<Statement>)> {
    for stmt in stmts {
        if let Statement::Foreach { item, list, body } = stmt {
            if let Expr::Range { end, .. } = list.as_ref() {
                let end = fold_consts(end, consts);
                if let Some(k) = lit(&end, consts) {
                    return Some((item.clone(), k, body.clone()));
                }
            }
        }
    }
    None
}

/// The decomposition names + literal strides the reduction match needs.
struct Decomp<'a> {
    m_name: &'a str,
    n_name: &'a str,
    k_item: &'a str,
    k: i64,
    n: i64,
}

/// The reduction statement: acc = acc + A[m*K + k] * B[k*N + n].
fn match_reduction(
    fbody: &[Statement],
    d: &Decomp,
    consts: &HashMap<String, i64>,
) -> Option<(String, String, String)> {
    if fbody.len() != 1 {
        return None;
    }
    let Statement::Assign(lhs, rhs) = &fbody[0] else {
        return None;
    };
    let Expr::Identifier(acc) = lhs else {
        return None;
    };
    let Expr::BinaryOp(crate::ast::BinaryOpKind::Add, a, b) = rhs else {
        return None;
    };
    if !matches!(a.as_ref(), Expr::Identifier(x) if x == acc) {
        return None;
    }
    let Expr::BinaryOp(crate::ast::BinaryOpKind::Mul, l, r) = b.as_ref() else {
        return None;
    };
    let a_idx_f = fold_consts(match_index_of(l.as_ref())?, consts);
    let b_idx_f = fold_consts(match_index_of(r.as_ref())?, consts);
    let a_field = match_field_of(l.as_ref())?;
    let b_field = match_field_of(r.as_ref())?;
    let a_row_stride = match_a_index(&a_idx_f, d.m_name, d.k_item)?;
    let b_col_stride = match_b_index(&b_idx_f, d.k_item, d.n_name)?;
    if a_row_stride != d.k || b_col_stride != d.n {
        return None;
    }
    Some((acc.clone(), a_field, b_field))
}

/// The store: y[i] = acc (LHS index is the BARE counter).
fn match_y_store(stmts: &[Statement], iv: &str, acc: &str) -> Option<String> {
    for stmt in stmts {
        let Statement::Assign(lhs, Expr::Identifier(v)) = stmt else {
            continue;
        };
        if v != acc {
            continue;
        }
        let Expr::Index(of, idx) = lhs else {
            continue;
        };
        if !matches!(idx.as_ref(), Expr::Identifier(x) if x == iv) {
            continue;
        }
        if let Expr::Identifier(f) = of.as_ref() {
            return Some(f.clone());
        }
    }
    None
}

fn match_index_of(e: &Expr) -> Option<&Expr> {
    match e {
        Expr::Index(_, idx) => Some(idx),
        _ => None,
    }
}

fn match_field_of(e: &Expr) -> Option<String> {
    match e {
        Expr::Index(of, _) => match of.as_ref() {
            Expr::Identifier(f) => Some(f.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Ids the caller (emit_kernel) wires up before calling `emit_tiled`.
pub(crate) struct TiledCtx {
    pub ssbo: Word,
    pub wgid: Word,
    pub lid: Word,
    /// Workgroup-shared float[TILE*TILE] arrays, declared in the entry block.
    pub shared_a: Word,
    pub shared_b: Word,
    /// The float element type id.
    pub f32_ty: Word,
    /// SSBO member positions (state fields are name-sorted).
    pub a_member: u32,
    pub b_member: u32,
    pub y_member: u32,
    pub exit_bb: Word,
}

/// Scalar access into a vec4-typed SSBO member: `[member][idx>>2][idx&3]`.
fn ssbo_scalar_ptr(
    builder: &mut super::SpirvBuilder,
    ctx: &TiledCtx,
    member: u32,
    idx: Word,
    int_ty: Word,
) -> Word {
    let f32_ty = ctx.f32_ty;
    let ptr = builder.ptr_class(StorageClass::StorageBuffer, f32_ty);
    let member_c = builder.u32_const(member);
    let two = builder.builder.constant_bit64(int_ty, 2);
    let three = builder.builder.constant_bit64(int_ty, 3);
    let q = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::ShiftRightArithmetic,
        Some(int_ty),
        Some(q),
        vec![Operand::IdRef(idx), Operand::IdRef(two)],
    ));
    let r = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::BitwiseAnd,
        Some(int_ty),
        Some(r),
        vec![Operand::IdRef(idx), Operand::IdRef(three)],
    ));
    let p = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::AccessChain,
        Some(ptr),
        Some(p),
        vec![
            Operand::IdRef(ctx.ssbo),
            Operand::IdRef(member_c),
            Operand::IdRef(q),
            Operand::IdRef(r),
        ],
    ));
    p
}

/// i64 arithmetic over the builder.
fn i64_binop(builder: &mut super::SpirvBuilder, op: spirv::Op, int_ty: Word, a: Word, b: Word) -> Word {
    let id = builder.gen_id();
    builder.emit(Instruction::new(
        op,
        Some(int_ty),
        Some(id),
        vec![Operand::IdRef(a), Operand::IdRef(b)],
    ));
    id
}

fn i64_const(builder: &mut super::SpirvBuilder, int_ty: Word, v: u64) -> Word {
    builder.builder.constant_bit64(int_ty, v)
}

/// 2026-09-01: shared-memory addressing uses u32 — real shaders index
/// workgroup arrays with 32-bit ids, and the i64 variant is both slower
/// and (observed on device) unreliable through this driver.
fn u32_binop(builder: &mut super::SpirvBuilder, op: spirv::Op, a: Word, b: Word) -> Word {
    let ty = builder.u32_type();
    let id = builder.gen_id();
    builder.emit(Instruction::new(
        op,
        Some(ty),
        Some(id),
        vec![Operand::IdRef(a), Operand::IdRef(b)],
    ));
    id
}

fn u32_const(builder: &mut super::SpirvBuilder, v: u32) -> Word {
    builder.u32_const(v)
}

fn widen_u2i(builder: &mut super::SpirvBuilder, v: Word, int_ty: Word) -> Word {
    let ulong = builder.builder.type_int(64, 0);
    let wide = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::UConvert,
        Some(ulong),
        Some(wide),
        vec![Operand::IdRef(v)],
    ));
    let signed = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::Bitcast,
        Some(int_ty),
        Some(signed),
        vec![Operand::IdRef(wide)],
    ));
    signed
}

/// Component `c` of a uvec3 Input builtin, as raw u32.
fn builtin_comp_u(builder: &mut super::SpirvBuilder, builtin: Word, c: u32) -> Word {
    let u32_ty = builder.u32_type();
    let ptr = builder.ptr_class(StorageClass::Input, u32_ty);
    let ci = builder.u32_const(c);
    let p = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::AccessChain,
        Some(ptr),
        Some(p),
        vec![Operand::IdRef(builtin), Operand::IdRef(ci)],
    ));
    let v = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::Load,
        Some(u32_ty),
        Some(v),
        vec![Operand::IdRef(p)],
    ));
    v
}

/// Component `c` of a uvec3 Input builtin, widened u32 → i64.
fn builtin_comp(
    builder: &mut super::SpirvBuilder,
    builtin: Word,
    c: u32,
    int_ty: Word,
) -> Word {
    let u32_ty = builder.u32_type();
    let ptr = builder.ptr_class(StorageClass::Input, u32_ty);
    let ci = builder.u32_const(c);
    let p = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::AccessChain,
        Some(ptr),
        Some(p),
        vec![Operand::IdRef(builtin), Operand::IdRef(ci)],
    ));
    let v = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::Load,
        Some(u32_ty),
        Some(v),
        vec![Operand::IdRef(p)],
    ));
    let ulong = builder.builder.type_int(64, 0);
    let wide = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::UConvert,
        Some(ulong),
        Some(wide),
        vec![Operand::IdRef(v)],
    ));
    let signed = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::Bitcast,
        Some(int_ty),
        Some(signed),
        vec![Operand::IdRef(wide)],
    ));
    signed
}

fn shared_ptr(builder: &mut super::SpirvBuilder, shared: Word, idx: Word, f32_ty: Word) -> Word {
    let ptr = builder.ptr_class(StorageClass::Workgroup, f32_ty);
    let id = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::AccessChain,
        Some(ptr),
        Some(id),
        vec![Operand::IdRef(shared), Operand::IdRef(idx)],
    ));
    id
}

/// Emit the tiled body. The caller has declared the shared arrays in the
/// entry block and positions the builder at the end of it; this function
/// emits the whole kernel body and branches to `ctx.exit_bb`.
pub(crate) fn emit_tiled(
    builder: &mut super::SpirvBuilder,
    plan: &GemmPlan,
    ctx: &TiledCtx,
) -> Result<(), String> {
    let int_ty = builder.lower_type(&crate::ast::Type::int())?;
    let bool_ty = builder.lower_type(&crate::ast::Type::Bits(1))?;
    let f32_ty = ctx.f32_ty;

    let tiles_x = (plan.n / (TILE as i64)) as u64;

    let wgid_x = builtin_comp(builder, ctx.wgid, 0, int_ty);
    let lx = builtin_comp(builder, ctx.lid, 0, int_ty);
    let ly = builtin_comp(builder, ctx.lid, 1, int_ty);

    // tile_n = wgid_x % tiles_x, tile_m = wgid_x / tiles_x.
    let tiles_x_c = i64_const(builder, int_ty, tiles_x);
    let tile_n = i64_binop(builder, spirv::Op::SRem, int_ty, wgid_x, tiles_x_c);
    let tile_m = i64_binop(builder, spirv::Op::SDiv, int_ty, wgid_x, tiles_x_c);

    // tid = ly*16 + lx in U32 (shared addressing); the SSBO-side index math
    // stays i64 (widened from the u32 pieces).
    let lx_u = builtin_comp_u(builder, ctx.lid, 0);
    let ly_u = builtin_comp_u(builder, ctx.lid, 1);
    let tid = {
        let ts = u32_const(builder, THREADS as u32);
        let t = u32_binop(builder, spirv::Op::IMul, ly_u, ts);
        u32_binop(builder, spirv::Op::IAdd, t, lx_u)
    };
    let row0 = {
        let ts = i64_const(builder, int_ty, TILE);
        let a = i64_binop(builder, spirv::Op::IMul, int_ty, tile_m, ts);
        let rg = i64_const(builder, int_ty, REG);
        let ly1 = builtin_comp(builder, ctx.lid, 1, int_ty);
        let b = i64_binop(builder, spirv::Op::IMul, int_ty, ly1, rg);
        i64_binop(builder, spirv::Op::IAdd, int_ty, a, b)
    };
    let col0 = {
        let ts = i64_const(builder, int_ty, TILE);
        let a = i64_binop(builder, spirv::Op::IMul, int_ty, tile_n, ts);
        let rg = i64_const(builder, int_ty, REG);
        let b = i64_binop(builder, spirv::Op::IMul, int_ty, lx, rg);
        i64_binop(builder, spirv::Op::IAdd, int_ty, a, b)
    };

    // 16 accumulators as loop-carried phis (zero on entry, the kk-chain
    // result on the back edge).
    let zero_f = builder.float_const(32, 0.0);
    let acc_backedges: Vec<Word> = (0..REG * REG).map(|_| builder.gen_id()).collect();
    let acc_phis: Vec<(Word, Word, Word)> = acc_backedges
        .iter()
        .map(|&be| (f32_ty, zero_f, be))
        .collect();

    let sig = CoopLoopSig {
        int_ty,
        bool_ty,
        groups: (plan.k / (TILE as i64)) as i64,
    };
    let (bbs, acc_phi_ids, cond_next, _cond0, kt_phi, kt_backedge) =
        begin_structured_loop(builder, &sig, &acc_phis)?;
    // The live accumulator SSA values start as the header phi ids; after
    // the loop the PHI ids are the final values (the backedge ids are
    // body-block definitions — they do not dominate the merge block).
    let mut acc_live: Vec<Word> = acc_phi_ids.clone();

    emit_panel_loads(builder, plan, ctx, tid, kt_phi, tile_m, tile_n, int_ty, f32_ty)?;

    // Panel-visibility barrier: AcquireRelease | WorkgroupMemory (0x108),
    // scope Workgroup for both execution and memory.
    let scope = builder.u32_const(2);
    let sem = builder.u32_const(0x108);
    builder.emit(Instruction::new(
        spirv::Op::ControlBarrier,
        None,
        None,
        vec![
            Operand::IdRef(scope),
            Operand::IdRef(scope),
            Operand::IdRef(sem),
        ],
    ));
    // ── Register-tiled inner product over the panel: kk = 0..64 unrolled ──
    // a_r[u] = As[(ly*4+u)*64 + kk]; b_r[v] = Bs[kk*64 + lx*4 + v];
    // acc[u][v] = Fma(a_r[u], b_r[v], acc[u][v]).
    // a_r[u] reads shared row (ly*REG + u) — the invocation's 4-row strip.
    let a_row_base: Vec<Word> = (0..REG)
        .map(|u| {
            let rg = u32_const(builder, REG as u32);
            let ly4 = u32_binop(builder, spirv::Op::IMul, ly_u, rg);
            let uu = u32_const(builder, u as u32);
            let r = u32_binop(builder, spirv::Op::IAdd, ly4, uu);
            let ts = u32_const(builder, TILE as u32);
            u32_binop(builder, spirv::Op::IMul, r, ts)
        })
        .collect();
    // Shared B rows are TILE-local (64 wide): the FMA read column is
    // lx*4 + v, NOT the global col0 — the tile offset would walk into the
    // next shared row (the +1-column fingerprint on 4096^3).
    let b_col_base: Vec<Word> = (0..REG)
        .map(|v| {
            let rg = u32_const(builder, REG as u32);
            let lx4 = u32_binop(builder, spirv::Op::IMul, lx_u, rg);
            let cv = u32_const(builder, v as u32);
            u32_binop(builder, spirv::Op::IAdd, lx4, cv)
        })
        .collect();

    for kk in 0..TILE {
        let kk_u = u32_const(builder, kk as u32);
        let mut a_r: Vec<Word> = Vec::with_capacity(REG as usize);
        for u in 0..REG {
            let base = a_row_base[u as usize];
            let idx = u32_binop(builder, spirv::Op::IAdd, base, kk_u);
            let p = shared_ptr(builder, ctx.shared_a, idx, f32_ty);
            a_r.push(builder.load(f32_ty, p));
        }
        let mut b_r: Vec<Word> = Vec::with_capacity(REG as usize);
        let ts2 = u32_const(builder, TILE as u32);
        let b_row = u32_binop(builder, spirv::Op::IMul, kk_u, ts2);
        for v in 0..REG {
            let bc = b_col_base[v as usize];
            let idx = u32_binop(builder, spirv::Op::IAdd, b_row, bc);
            let p = shared_ptr(builder, ctx.shared_b, idx, f32_ty);
            b_r.push(builder.load(f32_ty, p));
        }
        let mut next: Vec<Word> = Vec::with_capacity((REG * REG) as usize);
        for u in 0..REG {
            for v in 0..REG {
                let i = (u * REG + v) as usize;
                let out = if kk == TILE - 1 {
                    acc_backedges[i]
                } else {
                    builder.gen_id()
                };
                builder.glsl_fma_with_id(out, f32_ty, a_r[u as usize], b_r[v as usize], acc_live[i]);
                next.push(out);
            }
        }
        acc_live = next;
    }

    // Second barrier: every invocation must finish READING the panel before
    // any invocation overwrites it with the next k-panel (WAR hazard).
    builder.emit(Instruction::new(
        spirv::Op::ControlBarrier,
        None,
        None,
        vec![
            Operand::IdRef(scope),
            Operand::IdRef(scope),
            Operand::IdRef(sem),
        ],
    ));

    end_structured_loop(builder, &sig, &bbs, kt_phi, kt_backedge, cond_next)?;

    // ── Store the 4×4 register tile: y[(row0+u)*N + col0+v] ──
    for u in 0..REG {
        for v in 0..REG {
            let i = (u * REG + v) as usize;
            let ru = i64_const(builder, int_ty, u);
            let row = i64_binop(builder, spirv::Op::IAdd, int_ty, row0, ru);
            let cv = i64_const(builder, int_ty, v);
            let col = i64_binop(builder, spirv::Op::IAdd, int_ty, col0, cv);
            let nc2 = i64_const(builder, int_ty, plan.n as u64);
            let idx = i64_binop(builder, spirv::Op::IMul, int_ty, row, nc2);
            let idx = i64_binop(builder, spirv::Op::IAdd, int_ty, idx, col);
            let ptr = ssbo_scalar_ptr(builder, ctx, ctx.y_member, idx, int_ty);
            builder.store(ptr, acc_phi_ids[i]);
        }
    }

    builder.builder.branch(ctx.exit_bb);
    Ok(())
}

/// The cooperative k-panel load phase: 256 invocations × 16 elements each
/// stage the A (row strip) and B (column strip) panels into shared memory.
/// Panel addresses are TILE-relative: A[tile_m*64 + ar][kt*64 + ac] →
/// As[flat], B[kt*64 + br][tile_n*64 + bc] → Bs[flat].
#[allow(clippy::too_many_arguments)]
fn emit_panel_loads(
    builder: &mut super::SpirvBuilder,
    plan: &GemmPlan,
    ctx: &TiledCtx,
    tid: Word,
    kt_phi: Word,
    tile_m: Word,
    tile_n: Word,
    int_ty: Word,
    f32_ty: Word,
) -> Result<(), String> {
    let elems_per_thread = TILE * TILE / (THREADS * THREADS);
    for u in 0..elems_per_thread {
        let t256 = u32_const(builder, (THREADS * THREADS) as u32);
        let uu = u32_const(builder, u as u32);
        let off = u32_binop(builder, spirv::Op::IMul, t256, uu);
        let flat = u32_binop(builder, spirv::Op::IAdd, tid, off);
        let s6 = u32_const(builder, 6);
        let ar_u = u32_binop(builder, spirv::Op::ShiftRightLogical, flat, s6);
        let m63 = u32_const(builder, 63);
        let ac_u = u32_binop(builder, spirv::Op::BitwiseAnd, flat, m63);
        let ar = widen_u2i(builder, ar_u, int_ty);
        let ac = widen_u2i(builder, ac_u, int_ty);

        // A panel: A[tile_m*64 + ar][kt*64 + ac] → As[flat]. The PANEL is
        // tile-relative — row0 is the invoking thread's strip and would
        // load shifted rows (the 64^3 identity-probe fingerprint).
        let tm64 = i64_const(builder, int_ty, TILE);
        let a_tile_row = i64_binop(builder, spirv::Op::IMul, int_ty, tile_m, tm64);
        let a_row = i64_binop(builder, spirv::Op::IAdd, int_ty, a_tile_row, ar);
        let kt64 = i64_const(builder, int_ty, TILE);
        let a_off = i64_binop(builder, spirv::Op::IMul, int_ty, kt_phi, kt64);
        let a_col = i64_binop(builder, spirv::Op::IAdd, int_ty, a_off, ac);
        let kc = i64_const(builder, int_ty, plan.k as u64);
        let a_idx = i64_binop(builder, spirv::Op::IMul, int_ty, a_row, kc);
        let a_idx = i64_binop(builder, spirv::Op::IAdd, int_ty, a_idx, a_col);
        let a_ptr = ssbo_scalar_ptr(builder, ctx, ctx.a_member, a_idx, int_ty);
        let a_val = builder.load(f32_ty, a_ptr);
        let as_p = shared_ptr(builder, ctx.shared_a, flat, f32_ty);
        builder.store(as_p, a_val);

        // B panel: B[kt*64 + br][tile_n*64 + bc] → Bs[flat].
        let s6b = u32_const(builder, 6);
        let br_u = u32_binop(builder, spirv::Op::ShiftRightLogical, flat, s6b);
        let m63b = u32_const(builder, 63);
        let bc_u = u32_binop(builder, spirv::Op::BitwiseAnd, flat, m63b);
        let br = widen_u2i(builder, br_u, int_ty);
        let bc = widen_u2i(builder, bc_u, int_ty);
        let tn64 = i64_const(builder, int_ty, TILE);
        let b_tile_col = i64_binop(builder, spirv::Op::IMul, int_ty, tile_n, tn64);
        let b_col = i64_binop(builder, spirv::Op::IAdd, int_ty, b_tile_col, bc);
        let kt64b = i64_const(builder, int_ty, TILE);
        let b_off = i64_binop(builder, spirv::Op::IMul, int_ty, kt_phi, kt64b);
        let b_row = i64_binop(builder, spirv::Op::IAdd, int_ty, b_off, br);
        let nc = i64_const(builder, int_ty, plan.n as u64);
        let b_idx = i64_binop(builder, spirv::Op::IMul, int_ty, b_row, nc);
        let b_idx = i64_binop(builder, spirv::Op::IAdd, int_ty, b_idx, b_col);
        let b_ptr = ssbo_scalar_ptr(builder, ctx, ctx.b_member, b_idx, int_ty);
        let b_val = builder.load(f32_ty, b_ptr);
        let bs_p = shared_ptr(builder, ctx.shared_b, flat, f32_ty);
        builder.store(bs_p, b_val);
    }
    Ok(())
}
