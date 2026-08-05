# Optimization and Config Automation — Post-Phase 8 Plan

**Date:** 2026-07-22
**Status:** Plan (ready for implementation)
**Depends on:** Phase 8 complete (GLUE pipeline verified)

---

## Overview

Four automation targets remain after Phase 8. Each eliminates a class of
hand-written configuration by deriving it from the protocol system or
liveness analysis. They are listed in execution order (each builds on the
previous).

---

## Target 1: Protocol Path → Native Type Mapping

### Current state

`resolve_single_frgn()` at `src/analysis/frgn_dispatch.rs:126` computes the
protocol path between `briv_type` and `briv_type` — the same type twice.

```rust
compute_protocol_path(briv_type, briv_type, universe)
```

The foreign type from the target's `c_type_map` is **never passed**. The BFS
always finds identity (same type → cost 0), so `param_paths` is always all
`Identity`. The template engine at `src/glue/export.rs:512` reads the
hand-written `target.type_map` directly:

```rust
"return".to_string(), map_type(&export.return_type, &target.type_map)
```

### What to fix

**File: `src/analysis/frgn_dispatch.rs`** — lines 124-131

Change the protocol path computation to use the **actual foreign type** from
the target's type map:

```rust
let param_paths: Vec<ProtocolStep> = fb.inputs.iter()
    .map(|(_, briv_type)| {
        let foreign_type = lookup_foreign_type(briv_type, target);
        compute_protocol_path(briv_type, &foreign_type, universe)
            .and_then(|steps| steps.into_iter().next().ok_or_else(|| "empty path".to_string()))
    })
    .collect::<Result<Vec<_>, _>>()?;
let return_path: Option<ProtocolStep> = fb.success_output.first()
    .and_then(|(_, ty)| {
        let foreign_type = lookup_foreign_type(ty, target);
        compute_protocol_path(ty, &foreign_type, universe).ok()?.into_iter().next()
    });
```

Where `lookup_foreign_type` maps a Briv type to its foreign counterpart:

```rust
fn lookup_foreign_type(briv_type: &Type, target: &GlueTarget) -> Type {
    match briv_type {
        Type::Custom(name) => target.c_type_map.get(name)
            .map(|s| Type::Custom(s.clone()))
            .unwrap_or_else(|| briv_type.clone()),
        _ => briv_type.clone(),
    }
}
```

**File: `src/glue/export.rs`** — lines 512-538

Replace the hand-written `type_map` and `conversions` lookups with protocol
path queries. The protocol path's `TransformKind` determines the native type:

```rust
fn native_type_from_path(path: &[ProtocolStep], fallback: &str) -> String {
    if path.is_empty() { return fallback.to_string(); }
    let last_step = path.last().unwrap();
    match &last_step.kind {
        TransformKind::Identity => fallback.to_string(),
        TransformKind::Bitcast => "i64".to_string(),
        TransformKind::MeldShuffle => "i64".to_string(),  // raw handle
        TransformKind::ProtocolTransform(cat) => format!("{}", cat),
    }
}
```

This makes `type_map` in the TOML **optional** — a fallback when no protocol
path exists.

### Files changed

| File | Lines | Change |
|------|-------|--------|
| `src/analysis/frgn_dispatch.rs` | 124-131 | Pass foreign type to `compute_protocol_path` |
| `src/analysis/frgn_dispatch.rs` | +10 | Add `lookup_foreign_type` helper |
| `src/glue/export.rs` | 512-538 | Derive native types from protocol path |
| `src/glue/export.rs` | +20 | Add `native_type_from_path` helper |

### Verification

```bash
# Before: param_paths are all Identity (briv_type == briv_type)
# After: param_paths have actual transforms (Bitcast, etc.)
# The generated Rust crate should work identically
briv export pp-types.bv rust --out /tmp/test
cd /tmp/test/pp-types-bridge && cargo build
```

---

## Target 2: Protocol Path → Conversion Expressions

### Current state

The template engine at `export.rs:531` reads `target.conversions` entries
like `String.to_abi = "{name}"`. These are hand-written identity expressions
that don't actually convert anything — they just pass the value through.

The protocol path already knows the actual transform needed (Bitcast,
MeldShuffle, ProtocolTransform), but the template engine never queries it.

### What to fix

**File: `src/glue/export.rs`** — lines 529-538

Replace `target.conversions` lookups with protocol path → expression:

```rust
fn abi_expr_from_path(name: &str, path: &[ProtocolStep], direction: Direction) -> String {
    if path.is_empty() { return name.to_string(); }
    let step = path.last().unwrap();
    match step.kind {
        TransformKind::Identity => name.to_string(),
        TransformKind::Bitcast => {
            match direction {
                Direction::ToAbi => format!("{} as i64", name),
                Direction::FromAbi => format!("{} as *mut u8", name),
            }
        }
        TransformKind::MeldShuffle => {
            // Emit extractvalue/insertvalue chain for field reordering
            name.to_string()  // placeholder — will be specialized per meld
        }
        TransformKind::ProtocolTransform(ref cat) => {
            match direction {
                Direction::ToAbi => format!("{}.as_protocol::<{}>()", name, cat),
                Direction::FromAbi => format!("{}::from_protocol::<{}>({})", name, cat, name),
            }
        }
    }
}
```

This eliminates the `conversions` section from the TOML — they become
**optional** overrides, not the primary mechanism.

### Files changed

| File | Lines | Change |
|------|-------|--------|
| `src/glue/export.rs` | 529-538 | Derive conversion expressions from protocol path |
| `src/glue/export.rs` | +30 | Add `abi_expr_from_path` helper |
| `lib/glue.toml` | — | `conversions` becomes optional (can be removed) |

### Verification

Same as Target 1 — the generated crate should produce identical output
but without reading `conversions` from the TOML.

---

## Target 3: Optimizer Budget — Arena Allocator Control

### Current state

The arena allocator at `src/backend/llvm/mod.rs:1261` (`emit_arena_alloc`)
always uses the bump allocator from `%State` fields. When no arena scope is
active, it falls back to `malloc`. The `--optimize-budget` flag (default 256)
controls only compile-time simulation depth — it has no effect on allocation
strategy.

The arena grow path always allocates `65536` bytes (64KB) plus room for the
request, using `realloc`. For a small string like `"Bits(42)"` (8 chars), the
first grow allocates ~65KB while only 17 bytes are needed. This is 4000×
waste for tiny allocations.

### What to fix

**File: `src/backend/llvm/mod.rs`** — in `emit_arena_alloc` (line 1261)

Add a `--optimize-budget`-aware mode that uses heap (`malloc`) directly
instead of the bump allocator when the budget is below a configurable
threshold:

```rust
pub(crate) fn emit_arena_alloc(&mut self, out: &mut String, indent: &str, size_reg: &str) -> String {
    // 2026-07-22: Budget-aware allocation strategy.
    // Low budget → direct malloc (simpler IR, faster compile).
    // High budget → bump arena (better runtime perf, more complex IR).
    if self.ctx.optimize_budget < 128 {
        // Direct malloc — simpler, faster compile, no arena overhead
        let r = self.fun.next_reg_with_prefix("aam");
        writeln!(out, "{}{} = call noalias ptr @malloc(i64 {})", indent, r, size_reg).ok();
        let ri = self.fun.next_reg_with_prefix("aami");
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, ri, r).ok();
        return ri;
    }
    // Existing arena allocator code (budget >= 128)...
```

**File: `src/backend/llvm/mod.rs`** — around line 712-720

Make the initial arena size configurable via `stack_threshold` or a new CLI flag.
The `65536` magic number should be a parameter:

```rust
// Default: 64KB. CLI override: --arena-size <bytes>
pub arena_initial_size: u64,
```

### Files changed

| File | Lines | Change |
|------|-------|--------|
| `src/backend/llvm/mod.rs` | 1261-1269 | Add budget check before arena path |
| `src/backend/llvm/context.rs` | +1 | Add `arena_initial_size` field |
| `src/backend/llvm/mod.rs` | ~720 | Replace `65536` with `self.ctx.arena_initial_size` |
| `src/main.rs` | +2 | Add `--arena-size` CLI flag |

### Verification

```bash
# Low budget → direct malloc (simpler IR, fast compile)
briv build pp-types.bv --llvm --optimize-budget 0
grep -c 'realloc\|arena.*grow' target/pp-types.ll  # should be 0

# High budget → bump arena (optimal runtime)
briv build pp-types.bv --llvm --optimize-budget 256
grep -c 'realloc\|arena.*grow' target/pp-types.ll  # should be >0
```

---

## Target 4: Layout Inference from Liveness

### Current state

`optimize_layouts()` at `src/analysis/layout_optimizer.rs:52` is fully wired
in `compile.rs:239-244`. It scans bridge-path frgns, compares Briv type
layouts to foreign type layouts, and proposes `LayoutChange` to adopt the
foreign layout.

**BUT it's never triggered** — because `resolve_single_frgn` at line 126
always produces `Identity` paths (since it compares `briv_type == briv_type`).
No bridge frgn is ever found with a non-identity protocol path, so
`optimize_layouts` sees no benefit in changing layouts.

Additionally, even if it WERE triggered, `optimize_layouts` only checks the
foreign type name from `derive_foreign_type_name` — which doesn't use the
`c_type_map`. It uses a separate hardcoded mapping.

### What to fix

**File: `src/analysis/layout_optimizer.rs`** — lines 93-100

Replace `derive_foreign_type_name` with the same `lookup_foreign_type`
function from Target 1. The layout optimizer needs to use the SAME foreign
type mapping as the protocol path computation:

```rust
// Before:
let foreign_ty_name = derive_foreign_type_name(param_ty, &language);

// After:
let foreign_ty_name = lookup_foreign_type(param_ty, target);
```

**File: `src/analysis/layout_optimizer.rs`** — lines 68-77

Make the optimizer's safety check aware of the computed protocol path.
If the protocol path is already `Identity`, no layout change is needed.
If the path has `Bitcast` or `ProtocolTransform`, layout adoption might
eliminate the transform — the optimizer should propose the change.

**File: `src/compile.rs`** — lines 239-244

The optimizer is already called. After fixing the protocol path computation
(Target 1), this code path will start producing results automatically.

### Files changed

| File | Lines | Change |
|------|-------|--------|
| `src/analysis/layout_optimizer.rs` | 93-100 | Use `lookup_foreign_type` instead of `derive_foreign_type_name` |
| `src/analysis/layout_optimizer.rs` | 108-115 | Use protocol path length as cost signal for proposals |
| `src/compile.rs` | — | Already wired — no change needed |

### Verification

```bash
# Build with a bridge frgn, check if layout changes are proposed
briv build pp-types.bv --llvm --emit-beast all 2>&1
grep 'layout optimizer' pp-types.beast.codegen  # should show changes
```

---

## Execution Order

```
Target 1 (type mapping) → Target 2 (conversions) → Target 4 (layout) → Target 3 (arena)
```

The dependency chain:
1. **Target 1** is prerequisite for all others — without correct protocol paths,
   nothing downstream works
2. **Target 2** depends on Target 1 — conversion expressions come from protocol paths
3. **Target 4** depends on Target 1 — layout optimizer needs correct protocol paths
4. **Target 3** is independent — arena budget control has no dependencies

## Success Criteria

| Target | Criteria |
|--------|----------|
| **1** | `type_map` in TOML becomes optional; native types derived from protocol path |
| **2** | `conversions` in TOML becomes optional; expressions derived from protocol path |
| **3** | `--optimize-budget 0` produces no arena code; `--optimize-budget 256` uses arena |
| **4** | Layout optimizer proposes actual changes for bridge frgns with non-Identity paths |
