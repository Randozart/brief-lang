# Complete Phase 8 Plan: Fix Three Gaps in the GLUE Pipeline

**Date:** 2026-07-22
**Status:** Plan (ready for implementation)

---

## Overview

Three remaining gaps prevent the GLUE pipeline from working end-to-end with
no stubs. Each gap is independent but they form a dependency chain:

```
Gap 3 (real function bodies) → Gap 1 (state in templates) → Gap 2 (protocol transforms)
```

Gap 3 is prerequisite — without real bodies, wrappers and transforms have nothing to wrap.

---

## Gap 3: Export CLI Uses Stub Codegen

### Current state

`run_export_cli()` at `src/glue/export.rs:443` calls `library::generate_with_exports()`,
which calls `emit_definition()` at `src/library.rs:113`. This emits:
```llvm
define i64 @func(i64 %arg0) {
  ret i64 0          ; ← stub body, always returns zero
}
```

### Target state

After fix, `briev export pp-types.bv rust --out /tmp/x` produces a `.ll` with
real function bodies, identical to what `briev build --llvm` produces.

### Implementation

**File: `src/glue/export.rs`** — replace line 443

Before:
```rust
let llvm_ir = crate::library::generate_with_exports(&items, file_path)?;
```

After:
```rust
use crate::backend::llvm::LlvmBackend;
let mut b = LlvmBackend::new()
    .with_stdlib_path(...)
    .with_resolved_frgns(resolved_frgns);
b.generate(items, None)
```

Where `resolved_frgns` is populated the same way as in `src/compile.rs:226-233`
(iterating items, collecting ForeignBindings, resolving dispatch).

**File: `src/library.rs`** — `generate_with_exports` remains for backward compat
but is no longer called by the export CLI. Can be kept or removed (kept for now).

### Verification

```bash
briev export pp-types.bv rust --out /tmp/test
grep 'ret i64 0' /tmp/test/pp-types-bridge/bridge.ll  # should NOT match
grep 'call.*@pp_type_bits' /tmp/test/pp-types-bridge/bridge.ll  # should match
```

---

## Gap 1: State Parameter Missing from Generated Crate

### Current state

The Rust `ffi_template` declares functions without the `%state` parameter:
```rust
fn briev_pp_binop(kind: *mut u8) -> *mut u8;
```

But the actual LLVM function takes `ptr %state` as the first parameter.

The `fn_template` generates safe wrappers that don't pass state to the FFI call.

### Target state

Generated `src/ffi.rs`:
```rust
fn briev_pp_binop(state: *mut c_void, kind: *mut u8) -> *mut u8;
```

Generated `src/lib.rs`:
```rust
pub fn briev_pp_binop(kind: *mut u8) -> *mut u8 {
    unsafe { ffi::briev_pp_binop(STATE, kind) }
}
```

Where `STATE` is a static initialized in lib.rs via `init_state()`.

### Implementation

**File: `lib/glue.toml`** — update Rust templates

ffi_template:
```toml
"ffi_template" = """    pub fn {{name}}(state: *mut c_void, {{ffi_params}}) -> {{c_return}};"""
```

lib.rs template:
```toml
"src/lib.rs" = """
// GLUE bridge for {{bridge_name}} — auto-generated.
mod ffi;
use std::ffi::c_void;
static STATE: *mut c_void = std::ptr::null_mut();

fn init_state() {
    let state = std::alloc::alloc_zeroed(std::alloc::Layout::from_size_align(32, 8).unwrap());
    unsafe { ffi::init_state(state as *mut c_void) };
    STATE = state as *mut c_void;
}

{{exports}}
"""
```

fn_template:
```toml
"fn_template" = """pub fn {{name}}({{params}}) -> {{return}} { unsafe {
    let result_abi = ffi::{{name}}(STATE, {{args_abi}});
    {{return_expr}}
} }
"""
```

**File: `src/glue/export.rs`** — populate `s_param` and `s_init` per language

For LTO (Rust): `s_param` stays empty (state is part of `ffi_params` template)
For C ABI (Python/Node): `s_param` = `"_STATE, "` with space after comma

### Verification

```bash
briev export pp-types.bv rust --out /tmp/test
cat /tmp/test/pp-types-bridge/src/ffi.rs  # should have state: *mut c_void
cat /tmp/test/pp-types-bridge/src/lib.rs  # should have STATE in calls
cargo build --manifest-path /tmp/test/pp-types-bridge/Cargo.toml  # should compile
```

---

## Gap 2: Protocol Path Emission Is Stubbed

### Current state

Three sub-issues:

1. `resolve_single_frgn()` at `frgn_dispatch.rs:124-128` fills `param_paths` with
   `vec![]` — never calls `compute_protocol_path`.

2. `compute_protocol_path()` at `frgn_dispatch.rs:165-185` returns `Bitcast`
   for everything except identical types — never calls `find_cast_path` BFS.

3. `emit_protocol_chain()` at `glue/bridge.rs:30-55` is a no-op — the
   `MeldShuffle`, `Bitcast`, and `ProtocolTransform` match arms have
   comment-only bodies.

### Target state

When a frgn is resolved as `ResolvedFrgn::Bridge`:

1. `param_paths` contains computed `ProtocolStep` entries from BFS
2. `compute_protocol_path` uses `find_cast_path` via TypeUniverse
3. `emit_protocol_chain` emits actual LLVM IR for each transform kind

For example, converting `{i64, i64}` (Briev SSO String) to `*mut u8` (Rust `&str`):
- `MeldShuffle`: `extractvalue {i64, i64} %val, 0` (extract data pointer)
- `Bitcast`: `bitcast i64 %ptr to ptr` (inttoptr)
- `ProtocolTransform(#String<UTF8>)`: Call the `CastTo(#String<UTF8>)` func if registered

### Implementation

**Sub-issue 2.1 — Fix `resolve_single_frgn`:**

At `frgn_dispatch.rs:121-130`, replace:
```rust
return Ok(ResolvedFrgn::Bridge {
    language: target.language.clone(),
    param_paths: Vec::new(),       // ← empty, broken
    return_path: None,             // ← None, broken
    fallback: fb.fallback.clone(),
});
```

With:
```rust
let param_paths = fb.inputs.iter()
    .map(|(_, ty)| compute_protocol_path(ty, &map_to_foreign_type(ty, &target.c_type_map)))
    .collect::<Result<Vec<_>, _>>()?;
let return_path = fb.success_output.first()
    .map(|(_, ty)| compute_protocol_path(ty, &map_to_foreign_type(ty, &target.c_type_map)))
    .transpose()?;
```

Where `map_to_foreign_type` maps a Briev type name (like `"String"`) to a
foreign type name (like `"*mut u8"`) using `target.c_type_map`.

**Sub-issue 2.2 — Fix `compute_protocol_path`:**

At `frgn_dispatch.rs:165-185`, replace the Bitcast fallback with a call to
`find_cast_path` BFS:

```rust
pub fn compute_protocol_path(
    briev_type: &crate::ast::Type,
    foreign_type: &str,
) -> Result<Vec<ProtocolStep>, String> {
    let briev_str = format_type(briev_type);
    if briev_str == foreign_type {
        return Ok(vec![ProtocolStep::identity(briev_type.clone())]);
    }
    if let Some(universe) = ... {  // TypeUniverse from context
        if let Some(cast_path) = find_cast_path(universe, &briev_str, foreign_type) {
            return Ok(cast_path.into_iter().map(|type_name| {
                ProtocolStep {
                    source: parse_type(&type_name),
                    target: parse_type(&next_type),
                    kind: classify_transform(&type_name, &next_type),
                }
            }).collect());
        }
    }
    // Fallback: Cast(#Bits) bitcast
    Ok(vec![ProtocolStep { ..., kind: TransformKind::Bitcast }])
}
```

**Sub-issue 2.3 — Fix `emit_protocol_chain`:**

At `glue/bridge.rs:30-55`, implement each transform kind:

```rust
pub fn emit_protocol_chain(
    out: &mut String,
    value_reg: &str,
    path: &[ProtocolStep],
) -> Result<String, String> {
    let mut current_reg = value_reg.to_string();
    for step in path {
        match step.kind {
            TransformKind::Identity => {
                // No transformation needed
            }
            TransformKind::Bitcast => {
                let result = gen_reg();
                writeln!(out, "  {} = bitcast {} {} to {}", result, 
                    current_ty, current_reg, target_ty)?;
                current_reg = result;
            }
            TransformKind::MeldShuffle => {
                // Emit extractvalue/insertvalue sequence
                for (field, shuf) in step.shuffle.iter().enumerate() {
                    let ev = gen_reg();
                    writeln!(out, "  {} = extractvalue {} {}, {}", ev,
                        struct_ty, current_reg, shuf.src)?;
                    let iv = gen_reg();
                    writeln!(out, "  {} = insertvalue {} {}, {}, {}", iv,
                        struct_ty, current_reg, ev, field)?;
                    current_reg = iv;
                }
            }
            TransformKind::ProtocolTransform(ref category) => {
                // Emit call to CastTo#(category) intrinsic
                let result = gen_reg();
                writeln!(out, "  {} = call ptr @CastTo_{}({} {})", result,
                    category, current_ty, current_reg)?;
                current_reg = result;
            }
        }
    }
    Ok(current_reg)
}
```

Note: `emit_protocol_chain` currently returns `&str` but needs mutable `&mut String`
to write IR. The signature must change from:
```rust
pub fn emit_protocol_chain(value_reg: &str, path: &[ProtocolStep], _value_ty: &str) -> Result<String, String>
```
To:
```rust
pub fn emit_protocol_chain(out: &mut String, value_reg: &str, path: &[ProtocolStep],
    value_ty: &Type) -> Result<String, String>
```

This affects the call site at `emit_bridge_frgn_call` in `emit_expr.rs:1219-1222`
and `emit_expr.rs:1254-1257`.

**File: `src/glue/bridge.rs`** — rewrite `emit_protocol_chain` with full implementation

**File: `src/backend/llvm/emit_expr.rs`** — update call sites to pass `out`, `value_ty`

---

## Execution Order

```
Gap 3 ──→ Gap 1 ──→ Gap 2
(stubs → real)  (state param)  (protocol transforms)
```

Each step is independently verifiable:
- After Gap 3: `llc` accepts the `.ll` (no `ret i64 0` stubs remain)
- After Gap 1: generated Rust crate compiles with `cargo build`
- After Gap 2: generated `.ll` contains `extractvalue`/`bitcast`/`insertvalue` instructions
  at the bridge boundary

## Files Changed

| File | Gap |
|------|-----|
| `src/glue/export.rs` | 3, 1 |
| `src/library.rs` | 3 |
| `src/analysis/frgn_dispatch.rs` | 2 |
| `src/glue/bridge.rs` | 2 |
| `src/backend/llvm/emit_expr.rs` | 2 |
| `lib/glue.toml` | 1 |
