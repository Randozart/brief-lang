//! `.abv` standalone runtime generation (2026-08-31, plan
//! abv-gpu-by-default item 4): an Accelerator Briev Volume is PURE GPU code
//! — the compile emits one `.spv` per kernel PLUS a self-contained C runner
//! that drives them.
//!
//! The runner IS the .abv node graph: state = the kernel SSBO projection,
//! each eligible node = a kernel dispatched resident-mode, each scalar-only
//! node = a host-side body (the phase machine), the loop = declared node
//! order until convergence (a full pass fires nothing).
//!
//! v1 surface (everything else is a helpful gen-time error naming the fix):
//! - pre-conditions over scalar state + literals (`i < nb`, `phase == 1`)
//! - `BeginProgram` markers → true (the counter fast-forward terminates the
//!   node: after the pass, the host sets `i = N` so the pre goes false)
//! - scalar-only bodies for non-kernel nodes (assignments + term)
//! - `get_env_int!("K")` initializers, literal initializers
//! - constant-indexed array reads in conditions/prints (observables)
//!
//! Undo: delete this module and its call sites in compile.rs / main.rs.

use crate::analysis::accel::AccelDecision;
use crate::analysis::accel::AccelEntry;
use crate::ast::{BinaryOpKind, Expr, Statement, TopLevel, Type, UnaryOpKind};
use crate::backend::spirv::lower::collect_state_fields;
use crate::backend::spirv::SpirvBuilder;
use crate::type_universe::TypeUniverse;

/// One SSBO member: name, byte offset in the projection, element size,
/// element count, and whether it is an array.
#[derive(Debug, Clone)]
pub struct RunnerField {
    pub name: String,
    pub offset: u64,
    pub elem_bytes: u32,
    pub count: u64,
    pub is_array: bool,
    pub type_is_float: bool,
}

/// A kernel the runner dispatches: node name, embedded SPIR-V, the index
/// variable to fast-forward, and the work-item count expression.
pub struct RunnerKernel {
    pub name: String,
    pub spirv: Vec<u8>,
    pub index_var: String,
    pub count_expr: Expr,
    /// 2D dispatch width (None = 1D). Mirrors `KernelShape::work_cols` —
    /// the runner must dispatch the SAME geometry the blob was built for.
    pub work_cols: Option<u64>,
    /// Cooperative row kernel (plan 2026-09-01-cooperative-row-kernels):
    /// dispatch nx = 32 lanes x ny = rows.
    pub cooperative: bool,
}

/// The SSBO layout EXACTLY as the kernel sees it (name-sorted, real element
/// widths) — derived with the same builder helpers the emitter uses so the
/// two can never drift.
pub fn ssbo_layout(
    items: &[TopLevel],
    universe: &TypeUniverse,
    int_bits: u64,
) -> Result<Vec<RunnerField>, String> {
    let mut sb = SpirvBuilder::new().with_universe(universe, int_bits);
    let mut fields = collect_state_fields(items);
    fields.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out = Vec::with_capacity(fields.len());
    let mut offset: u64 = 0;
    for f in fields {
        let (elem, count, is_array, is_float) = match &f.ty {
            Type::Vector(inner, dims) => {
                let elems: u64 = dims
                    .iter()
                    .map(|d| match d {
                        crate::ast::Dimension::Anonymous(n) => *n as u64,
                        crate::ast::Dimension::Named(_, n) => *n as u64,
                    })
                    .product::<u64>()
                    .max(1);
                let e = sb.scalar_storage_bytes(inner.as_ref())?;
                let flt = sb.is_float_type(inner.as_ref())?;
                (e, elems, true, flt)
            }
            other => {
                let e = sb.scalar_storage_bytes(other)?;
                let flt = sb.is_float_type(other)?;
                (e, 1, false, flt)
            }
        };
        out.push(RunnerField {
            name: f.name.clone(),
            offset,
            elem_bytes: elem,
            count,
            is_array,
            type_is_float: is_float,
        });
        offset += elem as u64 * count;
    }
    Ok(out)
}

fn c_ident(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn field_by_name<'a>(fields: &'a [RunnerField], name: &str) -> Option<&'a RunnerField> {
    fields.iter().find(|f| f.name == name)
}

/// Generate a C expression reading scalars / constant-indexed array
/// elements. Err names the unsupported construct (v1 surface).
fn emit_scalar_read(
    e: &Expr,
    fields: &[RunnerField],
    consts: &std::collections::HashMap<String, Expr>,
    out: &mut String,
) -> Result<(), String> {
    match e {
        Expr::Decimal(n) => {
            out.push_str(&format!("(long long){}", n));
            Ok(())
        }
        Expr::Float(v) => {
            out.push_str(&format!("(double){:e}", v));
            Ok(())
        }
        Expr::Bool(b) => {
            out.push_str(if *b { "1" } else { "0" });
            Ok(())
        }
        Expr::Identifier(name) => match field_by_name(fields, name) {
            Some(f) if !f.is_array => {
                let t = if f.type_is_float { "double" } else { "long long" };
                out.push_str(&format!("(*({}*)(state + {}))", t, f.offset));
                Ok(())
            }
            Some(f) => Err(format!(
                "scalar expression reads array '{}' — conditions and counts \
                 read scalar state only",
                f.name
            )),
            None => match consts.get(name) {
                Some(ce) => emit_scalar_read(ce, fields, consts, out),
                None => Err(format!("unknown state field or const '{}'", name)),
            },
        },
        Expr::BinaryOp(kind, l, r) => {
            let op = match kind {
                BinaryOpKind::Add => " + ",
                BinaryOpKind::Sub => " - ",
                BinaryOpKind::Mul => " * ",
                BinaryOpKind::Div => " / ",
                BinaryOpKind::Mod => " % ",
                BinaryOpKind::Lt => " < ",
                BinaryOpKind::Gt => " > ",
                BinaryOpKind::Le => " <= ",
                BinaryOpKind::Ge => " >= ",
                BinaryOpKind::Eq => " == ",
                BinaryOpKind::Neq => " != ",
                BinaryOpKind::And => " && ",
                BinaryOpKind::Or => " || ",
                BinaryOpKind::BitAnd => " & ",
                BinaryOpKind::BitOr => " | ",
                BinaryOpKind::BitXor => " ^ ",
                BinaryOpKind::Shl => " << ",
                BinaryOpKind::Shr => " >> ",
                BinaryOpKind::Concat => return Err("string concat in a scalar condition".into()),
            };
            out.push('(');
            emit_scalar_read(l, fields, consts, out)?;
            out.push_str(op);
            emit_scalar_read(r, fields, consts, out)?;
            out.push(')');
            Ok(())
        }
        Expr::UnaryOp(kind, e) => {
            let op = match kind {
                UnaryOpKind::Neg => "(-",
                UnaryOpKind::Not => "(!",
                UnaryOpKind::BitNot => "(~",
            };
            out.push_str(op);
            emit_scalar_read(e, fields, consts, out)?;
            out.push(')');
            Ok(())
        }
        Expr::BeginProgram => {
            // The counter fast-forward makes `[i < N]` false after the pass;
            // the entry marker adds nothing to termination here.
            out.push_str("1");
            Ok(())
        }
        Expr::Index(obj, idx) => {
            let Some(fname) = field_name_of_expr(obj) else {
                return Err("indexed read of a non-field expression".into());
            };
            let Some(fd) = field_by_name(fields, fname) else {
                return Err(format!("unknown array '{}'", fname));
            };
            let Expr::Decimal(k) = idx.as_ref() else {
                return Err(format!(
                    "array '{}' read with a non-constant index in a scalar \
                     expression (v1 reads constants only)",
                    fname
                ));
            };
            let t = if fd.type_is_float { "double" } else { "long long" };
            out.push_str(&format!(
                "(*({}*)(state + {} + {} * {}))",
                t, fd.offset, k, fd.elem_bytes
            ));
            Ok(())
        }
        other => Err(format!(
            "unsupported scalar-expression construct ({:?}) — the runner v1 \
             evaluates literals, state fields, arithmetic, comparisons, logic",
            std::mem::discriminant(other)
        )),
    }
}

fn field_name_of_expr(e: &Expr) -> Option<&str> {
    match e {
        Expr::Identifier(n) => Some(n),
        _ => None,
    }
}

fn emit_host_stmt(
    s: &Statement,
    fields: &[RunnerField],
    consts: &std::collections::HashMap<String, Expr>,
    out: &mut String,
    exited: &mut bool,
) -> Result<(), String> {
    match s {
        Statement::Assign(lhs, rhs) => {
            let Some(name) = field_name_of_expr(lhs) else {
                return Err("assignment to a non-field target".into());
            };
            let Some(fd) = field_by_name(fields, name) else {
                return Err(format!("assignment to unknown field '{}'", name));
            };
            if fd.is_array {
                return Err(format!(
                    "array '{}' assignment in a host body — arrays are \
                     kernel-owned in .abv",
                    name
                ));
            }
            let t = if fd.type_is_float { "double" } else { "long long" };
            out.push_str(&format!("      *({}*)(state + {}) = ", t, fd.offset));
            emit_scalar_read(rhs, fields, consts, out)?;
            out.push_str(";\n");
            Ok(())
        }
        Statement::Term(_) | Statement::EndProgram(_) => {
            *exited = true;
            Ok(())
        }
        other => Err(format!(
            "host body statement ({:?}) outside the runner v1 surface",
            std::mem::discriminant(other)
        )),
    }
}

/// Emit the full self-contained runner C source. `kernels` = one entry per
/// eligible node; each SPIR-V binary is a separate module whose entry point
/// is "main".
pub fn emit_runner(
    program: &[TopLevel],
    universe: &TypeUniverse,
    int_bits: u64,
    kernels: &[RunnerKernel],
) -> Result<String, String> {
    let fields = ssbo_layout(program, universe, int_bits)?;
    // Module consts (literals only) usable in conditions, counts and bodies.
    let mut consts: std::collections::HashMap<String, Expr> = Default::default();
    for item in program {
        if let TopLevel::Constant(c) = item {
            if matches!(c.expr, Expr::Decimal(_) | Expr::Float(_)) {
                consts.insert(c.name.clone(), c.expr.clone());
            }
        }
    }
    let total: u64 = fields.iter().map(|f| f.elem_bytes as u64 * f.count).sum();
    let mut out = String::new();

    out.push_str("// Generated by brievc - .abv standalone runner. Do not edit.\n");
    out.push_str("#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n#include <stdint.h>\n\n");
    out.push_str("#include \"briev_accel_rt.c\"\n\n");
    out.push_str(&format!("static unsigned char state[{}];\n", total + 64));
    for f in &fields {
        let t = if f.type_is_float { "double" } else { "long long" };
        out.push_str(&format!(
            "#define S_{} (*({}*)(state + {}))\n",
            c_ident(&f.name),
            t,
            f.offset
        ));
    }
    for (i, k) in kernels.iter().enumerate() {
        out.push_str(&format!("static const uint8_t k{}[] = {{", i));
        for (j, b) in k.spirv.iter().enumerate() {
            if j % 20 == 0 {
                out.push('\n');
            }
            out.push_str(&format!("{},", b));
        }
        out.push_str("\n};\n");
        out.push_str(&format!(
            "static const uint32_t k{}_len = {}u;\n",
            i,
            k.spirv.len()
        ));
    }
    out.push_str("static BrievField fields[] = {\n");
    for f in &fields {
        out.push_str(&format!(
            "    {{ \"{}\", {}, {}, {}, {}, {} }},\n",
            f.name,
            if f.is_array { 1 } else { 2 },
            f.offset,
            f.elem_bytes,
            f.count,
            if f.is_array { 1 } else { 0 }
        ));
    }
    out.push_str("};\n");
    out.push_str("static BrievKernelDesc descs[] = {\n");
    for (i, k) in kernels.iter().enumerate() {
        out.push_str(&format!(
            "    {{ \"{}\", k{}, k{}_len, {}, fields }},\n",
            c_ident(&k.name),
            i,
            i,
            fields.len()
        ));
    }
    out.push_str(&format!(
        "}};\nstatic const uint32_t N_KERNELS = {};\n",
        kernels.len()
    ));

    // seed_state: literal + get_env_int! initializers.
    out.push_str("static void seed_state(void) {\n");
    for item in program {
        let stmt = match item {
            TopLevel::Statement(s) => s.as_ref(),
            _ => continue,
        };
        let Statement::Let { name, expr: Some(e), .. } = stmt else {
            continue;
        };
        let Some(fd) = field_by_name(&fields, name) else {
            continue;
        };
        if fd.is_array {
            continue;
        }
        let t = if fd.type_is_float { "double" } else { "long long" };
        match e {
            Expr::Decimal(n) => {
                out.push_str(&format!("  *({}*)(state + {}) = {}LL;\n", t, fd.offset, n));
            }
            Expr::Float(v) => {
                out.push_str(&format!("  *({}*)(state + {}) = {:e};\n", t, fd.offset, v));
            }
            Expr::Call(cname, args, _)
                if cname == "get_env_int!" || cname == "get_env_int#" =>
            {
                let key = match args.first() {
                    Some(Expr::Quoted(q)) => String::from_utf8_lossy(q).to_string(),
                    _ => continue,
                };
                out.push_str(&format!(
                    "  {{ const char* e = getenv(\"{}\"); *({}*)(state + {}) = e ? atoll(e) : 0; }}\n",
                    key, t, fd.offset
                ));
            }
            _ => {}
        }
    }
    out.push_str("}\n\n");

    // Scheduler: one pass per iteration, declared node order, exit on
    // convergence. Kernel nodes dispatch resident-mode and fast-forward
    // their counter; host nodes run their scalar bodies.
    out.push_str("int main(void) {\n");
    out.push_str("  seed_state();\n");
    out.push_str(
        "  if (!briev_accel_init(descs, N_KERNELS)) { fprintf(stderr, \"briev: no GPU device available\\n\"); return 1; }\n",
    );
    out.push_str("  long guard = 0;\n");
    out.push_str("  for (;;) {\n");
    out.push_str(
        "    if (++guard > 2000000000L) { fprintf(stderr, \"briev: run cap reached\\n\"); break; }\n",
    );
    out.push_str("    int fired = 0;\n");
    let mut done_label_used = false;
    for item in program {
        let TopLevel::Transaction(t) = item else {
            continue;
        };
        let name = &t.name;
        let mut pre = String::new();
        emit_scalar_read(&t.contract.pre_condition, &fields, &consts, &mut pre)?;
        let kidx = kernels.iter().position(|k| k.name == *name);
        if let Some(k) = kidx.map(|i| &kernels[i]) {
            // KERNEL node: dispatch + counter fast-forward (the pass covers
            // every work item, so `i = N` makes the pre false next pass).
            emit_kernel_node(&mut out, t, k, kidx.unwrap(), &fields, &consts);
            continue;
        }
        // HOST node: scalar body.
        let mut body = String::new();
        let mut exited = false;
        for s in &t.body {
            emit_host_stmt(s, &fields, &consts, &mut body, &mut exited)?;
            if exited {
                break;
            }
        }
        out.push_str(&format!("    // host node '{}'\n", name));
        out.push_str(&format!("    if ({}) {{\n", pre));
        out.push_str(&format!("      fired = 1;\n{}\n", body));
        if exited {
            out.push_str("      goto done;\n");
            done_label_used = true;
        }
        out.push_str("    }\n");
    }
    out.push_str("    if (!fired) break;\n");
    out.push_str("  }\n");
    if done_label_used {
        out.push_str("done:\n");
    }
    // Observability: dump scalar state.
    for f in &fields {
        if f.is_array {
            continue;
        }
        if f.type_is_float {
            out.push_str(&format!(
                "  printf(\"{} = %f\\n\", S_{});\n",
                f.name,
                c_ident(&f.name)
            ));
        } else {
            out.push_str(&format!(
                "  printf(\"{} = %lld\\n\", S_{});\n",
                f.name,
                c_ident(&f.name)
            ));
        }
    }
    out.push_str("  briev_accel_shutdown();\n  return 0;\n}\n");
    Ok(out)
}

/// Build the per-kernel list for emit_runner: one module per eligible node
/// (entry "main"), plus its index var and work-item count.
pub fn build_kernels(
    program: &[TopLevel],
    universe: &TypeUniverse,
    int_bits: u64,
    entries: &std::collections::HashMap<String, AccelEntry>,
) -> Result<Vec<RunnerKernel>, String> {
    let mut out = Vec::new();
    // .abv is PURE GPU: every eligible body is a kernel. The Gpu/Probe/Cpu
    // decision (a .bv offload concept — it compares against a CPU lane) does
    // not apply to a standalone volume with no CPU.
    let _ = AccelDecision::Cpu;
    let mut names: Vec<&String> = entries
        .iter()
        .filter(|(_, e)| e.shape.eligible)
        .map(|(n, _)| n)
        .collect();
    names.sort();
    for name in names {
        let e = &entries[name];
        let mut sb = SpirvBuilder::new().with_universe(universe, int_bits);
        let cooperative = e.shape.reduction.is_some()
            && crate::config_tuning::ir_lowering().spirv_row_cooperative;
        crate::backend::spirv::kernel::emit_kernel(&mut sb, "main", &e.shape, program, cooperative)?;
        out.push(RunnerKernel {
            name: name.clone(),
            spirv: sb.build()?,
            index_var: e.shape.index_var.clone(),
            count_expr: e.shape.count_expr.clone().unwrap_or(Expr::Decimal(0)),
            work_cols: e.shape.work_cols,
            cooperative: e.shape.reduction.is_some()
                && crate::config_tuning::ir_lowering().spirv_row_cooperative,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod runner_tests {
    use super::*;
    use std::collections::HashMap;

    fn scalar_field(name: &str, offset: u64) -> RunnerField {
        RunnerField {
            name: name.to_string(),
            offset,
            elem_bytes: 8,
            count: 1,
            is_array: false,
            type_is_float: false,
        }
    }

    fn lt(lhs: &str, rhs: &str) -> Expr {
        Expr::BinaryOp(
            BinaryOpKind::Lt,
            Box::new(Expr::Identifier(lhs.to_string())),
            Box::new(Expr::Identifier(rhs.to_string())),
        )
    }

    // TEMP: 2026-08-31: regression guard for the multi-const runner
    // fast-forward repro (handoff §5.5: "N2 resolved as NB"). Verified
    // non-reproducing end-to-end; these tests pin the correct behavior.
    // Remove when the runner's const handling gains a real proof pass.
    #[test]
    fn multi_const_bounds_resolve_each_const_distinctly() {
        let fields = vec![scalar_field("i", 0), scalar_field("j", 8)];
        let mut consts: HashMap<String, Expr> = HashMap::new();
        consts.insert("NB".to_string(), Expr::Decimal(4096));
        consts.insert("N2".to_string(), Expr::Decimal(16777216));

        let mut out = String::new();
        emit_scalar_read(&lt("i", "N2"), &fields, &consts, &mut out).unwrap();
        assert!(out.contains("16777216"), "N2 misresolved: {out}");
        assert!(!out.contains("4096"), "N2 resolved as NB: {out}");

        let mut out = String::new();
        emit_scalar_read(&lt("j", "NB"), &fields, &consts, &mut out).unwrap();
        assert!(out.contains("4096"), "NB misresolved: {out}");
        assert!(!out.contains("16777216"), "NB resolved as N2: {out}");
    }

    #[test]
    fn unknown_const_is_a_named_error_not_a_wrong_value() {
        let fields = vec![scalar_field("i", 0)];
        let consts: HashMap<String, Expr> = HashMap::new();
        let mut out = String::new();
        let err = emit_scalar_read(&lt("i", "N2"), &fields, &consts, &mut out)
            .expect_err("unknown const must error");
        assert!(err.contains("N2"), "error must name the const: {err}");
    }
}

/// Emit one KERNEL scheduler node: the pre-condition gate, the geometry
/// dispatch (see `dispatch_geometry_stmt`), and the counter fast-forward
/// (the pass covers every work item, so `i = N` makes the pre false next
/// pass).
fn emit_kernel_node(
    out: &mut String,
    t: &crate::ast::top::Transaction,
    k: &RunnerKernel,
    kidx: usize,
    fields: &[RunnerField],
    consts: &std::collections::HashMap<String, Expr>,
) {
    let name = &t.name;
    let mut pre = String::new();
    emit_scalar_read(&t.contract.pre_condition, fields, consts, &mut pre)
        .expect("kernel pre-condition lowers");
    let mut count_c = String::new();
    emit_scalar_read(&k.count_expr, fields, consts, &mut count_c)
        .expect("kernel count lowers");
    let ci = c_ident(name);
    out.push_str(&format!("    // kernel node '{}'\n", name));
    out.push_str(&format!("    if ({}) {{\n", pre));
    out.push_str(&format!(
        "      fired = 1;\n      long long n_{} = {};\n",
        ci, count_c
    ));
    out.push_str(&dispatch_geometry_stmt(k, kidx, &ci));
    out.push_str(&format!("      S_{} = n_{};\n", c_ident(&k.index_var), ci));
    out.push_str("    }\n");
}

/// The C dispatch statement for one kernel node, by blob geometry
/// (plan 2026-08-31-gpu-next §2b + 2026-09-01-cooperative-row-kernels):
/// cooperative rows (32 lanes × rows), 2D cols×rows, or the flat 1D
/// fallback. Coverage is identical in all three; only the hardware routing
/// of the work-item id differs.
fn dispatch_geometry_stmt(k: &RunnerKernel, kidx: usize, ci: &str) -> String {
    if k.cooperative {
        // One 32-lane workgroup per row. The driver's 2D launch takes
        // (nx = x work items, ny = workgroup rows) and dispatches
        // ceil(nx/local_x) * ny workgroups — with the kernel's LocalSize 32
        // and nx = 32 that is exactly ny = n one-per-row workgroups.
        // 2026-09-01: was `(n + 31) / 32` rows, which launched 32x too few
        // workgroups under the local_x-divided geometry (128 of 4096 rows).
        return format!(
            "      if (n_{ci} > 0 && !briev_accel_launch_resident_2d({}, state, 32, n_{ci})) {{ fprintf(stderr, \"briev: dispatch failed\\n\"); return 1; }}\n",
            kidx
        );
    }
    if let Some(cols) = k.work_cols {
        return format!(
            "      long long rows_{ci} = (n_{ci} + {cols} - 1) / {cols};\n      if (n_{ci} > 0 && !briev_accel_launch_resident_2d({}, state, {cols}, rows_{ci})) {{ fprintf(stderr, \"briev: dispatch failed\\n\"); return 1; }}\n",
            kidx
        );
    }
    format!(
        "      if (n_{ci} > 0 && !briev_accel_launch_resident({kidx}, state, n_{ci})) {{ fprintf(stderr, \"briev: dispatch failed\\n\"); return 1; }}\n"
    )
}
