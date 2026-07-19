// ── LLVM Normalizer — AST Annotation Pass ─────────────────────────────
// 2026-07-14: Walks the AST and attaches llvm_type to every type reference.
// Backend never reads config files or matches on primitive/bytes.

use crate::ast::*;
use crate::backend::normalizer;
use crate::config::{derive_llvm_type, OpConfig, TypeConfig};
use crate::type_universe::TypeUniverse;

/// 2026-07-14: Normalize the AST for LLVM backend emission.
/// Attaches category and llvm_type to every ResolvedType in the universe.
///
/// 2026-07-19: Category-driven architecture. The normalizer:
///   1. Validates ALU × CTD for every type (first pass)
///   2. Registers TypeDefs from the AST (register_typedefs)
///   3. Infers category from structure, derives llvm_type (second pass)
/// The normalizer is the single authority for llvm_type — stdlib never
/// sets it directly.
pub fn normalize(items: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> Result<(), String> {
    let prim_config = TypeConfig::load();

    // ── Pass 1: ALU × CTD validation only ─────────────────────────────────
    // 2026-07-19: llvm_type and category are computed in Pass 2 (after
    // register_typedefs), so struct types have their fields set for
    // category inference (String-likeness detection).
    for rt in universe.types.values_mut() {
        let ctd = rt.properties.get("ctd").and_then(|pv| match pv {
            PropertyValue::Identifier(s) => Some(s.as_str()),
            _ => None,
        });
        let alu = rt.properties.get("alu").and_then(|pv| match pv {
            PropertyValue::Identifier(s) => Some(s.as_str()),
            _ => None,
        });

        if let (Some(a), Some(c)) = (alu, ctd) {
            if let Err(e) = validate_alu_ctd(a, c) {
                return Err(e);
            }
        }

        if let Some(PropertyValue::String(layout_str)) = rt.properties.get("layout") {
            let cleaned = layout_str.strip_prefix('<').unwrap_or(layout_str);
            if let Ok(pat) = crate::bvir::layout::parse_layout_pattern(cleaned) {
                attach_layout_fields(rt, &pat);
            }
        }
    }

    // 2026-07-16: Register all TopLevel::TypeDef items into the TypeUniverse
    // before meld processing, so validate_bit_permutation can find types.
    register_typedefs(items, universe, &prim_config);

    // ── Pass 2: Infer category and derive llvm_type for ALL types ──────────
    // 2026-07-19: Runs after register_typedefs so struct types from
    // bootstrap.bv (like String with fields=[data:Int, len:Int]) are
    // available for structural category inference.
    // Category overrides CTD as the primary dispatch axis.
    for rt in universe.types.values_mut() {
        let category = infer_category(rt);
        rt.properties.insert("category".into(), PropertyValue::String(category.to_string()));

        let ctd = rt.properties.get("ctd").and_then(|pv| match pv {
            PropertyValue::Identifier(s) => Some(s.as_str()),
            _ => None,
        });

        // 2026-07-19: User-provided llvm override (only in user code, not stdlib).
        // Validate against known LLVM type strings; reject invalid values.
        let explicit_llvm = rt.properties.get("llvm").and_then(|pv| match pv {
            PropertyValue::String(s) => Some(s.as_str()),
            _ => None,
        });

        let llvm_ty = if let Some(llvm_val) = explicit_llvm {
            validate_explicit_llvm(llvm_val)?;
            llvm_val.to_string()
        } else {
            match category {
                "Float" => derive_llvm_type(ctd, rt.bytes, &prim_config),
                "String" => "{ i64, i64 }".to_string(),
                _ => derive_llvm_type(ctd, rt.bytes, &prim_config),
            }
        };
        rt.properties.insert("llvm_type".into(), PropertyValue::String(llvm_ty));
    }

    // 2026-07-16: P0+P6 — Process meld layout declarations in a single pass.
    //  1. Synthesize bit-shuffle metadata
    //  2. Register in TypeUniverse for bidirectional lookup
    //  3. Run 5-layer validation cascade (L1-3 fatal, L4-5 warnings)
    for item in items.iter() {
        if let TopLevel::Meld(m) = &item {
            // 1. Synthesize shuffle metadata
            synthesize_meld_shuffle(m, universe)?;
            // 2. Build canonical declaration
            let decl = crate::analysis::meld_validation::build_meld_declaration(
                &m.name, &m.target, &m.bindings, &m.span,
            );
            // 3. Register both orderings
            universe.melds.insert(
                (decl.name_a.clone(), decl.name_b.clone()),
                decl.clone(),
            );
            universe.melds.insert(
                (decl.name_b.clone(), decl.name_a.clone()),
                decl.clone(),
            );
            // 4. Run validation cascade
            if let Err(errs) = crate::analysis::meld_validation::validate_meld_layout(&decl, universe, false) {
                let msg: Vec<String> = errs.iter().map(|e| format!("{}", e)).collect();
                return Err(format!("meld validation failed for '{}': {}", m.name, msg.join("; ")));
            }
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
    // 2026-07-17: Keep ctd, alu, llvm_type, encoding, layout
    // 2026-07-19: Keep category (structural inference), tbaa_parent (TBAA hierarchy)
    // 2026-07-18: Keep ALL op.* entries (op.Add, op.Sub, etc.) for universe-driven
    // operator dispatch. op.InsertAt/op.ExtractFrom are also op.*, covered by prefix.
    // Note: "llvm" (user-facing property) is NOT kept — it's consumed by Pass 2
    // and replaced with derived "llvm_type".
    let keep: HashSet<String> = [
        "ctd", "alu", "category", "llvm_type", "encoding", "layout", "svo",
        "tbaa", "tbaa_parent",
        "op.InsertAt", "op.ExtractFrom",
    ].iter().map(|s| s.to_string()).collect();
    for rt in universe.types.values_mut() {
        // 2026-07-18: Keep all op.* keys — the hardcoded list above is
        // the minimum; any other op.* is also valid operator metadata.
        rt.properties.retain(|k, _| keep.contains(k) || k.starts_with("op."));
    }

    Ok(())
}

// 2026-07-17: Map a frontend-known CTD (PascalCase) to its LLVM type string.
// Unknown CTDs return None — the caller falls back to derive_llvm_type.
// String/Data are heap-allocated → "ptr" at the LLVM ABI level.
fn ctd_to_llvm(ctd: &str, bytes: u64) -> Option<&'static str> {
    match ctd {
        "Int" | "UInt" => match bytes {
            1 => Some("i8"), 2 => Some("i16"),
            4 => Some("i32"), 8 => Some("i64"),
            _ => None,
        },
        "Float" => Some("float"),
        "Double" => Some("double"),
        "Bool" => Some("i8"),
        "Char" => Some("i32"),
        "String" | "Data" | "Ptr" => Some("ptr"),
        "Void" => Some("void"),
        _ => None,
    }
}

// 2026-07-17: Validate that a PascalCase ALU is compatible with the type's CTD.
// Quoted ALUs (lowercase strings) bypass validation — the backend handles those.
// Returns Ok(()) if compatible, Err(description) if not.
fn validate_alu_ctd(alu: &str, ctd: &str) -> Result<(), String> {
    match (alu, ctd) {
        ("Float", "Int" | "UInt" | "Bool" | "Char" | "String" | "Data" | "Ptr" | "Void") =>
            Err(format!("ALU '{}' is incompatible with CTD '{}': float hardware cannot process {} values", alu, ctd, ctd)),
        ("Bool", "Float" | "Double") =>
            Err(format!("ALU '{}' is incompatible with CTD '{}': boolean logic cannot process float values", alu, ctd)),
        ("Bool", "Int" | "UInt" | "Char" | "String" | "Data" | "Ptr" | "Void") =>
            Err(format!("ALU '{}' is incompatible with CTD '{}': boolean logic cannot process integer-like types (use ALU Int)", alu, ctd)),
        ("Int", "Float" | "Double") =>
            Err(format!("ALU '{}' is incompatible with CTD '{}': integer hardware cannot process float values (use ALU Float)", alu, ctd)),
        ("Int", "Bool") =>
            Err(format!("ALU '{}' is incompatible with CTD '{}': integer hardware cannot process boolean values (use ALU Bool)", alu, ctd)),
        _ => Ok(()),
    }
}

/// 2026-07-19: Infer a type's category from its structure and properties.
/// "String" — 2 Int fields + encoding property (structural duck test).
/// "Float"  — ALU property says "Float" (uses float hardware).
/// "Bits"   — default for everything else (integer types, custom types).
fn infer_category(rt: &crate::type_universe::ResolvedType) -> &'static str {
    // String-like: two Int fields (data, len) + encoding property.
    // Catches user-defined `type MyString { data: Int; len: Int; encoding <~ "UTF-8" }`
    // without requiring any PascalCase CTD or explicit declaration.
    if rt.fields.len() == 2
        && rt.fields[0].1 == Type::int()
        && rt.fields[1].1 == Type::int()
        && rt.properties.contains_key("encoding")
    {
        return "String";
    }
    // Float-like: ALU says Float. Set by primordial type system
    // (Float → ALU=Float) or by user's `alu <~ "Float"` declaration.
    if let Some(PropertyValue::Identifier(s)) = rt.properties.get("alu") {
        if s == "Float" {
            return "Float";
        }
    }
    // Bits: default for all other types (Int, Bool, Char, Data, Posit32, etc.)
    "Bits"
}

/// 2026-07-19: Validate that a user-provided LLVM type string (`llvm <~ "..."`)
/// is syntactically valid. Called when a user type has an explicit `llvm`
/// property — stdlib types never set this, the normalizer derives it.
fn validate_explicit_llvm(llvm_val: &str) -> Result<(), String> {
    // Native float types
    match llvm_val {
        "half" | "bfloat" | "float" | "double" | "fp128"
        | "x86_fp80" | "ppc_fp128" => return Ok(()),
        _ => {}
    }
    // Native integer types: i1, i8, i16, i32, i64, i128, i256
    if llvm_val.starts_with('i') {
        let bits = &llvm_val[1..];
        if bits.parse::<u64>().is_ok() {
            return Ok(());
        }
    }
    // Pointer
    if llvm_val == "ptr" {
        return Ok(());
    }
    // Void
    if llvm_val == "void" {
        return Ok(());
    }
    // Simple struct: { type, type, ... }
    if llvm_val.starts_with('{') && llvm_val.ends_with('}') {
        return Ok(());
    }
    // Vector: <N x type>
    if llvm_val.starts_with('<') && llvm_val.contains(" x ") && llvm_val.ends_with('>') {
        return Ok(());
    }
    Err(format!(
        "invalid LLVM type '{}': expected a known LLVM type (float, double, half, bfloat, iN, ptr, ...)",
        llvm_val
    ))
}

/// 2026-07-16: Compute total bits from a layout pattern string.
/// Returns None for patterns with variable-length components.
fn compute_layout_total_bits(s: &str) -> Option<u64> {
    // 2026-07-16: Strip leading '<' that read_layout_body includes
    let cleaned = s.strip_prefix('<').unwrap_or(s);
    let pat = crate::bvir::layout::parse_layout_pattern(cleaned).ok()?;
    match &pat {
        crate::ast::layout::LayoutPattern::Slice(fields) => {
            Some(fields.iter().map(|f| f.bits as u64).sum())
        }
        crate::ast::layout::LayoutPattern::Sequence(seq) => {
            let mut total = 0u64;
            for p in seq {
                let bits = layout_pattern_bits(p)?;
                total += bits;
            }
            Some(total)
        }
        crate::ast::layout::LayoutPattern::Repetition(_) | crate::ast::layout::LayoutPattern::Optional(_) => {
            None  // Variable-length — can't determine at compile time
        }
        crate::ast::layout::LayoutPattern::ByteLiteral(_) => Some(8),
        crate::ast::layout::LayoutPattern::ByteRange(_, _) => None,  // Variable
        crate::ast::layout::LayoutPattern::AnyBytes(n) => Some(n * 8),
        crate::ast::layout::LayoutPattern::VariableRef(_) => None,
        crate::ast::layout::LayoutPattern::TypedRef(_, _) => None,
        crate::ast::layout::LayoutPattern::PointerRef(_) => None,
        crate::ast::layout::LayoutPattern::SemanticLabel(_, inner) => layout_pattern_bits(inner),
        crate::ast::layout::LayoutPattern::GenericParam(_) => None,
        crate::ast::layout::LayoutPattern::Alternation(_) => None,
    }
}

/// 2026-07-16: Compute total bits from a parsed LayoutPattern.
fn layout_pattern_bits(pat: &crate::ast::layout::LayoutPattern) -> Option<u64> {
    match pat {
        crate::ast::layout::LayoutPattern::Slice(fields) => {
            Some(fields.iter().map(|f| f.bits as u64).sum())
        }
        crate::ast::layout::LayoutPattern::Sequence(seq) => {
            let mut total = 0u64;
            for p in seq {
                total += layout_pattern_bits(p)?;
            }
            Some(total)
        }
        crate::ast::layout::LayoutPattern::ByteLiteral(_) => Some(8),
        crate::ast::layout::LayoutPattern::AnyBytes(n) => Some(n * 8),
        crate::ast::layout::LayoutPattern::SemanticLabel(_, inner) => layout_pattern_bits(inner),
        _ => None,
    }
}

/// 2026-07-16: Register all TopLevel::TypeDef items into the TypeUniverse.
/// Extracts byte size from layout metadata or slots, attaches field annotations,
/// and registers each type so meld validation can look it up.
fn register_typedefs(items: &[TopLevel], universe: &mut TypeUniverse, _prim_config: &TypeConfig) {
    for item in items {
        let td = match item {
            TopLevel::TypeDef(td) => td,
            _ => continue,
        };
        // Determine byte size: explicit metadata, field-derived, layout pattern, else 8
        let bytes = td.body.metadata.get("bytes")
            .and_then(|pv| {
                if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }
            })
            .or_else(|| {
                // 2026-07-18: Derive bytes from struct fields when no explicit bytes.
                // Sum each field's resolved byte size. Ptr fields are always 8.
                if td.body.slots.is_empty() { return None; }
                let total: u64 = td.body.slots.iter().map(|slot| {
                    if matches!(slot.ty, crate::ast::Type::Ptr(_)) { return 8u64; }
                    slot.ty.universe_key()
                        .and_then(|k| universe.get(k))
                        .map(|rt| rt.bytes)
                        .unwrap_or(8)
                }).sum();
                Some(total)
            })
            .or_else(|| {
                // Try to compute from layout pattern: sum field widths / 8
                td.body.metadata.get("layout").and_then(|pv| {
                    if let PropertyValue::String(s) = pv {
                        let total_bits = compute_layout_total_bits(s)?;
                        if total_bits % 8 == 0 { Some(total_bits / 8) } else { None }
                    } else { None }
                })
            })
            .or_else(|| {
                // 2026-07-16: Try struct-format layout: sum resolved type widths / 8
                td.body.metadata.get("layout_struct").and_then(|pv| {
                    if let PropertyValue::List(entries) = pv {
                        let mut total_bits = 0u64;
                        for entry in entries {
                            if let PropertyValue::List(parts) = entry {
                                if parts.len() >= 2 {
                                    if let PropertyValue::Identifier(type_name) = &parts[1] {
                                        let bits = universe.get(type_name)
                                            .map(|r| r.bytes * 8)
                                            .unwrap_or(64);
                                        total_bits += bits;
                                    }
                                }
                            }
                        }
                        if total_bits % 8 == 0 { Some(total_bits / 8) } else { None }
                    } else { None }
                })
            })
            .unwrap_or(8);
        // Determine alignment from metadata or default to bytes (clamped to 8)
        let alignment = td.body.metadata.get("alignment")
            .and_then(|pv| {
                if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }
            })
            .unwrap_or_else(|| bytes.min(8));
        // Collect properties from metadata only — slots are stored as typed
        // `fields` on ResolvedType (2026-07-18), replacing old slot.* properties.
        let mut properties: std::collections::HashMap<String, PropertyValue> = td.body.metadata.clone();
        // Build ResolvedType. The base type is the Expr name (e.g. "Bits"),
        // or "Bits" if the base expr is not an identifier.
        let base = match td.base.as_ref() {
            Expr::Identifier(name) => name.clone(),
            _ => "Bits".to_string(),
        };
        let fields: Vec<(String, Type)> = td.body.slots.iter()
            .map(|s| (s.name.clone(), s.ty.clone()))
            .collect();
        let mut rt = crate::type_universe::ResolvedType {
            name: td.name.clone(),
            base,
            bytes,
            alignment,
            properties,
            fields,
        };
        // Attach field-level layout annotations if layout property is set
        // 2026-07-16: Strip leading '<' that read_layout_body includes
        if let Some(PropertyValue::String(layout_str)) = rt.properties.get("layout") {
            let cleaned = layout_str.strip_prefix('<').unwrap_or(layout_str);
            if let Ok(pat) = crate::bvir::layout::parse_layout_pattern(cleaned) {
                attach_layout_fields(&mut rt, &pat);
            }
        }
        // 2026-07-16: Handle struct-format layout: layout <~ { field: Type }.
        // Resolves each type name in the universe to get byte width, then
        // builds a LayoutPattern::Slice with sequential offsets.
        if let Some(PropertyValue::List(entries)) = rt.properties.get("layout_struct") {
            let mut layout_fields = Vec::new();
            for entry in entries {
                let parts = match entry {
                    PropertyValue::List(p) => p,
                    _ => continue,
                };
                if parts.len() < 2 {
                    continue;
                }
                let name = match &parts[0] {
                    PropertyValue::String(s) => s.clone(),
                    _ => continue,
                };
                let type_name = match &parts[1] {
                    PropertyValue::Identifier(s) => s.clone(),
                    _ => continue,
                };
                // Look up the type in the universe to get byte size
                let bits = if let Some(resolved) = universe.get(&type_name) {
                    resolved.bytes * 8
                } else {
                    64  // Default to 64 bits if type not found
                };
                layout_fields.push(crate::ast::layout::LayoutField {
                    name,
                    bits,
                    mutable: false,
                    structural: false,
                });
            }
            if !layout_fields.is_empty() {
                attach_layout_fields(&mut rt, &crate::ast::layout::LayoutPattern::Slice(layout_fields));
            }
        }
        // 2026-07-18: Preserve primordial properties that the TypeDef may not set
        // (ctd, alu, encoding). This ensures bootstrap.bv types inherit their
        // semantic identity from the primordial without requiring explicit
        // declarations in every .bv file.
        for key in &["ctd", "alu", "encoding"] {
            if !rt.properties.contains_key(*key) {
                if let Some(existing) = universe.get(&rt.name) {
                    if let Some(val) = existing.properties.get(*key) {
                        rt.properties.insert(key.to_string(), val.clone());
                    }
                }
            }
        }
        // 2026-07-19: llvm_type is set by Pass 2 of the main normalize()
        // function (category-driven inference). No need to compute it here.
        // category is also set by Pass 2.
        universe.register(rt);
    }
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
    for name in &["GetEnv#", "GetEnvInt#", "GetGlobalId#", "GetGlobalSize#", "GetLocalId#",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ctd_to_llvm_string() {
        assert_eq!(ctd_to_llvm("String", 24), Some("ptr"));
    }

    #[test]
    fn test_ctd_to_llvm_data() {
        assert_eq!(ctd_to_llvm("Data", 8), Some("ptr"));
    }

    #[test]
    fn test_ctd_to_llvm_int() {
        assert_eq!(ctd_to_llvm("Int", 8), Some("i64"));
        assert_eq!(ctd_to_llvm("Int", 4), Some("i32"));
        assert_eq!(ctd_to_llvm("Int", 1), Some("i8"));
    }

    #[test]
    fn test_ctd_to_llvm_float() {
        assert_eq!(ctd_to_llvm("Float", 4), Some("float"));
        assert_eq!(ctd_to_llvm("Double", 8), Some("double"));
    }

    #[test]
    fn test_ctd_to_llvm_bool() {
        assert_eq!(ctd_to_llvm("Bool", 1), Some("i8"));
    }

    #[test]
    fn test_ctd_to_llvm_unknown() {
        assert_eq!(ctd_to_llvm("CustomType", 8), None);
    }

    #[test]
    fn test_validate_alu_ctd_float_double_ok() {
        assert!(validate_alu_ctd("Float", "Double").is_ok());
    }

    #[test]
    fn test_validate_alu_ctd_float_string_err() {
        assert!(validate_alu_ctd("Float", "String").is_err());
    }

    #[test]
    fn test_validate_alu_ctd_bool_double_err() {
        assert!(validate_alu_ctd("Bool", "Double").is_err());
    }

    #[test]
    fn test_validate_alu_ctd_int_float_err() {
        assert!(validate_alu_ctd("Int", "Float").is_err());
    }

    #[test]
    fn test_validate_alu_ctd_int_bool_err() {
        assert!(validate_alu_ctd("Int", "Bool").is_err());
    }

    #[test]
    fn test_validate_alu_ctd_bool_int_err() {
        assert!(validate_alu_ctd("Bool", "Int").is_err());
    }

    #[test]
    fn test_validate_alu_ctd_int_int_ok() {
        assert!(validate_alu_ctd("Int", "Int").is_ok());
    }

    #[test]
    fn test_validate_alu_ctd_quoted_alu_ok() {
        // Quoted ALUs bypass validation (no logic change needed — just verifying interface)
        assert!(validate_alu_ctd("Int", "String").is_ok());
    }
}
