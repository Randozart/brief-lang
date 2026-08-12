# String Model, Encoding Registry, Alloc# Dispatch & Fat Pointer Provenance

**Date:** 2026-07-18
**Status:** Plan — pre-implementation
**Depends on:**
  - Phase 1 (InsertAt/ExtractFrom property pipeline fixes) — **DONE**
  - Phase 2 (Alloc# intrinsic, AllocStrategy enum, strategy-aware Free#) — **DONE**
  - `config/llvm-ops.toml` — config-driven operation dispatch (existing)
**See also:**
  - `docs/plans/2026-07-18-allocation-strategy-system.md` (Phase 3-5 remaining)
  - `docs/plans/2026-07-18-ptr-level3-borrow-checking.md` (provenance tracking)
  - `docs/architecture/ctd-and-alu.md`
  - `docs/architecture/backend-type-dispatch.md`
  - `docs/architecture/layout-dsl.md`

---

## Executive Summary

Four closely related designs that together form Briev's memory model:

1. **SSO String handle** — 16-byte handle passed in 2 registers, short strings inline
2. **Encoding registry** — PascalCase = compiler-known `char_width`, quoted = config file
3. **Alloc# dispatch extension** — strategy arg with 3 categories: PascalCase (intrinsic), quoted (config/plugin), identifier (user fn)
4. **Fat pointer provenance** — every `Ptr<T>` carries base + offset + remaining for O(1) `Length#(ptr)` and bounded walk

These replace no existing functionality — they are additive layers on top of the current architecture.

---

## 1. SSO String Handle

### Current state

Strings use a **3-slot header** format:
```
handle (i64) → [data_ptr (i64), length (i64), UTF8_data (N bytes)]
```

Every concatenation heap-allocates. No SSO. Every `Length#` pointer-chases through the handle.

### New design: 16-byte handle, passed in 2 registers

```llvm
; String as <{ i64, i64 }> — two SSA values or one <2 x i64>
```

**Discriminant:** bit 0 of word 0.
- **1** = inline (short string, ≤ 15 bytes)
- **0** = heap/arena pointer

```
Short string (bit 0 of word 0 = 1):
  word0[0..7]    bytes 0-7 of inline data (masked bit 0 preserves alignment)
  word1[0..7]    bytes 8-14 of inline data + length in byte 15 (=> max 15 chars)

Long string (bit 0 of word 0 = 0):
  word0[0..7]    pointer to arena/heap allocation (masked, tag bits preserve)
  word1[0..7]    length in bytes
```

### Effects

- `Length#(s)` → read word1 directly (no pointer chase, no branch — LLVM handles the select)
- SSO concatenations (`"hello " + "world"`) → zero alloc, data stays in registers
- Arena string concatenation → one bump alloc, no header fixup

### Type property

```briev
type String {
    ctd <~ String;
    alu <~ Int;
    encoding <~ UTF8;
    memory_strategy <~ Arena;     // backend handles (PascalCase)
    sso_max <~ 15;                // bytes; 0 = no SSO
    from_bytes: fn(bytes: List<Byte>) -> String;
    to_bytes: fn(self) -> List<Byte>;
}
```

`sso_max` is a type property the codegen reads to determine whether to emit the inline path. Stdlib String declares 15; `--no-stdlib` types can set 0.

### Implementation phases

| Step | File(s) | Change |
|------|---------|--------|
| 3a | `src/backend/llvm/helpers.rs` | `emit_string_literal` → emit 16-byte handle instead of stack alloca |
| 3b | `src/backend/llvm/intrinsics.rs` | `emit_len` → read handle word1 instead of GEP+load |
| 3c | `src/backend/llvm/helpers.rs` | `emit_inline_concat` → SSO check + inline path |
| 3d | `src/backend/llvm/mod.rs` | `sso_max` property read in string dispatch |
| 3e | `lib/std/core/string.bv` | `from_bytes`/`to_bytes` reconstruction protocol |

---

## 2. Encoding Registry

### Convention

| Category | Examples | Lookup | backend guarantee |
|----------|----------|--------|-------------------|
| **PascalCase** | `UTF8`, `ASCII`, `Latin1`, `UTF16`, `UTF32` | Hardcoded `encoding_registry.rs` | Must understand `char_width` |
| **Quoted** | `"shift_jis"`, `"windows_1252"` | `config/encodings.toml` | May delegate to runtime |

### Registry

**Hardcoded** (`src/encoding_registry.rs`):

```rust
pub struct EncodingInfo {
    pub char_width: u64,       // 0 = variable; 1, 2, 4 = fixed
    pub index_mode: IndexMode, // Direct | Scan
}

pub enum IndexMode { Direct, Scan }

pub fn get_encoding_info(name: &str) -> Option<EncodingInfo> {
    match name {
        "ASCII"  => Some(EncodingInfo { char_width: 1, index_mode: IndexMode::Direct }),
        "Latin1" => Some(EncodingInfo { char_width: 1, index_mode: IndexMode::Direct }),
        "UTF8"   => Some(EncodingInfo { char_width: 0, index_mode: IndexMode::Scan }),
        "UTF16"  => Some(EncodingInfo { char_width: 0, index_mode: IndexMode::Scan }),
        "UTF32"  => Some(EncodingInfo { char_width: 4, index_mode: IndexMode::Direct }),
        _ => None,
    }
}
```

**Config-driven** (`config/encodings.toml`):

```toml
[encoding.shift_jis]
char_width = 0

[encoding.windows_1252]
char_width = 1

[encoding.custom_4byte]
char_width = 4
```

### Lookup chain

```
resolve_encoding(name):
  1. get_encoding_info(name)           → hardcoded match
  2. lookup config["encoding"][name]   → config/encodings.toml
  3. char_width = 0                     → delegate to stdlib scan
```

### Effect on codegen

- `char_width > 0` → `Index#` emits `GEP` (O(1)): `s[i]` = `load i{char_width} @ data + i * char_width`
- `char_width == 0` → `Index#` emits a runtime scan loop or calls stdlib scan function

### Property on types

```briev
type String {
    encoding <~ UTF8;              // PascalCase = compiler understands
}

type WebPage {
    ctd <~ String;
    encoding <~ "shift_jis";       // quoted = config-driven
}
```

---

## 3. Alloc# Strategy Dispatch Extension

### Current (Phase 2 — DONE)

```
Alloc#(size)
  → arena active    = arena bump
  → bounded+noescape = alloca
  → default          = @malloc
```

### Extension: optional 2nd argument

```briev
Alloc#(256)                       // compiler picks (existing triple dispatch)
Alloc#(256, Arena)                // PascalCase — intrinsic dispatch table
Alloc#(256, Malloc)               // PascalCase — intrinsic dispatch table
Alloc#(256, Alloca)               // PascalCase — intrinsic dispatch table
Alloc#(256, "pool_serial")        // quoted — config/alloc-strategies.toml or plugin
Alloc#(256, my_custom_alloc_fn)   // identifier — Briev function call
```

### PascalCase dispatch (intrinsic)

```rust
// In emit_alloc, after triple-dispatch fallback:
if args.len() >= 2 {
    let strategy_expr = &args[1];
    match strategy_name(strategy_expr) {
        "Arena"  => { /* force arena bump */ return; }
        "Malloc" => { /* force @malloc */ return; }
        "Alloca" => { /* force alloca */ return; }
        _ => {} // fall through to config/plugin
    }
}
```

### Quoted (config/plugin)

```
Alloc#(256, "pool_serial")
  → emit_alloc checks config/alloc-strategies.toml
  → if found: emits the matching LLVM IR template
  → if not found: emit_warning + fallback to @malloc
```

Config format:

```toml
[alloc.pool_serial]
template = """
  %{v} = call ptr @pool_alloc(i64 {size})
  %{v}_ptr = ptrtoint ptr %{v} to i64
"""
free_template = """
  call void @pool_free(ptr {ptr})
"""
```

### Identifier (user function)

```briev
defn my_alloc(size: Int) -> Ptr<Byte> {
    &result <- Malloc#(size);    // or arena, or whatever
    term result;
}

let buf = Alloc#(256, my_alloc);
```

Semantics: `Alloc#(size, fn_name)` emits `call @fn_name(i64 %size)`, returns the `i64` result. The strategy annotation (`Malloc`) is recorded so `Free#(buf)` dispatches correctly.

### Strategy propagation for custom functions

When `Alloc#(256, my_alloc)` is used, the compiler doesn't know the strategy. `Free#(buf)` conservatively emits `@free`. The programmer can annotate:

```briev
let buf = Alloc#(256, my_alloc) using Malloc;   // tells Free#: call @free
let buf = Alloc#(256, my_alloc) using Arena;    // tells Free#: no-op
```

Or the custom function itself declares:

```briev
defn my_alloc(size: Int) -> Ptr<Byte> [result != null] {
    metadata alloc_strategy <~ Malloc;
    &result <- Malloc#(size);
    term result;
}
```

---

## 4. Fat Pointer Provenance (Ptr Level 3)

### Problem

Current `Ptr<T>` is a raw address (i64):
- `Length#(ptr)` must chase through the String handle to find length
- Walking past bounds is not detectable at runtime
- Sub-slices (`&s[6]`) lose the parent's provenance information

### Solution: 3-word fat pointer

Every `Ptr<T>` carries **base + offset + remaining**:

```
Ptr<String::Byte> = { base: i64, offset: i64, remaining: i64 }
```

When the compiler emits a pointer-to-member (`&s.data[i]`):

```
base      = start of the String allocation (or heap block)
offset    = bytes from start of char data to the pointed-to element
remaining = total bytes from this position to end of buffer
```

### O(1) Length and bounds check

```briev
let s: String = "hello world";
let mid = &s[6];          // {base=&s, offset=6, remaining=5}
let remaining = Length#(mid);  // 5 — no pointer chase
```

In LLVM IR, fat pointers are a `<{ i64, i64, i64 }>` struct (3 SSA values, or 1 struct + 1 GEP for field access).

### How Index# emits

Current `emit_intrinsic_index` (`intrinsics.rs:608-621`):

```rust
// Before: raw pointer, no provenance
writeln!(out, "{}{} = getelementptr {}, ptr {}, i64 {}", indent, gep, llvm_ty, obj_reg.name, idx_reg.name).ok();
writeln!(out, "{}{} = load {}, ptr {}, align {}", indent, v, llvm_ty, gep, ...).ok();
```

After:

```rust
// Fat pointer index:
//   base = obj_reg.base
//   new_offset = obj_reg.offset + idx * char_width
//   new_remaining = obj_reg.remaining - idx * char_width
writeln!(out, "  {} = extractvalue {{ i64, i64, i64 }} {}, 0", base, obj_reg.name).ok();
writeln!(out, "  {} = extractvalue {{ i64, i64, i64 }} {}, 1", off, obj_reg.name).ok();
writeln!(out, "  {} = extractvalue {{ i64, i64, i64 }} {}, 2", rem, obj_reg.name).ok();
// Error if idx * char_width > remaining — contract [idx * char_width < remaining]
writeln!(out, "  {} = icmp ule i64 {}, {}", ok_check, idx_sz, rem).ok();
writeln!(out, "  call void @llvm.assume(i1 {})", ok_check).ok(); // or trap
// New offset
writeln!(out, "  {} = add i64 {}, {}", new_off, off, idx_sz).ok();
writeln!(out, "  {} = sub i64 {}, {}", new_rem, rem, idx_sz).ok();
// This element's address = base + new_offset
writeln!(out, "  {} = add i64 {}, {}", addr, base, new_off).ok();
```

### Impact on TypedRegister

```rust
pub struct TypedRegister {
    pub name: String,
    pub ty: Type,
    // 2026-07-18: Fat pointer provenance — None for non-Ptr values.
    // base_reg + offset_reg + remaining_reg are SSA register names
    // for the three components of a fat pointer.
    pub base_reg: Option<String>,
    pub offset_reg: Option<String>,
    pub remaining_reg: Option<String>,
}
```

### When fat pointers are created

1. `&s` (address-of on a String handle) → fat pointer with `base = &s.data`, `offset = 0`, `remaining = s.length`
2. `&s[i]` → fat pointer with `base = &s.data`, `offset = i * char_width`, `remaining = s.length - i * char_width`
3. `&buf[i]` on `Ptr<Byte>` → inherit `base` from parent, adjust `offset` and `remaining`
4. `Alloc#(256)` → fat pointer with `base = alloc_addr`, `offset = 0`, `remaining = 256`
5. `Malloc#(256)` → fat pointer with `base = alloc_addr`, `offset = 0`, `remaining = 256`
6. `let p = raw_addr; Ptr#(p)` → no provenance (base = 0, offset = 0, remaining = 0) — conservative `Length#` returns 0

### Integration with Ptr Level 3 borrow checker

The `docs/plans/2026-07-18-ptr-level3-borrow-checking.md` plan describes provenance tracking for borrow-checking. Fat pointers provide the runtime component:

- `base` → identifies which allocation this pointer belongs to
- `offset` + `remaining` → bounds for contract verification `[i < remaining]`
- Borrow checker at compile time: "does this pointer alias another live pointer into the same `base`?"

---

## 5. Integration with Existing Phases

| Phase | Status | Relevance |
|-------|--------|-----------|
| Phase 1 (InsertAt/ExtractFrom pipeline) | **DONE** | Precondition for all collection operations |
| Phase 2 (Alloc# intrinsic, strategy dispatch) | **DONE** | `Alloc#` extension adds PascalCase/quoted/identifier dispatch |
| Phase 3a-3e (SSO string) | **PLAN** | Step 3a-3e in implementation section below |
| Phase 4 (encoding registry) | **PLAN** | Encoding lookup + config |
| Phase 5 (fat pointer codegen) | **PLAN** | Ptr Level 3 integration |
| Phase 6 (benchmark baseline) | **PLAN** | Verify no regressions on existing benchmark suite |

### Which comes first

Ordered by dependency:

```
3a. SSO String handle (helpers.rs + intrinsics.rs)
3b. Encoding registry (encoding_registry.rs + config/encodings.toml)
3c. Alloc# 2nd arg dispatch (intrinsics.rs + config/alloc-strategies.toml)
3d. Fat pointer TypedRegister extension (mod.rs + intrinsics.rs)
3e. Fat pointer Index#/Deref#/Length# (intrinsics.rs)
3f. from_bytes/to_bytes on stdlib String (lib/std/core/string.bv)
3g. Benchmark baseline
```

---

## 6. Key Design Decisions

### Why PascalCase for fixed set?

The compiler must guarantee `char_width` is known for O(1) indexing. PascalCase means "the compiler understands this encoding's character width and index mode." Quoted means "the compiler defers to runtime." A config file bridges the gap for new fixed-width encodings without recompiling.

### Why no CoW?

- CoW requires atomic refcounts → unpredictable in threaded code
- Strings are immutable after construction in Briev (every operation creates a new handle)
- Arena reset at txn/tick boundaries is the correct bulk reclamation strategy
- SSO eliminates allocation entirely for the common case (strings ≤ 15 bytes)

### Why fat pointers in TypedRegister instead of a side channel?

Fat pointers change codegen for every `Ptr<T>` operation — load, store, GEP, index, deref. Adding `base_reg`/`offset_reg`/`remaining_reg` to `TypedRegister` ensures every code path that handles a `Ptr<T>` has access to provenance information. A side channel would require a separate lookup on every operation, defeating the purpose.

### What happens with `--no-stdlib`?

- `Alloc#(size)` still works (triple dispatch is intrinsic)
- `Alloc#(size, Arena)` still works (PascalCase is intrinsic)
- Quoted strategies and encodings fall through gracefully (no config → emit warning → fall back to malloc/scan)
- String type is undefined — user provides their own, or uses `Ptr<Byte>` directly
- Fat pointer provenance still works (it's in the compiler codegen, not the type system)
