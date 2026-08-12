# Protocol-Driven GLUE: Eliminate `type_map`, `c_type_map`, `conversions`

**Date:** 2026-07-22
**Status:** Implementation

---

## The Core Insight

A type never needs to know how to cast to another type. It only needs to know
how to cast to its protocol category. The protocol category — `#String`,
`#Int`, `#Float` — is the **language-agnostic concept**. The TOML maps
protocol categories (not Briev types) to language-native types.

Any path between two types that goes through the SAME protocol category gets
its redundant transforms eliminated at compile time. An ASCIIString that
declares `CastTo(#String)` as UTF-8 packing, and a Rust `&str` that declares
`CastFrom(#String)` as UTF-8 unpacking — the pair of operations that both
know their difference from UTF-8 cancels out, leaving zero work at the boundary.

## What Changes

### TOML: Replace `type_map` + `c_type_map` + `conversions` with `protocols`

```toml
# Before (3 sections, all hand-written):
[rust.type_map]
String = "String"
Int = "i64"

[rust.c_type_map]
String = "i64"
Int = "i64"

[rust.conversions.String]
to_abi = "{name}.as_ptr() as i64"
from_abi = "String::from_raw_parts(...)"

# After (1 section, protocol-only):
[rust.protocols]
"#String" = { native = "str", c_abi = "i64" }
"#Int" = { native = "i64", c_abi = "i64" }
"#Float" = { native = "f64", c_abi = "double" }
"#Bool" = { native = "bool", c_abi = "i32" }
"#Char" = { native = "char", c_abi = "i32" }
```

### Resolution Flow

For a parameter of type `Custom("String")`:

```
1. Query universe: what CastTo does String participate in?
   → universe.get("String").properties has "Cast.#String"

2. Protocol = #String
   Look up in target.protocols: "#String" → { native = "str", c_abi = "i64" }

3. The wrapper type is the protocol's native type ("str").
   The FFI type is the protocol's c_abi type ("i64").

4. Conversion expression = derived from the CastTo/CastFrom delta.
   If both sides declare identity transforms to #String → zero instructions.
   If there's a real transform → emit the delta code.
```

### BFS and Cost Elimination

The `find_cast_path` BFS already walks the protocol graph:

```
Path: [String, #String, str]
  Step 1: String.CastTo(#String) = 5 bytes packed into {i64,i64}
  Step 2: str.CastFrom(#String) = {i64,i64} unpacked to {ptr,len}

Cost = Cost(Step1) + Cost(Step2) = identity + identity = 0
→ Zero instructions at the boundary.
```

The `compute_protocol_path` function already handles this. The fix is:
- Use the protocol category (found by walking CastTo on the Briev type)
- NOT the foreign type name (which doesn't exist in the type universe)

## Files Changed

| File | Change |
|------|--------|
| `lib/glue.toml` | Replace `type_map`, `c_type_map`, `conversions` with `protocols` |
| `src/glue/config.rs` | Replace `type_map`/`c_type_map`/`conversions` with `protocols` HashMap |
| `src/glue/export.rs` | Derive wrapper types from protocol lookups instead of `type_map` |
| `src/analysis/frgn_dispatch.rs` | Derive foreign types from protocol lookup (not `c_type_map`) |
| `src/analysis/layout_optimizer.rs` | Derive foreign layouts from protocol types |

## Execution

### Step 1 — Update data structures

Add `protocols: HashMap<String, ProtocolEntry>` to `GlueTarget`.

```rust
pub struct ProtocolEntry {
    pub native: String,  // language-native type name (e.g., "str")
    pub c_abi: String,   // C ABI type name (e.g., "i64")
}
```

### Step 2 — Update TOML

Replace all `type_map`, `c_type_map`, `conversions` sections with `protocols`
for all three languages (python, node, rust).

### Step 3 — Update `resolve_single_frgn` to use protocol lookup

Replace the `lookup_foreign_type` via `c_type_map` with: "find Briev type's
CastTo, get protocol category, look up in target's `protocols`."

`HashMap` import already exists.

### Step 4 — Update template engine to use protocol lookup

Replace `map_type(&ty, &target.type_map)` and `target.conversions` lookups
with protocol category → native/C ABI type mapping.

### Step 5 — Update layout optimizer similarly

### Step 6 — Verify

```bash
cargo test --lib          # 969 pass
cargo test --test pp_roundtrip_tests -- --test-threads=1  # 8 pass
briev export pp-types.bv rust --out /tmp/test
cd /tmp/test/pp-types-bridge && cargo build  # compiles
```
