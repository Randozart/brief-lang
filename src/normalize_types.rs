// 2026-07-08: Phase 2C — type normalization pass
// Resolves Custom/Applied types to their concrete Bits widths using
// universe defaults. Runs after parsing, before typechecking and codegen.
//
// NOTE: Only normalizes user-defined types (those NOT in the bridge table).
// Built-in types (Int, Float, Bool, etc.) are handled by the bridge table
// helpers (bit_width(), is_float_type(), etc.) and are left as Custom(name).

use crate::ast::*;
use crate::type_universe::TypeUniverse;

/// Entry point: normalize all type annotations in a program.
/// Only normalizes user-defined types not in the bridge table.
pub fn normalize_types(program: &mut Program, universe: &TypeUniverse) {
    for item in &mut program.items {
        normalize_toplevel(item, universe);
    }
}

fn normalize_toplevel(item: &mut TopLevel, universe: &TypeUniverse) {
    match item {
        TopLevel::StateDecl(decl) => {
            // 2026-07-11: Phase 5 — detect deferred literals BEFORE type normalization.
            // The type name (e.g. "Color") is needed to look up the codec, but
            // normalize_type may replace Custom("Color") with Bits(64), losing info.
            if let Some(ref mut expr) = decl.expr {
                if let Expr::Identifier(name) = &*expr {
                    let type_name = match &decl.ty {
                        Type::Custom(n) => n.as_str(),
                        _ => "",
                    };
                    if !type_name.is_empty() {
                        if let Some(rt) = universe.get(type_name) {
                            if let Some(ref codec_name) = rt.codec {
                                if universe.codecs.get(codec_name).and_then(|c| c.parse_handler.as_ref()).is_some() {
                                    let text = name.to_string();
                                    let expected_type = decl.ty.clone();
                                    *expr = Expr::DeferredLiteral { text, expected_type: Box::new(expected_type) };
                                }
                            }
                        }
                    }
                }
            }
            decl.ty = normalize_type(&decl.ty, universe);
        }
        TopLevel::Constant(constant) => {
            constant.ty = normalize_type(&constant.ty, universe);
        }
        TopLevel::Signature(sig) => {
            for (_, ty) in &mut sig.params {
                *ty = normalize_type(ty, universe);
            }
            if let Some(ref mut ot) = sig.output_type {
                normalize_output_type(ot, universe);
            }
        }
        TopLevel::Definition(defn) => {
            for (_, ty) in &mut defn.parameters {
                *ty = normalize_type(ty, universe);
            }
            for ty in &mut defn.outputs {
                *ty = normalize_type(ty, universe);
            }
            if let Some(ref mut ot) = defn.output_type {
                normalize_output_type(ot, universe);
            }
        }
        TopLevel::Transaction(txn) => {
            for (_, ty) in &mut txn.parameters {
                *ty = normalize_type(ty, universe);
            }
            for ty in &mut txn.outputs {
                *ty = normalize_type(ty, universe);
            }
            if let Some(ref mut ot) = txn.output_type {
                normalize_output_type(ot, universe);
            }
        }
        TopLevel::Struct(struct_def) => {
            for field in &mut struct_def.fields {
                field.ty = normalize_type(&field.ty, universe);
            }
        }
        TopLevel::Test { item, .. } | TopLevel::Fuzzed { item, .. } => {
            normalize_toplevel(item, universe);
        }
        _ => {}
    }
}

/// Normalize types inside expressions (recursive).
/// 2026-07-11: Phase 5 — handles Expr::DeferredLiteral.
fn normalize_expr(expr: &mut Expr, universe: &TypeUniverse) {
    match expr {
        Expr::DeferredLiteral { expected_type, .. } => {
            let normalized = normalize_type(expected_type, universe);
            *expected_type = Box::new(normalized);
        }
        _ => {}
    }
}

fn normalize_output_type(ot: &mut OutputType, universe: &TypeUniverse) {
    match ot {
        OutputType::Single(ty) => *ty = normalize_type(ty, universe),
        OutputType::Union(types) | OutputType::Tuple(types) => {
            for t in types {
                normalize_output_type(t, universe);
            }
        }
        OutputType::Array(ty) => *ty = Box::new(normalize_type(ty, universe)),
        OutputType::Named(_, inner) => normalize_output_type(inner, universe),
    }
}

/// Normalize a type: resolve Custom(name) → Applied(name, [default]) using universe.
/// Skips built-in types that are in the bridge table (Int, Float, Bool, etc.).
fn normalize_type(ty: &Type, universe: &TypeUniverse) -> Type {
    match ty {
        Type::Custom(name) => {
            // 2026-07-08: Phase 2C — skip built-in types (handled by bridge tables).
            if Type::bit_width_for_name(name).is_some() {
                return ty.clone();
            }
            if let Some(rt) = universe.get(name) {
                if let Some((_, default_ty)) = rt.default_params.first() {
                    return Type::Applied(name.clone(), vec![default_ty.clone()]);
                }
                if rt.bytes > 0 {
                    return Type::Bits(rt.bytes * 8);
                }
            }
            ty.clone()
        }
        Type::Applied(name, args) => {
            let resolved_args: Vec<Type> = args.iter().map(|a| normalize_type(a, universe)).collect();
            Type::Applied(name.clone(), resolved_args)
        }
        Type::Constrained(inner, br) => {
            Type::Constrained(Box::new(normalize_type(inner, universe)), br.clone())
        }
        _ => ty.clone(),
    }
}

/// Convert string literals to struct instances in String-typed assignments.
/// "hello" → String { ptr: &"hello", len: 5, codec: 0 }
pub fn desugar_string_literals(program: &mut Program, universe: &TypeUniverse) {
    for item in &mut program.items {
        desugar_toplevel_strings(item, universe);
    }
}

fn desugar_toplevel_strings(item: &mut TopLevel, universe: &TypeUniverse) {
    match item {
        TopLevel::StateDecl(decl) => {
            if decl.ty == Type::string() {
                if let Some(expr) = &mut decl.expr {
                    if let Expr::String(s) = expr {
                        *expr = make_string_struct(s.clone());
                    }
                }
            }
        }
        TopLevel::Constant(constant) => {
            if constant.ty == Type::string() {
                if let Expr::String(s) = &constant.expr {
                    constant.expr = make_string_struct(s.clone());
                }
            }
        }
        _ => {}
    }
}

fn make_string_struct(s: String) -> Expr {
    let len = s.len() as i64;
    Expr::StructInstance("String".to_string(), vec![
        ("ptr".to_string(), Expr::String(s)),
        ("len".to_string(), Expr::Integer(len)),
        ("codec".to_string(), Expr::Integer(0)),
    ])
}
