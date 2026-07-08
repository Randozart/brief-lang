// 2026-07-08: Phase 2C — type normalization pass
// Resolves Custom/Applied types to their concrete Bits widths using
// universe defaults. Runs after parsing, before typechecking and codegen.

use crate::ast::*;
use crate::type_universe::TypeUniverse;

/// Entry point: normalize all type annotations in a program.
pub fn normalize_types(program: &mut Program, universe: &TypeUniverse) {
    for item in &mut program.items {
        normalize_toplevel(item, universe);
    }
}

fn normalize_toplevel(item: &mut TopLevel, universe: &TypeUniverse) {
    match item {
        TopLevel::StateDecl(decl) => {
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
fn normalize_type(ty: &Type, universe: &TypeUniverse) -> Type {
    match ty {
        Type::Custom(name) => {
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
