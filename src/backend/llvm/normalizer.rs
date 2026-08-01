// ── LLVM Normalizer — AST Annotation Pass ─────────────────────────────
//
// 2026-07-14: Walks the AST and attaches llvm_type to every type reference.
// Backend never reads config files or matches on primitive/bytes.
//
// 2026-07-20: Simplified for hashword protocol architecture.
// llvm_type is derived from structure only (fields + bytes), never from
// CTD, ALU, or TOML config. Types don't belong to categories — they
// interact with categories through hashword op signatures.

use crate::ast::*;
use crate::backend::normalizer;
use crate::type_universe::{ResolvedType, TypeUniverse};
use crate::ast::PropertyValue;

// 2026-07-30: Walks the AST and registers types in the TypeUniverse with
// their protocol membership and metadata. The casting graph resolves LLVM
// types from (protocol, metadata) at codegen time — no llvm_type needed.

/// Register type definitions in the universe and process meld declarations.
///
/// 2026-07-30: Protocol-driven type registration only. LLVM type resolution
/// is deferred to the casting graph's `resolve_llvm_type()`.
/// - Flexible types (Int, UInt, Bit) have no baked-in width — resolved per-target.
/// - Fixed-width types (Int32, Float) carry explicit !> bits metadata.
/// - Struct types derive LLVM type from field shapes at codegen time.
/// - Protocol (Cast.#) properties are retained for graph-based membership checks.
/// - Explicit user `llvm <~` is validated against known LLVM type strings.
pub fn normalize(items: &mut Vec<TopLevel>, universe: &mut TypeUniverse, int_bits: u64) -> Result<(), String> {
    // ── Register all TopLevel::TypeDef items into the TypeUniverse ─────────
    // After registration, the casting graph resolves LLVM types from
    // (protocol, metadata) at codegen time — no llvm_type derivation needed.
    // 2026-07-31: Phase 3 (§8.6) — int_bits threaded through so a type with no
    // width metadata falls back to the TARGET default width, not a hardcoded 64.
    register_typedefs(items, universe, int_bits)?;

    // 2026-07-16: P0+P6 — Process meld layout declarations in a single pass.
    for item in items.iter() {
        if let TopLevel::Meld(m) = &item {
            synthesize_meld_shuffle(m, universe)?;
            let decl = crate::analysis::meld_validation::build_meld_declaration(
                &m.name, &m.target, &m.bindings, &m.span,
            );
            universe.melds.insert(
                (decl.name_a.clone(), decl.name_b.clone()),
                decl.clone(),
            );
            universe.melds.insert(
                (decl.name_b.clone(), decl.name_a.clone()),
                decl.clone(),
            );
            if let Err(errs) = crate::analysis::meld_validation::validate_meld_layout(&decl, universe, false) {
                let msg: Vec<String> = errs.iter().map(|e| format!("{}", e)).collect();
                return Err(format!("meld validation failed for '{}': {}", m.name, msg.join("; ")));
            }
        }
    }

    // Validate intrinsics against supported set
    let errors = normalizer::validate_intrinsics(items, &build_supported_ops());
    if !errors.is_empty() {
        return Err(format!("LLVM normalizer:\n  {}", errors.join("\n  ")));
    }

    // 2026-07-29: Validate that no TypeDef overrides non-overridable ops.
    // Bitwise operations (BitAnd, BitOr, BitXor, BitNot, Shl, Shr) are axioms
    // of the #Bit protocol — they must never be semantically overloaded.
    // Parsing/lexing operations (Parse, Lex) are compile-time structural phases —
    // types inherit their parent protocol's parsing rules.
    let mut forbidden_names: HashSet<&str> = [
        "BitAnd", "BitOr", "BitXor", "BitNot", "Shl", "Shr",
    ].iter().cloned().collect();
    for item in items.iter() {
        if let TopLevel::TypeDef(td) = item {
            for op in &td.body.op_bindings {
                if forbidden_names.contains(op.name.as_str()) {
                    return Err(format!(
                        "{} '{}' cannot be overridden — it is an axiom of the #Bit protocol",
                        op.name, td.name,
                    ));
                }
                if op.name == "Parse" || op.name == "Lex" {
                    return Err(format!(
                        "'{}' is a built-in compile-time operation and cannot be overridden (in type '{}')",
                        op.name, td.name,
                    ));
                }
            }
        }
    }

    // Strip metadata LLVM doesn't use
        // 2026-07-30: Keep Cast.# properties for protocol membership,
        // plus width metadata (bits, maxbits, minbits) for resolve_llvm_type(),
        // and alignment for align_of().
        for rt in universe.types.values_mut() {
            rt.properties.retain(|k, _| {
                k.starts_with("Cast.")
                    || k == "bits" || k == "maxbits" || k == "minbits"
                    || k == "alignment"
            });
        }

    Ok(())
}

/// 2026-07-19: Validate that a user-provided LLVM type string is valid.
/// Returns Ok(()) if the type string is syntactically valid, Err with
/// a descriptive message otherwise.
fn validate_explicit_llvm(llvm_val: &str) -> Result<(), String> {
    match llvm_val {
        "half" | "bfloat" | "float" | "double" | "fp128"
        | "x86_fp80" | "ppc_fp128" => return Ok(()),
        _ => {}
    }
    if llvm_val.starts_with('i') {
        let bits = &llvm_val[1..];
        if bits.parse::<u64>().is_ok() {
            return Ok(());
        }
    }
    if llvm_val == "ptr" || llvm_val == "void" {
        return Ok(());
    }
    if llvm_val.starts_with('{') && llvm_val.ends_with('}') {
        return Ok(());
    }
    if llvm_val.starts_with('<') && llvm_val.contains(" x ") && llvm_val.ends_with('>') {
        return Ok(());
    }
    Err(format!(
        "invalid LLVM type '{}': expected a known LLVM type (float, double, half, bfloat, iN, ptr, ...)",
        llvm_val
    ))
}

/// 2026-07-16: Register all TopLevel::TypeDef items into the TypeUniverse.
/// Extracts byte size from layout metadata or slots, attaches field annotations,
/// and registers each type so meld validation can look it up.
fn register_typedefs(items: &[TopLevel], universe: &mut TypeUniverse, int_bits: u64) -> Result<(), String> {
    for item in items {
        let td = match item {
            TopLevel::TypeDef(td) => td,
            _ => continue,
        };
        // 2026-07-30: Bit is the axiomatic anchor — never overrideable.
        // A type declaration for Bit in user code or stdlib is an error.
        if td.name == "Bit" {
            return Err("'Bit' is a compiler primitive and cannot be redeclared".to_string());
        }
        // 2026-07-26: Read bits/maxbits/minbits metadata independently.
        // bits <~ N → exact (min=max=N), maxbits <~ N → ceiling (min=0, max=N),
        // minbits <~ N → floor (min=N, max=primordial). Fallback: primordial values.
        let exact_bits = td.body.metadata.get("bits")
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None });
        let ceiling = td.body.metadata.get("maxbits")
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None });
        let floor = td.body.metadata.get("minbits")
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None });
        // 2026-07-31: Phase 3 (§8.6) — clone the primordial so the warning
        // closures below can borrow `universe.warnings` mutably without
        // conflicting with an outstanding immutable borrow of the universe.
        let primordial = universe.get(&td.name).cloned();
        // 2026-07-31: Phase 3 (§8.6) — a type with no primordial falls back to
        // the TARGET int width (int_bits), not a hardcoded 64; a diagnostic is
        // recorded so the fallback is never silent.
        let prim_max = primordial.as_ref().map(|p| p.max_bits).unwrap_or_else(|| {
            universe.warnings.push(format!(
                "normalizer: type '{}' has no primordial entry and no `!> bits` \
                 metadata — defaulting max width to target int width ({})",
                td.name, int_bits
            ));
            int_bits
        });
        let prim_min = primordial.as_ref().map(|p| p.min_bits).unwrap_or(0);
        let (min_bits, max_bits) = if let Some(bits) = exact_bits {
            (bits, bits)
        } else {
            let f = floor.unwrap_or(prim_min);
            let c = ceiling.unwrap_or(prim_max);
            (f.min(c), c.max(f))
        };
        let bytes = ceiling
            .map(|b| b / 8)
            .or_else(|| exact_bits.map(|b| b / 8))
            .or_else(|| {
                if td.body.slots.is_empty() { return None; }
                let total: u64 = td.body.slots.iter().map(|slot| {
                    if matches!(slot.ty, crate::ast::Type::Ptr(_)) { return 8u64; }
                    slot.ty.universe_key()
                        .and_then(|k| universe.get(k))
                        .map(|rt| rt.bytes)
                        .unwrap_or_else(|| {
                            // 2026-07-31: Phase 3 (§8.6) — unknown slot type falls
                            // back to 8-byte (conservative max scalar); recorded so
                            // the assumption is not silent.
                            universe.warnings.push(format!(
                                "normalizer: slot type {:?} of type '{}' is not in the \
                                 universe — assuming 8-byte slot size",
                                slot.ty, td.name
                            ));
                            8
                        })
                }).sum();
                Some(total)
            })
            .or_else(|| {
                td.body.metadata.get("layout").and_then(|pv| {
                    if let PropertyValue::String(s) = pv {
                        let total_bits = compute_layout_total_bits(s)?;
                        if total_bits % 8 == 0 { Some(total_bits / 8) } else { None }
                    } else { None }
                })
            })
            .or_else(|| {
                td.body.metadata.get("layout_struct").and_then(|pv| {
                    if let PropertyValue::List(entries) = pv {
                        let mut total_bits = 0u64;
                        for entry in entries {
                            if let PropertyValue::List(parts) = entry {
                                if parts.len() >= 2 {
                                    if let PropertyValue::Identifier(type_name) = &parts[1] {
                                        // 2026-07-31: Phase 3 (§8.6) — unknown layout
                                        // type falls back to the target int width;
                                        // recorded so it is not silent.
                                        let bits = universe.get(type_name)
                                            .map(|r| r.bytes * 8)
                                            .unwrap_or_else(|| {
                                                universe.warnings.push(format!(
                                                    "normalizer: layout type '{}' of '{}' is not \
                                                     in the universe — assuming {} bits",
                                                    type_name, td.name, int_bits
                                                ));
                                                int_bits
                                            });
                                        total_bits += bits;
                                    }
                                }
                            }
                        }
                        if total_bits % 8 == 0 { Some(total_bits / 8) } else { None }
                    } else { None }
                })
            })
            .unwrap_or_else(|| primordial.as_ref().map(|p| p.bytes).unwrap_or_else(|| {
                // 2026-07-31: Phase 3 (§8.6) — no metadata-derived size and no
                // primordial: assume 8 bytes (conservative max scalar) and record
                // the fallback so it is not silent.
                universe.warnings.push(format!(
                    "normalizer: type '{}' has no size metadata and no primordial \
                     entry — assuming 8 bytes",
                    td.name
                ));
                8
            }));
        let alignment = td.body.metadata.get("alignment")
            .and_then(|pv| {
                if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }
            })
            .unwrap_or_else(|| primordial.as_ref().map(|p| p.alignment).unwrap_or_else(|| {
                // 2026-07-31: Phase 3 (§8.6) — conservative alignment fallback
                // (min(bytes, 8)); recorded so it is not silent.
                universe.warnings.push(format!(
                    "normalizer: type '{}' has no alignment metadata and no \
                     primordial entry — assuming alignment {}",
                    td.name, bytes.min(8)
                ));
                bytes.min(8)
            }));
        let mut properties: std::collections::HashMap<String, PropertyValue> = td.body.metadata.clone();
        let base = td.parent.as_ref()
            .and_then(|e| match e.as_ref() { Expr::Identifier(n) => Some(n.clone()), _ => None })
            .unwrap_or_else(|| "Bit".to_string());
        let fields: Vec<(String, Type)> = td.body.slots.iter()
            .map(|s| (s.name.clone(), s.ty.clone()))
            .collect();
        let mut rt = crate::type_universe::ResolvedType {
            name: td.name.clone(),
            base,
            bytes,
            min_bits,
            max_bits,
            alignment,
            properties,
            fields,
        };
        if let Some(PropertyValue::String(layout_str)) = rt.properties.get("layout") {
            let cleaned = layout_str.strip_prefix('<').unwrap_or(layout_str);
            if let Ok(pat) = crate::beast::layout::parse_layout_pattern(cleaned) {
                attach_layout_fields(&mut rt, &pat);
            }
        }
        if let Some(PropertyValue::List(entries)) = rt.properties.get("layout_struct") {
            let mut layout_fields = Vec::new();
            for entry in entries {
                let parts = match entry {
                    PropertyValue::List(p) => p,
                    _ => continue,
                };
                if parts.len() < 2 { continue; }
                let name = match &parts[0] {
                    PropertyValue::String(s) => s.clone(),
                    _ => continue,
                };
                let type_name = match &parts[1] {
                    PropertyValue::Identifier(s) => s.clone(),
                    _ => continue,
                };
                let bits = if let Some(resolved) = universe.get(&type_name) {
                    resolved.bytes * 8
                } else { 64 };
                layout_fields.push(crate::ast::layout::LayoutField {
                    name, bits, mutable: false, structural: false,
                });
            }
            if !layout_fields.is_empty() {
                attach_layout_fields(&mut rt, &crate::ast::layout::LayoutPattern::Slice(layout_fields));
            }
        }

        // 2026-07-20: No CTD/ALU/encoding inheritance.
        // Hashword op signatures replace these properties entirely.

        universe.register(rt);
    }

    // 2026-07-30: Cast.# properties are no longer injected by the normalizer.
    // Protocol membership is determined by the casting graph via type_to_protocol()
    // and is_protocol_member(). The casting graph hardcodes base protocol lanes
    // and receives proto declarations via register_protocol_def().
    //
    // Cast.# properties from primordial seeding are retained for backward compat
    // during migration. They will be removed in a future cleanup pass after
    // is_protocol_member() fully transitions to the casting graph.
    Ok(())
}

use std::collections::HashSet;

/// 2026-07-16: Compute total bits from a layout pattern string.
fn compute_layout_total_bits(s: &str) -> Option<u64> {
    let cleaned = s.strip_prefix('<').unwrap_or(s);
    let pat = crate::beast::layout::parse_layout_pattern(cleaned).ok()?;
    match &pat {
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
        crate::ast::layout::LayoutPattern::Repetition(_) | crate::ast::layout::LayoutPattern::Optional(_) => None,
        crate::ast::layout::LayoutPattern::ByteLiteral(_) => Some(8),
        crate::ast::layout::LayoutPattern::ByteRange(_, _) => None,
        crate::ast::layout::LayoutPattern::AnyBytes(n) => Some(n * 8),
        crate::ast::layout::LayoutPattern::VariableRef(_) => None,
        crate::ast::layout::LayoutPattern::TypedRef(_, _) => None,
        crate::ast::layout::LayoutPattern::PointerRef(_) => None,
        crate::ast::layout::LayoutPattern::SemanticLabel(_, inner) => layout_pattern_bits(inner),
        crate::ast::layout::LayoutPattern::GenericParam(_) => None,
        crate::ast::layout::LayoutPattern::Alternation(_) => None,
    }
}

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

/// 2026-07-14: Walk a LayoutPattern and attach field-level annotations.
fn attach_layout_fields(rt: &mut crate::type_universe::ResolvedType, pat: &crate::ast::layout::LayoutPattern) {
    if let crate::ast::layout::LayoutPattern::Slice(fields) = pat {
        let mut offset = 0u64;
        for field in fields {
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

/// 2026-07-14: For a meld with layout mappings, compute bit positions and
/// attach shuffle metadata to the source type's properties.
fn synthesize_meld_shuffle(meld: &crate::ast::top::Meld, universe: &mut TypeUniverse) -> Result<(), String> {
    let layout_mappings: Vec<(&str, &str)> = meld.bindings.iter()
        .filter(|(k, _)| k.starts_with("layout."))
        .map(|(k, v)| (k.strip_prefix("layout.").unwrap(), v.as_str()))
        .collect();

    if layout_mappings.is_empty() {
        return Ok(());
    }

    let source_rt = match universe.get(&meld.name) {
        Some(rt) => rt.clone(),
        None => return Ok(()),
    };
    let target_rt = match universe.get(&meld.target) {
        Some(rt) => rt.clone(),
        None => return Ok(()),
    };

    for (src_field, dst_field) in &layout_mappings {
        let src_offset = source_rt.properties.get(&format!("field.{}.offset", src_field))
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }).unwrap_or(0);
        let src_width = source_rt.properties.get(&format!("field.{}.width", src_field))
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None })
            .unwrap_or_else(|| {
                // 2026-07-31: Phase 3 (§8.6) — meld shuffle width missing: assume
                // 64 bits and record the fallback so it is not silent.
                universe.warnings.push(format!(
                    "normalizer: meld '{}' field '{}' has no width — assuming 64 bits",
                    meld.name, src_field
                ));
                64
            });
        let dst_offset = target_rt.properties.get(&format!("field.{}.offset", dst_field))
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }).unwrap_or(0);
        let dst_width = target_rt.properties.get(&format!("field.{}.width", dst_field))
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None })
            .unwrap_or_else(|| {
                universe.warnings.push(format!(
                    "normalizer: meld '{}' field '{}' has no width — assuming 64 bits",
                    meld.name, dst_field
                ));
                64
            });

        let rt = universe.types.get_mut(&meld.name).unwrap();
        rt.properties.insert(format!("shuffle.{}.src_offset", dst_field), PropertyValue::Int(src_offset as i64));
        rt.properties.insert(format!("shuffle.{}.src_width", dst_field), PropertyValue::Int(src_width as i64));
        rt.properties.insert(format!("shuffle.{}.dst_offset", dst_field), PropertyValue::Int(dst_offset as i64));
        rt.properties.insert(format!("shuffle.{}.dst_width", dst_field), PropertyValue::Int(dst_width as i64));
    }

    Ok(())
}

/// Build the set of supported intrinsic names from the op config.
/// 2026-07-20: Simplified — no TOML config lookup. Standard ops only.
fn build_supported_ops() -> HashSet<String> {
    let mut set = HashSet::new();
    for op_name in STANDARD_OPS {
        set.insert(format!("{}#", op_name));
    }
    for name in &["GetEnv#", "GetEnvInt#", "GetGlobalId#", "GetGlobalSize#", "GetLocalId#",
                   "ToInt#", "ToFloat#", "ToString#", "Concat#", "Length#",
                   "AddressOf#", "SysCall#", "SysConf#",
                   "Load#", "Store#", "Copy#", "Fill#",
                   "AtomicLoad#", "AtomicStore#", "AtomicCas#", "AtomicXchg#",
                   "AtomicAdd#", "Fence#",
                   "DlOpen#", "DlSym#", "DlClose#",
                    "Backtrace#", "WorkgroupSize#",
                    // 2026-08-01: The print/println macros expand to these
                    // direct runtime calls. They were previously invisible to
                    // this validator because the old PrintLn! always wrapped
                    // them in an Expr::Block, which the validator does not
                    // descend into; the bare println!() form exposes them.
                    "PrintInt#", "PrintChar#", "PrintFloat#", "PrintStr#"] {
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
    fn test_validate_explicit_llvm_float() {
        assert!(validate_explicit_llvm("float").is_ok());
    }

    #[test]
    fn test_validate_explicit_llvm_double() {
        assert!(validate_explicit_llvm("double").is_ok());
    }

    #[test]
    fn test_validate_explicit_llvm_invalid() {
        assert!(validate_explicit_llvm("not_a_type").is_err());
    }

    #[test]
    fn test_validate_explicit_llvm_i64() {
        assert!(validate_explicit_llvm("i64").is_ok());
    }

    #[test]
    fn test_validate_explicit_llvm_ptr() {
        assert!(validate_explicit_llvm("ptr").is_ok());
    }
}
