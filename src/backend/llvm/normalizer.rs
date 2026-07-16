// ── LLVM Normalizer — AST Annotation Pass ─────────────────────────────
// 2026-07-14: Walks the AST and attaches llvm_type to every type reference.
// Backend never reads config files or matches on primitive/bytes.

use crate::ast::*;
use crate::backend::normalizer;
use crate::config::{derive_llvm_type, OpConfig, TypeConfig};
use crate::type_universe::TypeUniverse;

/// 2026-07-14: Normalize the AST for LLVM backend emission.
/// Attaches llvm_type property to every ResolvedType in the universe.
/// For types with fixed-width layout, parses the pattern and attaches
/// field-level bit offset annotations. For melds with layout mappings,
/// synthesizes bit-shuffle instructions.
pub fn normalize(items: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> Result<(), String> {
    let prim_config = TypeConfig::load();

    // Attach llvm_type to every type
    for rt in universe.types.values_mut() {
        let prim = rt.primitive();
        let llvm_ty = derive_llvm_type(prim, rt.bytes, &prim_config);
        rt.properties.insert("llvm_type".into(), PropertyValue::String(llvm_ty));

        // 2026-07-14: Parse layout pattern and attach field annotations
        if let Some(PropertyValue::String(layout_str)) = rt.properties.get("layout") {
            if let Ok(pat) = crate::bvir::layout::parse_layout_pattern(layout_str) {
                attach_layout_fields(rt, &pat);
            }
        }
    }

    // 2026-07-14: Process meld layout mappings — synthesize bit-shuffles
    for item in items.iter() {
        if let TopLevel::Meld(m) = &item {
            synthesize_meld_shuffle(m, universe)?;
        }
    }

    // 2026-07-16: P0 — register meld declarations in TypeUniverse for bidirectional lookup.
    // This is the critical wiring that populates universe.melds (previously always empty).
    // Without this, find_meld() returns None and all meld-dependent features are dead.
    for item in items.iter() {
        if let TopLevel::Meld(m) = &item {
            // Convert layout bindings to MeldRouteDef entries.
            // bindings["layout.<src_field>"] = "<dst_field>" → route
            //   with accessor = dst_field (field on partner type)
            //   and dest_expr = Field(Ident(source_type), src_field)
            let mut routes = Vec::new();
            for (key, val) in &m.bindings {
                if let Some(src_field) = key.strip_prefix("layout.") {
                    routes.push(MeldRouteDef {
                        accessor: val.clone(),
                        dest_expr: Expr::Field(
                            Box::new(Expr::Identifier(m.name.clone())),
                            src_field.to_string(),
                        ),
                    });
                }
            }
            let decl = MeldDeclaration {
                name_a: m.name.clone(),
                name_b: m.target.clone(),
                routes,
                span: m.span.clone(),
            };
            // Store both orderings for bidirectional O(1) lookup.
            universe.melds.insert(
                (decl.name_a.clone(), decl.name_b.clone()),
                decl.clone(),
            );
            universe.melds.insert(
                (decl.name_b.clone(), decl.name_a.clone()),
                decl,
            );
        }
    }

    // Validate intrinsics against supported set
    let op_config = OpConfig::load();
    let supported = build_supported_ops(&op_config);
    let errors = normalizer::validate_intrinsics(items, &supported);
    if !errors.is_empty() {
        return Err(format!("LLVM normalizer:\n  {}", errors.join("\n  ")));
    }

    // Strip metadata LLVM doesn't use
    let keep: HashSet<String> = ["primitive", "llvm_type", "encoding", "layout"]
        .iter().map(|s| s.to_string()).collect();
    for rt in universe.types.values_mut() {
        rt.properties.retain(|k, _| keep.contains(k));
    }

    Ok(())
}

/// 2026-07-14: For a meld with layout mappings, compute bit positions and
/// attach shuffle metadata to the source type's properties.
fn synthesize_meld_shuffle(meld: &crate::ast::top::Meld, universe: &mut TypeUniverse) -> Result<(), String> {
    // Find layout mappings: bindings["layout.sign"] = "sign"
    let layout_mappings: Vec<(&str, &str)> = meld.bindings.iter()
        .filter(|(k, _)| k.starts_with("layout."))
        .map(|(k, v)| (k.strip_prefix("layout.").unwrap(), v.as_str()))
        .collect();

    if layout_mappings.is_empty() {
        return Ok(());
    }

    // Get resolved types for both source and target
    let source_rt = match universe.get(&meld.name) {
        Some(rt) => rt.clone(),
        None => return Ok(()),
    };
    let target_rt = match universe.get(&meld.target) {
        Some(rt) => rt.clone(),
        None => return Ok(()),
    };

    // For each mapped field, compute the shuffle
    for (src_field, dst_field) in &layout_mappings {
        let src_offset = source_rt.properties.get(&format!("field.{}.offset", src_field))
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }).unwrap_or(0);
        let src_width = source_rt.properties.get(&format!("field.{}.width", src_field))
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }).unwrap_or(64);
        let dst_offset = target_rt.properties.get(&format!("field.{}.offset", dst_field))
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }).unwrap_or(0);
        let dst_width = target_rt.properties.get(&format!("field.{}.width", dst_field))
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }).unwrap_or(64);

        // Attach shuffle annotation on the source type
        let uni = format!("shuffle.{}.src_offset", dst_field);
        let rt = universe.types.get_mut(&meld.name).unwrap();
        rt.properties.insert(format!("shuffle.{}.src_offset", dst_field), PropertyValue::Int(src_offset as i64));
        rt.properties.insert(format!("shuffle.{}.src_width", dst_field), PropertyValue::Int(src_width as i64));
        rt.properties.insert(format!("shuffle.{}.dst_offset", dst_field), PropertyValue::Int(dst_offset as i64));
        rt.properties.insert(format!("shuffle.{}.dst_width", dst_field), PropertyValue::Int(dst_width as i64));
    }

    Ok(())
}

/// 2026-07-14: Walk a LayoutPattern and attach field-level annotations.
fn attach_layout_fields(rt: &mut crate::type_universe::ResolvedType, pat: &crate::ast::layout::LayoutPattern) {
    if let crate::ast::layout::LayoutPattern::Slice(fields) = pat {
        let mut offset = 0u64;
        for field in fields {
            // Attach offset and width as properties
            rt.properties.insert(
                format!("field.{}.offset", field.name),
                PropertyValue::Int(offset as i64),
            );
            rt.properties.insert(
                format!("field.{}.width", field.name),
                PropertyValue::Int(field.bits as i64),
            );
            if field.mutable {
                rt.properties.insert(
                    format!("field.{}.mutable", field.name),
                    PropertyValue::Bool(true),
                );
            }
            offset += field.bits;
        }
    }
}

use std::collections::HashSet;

/// Build the set of supported intrinsic names from the op config.
fn build_supported_ops(config: &OpConfig) -> HashSet<String> {
    let mut set = HashSet::new();
    // Generic operations (from llvm-ops.toml section keys "op.Add" etc.)
    for op_name in STANDARD_OPS {
        set.insert(format!("{}#", op_name));
    }
    // Also add some well-known intrinsics that don't appear as operations
    for name in &["GetEnv#", "GetGlobalId#", "GetGlobalSize#", "GetLocalId#",
                   "ToInt#", "ToFloat#", "ToString#", "Concat#", "Length#",
                   "AddressOf#", "SysCall#", "SysConf#",
                   "Load#", "Store#", "Copy#", "Fill#",
                   "AtomicLoad#", "AtomicStore#", "AtomicCas#", "AtomicXchg#",
                   "AtomicAdd#", "Fence#",
                   "DlOpen#", "DlSym#", "DlClose#",
                   "Backtrace#", "WorkgroupSize#"] {
        set.insert(name.to_string());
    }
    set
}

/// Standard generic operation names (without the # suffix).
const STANDARD_OPS: &[&str] = &[
    "Add", "Sub", "Mul", "Div", "Rem",
    "Eq", "Neq", "Lt", "Gt", "Le", "Ge",
    "Neg", "Abs",
    "BitAnd", "BitOr", "BitXor", "Shl", "Shr",
    "Sqrt", "Sin", "Cos", "Fabs", "Ceil", "Floor", "Pow",
    "Print",
    "Malloc", "Free",
];
