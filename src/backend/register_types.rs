// ── Shared Type Registration — Backend-Agnostic Universe Population ────
//
// 2026-08-10: Extracted from llvm/normalizer.rs so every backend normalizer
// registers user TypeDefs into the TypeUniverse uniformly. The casting graph
// resolves each backend's native type from (protocol, metadata) at codegen
// time — no per-backend llvm_type/js_type/bit_width baking here.
//
// Backends that derive a richer native type (LLVM IR types) do that
// separately in their own normalizer AFTER calling register_typedefs.

use crate::ast::*;
use crate::ast::PropertyValue;
use crate::type_universe::ResolvedType;
use crate::type_universe::TypeUniverse;

/// 2026-07-16: Register all TopLevel::TypeDef items into the TypeUniverse.
/// Extracts byte size from layout metadata or slots, attaches field annotations,
/// and registers each type so later validation can look it up.
///
/// 2026-08-10: Extracted from llvm/normalizer.rs — shared by all backends.
/// int_bits is the TARGET default width used when a type has no width metadata
/// and no primordial entry (Phase 3, §8.6).
pub fn register_typedefs(items: &[TopLevel], universe: &mut TypeUniverse, int_bits: u64) -> Result<(), String> {
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
                // 2026-08-04 (compiler-in-Briev): sum slot sizes via
                // type_size (NOT raw rt.bytes). Flexible primordials (Int,
                // String, ...) register bytes=0 ("not yet resolved"); reading
                // rt.bytes directly collapsed `ListBuffer<T> { data: Ptr<T>,
                // cap: Int }` to 8 bytes (Ptr=8 + Int=0) and List<T>.len
                // collided with inner.cap at offset 8. type_size resolves the
                // flexible protocols (Cast.#Int → 8, Cast.#String → 8) and
                // fixes Ptr at one word.
                let total: u64 = td.body.slots.iter().map(|slot| {
                    crate::backend::llvm::types::type_size(&slot.ty, Some(universe))
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
        // 2026-08-04 (compiler-in-Briev): when re-registering a primordial (e.g.
        // `type Int: #Int { ... }` in bootstrap.bv), inherit the primordial's
        // protocol Cast.#* properties. The flexible-protocol fallbacks in
        // type_size (types.rs) key on Cast.#Int/#String/#Float/#Bool — without
        // them, `type Int: #Int` (empty metadata) registers bytes=0 and
        // `type_size(Int)` returns 0, collapsing any struct containing an Int
        // slot (ListBuffer.cap → 0 → List<T>.len collides with inner.cap).
        if let Some(prim) = &primordial {
            for (k, v) in &prim.properties {
                if k.starts_with("Cast.") {
                    properties.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
        }
        // 2026-08-03: the declared protocol hashword is the base when there is
        // no parent type — `type CStr: #String<C_String>` must register base
        // "#String<C_String>" (not "Bit") so type_to_protocol resolves it to
        // (String, C_String) and the casting graph derives its ABI (ptr).
        let base = td.parent.as_ref()
            .and_then(|e| match e.as_ref() { Expr::Identifier(n) => Some(n.clone()), _ => None })
            .or_else(|| td.protocol.clone())
            .unwrap_or_else(|| "Bit".to_string());
        let fields: Vec<(String, Type)> = td.body.slots.iter()
            .map(|s| (s.name.clone(), s.ty.clone()))
            .collect();
        let mut rt = ResolvedType {
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
fn attach_layout_fields(rt: &mut ResolvedType, pat: &crate::ast::layout::LayoutPattern) {
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

#[cfg(test)]
mod tests {
    use crate::ast::top::{TypeDef, TypeDefBody, TypeDefSlot};
    use std::collections::HashMap;
    use super::*;

    fn make_type_def(name: &str, slots: Vec<(&str, Type)>) -> TopLevel {
        TopLevel::TypeDef(Box::new(TypeDef {
            name: name.to_string(),
            type_params: vec![],
            parent: None,
            protocol: Some("#Bit".to_string()),
            traits: vec![],
            bit_range: None,
            body: TypeDefBody {
                slots: slots.into_iter().map(|(n, ty)| TypeDefSlot {
                    name: n.to_string(), ty, bit_range: None,
                }).collect(),
                metadata: HashMap::new(),
                projections: vec![],
                bindings: vec![],
                operators: vec![],
                op_bindings: vec![],
                constraints: vec![],
                members: vec![],
                span: None,
            },
            span: None,
        }))
    }

    #[test]
    fn test_register_typedefs_bit_is_forbidden() {
        let mut u = TypeUniverse::new();
        let items = vec![make_type_def("Bit", vec![])];
        assert!(register_typedefs(&items, &mut u, 64).is_err());
    }

    #[test]
    fn test_register_typedefs_registers_custom_struct() {
        let mut u = TypeUniverse::new();
        let items = vec![make_type_def(
            "Point",
            vec![("x", Type::int()), ("y", Type::int())],
        )];
        register_typedefs(&items, &mut u, 64).unwrap();
        let rt = u.get("Point").expect("registered");
        assert_eq!(rt.bytes, 16);
        // The declared protocol hashword becomes the base.
        assert_eq!(rt.base, "#Bit");
    }

    #[test]
    fn test_layout_total_bits() {
        assert_eq!(compute_layout_total_bits("le:[x:8, y:8]"), Some(16));
        assert_eq!(compute_layout_total_bits("0xAA"), Some(8));
    }
}