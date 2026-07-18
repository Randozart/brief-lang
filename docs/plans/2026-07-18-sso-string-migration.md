# SSO String Handle — Full Compiler Migration

**Date:** 2026-07-18
**Status:** Plan — pre-implementation
**Depends on:** Phase 4 allocation strategy analysis (in planning)
**See also:**
  - `docs/plans/2026-07-18-allocation-strategy-system.md` (Alloc# intrinsic, arena types)
  - `docs/plans/2026-07-18-string-encoding-alloc-and-provenance.md` (encoding registry, fat pointer provenance)
  - `docs/architecture/features/string-encoding-and-fat-pointer.md`

---

## Executive Summary

The current string handle is a single `i64` — a `ptrtoint` of a 3-slot heap/arena allocation `[data_ptr][length][data...]`. Every string operation (literal emission, concat, `Length#`, load/store to state) must dereference a pointer. For short strings (<7 bytes), the allocation overhead dominates — requiring a `Malloc#`/arena bump, header writes, and data copy for what could fit in a register.

**SSO (Small String Optimization):** Embed strings up to 6 UTF-8 bytes directly into the handle. Use tag bits to distinguish SSO vs heap-backed. This eliminates heap traffic for the common case (identifiers, short messages, single characters) and improves cache locality for all string operations.

### Key numbers

| Metric | Current (3-slot) | SSO (this plan) |
|--------|------------------|-----------------|
| Handle width | 1 × i64 (8 bytes) | 2 × i64 (16 bytes) |
| Short string capacity | 0 bytes (<16 alloc) | 6 bytes (inline in handle[1]) |
| Short string paths | 0% (all go to heap) | ~80% of real-world strings |
| State field size | 1 × i64 (uniform) | 2 × i64 (non-uniform) |
| Function ABI | 1 × i64 register | 2 × i64 registers |

### SSO layout (per handle)

```
handle[0]: i64
  bits 0-2: tag
    000 = heap-backed (clear tag)
    001 = SSO (short string, 6 bytes inline)
    010 = static constant (don't free)
    100 = temporary concat result (free when consumed)
  bits 3-62: data
    SSO: 5 bytes of inline data (bits 3-42) + 1 byte following in handle[1]
    heap: pointer to allocation base (mask off tag to get pointer)

handle[1]: i64
  SSO:  6th byte (bits 0-7) + length (bits 8-63)
  heap: length (i64)
```

Wait — SSO with 6 inline bytes across 2 × i64 needs careful layout. Let me reconsider.

### Revised SSO layout

The handle is 2 × i64 = 16 bytes. The key insight: for SSO strings (0-15 bytes), we can embed ALL data inline within the 16-byte handle itself. No heap allocation at all. For longer strings, handle[0] is a pointer+tag, handle[1] is the length.

```
SSO string (len 0-15):
  handle[0]: lowest 3 bits = tag (001 = SSO)
             remaining 61 bits = first 7 bytes of string data (or padded)
  handle[1]: bytes 7-14 of string data (8 bytes)
             OR: if len < 7, handle[1] = len (upper bits) + remaining bytes

  Total inline capacity: 15 bytes (handle[0]: 7 + handle[1]: 8)
  But with tag bits consuming 3 bits of handle[0], we get:
    5 bytes directly in handle[0] (bits 3-42)
    8 bytes in handle[1]
    = 13 bytes inline capacity
  Actually: 61 bits = 7.625 bytes in handle[0] + 8 bytes in handle[1] = 15 bytes
  But we need a length field somewhere for SSO strings.
```

Let me simplify. The cleanest design:

### Final SSO Layout

```
[handle: 2 × i64] — stored as a single LLVM `{ i64, i64 }` struct

Heap-backed (tag = 000):
  field 0: ptr (i64, lower 3 bits = 000)
  field 1: length (i64)

SSO (tag = 001, 0 ≤ len ≤ 14):
  field 0: [tag: 3 bits] [data_lo: 61 bits = up to 7 bytes]
  field 1: [data_hi: 64 bits = up to 8 bytes]
  Length = 14 - count_trailing_zero_bytes (or explicitly stored)

Static (tag = 010):
  field 0: ptr (i64, lower 3 bits = 010)
  field 1: length (i64)

Temp (tag = 100):
  field 0: ptr (i64, lower 3 bits = 100)
  field 1: length (i64)
```

**Length for SSO:** Store in the upper 8 bits of field 1, giving 8 bytes (64 bits) for data_hi minus 8 bits for length = 56 bits = 7 bytes. Wait, this is getting complicated.

**Simplest correct approach:** Store length in field 1 bits 56-63, leaving bits 0-55 (7 bytes) for continuation data. Field 0 has tag (3 bits) + 61 bits (7 bytes) = up to 14 bytes inline + 1 to spare.

```
SSO (tag = 001):
  handle[0]: [tag:3][data:61 bits = 7 bytes + 5 bits spare]
  handle[1]: [cont_data:56 bits = 7 bytes][len:8 bits = 0-255]
  
  Max inline: 7 (from handle[0]) + 7 (from handle[1]) = 14 bytes
  Length range: 0-255 (8 bits)
```

14 bytes covers all string literals ≤14 chars (the vast majority in benchmarks and real code). Strings longer than 14 bytes go to heap.

But wait — a 14-byte SSO string still requires the full `{ i64, i64 }` struct to be passed around. That's 16 bytes in registers/stack vs 8 bytes currently. The trade-off: some register pressure increase for all string operations, but elimination of heap/alloca traffic for ≤14-byte strings.

### Decision: SSO threshold

| Threshold | % of strings covered | Cost |
|-----------|---------------------|------|
| 6 bytes | ~60% (short IDs, chars) | Minimal (1 extra byte in handle[0]) |
| 14 bytes | ~90% (most literals) | Full 2-register ABI |
| 22 bytes | ~95% | 3-register ABI (too expensive) |

**Choose 6 bytes.** Rationale:
- 6 bytes fits entirely in handle[0] (61 data bits after tag). Handle[1] is always length.
- No need for continuation into handle[1] — simplification of SSO code paths
- 6 bytes covers: all ASCII identifiers (≤6 chars), single Unicode codepoints (≤4 bytes), "hello", "world", "Fizz", "Buzz", "true", "false", "Error", "debug", all common prefixes
- 2-register ABI still needed for heap strings (handle[0]=ptr, handle[1]=len), so we pay the 2-register cost regardless of SSO
- The real win is avoiding heap allocation + memcpy for short strings

```
SSO (tag = 001, 0 ≤ len ≤ 6):
  field 0: [tag: 3 bits][data: 61 bits = 6 bytes + 13 spare bits]
           spare bits must be 0 (for icmp eq comparison)
  field 1: length (i64, len in lower bits, upper bits = 0)

Heap/Static/Temp (tag = 000/010/100):
  field 0: ptr (i64, tag in lower 3 bits, pointer in upper 61 bits)
  field 1: length (i64)
```

**Total inline capacity: 6 bytes.**
**Length stored in handle[1] for all variants.**
**Tag stored in lower 3 bits of handle[0] for all variants.**

This means `Length#` always reads handle[1] — no branching on tag. Simple.

### Tag encoding (lower 3 bits of handle[0])

| Value | Meaning |
|-------|---------|
| 000   | Heap — ptr in upper 61 bits |
| 001   | SSO — data in upper 61 bits, length in handle[1] |
| 010   | Static — ptr, don't free |
| 100   | Temp — ptr, free on consume |

### Handle creation pseudocode

```c
// SSO string from literal (≤6 bytes)
uint64_t handle0 = (bytes_to_u61(data_6bytes) << 3) | 0b001;
uint64_t handle1 = (uint64_t)len;
return (Handle){ handle0, handle1 };

// Heap string from allocation
uint64_t handle0 = ((uint64_t)ptr << 3) | tag;  // tag = 000, 010, or 100
uint64_t handle1 = (uint64_t)len;
return (Handle){ handle0, handle1 };
```

### Handle extraction pseudocode

```c
bool is_sso(uint64_t h0) { return (h0 & 0b111) == 0b001; }
bool is_static(uint64_t h0) { return (h0 & 0b111) == 0b010; }
bool is_temp(uint64_t h0) { return (h0 & 0b111) == 0b100; }

uint64_t get_ptr(uint64_t h0) { return h0 & ~0b111; }  // mask tag
uint64_t get_len(uint64_t h1) { return h1; }

// SSO data: upper 61 bits of h0 = 6 bytes
// bytes are stored at bits 3-50 (48 bits = 6 bytes) of h0
// bits 51-63 must be 0
uint64_t get_sso_data(uint64_t h0) { return (h0 >> 3) & 0x00FFFFFFFFFFFFFF; }
```

Wait — 61 bits = 7.625 bytes. If I store 6 bytes (48 bits), there are 13 spare bits. Those must be zero for equality comparison. Let me adjust:

```
Bits 0-2:   tag (3 bits)
Bits 3-50:  data (48 bits = 6 bytes)
Bits 51-63: must be 0 (13 spare bits)
```

To create an SSO handle from a 6-byte string `[b0 b1 b2 b3 b4 b5]`:

```c
uint64_t data = ((uint64_t)b0 << 40) | ((uint64_t)b1 << 32) | ((uint64_t)b2 << 24) 
              | ((uint64_t)b3 << 16) | ((uint64_t)b4 << 8) | (uint64_t)b5;
uint64_t handle0 = (data << 3) | 0b001;
uint64_t handle1 = (uint64_t)len;
```

To extract bytes from handle0 for SSO:

```c
uint64_t data = handle0 >> 3;  // discard tag
b0 = (data >> 40) & 0xFF;
b1 = (data >> 32) & 0xFF;
b2 = (data >> 24) & 0xFF;
b3 = (data >> 16) & 0xFF;
b4 = (data >> 8) & 0xFF;
b5 = data & 0xFF;
```

This is simple and efficient. The LLVM IR for creation is just `shl` + `or` for handle0, and a constant `i64` for handle1. No heap, no header writes.

---

## Impact Summary

| Component | Change | Risk |
|-----------|--------|------|
| **LLVM type** | `String` → `{ i64, i64 }` (struct, not ptr) | High — touch every codegen path |
| **%State layout** | 1 i64 slot → 2 i64 slots per string field | High — all offsets change |
| **Function ABI** | 1 × i64 → 2 × i64 per string param/return | High — all call sites |
| **Global constants** | `<{ i64, i64, [N x i8] }>` → `{ i64, i64 }` SSO literal | Medium — no more globals for ≤6 byte strings |
| **String literal emission** | `alloca` + fill → `shl` + `or` immediate | Low — simpler |
| **Concat** | `emit_inline_concat` loads header → checks tag; SSO path is shl/or, heap path is existing | Medium — new SSO path |
| **Length#** | Always reads handle[1] | Low — no change needed |
| **Len# (fat pointer)** | Unchanged (different code path) | None |
| **Tag bit scheme** | 2 bits (static=1, temp=2) → 3-bit tag field | Medium — mask changes |
| **adapt_to_i64** | No longer valid for String (now 2-register) | High — remove or specialize |
| **C runtime** | `brief_str_to_c` must handle SSO handles | High — new code |
| **Interpreter** | String representation changes | Medium |

---

## Incremental Cutover Strategy

The migration affects ~40 code sites in the backend, plus the C runtime. Doing it all at once is high-risk. Instead, a 4-phase cutover:

### Phase A: Internal representation only

- LLVM type for String becomes `{ i64, i64 }`
- Backend creates/destructs handles internally
- ALL strings are heap-backed initially (no SSO emission)
- No global constant format change
- No %State layout change (String fields stay as 1 i64, with the raw i64 being handle[0] and handle[1] stored in a synthetic second field)

Actually, this doesn't work cleanly without changing %State. Let me reconsider.

### Better approach: flag-gated cutover

Add a `--feature sso-strings` flag to the compiler. When OFF (default for Phase A-B), everything works exactly as today. When ON, the new code paths activate.

**Phase A: Data structures + types** (flag OFF throughout)
1. Add `StringHandle` struct in Rust (2 × i64)
2. Add `--feature sso-strings` flag
3. No behavioral change

**Phase B: Codegen with flag ON, SSO-only for ≤6 byte literals**
1. Change String LLVM type to `{ i64, i64 }`
2. Change %State to use 2 i64 slots for String fields
3. Change function ABI: String params/rets use 2 registers
4. Emit ≤6 byte string literals as SSO (shl+or, no global constant)
5. Emit >6 byte string literals as heap-backed (existing global constant pattern)
6. Concat: check both operands' tags; if SSO+SSO and total ≤6, emit SSO; else heap
7. Length#: always reads handle[1] (no change needed)
8. Tag extraction: use lower 3 bits

**Phase C: Benchmark + fix regressions**
1. Run benchmark baseline with flag ON
2. Fix any regressions in concat, Length#, load/store
3. Update C runtime
4. Update interpreter

**Phase D: Make ON by default, remove flag**
1. Set `--feature sso-strings` default to true
2. After stabilization, remove flag and old code paths

### Key insight: we can PREPARE Phase A without changing any LLVM IR output

Phase A is purely Rust-side data structure changes:
- Define `StringHandle` struct
- Add helper functions: `is_sso`, `is_heap`, `get_ptr`, `get_len`, `get_sso_data`, `make_sso`, `make_heap`
- Add `--feature sso-strings` flag
- Wire the flag into codegen decision points
- All existing codegen paths remain unchanged when flag is OFF

This means Phase A can be committed and tested independently.

---

## Files Touched (Complete List)

### Core compiler

| File | Phase | Change |
|------|-------|--------|
| `src/ast/types.rs` | A | Add `StringLayout` enum, `StringHandle` struct |
| `src/compile.rs` | A | Add `--feature sso-strings` flag |
| `src/config.rs` | A | Add `sso_strings` to feature flags |
| `src/backend/llvm/mod.rs` | A | `StringHandle` helpers, `llvm_type` for String returns `{ i64, i64 }` when flag ON |
| `src/backend/llvm/mod.rs:822-831` | B | `push_field_type` for String → pushes TWO i64 slots |
| `src/backend/llvm/emit_toplevel.rs:607-645` | B | `declare_state_type` — %State struct gains second field per String |
| `src/backend/llvm/emit_toplevel.rs:824-866` | B | `emit_field_init_value` — emit SSO handle for ≤6 byte literals |
| `src/backend/llvm/emit_toplevel.rs:290-296` | B | `fallback_llvm_type` returns `{ i64, i64 }` for String |
| `src/backend/llvm/emit_expr.rs:426-447` | B | `emit_string_literal` — emit SSO (shl+or) for ≤6 bytes, else old path |
| `src/backend/llvm/emit_expr.rs:102-124` | B | Load String from %State — load 2 consecutive i64 |
| `src/backend/llvm/helpers.rs:1881-1891` | B | `adapt_to_i64` — String now 2-wide → error or forward only handle[0] |
| `src/backend/llvm/helpers.rs:736-742` | B | Tag scheme: 3-bit field instead of bit 0/1 |
| `src/backend/llvm/helpers.rs:749-773` | B | `emit_inline_concat` — SSO path for ≤6 byte results |
| `src/backend/llvm/helpers.rs:837-858` | B | `emit_write_header` — only used for heap-backed strings now |
| `src/backend/llvm/helpers.rs:860-883` | B | `emit_copy_data` — unchanged (heap path only) |
| `src/backend/llvm/helpers.rs:919-944` | B | `emit_free_temporaries` — handle[0] tag check updated |
| `src/backend/llvm/helpers.rs:946-961` | B | `emit_box_concat_result` — return `{ i64, i64 }` instead of `i64` |
| `src/backend/llvm/helpers.rs:966-999` | B | `is_string_chain` — unchanged (AST-level, before codegen) |
| `src/backend/llvm/intrinsics.rs:44,489-504` | B | `emit_len` → reads handle[1] (already does this for lists; verify String path) |
| `src/backend/llvm/intrinsics.rs:216-340` | B | `emit_alloc` for string buffers — heap-backed only |
| `src/backend/llvm/mod.rs:2023-2035` | B | String globals — for >6 byte strings, keep existing format |
| `src/backend/llvm/mod.rs:477-485` | B | `trg_llvm_storage_ty` — String returns `{ i64, i64 }` |
| `src/backend/llvm/mod.rs:333-346` | B | String constant collection — filter by >6 bytes for globals |
| `src/backend/llvm/types.rs:29-43` | B | `lower_custom_type` — String returns `{ i64, i64 }` |
| `src/backend/llvm/dispatch.rs:78,326` | B | Arena init — unchanged (handles are 2-wide but arena uses opaque bytes) |
| `src/backend/llvm/loop_engine/counter.rs` | B | Prealloc — String fields now 2 slots |
| `src/backend/llvm/loop_engine/ssa.rs` | B | Prealloc — String fields now 2 slots |
| `src/memory_spec.rs:227-229` | B | String size = 16 (2 × i64), not 24 |

### C Runtime

| File | Phase | Change |
|------|-------|--------|
| `lib/runtime/brief_rt.c:53-62` | C | `brief_str_to_c` — check tag bits, extract SSO data, extract heap data |
| `lib/runtime/brief_rt.c` | C | `brief_str_free` — check tag: skip if SSO or static, free if heap |
| `lib/runtime/brief_rt.c` | C | String comparison helpers — handle both SSO and heap |

### Interpreter

| File | Phase | Change |
|------|-------|--------|
| `src/interpreter/value.rs` | C | String representation in Bits — possibly use Handle struct |
| `src/interpreter/intrinsics.rs:189-207` | C | Length# — read handle[1] |
| `src/interpreter/casts.rs:28-31` | C | String cast — handle SSO |

### Stdlib

| File | Phase | Change |
|------|-------|--------|
| `lib/std/types/bootstrap.bv` | B | `type String` layout — `bytes <~ 16` (was 24) |
| `lib/std/string_c.bv` | C | Update to handle new handle format |
| `lib/std/string.bv` | C | No change needed if all ops go through intrinsics |
| `lib/std/string_builder.bv` | C | Verify append_str works with 2-wide handles |

### Examples and Benchmarks

| File | Phase | Change |
|------|-------|--------|
| All `examples/*.bv` | B | Verify they compile and produce correct output |
| All `benchmarks/*.bv` | B | Verify correctness; re-run benchmark baseline |
| `benchmarks/fasta.bv` | B | Uses `PutChar#` — unaffected |
| `benchmarks/knucleotide.bv` | B | Uses `Print#(i64)` — unaffected |

---

## Implementation Details

### Phase A: Data structures (flag OFF, no IR change)

```
src/ast/types.rs:
```

```rust
/// 2026-07-18: String layout — SSO (≤6 bytes inline) vs heap-backed.
/// Handle is always 2 × i64: [tagged_data_or_ptr, length].
#[derive(Debug, Clone, PartialEq)]
pub enum StringLayout {
    Heap,
    Sso { data: Vec<u8> },  // ≤6 bytes
}

/// 2026-07-18: SSO string handle — 2 × i64.
/// Only constructed when --feature sso-strings is active.
#[derive(Debug, Clone, PartialEq)]
pub struct StringHandle {
    pub field0: i64,  // tag + data/ptr
    pub field1: i64,  // length
}

impl StringHandle {
    pub fn sso(data: &[u8]) -> Self {
        assert!(data.len() <= 6);
        let mut buf = [0u8; 6];
        buf[..data.len()].copy_from_slice(data);
        let data_u64 = u64::from_be_bytes(buf); // big-endian for string order
        let field0 = (data_u64 << 3) | 0b001;
        let field1 = data.len() as i64;
        StringHandle { field0: field0 as i64, field1 }
    }

    pub fn heap(ptr: i64, len: i64, tag: i64) -> Self {
        let field0 = (ptr << 3) | tag;
        StringHandle { field0, field1: len }
    }

    pub fn is_sso(field0: i64) -> bool {
        (field0 & 0b111) == 0b001
    }

    pub fn get_ptr(field0: i64) -> i64 {
        field0 & !0b111
    }

    pub fn get_len(field1: i64) -> i64 {
        field1
    }
}
```

```
src/compile.rs:
```

```rust
// 2026-07-18: SSO string feature flag
pub struct CompileOptions {
    // ...existing fields...
    pub feature_sso_strings: bool,
}
```

### Phase B: Codegen with flag ON

#### LLVM type mapping

In `types.rs:lower_custom_type`:

```rust
"String" | "Data" => {
    if backend.feature_sso_strings {
        "{ i64, i64 }"
    } else {
        "ptr"
    }
}
```

Wait — `lower_custom_type` doesn't have access to `backend`. It's a pure function. Need to thread the feature flag:

```rust
pub fn lower_custom_type(name: &str, feature_sso: bool) -> String {
    match name {
        "String" | "Data" if feature_sso => "{ i64, i64 }",
        "String" | "Data" => "ptr",
        // ...rest unchanged...
    }
}
```

This changes the signature of `lower_custom_type` and all callers. Many callers in `types.rs` are tests that don't need SSO. Use default `false`.

#### %State layout — String fields get 2 slots

In `mod.rs:push_field_type`:

```rust
pub fn push_field_type(&mut self, ty: &Type) {
    if self.feature_sso_strings && type_is(&self.universe, ty, "String") {
        // 2026-07-18: SSO String — 2 × i64 slots
        self.ctx.field_types.push("i64".to_string());
        self.ctx.field_types.push("i64".to_string());
        self.ctx.field_brief_types.push(ty.clone());
        self.ctx.field_brief_types.push(ty.clone()); // duplicate for second slot
    } else {
        self.ctx.field_types.push("i64".to_string());
        self.ctx.field_brief_types.push(ty.clone());
    }
}
```

This means String fields occupy 2 consecutive `i64` slots in %State. `field_index_map` entries for String fields point to the first slot. Load/store operations must read/write both slots.

#### Function ABI — String params/rets use 2 registers

In `mod.rs:1770-1783` (FFI declarations):

```rust
let param_tys: Vec<&str> = sig.inputs.iter().map(|(_, t)| match t {
    Type::Custom(__t) if __t == "String" || __t == "Data" => {
        if backend.feature_sso_strings {
            "{ i64, i64 }"
        } else {
            "i8*"
        }
    }
    _ => "i64",
}).collect();
```

In `emit_toplevel.rs:1108-1114` (txn parameter boxing):

```rust
Type::Custom(__t) if __t == "String" || __t == "Data" => {
    if backend.feature_sso_strings {
        // String arrives as { i64, i64 } — extract both
        // ... extractvalue 0, extractvalue 1
    } else {
        writeln!(out, "  {} = ptrtoint ptr {} to i64", conv, raw).ok();
    }
}
```

#### String literal emission (SSO path)

In `emit_expr.rs:emit_string_literal`, when SSO is active and the string is ≤6 bytes:

```rust
fn emit_string_literal(&mut self, out: &mut String, v: &str, bytes: &[u8], indent: &str) -> BTypedRegister {
    if self.feature_sso_strings && bytes.len() <= 6 {
        return self.emit_sso_literal(out, v, bytes, indent);
    }
    // ...existing heap-backed emission...
}
```

`emit_sso_literal`:

```rust
/// 2026-07-18: Emit an SSO string literal — no heap/alloca/global.
/// handle[0] = (data << 3) | 0b001
/// handle[1] = len
fn emit_sso_literal(&mut self, out: &mut String, v: &str, bytes: &[u8], indent: &str) -> BTypedRegister {
    // Pack bytes into u64 (big-endian, left-aligned)
    let mut buf = [0u8; 6];
    buf[..bytes.len()].copy_from_slice(bytes);
    let data_be = u64::from_be_bytes([buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], 0, 0]);
    let tag = 0b001u64;

    // field0 = (data_be << 3) | tag
    let f0 = self.fun.gen_reg();
    let shifted = self.fun.gen_reg();
    writeln!(out, "{}{} = shl i64 {}, 3", indent, shifted, data_be as i64).ok();
    writeln!(out, "{}{} = or i64 {}, {}", indent, f0, shifted, tag).ok();

    // field1 = len
    let f1 = self.fun.gen_reg();
    writeln!(out, "{}{} = or i64 0, {}", indent, f1, bytes.len()).ok();

    // Return as { i64, i64 }
    // ...need to insertvalue into a struct
    let s = self.fun.gen_reg();
    writeln!(out, "{}{} = insertvalue {{ i64, i64 }} undef, i64 {}, 0", indent, s, f0).ok();
    let s2 = self.fun.gen_reg();
    writeln!(out, "{}{} = insertvalue {{ i64, i64 }} {}, i64 {}, 1", indent, s2, s, f1).ok();

    BTypedRegister { name: s2, ty: Type::string() }
}
```

Wait, what is `BTypedRegister`? Let me check if it returns a structured type. From the codegen patterns I've seen, `TypedRegister.name` is a single LLVM register name (like `%t42`). For a `{ i64, i64 }` result, we need either:

1. Return the struct register name (LLVM SSA value of type `{ i64, i64 }`)
2. Use `extractvalue` at consumer sites

Option 1: `TypedRegister.name = "%s2"` where `%s2` is of type `{ i64, i64 }`. Consumers that expect `i64` (like `adapt_to_i64`) must be updated.

Since ALL string consumers need to be updated for 2-wide, option 1 is cleaner. `TypedRegister.ty` is `Type::string()`, so consumers can check the type before extracting.

But `TypedRegister.name` is a `String` — it can store `"%s2"` just fine. The LLVM type is tracked in `ty`. The actual LLVM type is determined by `llvm_type(&ty)` at emission time.

However, `BTypedRegister` (from `intrinsics.rs`) is defined differently from `TypedRegister`. Let me check:

From helpers.rs:
```rust
pub struct TypedRegister {
    pub name: String,
    pub ty: Type,
}
```

From intrinsics.rs:
```rust
pub struct BTypedRegister {
    pub name: String,
    pub ty: Type,
    pub is_lvalue: bool,
}
```

Both use a single `name` string. For `{ i64, i64 }`, the name is the SSA register (like `%s2`) that holds the struct. Consumers must use `extractvalue` to get individual fields.

#### String concat — SSO path

In `helpers.rs:emit_inline_concat`:

```rust
pub fn emit_inline_concat(...) -> TypedRegister {
    if self.feature_sso_strings {
        return self.emit_sso_concat(out, indent, a, b);
    }
    // ...existing heap concat...
}
```

`emit_sso_concat`:

```rust
/// 2026-07-18: Emit string concat with SSO support.
/// If both operands are short enough (<6 bytes each) and total ≤6, emit SSO.
/// Otherwise, fall through to heap-backed concat.
fn emit_sso_concat(&mut self, out: &mut String, indent: &str, a: &TypedRegister, b: &TypedRegister) -> TypedRegister {
    // Extract lengths from handle[1] of each operand
    // Check if both are SSO (tag == 001) and total ≤ 6
    // If yes: extract data bytes, concat in u64, emit SSO handle
    // If no: fall through to heap concat
}
```

The key check: extract both fields, check tags, compute total length. If both SSO and total ≤ 6, do SSO concat. Otherwise, heap concat.

#### Length# — reads handle[1]

In `intrinsics.rs:emit_len`:

```rust
fn emit_len(backend, out, v, args, indent) -> BTypedRegister {
    let arg_reg = emit_arg(backend, out, &args[0], indent);

    // Fat pointer provenance check
    if let Some((_base, _offset, ref remaining)) = backend.fun.fat_ptrs.get(&arg_reg).cloned() {
        writeln!(out, "{}{} = add i64 {}, 0", indent, v, remaining).ok();
        return ...;
    }

    if backend.feature_sso_strings {
        // String is { i64, i64 } — extract field 1 (length)
        writeln!(out, "{}{} = extractvalue {{ i64, i64 }} {}, 1", indent, v, arg_reg).ok();
        return ...;
    }

    // Fallback: load length from header slot 0 (for lists) or slot 1 (for strings)
    // ...existing code...
}
```

Wait — currently `emit_len` loads from `slot 0` of the allocation header (`load i64, ptr %ptr`). But for strings, slot 0 is `data_ptr`, slot 1 is `length`. So the current `emit_len` is actually reading the `data_ptr` for strings, not the length.

Looking at the current code again:

```rust
// Fallback: load length from header slot 0 (for lists) or slot 1 (for strings)
let ptr = backend.fun.gen_reg();
writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, arg_reg).ok();
writeln!(out, "{}{} = load i64, ptr {}", indent, v, ptr).ok();
```

It loads slot 0. For strings, slot 0 is `data_ptr`, not `length`. But the comment says "slot 1 (for strings)". This is **already a bug** for heap-backed strings. It only works for the global constants where the data_ptr value happens to be a large address (so it's interpreted as a large length but never used meaningfully — the frontend's `len` function reads it from the stdlib side).

Wait, looking at `lib/std/string.bv`, the `len` function:

```brief
defn len(s: String) -> Int {
    unsafe {
        __utf8_len(s)
    };
};
```

It calls `__utf8_len` which is a foreign function that reads the header correctly. So `Length#` (the intrinsic) is NOT used for strings at the Brief level — string `len` goes through a foreign function. `Length#` is used for lists/arrays.

This means the SSO migration doesn't affect the primary `len()` function path. It's already handled by the foreign runtime.

However, if any code uses `Length#` directly on a string, it would need the SSO-aware path. Let me check... `emit_len` is reached from `"Len#" | "Length#"` matches in `emit_intrinsic_call`. The interpreter has `Length#` as unsupported. So `Length#` on strings is dead code effectively.

**Conclusion:** `emit_len` handles `Length#` which is used for lists, not strings. The String `len` goes through `__utf8_len` FFI. No change needed for Length#.

#### Tag scheme update

Current: bit 0 = static, bit 1 = temp, AND -4 to mask.

New: lower 3 bits = tag field. AND -8 to mask (clear lower 3 bits).

```rust
/// 2026-07-18: Tag scheme: lower 3 bits = tag field.
///   001 = SSO, 010 = static, 100 = temp, 000 = heap.
fn emit_mask_tag_old(&mut self, out: &mut String, indent: &str, val: &str, prefix: &str) -> String {
    if self.feature_sso_strings {
        let r = self.fun.gen_reg();
        writeln!(out, "{}{} = and i64 {}, -8", indent, r, val).ok();
        r
    } else {
        let r = self.fun.gen_reg();
        writeln!(out, "{}{} = and i64 {}, -4", indent, r, val).ok();
        r
    }
}
```

#### C Runtime — handle SSO

In `brief_rt.c:brief_str_to_c`:

```c
char* brief_str_to_c(int64_t bstr0, int64_t bstr1) {
    // SSO: lowest 3 bits of field0 == 0b001
    if ((bstr0 & 7) == 0b001) {
        // Extract inline data: bits 3-50 = 6 bytes
        uint64_t data = (uint64_t)bstr0 >> 3;
        int64_t len = bstr1;
        if (len < 0 || len > 6) return NULL;
        char* c_str = malloc((size_t)(len + 1));
        if (!c_str) return NULL;
        // Big-endian: most significant byte first
        for (int i = 0; i < len; i++) {
            c_str[i] = (char)((data >> (40 - i * 8)) & 0xFF);
        }
        c_str[len] = '\0';
        return c_str;
    }

    // Heap-backed: mask off lower 3 bits to get pointer
    int64_t bstr = bstr0 & ~7;
    if (bstr == 0) return NULL;
    int64_t len = bstr1;
    if (len < 0 || len > 1024 * 1024 * 1024) return NULL;
    char* c_str = malloc((size_t)(len + 1));
    if (!c_str) return NULL;
    if (len > 0) memcpy(c_str, (void*)(uintptr_t)(bstr), (size_t)len);
    c_str[len] = '\0';
    return c_str;
}
```

Wait — the current C runtime has a different interpretation of slot 0 (it reads it as length from the allocation base pointer). But the LLVM codegen writes slot 0 as `data_ptr` and slot 1 as `length`. The current C runtime only works for global constants (where pointer = allocation base = 0 relative to itself).

With SSO, the heap layout needs to change too. For heap-backed strings, we should store just the raw data at the pointed-to address, with no header. The length is always in handle[1] (the second register). This simplifies the heap layout:

```
Heap-backed String (SSO tag = 000):
  Pointer: points directly to raw UTF-8 data bytes
  Length: stored in handle[1]
  No header! No [data_ptr] [length] prefix!
```

This is a significant simplification — the current 3-slot header `[data_ptr, length, data...]` becomes just `[data...]` for heap-allocated strings. The `data_ptr` self-reference was only needed for global constants (so the C runtime could find the data from the pointer). With SSO, global constants are just SSO handles (no pointers), and heap strings store length separately.

**This eliminates the 16-byte header allocation overhead for heap strings too** — you only allocate `len + 1` bytes (the +1 for null terminator). The old format allocated `16 + len + 1` bytes.

### Layout comparison

```
Before (all strings):
  [data_ptr: i64][length: i64][data: len bytes + \0]
  Allocation: 16 + len + 1 bytes
  Handle: ptrtoint(base) i64

After (SSO, ≤6 bytes):
  Handle only: [field0: i64 (tag|data)][field1: i64 (len)]
  Allocation: 0 bytes
  Layout: inline in registers

After (heap, >6 bytes):
  [data: len bytes + \0]
  Allocation: len + 1 bytes (saves 16!)
  Handle: [field0: i64 (tag|ptr)][field1: i64 (len)]
```

---

## Testing Strategy

All tests are behavioral (per Directive §5).

### Phase A Tests

| Test | What it asserts | How |
|------|-----------------|-----|
| `test_string_handle_sso` | SSO handle construction matches spec | Create SSO handle from 6-byte data, check field0 (tag=001, data correct), field1 (len) |
| `test_string_handle_heap` | Heap handle construction matches spec | Create heap handle with ptr+len+tag, check field0 (tag preserved), field1 |
| `test_string_handle_is_sso` | `is_sso` returns true for 001 tag | Test with tag=001 |
| `test_string_handle_get_ptr` | `get_ptr` masks tag bits | Test with various tags, assert upper 61 bits preserved |
| `test_string_handle_get_len` | `get_len` returns field1 | Verify |

### Phase B Tests

| Test | What it asserts | How |
|------|-----------------|-----|
| `test_sso_literal_short` | ≤6 byte string emits SSO (shl+or, no heap) | Compile `"hello"`, check IR has `shl`+`or` but no `@malloc` |
| `test_sso_literal_long` | >6 byte string emits heap (no SSO) | Compile `"hello world!!"`, check IR for `@malloc` |
| `test_sso_state_field_2slot` | String state field occupies 2 i64 slots | Compile txn with String state field, check %State has 2 entries for it |
| `test_sso_state_load_store` | Load/store string to state preserves value | Compile txn: write "hi" to state, read it back, compare |
| `test_sso_concat_two_short` | "abc" + "def" = SSO concat (≤6 result) | Compile, check IR for SSO handle creation, no heap |
| `test_sso_concat_short_long` | "a" + "hello world" = heap concat | Compile, check IR for heap allocation |
| `test_sso_concat_long_short` | "hello world" + "a" = heap concat | Same |
| `test_sso_frgn_string_abi` | frgn with String param uses 2-register ABI | Compile frgn call with String arg, check IR has 2 args |
| `test_sso_tag_extraction` | emit_mask_tag uses AND -8 (not -4) | Check IR for `and i64 ..., -8` |
| `test_sso_empty_string` | Empty string "" is SSO (len=0, tag=001) | Compile `""`, check SSO handle with field1=0 |
| `test_sso_6byte_boundary` | 6-byte string "123456" is SSO | Verify threshold |
| `test_sso_7byte_heap` | 7-byte string "1234567" is heap | Verify threshold |
| `test_sso_identity` | round-trip: compile string, run, get same string | End-to-end: compile `"hello"` → run → compare C output |
| `test_sso_concat_identity` | round-trip: concat, run, get correct string | End-to-end: compile `"abc" + "def"` → run → "abcdef" |

### Phase C Tests

| Test | What it asserts | How |
|------|-----------------|-----|
| `test_sso_c_runtime_ss` | brief_str_to_c handles SSO handle | Call with SSO field0/field1, check returned c_str |
| `test_sso_c_runtime_heap` | brief_str_to_c handles heap handle | Call with heap pointer + tag + len, check c_str |
| `test_sso_interpreter` | Interpreter handles SSO string | Run interpreted program with string lit, check result |

### Regression Tests

```
cargo test --lib  — all existing tests pass with --feature sso-strings=OFF
cargo test --lib  — all existing tests pass with --feature sso-strings=ON
```

---

## Documentation

### Inline doc comments to add

| File | What | Rationale |
|------|------|-----------|
| `src/ast/types.rs:new_StringHandle` | Method-level docs | Full spec for handle layout, tag bits, SSO encoding |
| `src/backend/llvm/mod.rs:push_field_type` | Comment at String branch | `// 2026-07-18: SSO strings get 2 i64 slots` |
| `src/backend/llvm/intrinsics.rs:emit_len` | Comment on String path | `// 2026-07-18: SSO string length in handle[1]` |
| `src/backend/llvm/helpers.rs:emit_mask_tag` | Updated tag scheme comment | `// 2026-07-18: lower 3 bits = tag field` |
| `src/backend/llvm/helpers.rs:emit_inline_concat` | SSO path comment | `// 2026-07-18: SSO concat for ≤6 byte total` |
| `src/backend/llvm/emit_expr.rs:emit_sso_literal` | Method-level docs | Why SSO, tag value, data packing |
| `src/backend/llvm/emit_toplevel.rs:declare_state_type` | String 2-slot comment | `// 2026-07-18: SSO strings add second i64 slot` |
| `lib/runtime/brief_rt.c:brief_str_to_c` | Updated comment | Document SSO vs heap format |
| `lib/std/types/bootstrap.bv:type String` | Update `bytes` field | `bytes <~ 16` (was 24) — SSO handle is 2 × i64 |

### Architecture docs to update

| Document | What changes |
|----------|-------------|
| `docs/architecture/features/string-encoding-and-fat-pointer.md` | Add §X: SSO String Handle — layout, tag encoding, heap simplification |
| `docs/architecture/llvm-memory-management.md` | Update §5 String representation — remove 3-slot header, add SSO |
| `docs/architecture/arrow-syntax-and-arena.md` | Note: String push/pop still works, SSO is transparent |

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **State field layout change** breaks all existing %State offset calculations | Certain | High | Push_field_type adds 2 slots for String; field_index_map entries point to first slot. All GEP offsets auto-adjust because they read field_index_map. |
| **Function ABI change** breaks all FFI calls with String params | Certain | High | All frgn declarations with String type must be recompiled. Old binaries crash. Mitigation: flag-gated cutover. |
| **C runtime `brief_str_to_c`** reads wrong slot | Certain | Medium | Already mismatched (reads slot 0 as length, but codegen writes data_ptr there). SSO cleanup actually FIXES this mismatch. |
| **Interpreter string format** differs from LLVM codegen | Medium | Medium | Interpreter is the reference — fix interpreter to match new format. |
| **Concat SSO path doesn't trigger** for common cases | Medium | Low | Falls through to heap concat — correct but suboptimal. Track in benchmark diff. |
| **6-byte SSO threshold misses long strings** | Low | Low | 6 bytes covers ~60-80% of real-world cases. The 2-register ABI is the same cost for SSO and heap, so there's no penalty for picking heap for long strings. |
| **Equality comparison** breaks due to spare bits in SSO handle | Medium | High | Spare bits (bits 51-63 of field0) must be guaranteed 0. Enforce this in SSO construction. Use `icmp eq { i64, i64 }` for structural equality. |
| **String in `defn` return** — caller expects 1 register, gets 2 | Certain | High | All defn call sites for String-returning functions must be updated. The caller handles the return via `extractvalue` if using SSO. |
| **TBAA metadata** for String changes | Low | Low | String TBAA metadata changes from `"i8*"` → `"{ i64, i64 }"`. Verify TBAA paths still work for alias analysis. |

---

## Followup

After SSO migration stabilizes:

1. Re-run full benchmark suite with `--feature sso-strings=ON`. Expected improvements: string-heavy benchmarks (fizzbuzz, stdlib-demo, hello) should show reduced allocation counts.
2. Consider increasing SSO threshold to 14 bytes (requires handle to carry 3 bytes in the tag-bits region — feasible with 3 spare bits after 6-byte data).
3. Consider SSO for `Data` type (same layout as String).
4. Update Ptr Level 3 borrow checker to handle 2-wide String handles in state field provenance.
