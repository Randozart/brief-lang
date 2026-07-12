# Allocation Metadata System

**Date:** 2026-07-12
**Status:** Plan — pre-implementation
**Depends on:** Phase 1B (property system for `<~` metadata), Phase 8G
(metadata dispatch), escape analysis pass (existing), LLVM codegen
**See also:** `docs/architecture/features/metadata-dispatch.md` for the
distributed validation architecture this plan extends.

---

## Overview

Add `alloc` as a metadata annotation on variable bindings via the existing
`<~` syntax. The annotation controls where and how a variable's memory is
allocated — stack, heap, physical address, or dynamic pointer — with the
compiler statically verifying constraints it can prove and backends
enforcing what they can execute.

### Design Principles

1. **Frontend validates what it can prove:** `"Stack"` and physical address
   literals (`0x4000_2000`) are validated at compile time. Unknown values
   are passed through to the backend as opaque metadata.

2. **Backend validates what it must execute:** Each backend interprets
   `alloc` values it understands. Unknown values produce an error
   (the backend knows the key but cannot fulfill it).

3. **Physical addresses imply side effects:** `alloc(0x4000_2000)` implicitly
   sets `observable <~ true` and `volatile <~ true` — every access is a
   side effect that DCE must preserve and LLVM must not reorder.

4. **Distributed validation:** Known key + unparseable value → backend error.
   Unknown key → silently ignored (forward compatibility).

### Syntax

```brief
// 1. Force stack allocation (frontend verifies no-escape)
let buffer: List<Int>;
buffer <~ alloc("Stack");

// 2. Physical memory-mapped I/O (frontend verifies constant)
let uart_status: UInt32 <~ alloc(0x4000_2000);

// 3. Arena allocation (opaque — backend handles it)
let node: TreeNode;
node <~ alloc("Arena", scratchpad);

// 4. Placement new / dynamic pointer binding (opaque — backend handles it)
let header: PacketHeader;
header <~ alloc(raw_ptr);

// 5. Default — escape analysis decides
let x = compute();
```

---

## Phase — Allocation Metadata

### Step A.0 — Add `alloc` to the metadata parser

**File:** `src/parser.rs` — `parse_metadata()` or `parse_body_metadata()`

**What:** Ensure the parser already accepts `alloc` as a metadata key in
`<~` declarations. The property system (Phase 1B) already stores metadata
as `HashMap<String, PropertyValue>` — `alloc` is just a new key with a
value that is either a string, an integer, or a list.

The parser already handles:
```
key <~ "string";
key <~ 0x4000_2000;
key <~ "Arena", ptr;
```

**Check:** Verify that comma-separated values in `<~` are supported:
```
x <~ "Stack";              // single value
y <~ alloc(0x4000_2000);   // single integer value  
z <~ alloc("Arena", ptr);  // list/tuple value
```

If not, extend `parse_body_metadata()` to support multi-value metadata
with comma separation inside `<>` or as a list literal.

**Tests:**
- `test_parse_alloc_stack`: `buffer <~ alloc("Stack");` → metadata key `alloc`
  with value `"Stack"`
- `test_parse_alloc_physical`: `reg <~ alloc(0x4000_2000);` → metadata key
  `alloc` with integer value `0x4000_2000`
- `test_parse_alloc_arena`: `node <~ alloc("Arena", local_pool);` → metadata
  key `alloc` with list value `["Arena", local_pool]`
- `test_parse_alloc_placement`: `h <~ alloc(raw_ptr);` → metadata key `alloc`
  with identifier value `raw_ptr`

### Step A.1 — Frontend validation of known `alloc` values

**File:** `src/typechecker.rs` — new pass `validate_alloc_annotations()`

**What:** After type-checking, run a pass over all variable bindings with
`alloc` metadata. Validate what the frontend can prove; expand implied
metadata; pass through unknowns.

```rust
/// Validate alloc annotations on variable bindings.
/// 2026-07-12: Phase A.1
fn validate_alloc_annotations(program: &mut Program) -> Result<(), Vec<AllocError>> {
    let mut errors = Vec::new();

    for binding in program.all_bindings() {
        let Some(alloc_value) = binding.get_metadata("alloc") else { continue; };

        match alloc_value {
            // Case 1: "Stack" — frontend must verify no-escape
            PropertyValue::String(s) if s == "Stack" => {
                if !escape_analysis::proves_no_escape(binding, program) {
                    errors.push(AllocError::Escape {
                        name: binding.name.clone(),
                        span: binding.span,
                    });
                }
                // Implicitly expand: alloc("Stack") → alloca
                binding.set_metadata("alloca", PropertyValue::Bool(true));
            }

            // Case 2: Physical address literal (integer constant)
            PropertyValue::Integer(addr) => {
                // Must be a compile-time constant (already guaranteed by parser)
                // Expand: alloc(0x...) → volatile + observable + fixed_addr
                binding.set_metadata("volatile", PropertyValue::Bool(true));
                binding.set_metadata("observable", PropertyValue::Bool(true));
                binding.set_metadata("fixed_addr", PropertyValue::Integer(*addr));
            }

            // Case 3-5: Unknown values — pass through to backend
            // "Arena", "Heap", raw pointer — frontend cannot validate
            _ => { /* opaque — pass through */ }
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
```

**Tests:**
- `test_alloc_stack_no_escape`: Variable stays in scope → passes
- `test_alloc_stack_does_escape`: Variable returned from function → error
- `test_alloc_physical_constant`: `alloc(0x4000_2000)` with literal → expands
  to `volatile`, `observable`, `fixed_addr`
- `test_alloc_physical_non_constant`: `alloc(some_variable)` → error
  (must be compile-time constant)
- `test_alloc_unknown_value`: `alloc("CustomRegion")` → pass through, no error
- `test_alloc_no_annotation`: No `alloc` metadata → no validation needed

### Step A.2 — LLVM backend: handle expanded metadata

**File:** `src/backend/llvm/emit_binding.rs` (or existing emission site)

**What:** When the LLVM backend encounters a binding with expanded alloc
metadata, emit the appropriate LLVM IR:

| Expanded metadata | LLVM IR |
|------------------|---------|
| `alloca: true` | `%ptr = alloca i8, i64 <size>` |
| `volatile: true` + `fixed_addr: N` | `store volatile i64 %val, ptr inttoptr(i64 N to ptr)` |
| `arena_offset` + arena base | `%ptr = getelementptr i8, ptr %arena_base, i64 %offset` |
| (none) | default `alloca` or heap call |

```rust
/// Emit LLVM IR for a variable binding with alloc metadata.
/// 2026-07-12: Phase A.2
fn emit_alloc_binding(
    binding: &Binding,
    builder: &mut LlvmBuilder,
    out: &mut String,
) -> Result<(), LlvmError> {
    // Check expanded metadata (set by frontend validation in A.1)
    if let Some(addr) = binding.get_metadata_int("fixed_addr") {
        // Physical MMIO — no allocation needed, just a typed pointer
        let ptr = format!("inttoptr(i64 {} to ptr)", addr);
        builder.bind_variable(&binding.name, &ptr, binding.ty)?;
        return Ok(());
    }

    if binding.get_metadata_bool("alloca").unwrap_or(false) {
        // Stack allocation
        let size = builder.type_size(&binding.ty)?;
        writeln!(out, "  %{} = alloca i8, i64 {}", 
            binding.name, size)?;
        return Ok(());
    }

    if let Some(arena_ptr) = binding.get_metadata_str("arena_base") {
        // Arena allocation
        let offset = binding.get_metadata_int("arena_offset").unwrap_or(0);
        writeln!(out, "  %{} = getelementptr i8, ptr %{}, i64 {}",
            binding.name, arena_ptr, offset)?;
        // Bind to the memory location
        return Ok(());
    }

    if let Some(placement_ptr) = binding.get_metadata_str("placement_ptr") {
        // Placement new — alias to existing pointer
        writeln!(out, "  %{} = bitcast ptr %{} to ptr",
            binding.name, placement_ptr)?;
        return Ok(());
    }

    // Default: standard alloca
    writeln!(out, "  %{} = alloca i8, i64 {}",
        binding.name, builder.type_size(&binding.ty)?)?;
    Ok(())
}
```

**Tests:**
- `test_emit_alloc_stack`: LLVM IR contains `alloca` for `alloc("Stack")`
- `test_emit_alloc_mmio`: LLVM IR contains `inttoptr` + `volatile store`
  for `alloc(0x4000_2000)`
- `test_emit_alloc_arena`: LLVM IR contains `getelementptr` for arena
- `test_emit_alloc_placement`: LLVM IR contains `bitcast` for placement
- `test_emit_default_no_alloc`: No metadata → standard `alloca`

### Step A.3 — LLVM backend: validate known `alloc` values

**File:** `src/backend/llvm/validate.rs` (new or existing validation pass)

**What:** Before emission, the LLVM backend validates alloc metadata it
recognizes but the frontend could not fully validate. Unknown values for
known keys produce errors.

```rust
/// Validate alloc metadata in the LLVM backend.
/// 2026-07-12: Phase A.3
fn validate_alloc_metadata(defn: &Definition) -> Result<(), Vec<LlvmValidationError>> {
    let mut errors = Vec::new();

    for binding in defn.all_bindings() {
        let Some(alloc_value) = binding.get_raw_metadata("alloc") else { continue; };

        // Only validate values the LLVM backend knows about
        // Unknown values are silently passed through (forward compat)
        match alloc_value {
            PropertyValue::String(s) => {
                match s.as_str() {
                    "Stack" | "Heap" => {} // known — validate structure (done)
                    "Arena" => {
                        // Validate that arena pointer exists and is non-null
                        if !binding.has_metadata("arena_base") {
                            errors.push(LlvmValidationError::MissingArenaPointer {
                                binding: binding.name.clone(),
                            });
                        }
                    }
                    // Unknown string value → error: known key, unparseable value
                    other => {
                        errors.push(LlvmValidationError::UnknownAllocTarget {
                            target: other.to_string(),
                            binding: binding.name.clone(),
                        });
                    }
                }
            }
            PropertyValue::Integer(_) => {
                // Integer means physical address — validated by frontend in A.1
                // Backend checks: is this address valid for the target?
                if !ctx.target_memory_map().contains(alloc_value.as_int()) {
                    errors.push(LlvmValidationError::AddressNotInMemoryMap {
                        address: alloc_value.as_int(),
                        target: ctx.target_triple().to_string(),
                    });
                }
            }
            // List values — unpack and check known cases
            PropertyValue::List(items) => {
                if let Some(PropertyValue::String(s)) = items.first() {
                    if s == "Arena" {
                        // "Arena", ptr → validate ptr exists
                        if items.len() < 2 {
                            errors.push(LlvmValidationError::MissingArenaPointer {
                                binding: binding.name.clone(),
                            });
                        }
                    }
                }
            }
            // All other value types → pass through
            _ => {}
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
```

**Key rule:** Unknown key → silently ignored. Known key + unparseable
value → error. This means `alloc("QuantumGravityZone")` produces an error
because the LLVM backend knows the `alloc` key and `"QuantumGravityZone"`
is a string it doesn't recognize. A future backend that DOES recognize it
simply handles it — no other backend changes needed.

**Tests:**
- `test_alloc_unknown_string_error`: `alloc("UnknownStrategy")` → LLVM error
- `test_alloc_physical_out_of_range`: `alloc(0xFFFF_FFFF_FFFF_FFFF)` for
  32-bit target → error (address not in memory map)
- `test_alloc_placement_no_ptr`: `alloc("Arena")` without a pointer → error
- `test_alloc_unknown_key_passthrough`: `custom_key <~ "value"` → silently
  ignored (unknown key, not validated)

### Step A.4 — CIRCT backend: handle expanded metadata

**File:** `src/backend/circt.rs` — emission and validation

**What:** The CIRCT (hardware) backend reads the expanded metadata from the
`.dbvl` archive. Physical addresses become fixed hardware register
bindings. Arena allocations become BRAM blocks. Stack allocations map to
on-chip memory.

| Expanded metadata | CIRCT lowering |
|------------------|----------------|
| `alloca: true` | On-chip memory (LUTRAM / BRAM depending on size) |
| `volatile: true` + `fixed_addr: N` | Hardware register at physical address `N` |
| `arena_base` + `arena_offset` | BRAM block offset |

**Validation:** CIRCT backend checks `fixed_addr` against the target
device's memory map (same pattern as LLVM). Unknown `alloc` string values
produce CIRCT-specific errors.

```rust
// CIRCT-specific validation
fn validate_alloc_circt(binding: &Binding, target: &CirctTarget) -> Result<(), CirctError> {
    if let Some(addr) = binding.get_metadata_int("fixed_addr") {
        if !target.memory_map().contains(addr) {
            return Err(CirctError::AddressNotMapped {
                address: addr,
                device: target.device_name(),
                available_range: target.memory_map_range(),
            });
        }
    }
    Ok(())
}
```

**Tests:**
- `test_circt_alloc_mmio`: Physical address becomes hardware register binding
- `test_circt_alloc_out_of_range`: Physical address not on device → error

### Step A.5 — `.dbvl` archive: serialize expanded metadata

**File:** `src/archive/writer.rs`

**What:** The archive writer serializes the expanded form of alloc
metadata. The frontend writes the metadata it expanded (Step A.1) plus
the original `alloc` value for backends that want it:

```dbvl
// Expanded alloc metadata in the archive:
// alloc("Stack") expands to:
defn,process,,Void,"{...}",,
  {alloc:"Stack" alloca:true}

// alloc(0x4000_2000) expands to:
defn,read_switch,,UInt32,"{...}",,
  {alloc:1073745920 observable:true volatile:true fixed_addr:1073745920}
```

**Tests:**
- `test_archive_alloc_expanded`: Original `alloc` + expanded metadata
  both present in archive
- `test_archive_alloc_passthrough`: Unknown `alloc` value serialized as-is

### Step A.6 — Update metadata-dispatch.md

**File:** `docs/architecture/features/metadata-dispatch.md`

**What:** Add the `alloc` key to the namespace convention table. Add
the "known key + unparseable value → error" rule to the distributed
validation model.

Add to key namespace table:
```
| `alloc` | Frontend + all backends | Allocation annotation | `"Stack"`, `0x4000_2000`, `"Arena", ptr` |
```

Add to the distributed validation rules:
```
- Known key + unsupported value → backend error
  (the backend recognizes the key but cannot fulfill the value)
- Unknown key → silently ignored
  (the backend has no opinion about the key)
```

---

## Testing Strategy Summary

| Step | Focus | Test count delta |
|------|-------|-----------------|
| A.0 | Parse `alloc` metadata variants | ~+8 (single, integer, list, identifier) |
| A.1 | Frontend validation + expansion | ~+12 (stack escape, physical const, pass-through) |
| A.2 | LLVM emission of expanded metadata | ~+8 (alloca, MMIO, arena, placement, default) |
| A.3 | LLVM validation of alloc values | ~+8 (unknown string → error, address range, missing ptr) |
| A.4 | CIRCT emission + validation | ~+4 (register binding, out-of-range) |
| A.5 | Archive serialization | ~+4 (expanded + original both present) |

---

## Risk Register

| Risk | Mitigation |
|------|------------|
| Escape analysis is too conservative → many `alloc("Stack")` false positives | Improve escape analysis iteratively; `alloc("Stack")` failure is a compile error, not silent miscompilation |
| Physical address out of range for target | Backend validates against target memory map (Step A.3, A.4) |
| `alloc("Arena")` with null pointer | Backend checks for null; contract system can add `[arena != null]` |
| `alloc("Stack")` on GPU with no stack | Backend rejects with clear error: "GPU target has no stack" |
| Third-party alloc strategy name conflicts | `alloc` is a single key; strategy names are values, not keys — no conflict |

---

## Integration with Existing Plans

### Dependency chain

```
Extensible Types (0-7) ─── provides property system for <~ metadata
  └─ Pure Bits (8A-8F) ─── provides Value::Bits for MMIO values
      └─ Phase 8G ─── provides metadata dispatch infrastructure
          └─ Alloc Metadata (this plan) ← YOU ARE HERE
              └─ Phase 15: Library mode ─── MMIO relevant for embedded targets
              └─ Modifiers/Entry (16A-16F) ─── independent
```

### What changes in existing plans

The `.dbvl` archive plan (Phase 12) gets a new requirement: serialize
expanded alloc metadata. The metadata-dispatch architecture doc gets
the `alloc` key added to its tables and the "known key + unknown value"
rule added to its validation model.

No existing phase needs restructuring — alloc metadata is purely additive
(new `<~` key, new validation pass, new backend emission paths).
