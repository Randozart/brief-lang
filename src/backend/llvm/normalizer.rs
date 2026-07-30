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

/// 2026-07-14: Normalize the AST for LLVM backend emission.
/// Attaches llvm_type to every ResolvedType in the universe.
///
/// 2026-07-29: Protocol-driven llvm_type resolution.
/// - Flexible types (Int, UInt, Bit) have no baked-in llvm_type —
///   resolved from protocol membership + int_bits + explicit !> bits metadata.
/// - Fixed-width types (Int32, Float) keep primordial llvm_type.
/// - Explicit user `llvm <~` is validated against known LLVM type strings.
pub fn normalize(items: &mut Vec<TopLevel>, universe: &mut TypeUniverse, int_bits: u64) -> Result<(), String> {
    // ── Register all TopLevel::TypeDef items into the TypeUniverse ─────────
    // Must run before llvm_type derivation so struct types from bootstrap.bv
    // (like String with fields=[data:Int, len:Int]) are available for
    // field-based llvm_type computation.
    register_typedefs(items, universe);

    // ── Derive llvm_type for ALL types ─────────────────────────────────────
    // 2026-07-29: Protocol-driven resolution. Three phases:
    //   1. Strip primordial llvm_type for protocol-resolved types
    //   2. Skip types that still have llvm_type (fixed-width, explicit user llvm)
    //   3. Resolve llvm_type from protocol membership + int_bits + !> bits metadata
    for rt in universe.types.values_mut() {
        // ── Phase 1: Strip primordial llvm_type for target-dependent types ──
        // Cast.#Int, Cast.#UInt, and Cast.#Float types need target-aware width
        // resolution (int_bits for Int/UInt, explicit bits for Float). Strip the
        // primordial llvm_type so Phase 3 recomputes it from protocol + int_bits.
        // Cast.#Bool, Cast.#Bit, and types without these protocols keep theirs
        // (Bool→i8, Data→ptr, Char→i32, fixed-width types, structs).
        let needs_target_resolution = rt.properties.contains_key("Cast.#Int")
            || rt.properties.contains_key("Cast.#UInt")
            || rt.properties.contains_key("Cast.#Float");

        if needs_target_resolution {
            rt.properties.remove("llvm_type");
        }

        // ── Phase 2: Skip if llvm_type is already set ──
        // This catches fixed-width types (Int32, Float) that have primordial
        // llvm_type and don't need protocol resolution.
        if rt.properties.contains_key("llvm_type") {
            continue;
        }

        // ── Phase 3: Resolve llvm_type ──
        // 2026-07-19: User-provided llvm override (only in user code, not stdlib).
        let explicit_llvm = rt.properties.get("llvm").and_then(|pv| match pv {
            PropertyValue::String(s) => Some(s.as_str()),
            _ => None,
        });

        let llvm_ty = if let Some(llvm_val) = explicit_llvm {
            validate_explicit_llvm(llvm_val)?;
            llvm_val.to_string()
        } else if needs_target_resolution {
            // Protocol-driven resolution: #Int/#UInt → i{int_bits or explicit bits},
            // #Float → float/double/half/bfloat, #Bool → i8, #Bit → i8
            resolve_protocol_llvm_type(rt, int_bits)?
        } else if rt.bytes == 2 {
            // 2026-07-20: 2-byte types are ambiguous between half (IEEE 754),
            // bfloat (Google Brain), and i16 (integer). The disamb hint
            // resolves the ambiguity: disamb <~ "bfloat" → "bfloat",
            // disamb absent or any other value → "half" (IEEE 754 default).
            // If the type is an integer, the user should set llvm <~ "i16".
            match rt.properties.get("disamb") {
                Some(PropertyValue::String(s)) if s == "bfloat" => "bfloat".to_string(),
                Some(PropertyValue::Identifier(s)) if s == "bfloat" => "bfloat".to_string(),
                _ => "half".to_string(),
            }
        } else {
            // No fields, no explicit llvm, no protocol: derive from bytes
            format!("i{}", rt.bytes * 8)
        };

        rt.properties.insert("llvm_type".into(), PropertyValue::String(llvm_ty));
    }

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
        // 2026-07-20: Keep llvm_type, tbaa, and the <- operator bindings.
        // op.InsertAt and op.ExtractFrom are used by the <- (push/pop) operator
        // dispatch (e.g., stdlib's InsertAt <~ ring_push). Old-style op Add ~>
        // "string" metadata is no longer retained — hashword OperatorDef from
        // the AST replaces it.
        let keep: HashSet<String> = [
            "llvm_type", "disamb",
        ].iter().map(|s| s.to_string()).collect();
        for rt in universe.types.values_mut() {
            rt.properties.retain(|k, _| keep.contains(k) || k.starts_with("Cast."));
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
fn register_typedefs(items: &[TopLevel], universe: &mut TypeUniverse) {
    for item in items {
        let td = match item {
            TopLevel::TypeDef(td) => td,
            _ => continue,
        };
        // 2026-07-26: Read bits/maxbits/minbits metadata independently.
        // bits <~ N → exact (min=max=N), maxbits <~ N → ceiling (min=0, max=N),
        // minbits <~ N → floor (min=N, max=primordial). Fallback: primordial values.
        let exact_bits = td.body.metadata.get("bits")
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None });
        let ceiling = td.body.metadata.get("maxbits")
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None });
        let floor = td.body.metadata.get("minbits")
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None });
        let primordial = universe.get(&td.name);
        let prim_max = primordial.map(|p| p.max_bits).unwrap_or(64);
        let prim_min = primordial.map(|p| p.min_bits).unwrap_or(0);
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
                        .unwrap_or(8)
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
            .unwrap_or_else(|| primordial.map(|p| p.bytes).unwrap_or(8));
        let alignment = td.body.metadata.get("alignment")
            .and_then(|pv| {
                if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }
            })
            .unwrap_or_else(|| primordial.map(|p| p.alignment).unwrap_or_else(|| bytes.min(8)));
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
        // 2026-07-26: Preserve primordial llvm_type when the declaration
        // doesn't explicitly set it. The normalizer would otherwise derive
        // a raw "i{N}" from bytes, losing Float's "float" LLVM type.
        if let Some(prim) = primordial {
            if !rt.properties.contains_key("llvm_type")
                && prim.properties.contains_key("llvm_type")
            {
                rt.properties.insert("llvm_type".to_string(),
                    prim.properties.get("llvm_type").unwrap().clone());
            }
        }
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

    // 2026-07-23: Inject Cast.# properties from operator_defs and TypeDef.protocol.
    // This makes protocol edges visible to find_cast_path BFS.
    for item in items {
        let td = match item {
            TopLevel::TypeDef(td) => td,
            _ => continue,
        };
        let type_name = &td.name;

        // Implicit CastTo from TypeDef.protocol field (e.g., type Int: #Int)
        if let Some(ref proto) = td.protocol {
            let cat = proto.strip_prefix('#').unwrap_or(proto).to_string();
            if let Some(rt) = universe.types.get_mut(type_name) {
                rt.properties.insert(format!("Cast.#{}", cat), PropertyValue::Bool(true));
            }
        }

        // Explicit CastTo/CastFrom from operator definitions
        for op in &td.body.operators {
            if op.op == "CastTo" || op.op == "CastFrom" {
                for param in &op.params {
                    let cat = match param {
                        Type::HashWord(name) | Type::HashWordVariant(name, _)
                            => name.strip_prefix('#').unwrap_or(name).to_string(),
                        _ => continue,
                    };
                    if let Some(rt) = universe.types.get_mut(type_name) {
                        rt.properties.insert(format!("Cast.#{}", cat), PropertyValue::Bool(true));
                    }
                }
            }
        }
    }

    // 2026-07-30: Inject Cast.#Bit for all types with base == "Bit".
    // Since every type without an explicit parent defaults to base: "Bit",
    // this ensures all types are reachable from #Bits in the protocol graph.
    // Previously, only types explicitly seeded in PRIMORDIALS had Cast.#Bit.
    for rt in universe.types.values_mut() {
        if rt.base == "Bit" && !rt.properties.contains_key("Cast.#Bit") {
            rt.properties.insert("Cast.#Bit".to_string(), PropertyValue::Bool(true));
        }
    }
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
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }).unwrap_or(64);
        let dst_offset = target_rt.properties.get(&format!("field.{}.offset", dst_field))
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }).unwrap_or(0);
        let dst_width = target_rt.properties.get(&format!("field.{}.width", dst_field))
            .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }).unwrap_or(64);

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

// 2026-07-29: Protocol-driven LLVM type resolution for types whose width
// depends on target int_bits or explicit !> bits metadata. Called from
// Phase 3 of the normalizer main loop for Cast.#Int/.#UInt/.#Float types.
fn resolve_protocol_llvm_type(rt: &ResolvedType, int_bits: u64) -> Result<String, String> {
    let has_cast_int = rt.properties.contains_key("Cast.#Int");
    let has_cast_uint = rt.properties.contains_key("Cast.#UInt");
    let has_cast_float = rt.properties.contains_key("Cast.#Float");

    if has_cast_int || has_cast_uint {
        // Priority: explicit !> bits: N > !> maxbits: N > int_bits
        if let Some(bits) = get_exact_bits(rt) {
            return Ok(format!("i{}", bits));
        }
        if let Some(ceiling) = get_maxbits(rt) {
            return Ok(format!("i{}", int_bits.min(ceiling)));
        }
        return Ok(format!("i{}", int_bits));
    }

    if has_cast_float {
        let bits = get_exact_bits(rt).unwrap_or(32);
        let llvm_ty = match bits {
            16 => {
                let is_bfloat = rt.properties.get("disamb")
                    .map(|pv| matches!(pv, PropertyValue::String(s) if s == "bfloat"))
                    .unwrap_or(false);
                if is_bfloat { "bfloat".to_string() } else { "half".to_string() }
            }
            32 => "float".to_string(),
            64 => "double".to_string(),
            80 => "x86_fp80".to_string(),
            128 => "fp128".to_string(),
            _ => return Err(format!("Float type '{}' has unsupported bit width {}", rt.name, bits)),
        };
        return Ok(llvm_ty);
    }

    Err(format!(
        "cannot determine LLVM type for type '{}' — \
         has Cast.#Int/.#UInt/.#Float but no resolution path succeeded",
        rt.name
    ))
}

/// Read exact !> bits: N metadata from type properties.
fn get_exact_bits(rt: &ResolvedType) -> Option<u64> {
    rt.properties.get("bits").and_then(|pv| {
        if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }
    })
}

/// Read !> maxbits: N metadata from type properties.
fn get_maxbits(rt: &ResolvedType) -> Option<u64> {
    rt.properties.get("maxbits").and_then(|pv| {
        if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }
    })
}

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
