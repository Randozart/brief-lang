# Plan: emit_expr.rs Full Cleanup

**Goal:** Restructure the remaining ~4,000 lines of `emit_expr.rs` so that
adding a new expression variant means adding one match arm — not wading
through intertwined handler + helper code.

## Problem

`emit_expr.rs` has two tightly-coupled sections:

| Section | Lines | Contents |
|---------|-------|----------|
| **Dispatch** | 21–2409 | `pub(crate) fn emit_expr()` — the big `match &expr { ... }` with ~30 inline handlers |
| **Helpers** | 2410–4002 | ~20 shared functions (`emit_binop`, `emit_fcmp`, `is_string_chain`, `emit_decay`, etc.) |

The helpers call each other AND the dispatch. The dispatch calls the
helpers. They're intertwined in one file because Rust's visibility rules
let items within the same module see each other freely, but splitting
across modules requires explicit `pub(crate)`/`pub(super)` annotations.

## Solution: Three-Step Restructuring

```
Before:
  llvm/
    mod.rs (struct + pub items)
    emit_expr.rs (dispatch + helpers)

After:
  llvm/
    mod.rs (struct + pub items)
    helpers.rs (ALL shared helper functions — pub(super))
    emit_expr.rs (dispatch only — thin match → submodules)
    expr/
      literal.rs    ✅ existing
      math.rs       ✅ existing
      compare.rs    ✅ existing
      collections.rs ✅ existing
      intrinsics.rs ✅ existing
      identifier.rs (NEW — Expr::Identifier)
      call.rs       (NEW — Expr::Call, CellCall)
      field.rs      (NEW — FieldAccess, StructInstance, ObjectLiteral)
      arrow.rs      (NEW — ArrowMut, ArrowDiscard, ArrowTransfer)
      control.rs    (NEW — Match, PatternMatch, Slice, MultiSlice, Within, Block)
      projection.rs (NEW — Projection, SubtypeProjection)
      misc.rs       (NEW — Cast, IsType, FromCheck, Like, OwnedRef, PriorState)
```

## Step 1: Extract Helpers → `helpers.rs` (unchanged visibility)

Create `src/backend/llvm/helpers.rs` containing lines 2410–4002 from
`emit_expr.rs`, wrapped in `impl LlvmBackend { ... }`.

**Visibility changes required** (all existing functions keep their current
visibility level, which is already `pub(crate)` or `pub(super)` for most):

| Current | Functions | Issue |
|---------|-----------|-------|
| `fn ptrtoint_if_string` | private | Only called by helpers → stays private |
| `fn is_ptr_ty` | private | Only called by helpers → stays private |
| `fn is_linked_string_trigger` | private | Only called by helpers → stays private |
| `fn try_emit_fn_projection` | private | Only called by helpers → stays private |
| `fn try_projection_fast_path` | private | Only called by helpers → stays private |
| `fn emit_route_expression` | private | Only called by helpers → stays private |
| `fn emit_direct_projection` | private | Only called by helpers → stays private |
| `pub(super) fn as_bool_reg` | `pub(super)` → child modules can see it ✅ |
| All other `pub(crate) fn` | `pub(crate)` → visible everywhere ✅ |

**Key insight:** `helpers.rs` is a sibling of `emit_expr.rs` and a PARENT
of `expr/`. Functions with `pub(super)` in `helpers.rs` are visible to
the parent `llvm` module, which means all CHILDREN of `llvm` (including
`emit_expr.rs` and `expr/*.rs`) can access them via `super::helpers::fn_name`.

`pub(crate)` items are visible everywhere anyway.

After this step: `emit_expr.rs` shrinks from 4002 to 2409 lines.
The dispatch and helpers compile and pass all tests. No behavioral change.

## Step 2: Extract Handlers → `expr/*.rs` (one module per group)

Once helpers are accessible from all child modules, extract each handler
group into its own submodule. Each submodule defines a standalone function:

```rust
// expr/identifier.rs
pub fn emit_identifier(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    name: &str,
    indent: &str,
) -> TypedRegister {
    // ... uses helpers via super::helpers::* or just calls backend methods
}
```

The `emit_expr` dispatch becomes a thin dispatcher:

```rust
pub(crate) fn emit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> TypedRegister {
    let v = format!("%t{}", self.fun.next_reg());
    match expr {
        // Already dispatched to submodules:
        Expr::Integer(_) => expr::literal::emit_integer(self, out, &v, expr, indent),
        Expr::Float(_)  => expr::literal::emit_float(self, out, &v, expr, indent),
        Expr::Add(_, _) => expr::math::emit_add(self, out, &v, expr, indent),
        // ... (existing dispatches stay)

        // NEW dispatches:
        Expr::Identifier(_) => expr::identifier::emit_identifier(self, out, &v, expr, indent),
        Expr::Call(_, _)   => expr::call::emit_call(self, out, &v, expr, indent),
        Expr::Match{..}    => expr::control::emit_match(self, out, &v, expr, indent),
        // ...

        // Catch-all for unmatched variants:
        _ => TypedRegister { name: v, ty: Type::Int },
    }
}
```

### Extraction Order (least risky first)

| # | Module | Expr Variants | Est. Lines | Risk |
|---|--------|--------------|------------|------|
| 1 | `identifier.rs` | Identifier, OwnedRef, PriorState | 280 | Medium — uses ssa_old_regs, field_index_map |
| 2 | `projection.rs` | Projection, SubtypeProjection | 150 | Low — self-contained projection logic |
| 3 | `misc.rs` | Cast, IsType, FromCheck, Like, Block, Concat | 100 | Low — small handlers |
| 4 | `control.rs` | Match, PatternMatch, Within | 400 | Medium — recursive pattern matching |
| 5 | `field.rs` | FieldAccess, StructInstance, ObjectLiteral | 150 | Low — field offset lookup |
| 6 | `arrow.rs` | ArrowMut, ArrowDiscard, ArrowTransfer | 400 | Medium — state mutation logic |
| 7 | `slice.rs` | Slice, MultiSlice, ListIndex | 300 | Medium — array manipulation |
| 8 | `call.rs` | Call, CellCall | 400 | High — deep nesting, fallthrough returns |

Each extraction is verified independently (`cargo test --lib`).

## Step 3: Inline Submodule → Expandable Architecture

After extraction, `emit_expr.rs` is a thin ~300-line dispatcher. Adding a
new expression variant requires:
1. Create `expr/newthing.rs` with a `pub fn emit_newthing(...)` function
2. Add one match arm to `emit_expr.rs`: `Expr::NewThing(_) => expr::newthing::emit_newthing(self, out, &v, expr, indent)`
3. Add the module to `expr/mod.rs`

No changes to helpers, no entangling with existing code, no visibility
issues. True modular expression codegen.

## Timeline

| Step | Est. Time | Verification |
|------|-----------|-------------|
| 1. Extract helpers → `helpers.rs` | 30 min | `cargo test --lib` — 1318 pass |
| 2a. Extract identifier.rs | 20 min | `cargo test --lib` — 1318 pass |
| 2b. Extract projection.rs | 15 min | `cargo test --lib` — 1318 pass |
| 2c. Extract misc.rs | 10 min | `cargo test --lib` — 1318 pass |
| 2d. Extract control.rs | 30 min | `cargo test --lib` — 1318 pass |
| 2e. Extract field.rs | 15 min | `cargo test --lib` — 1318 pass |
| 2f. Extract arrow.rs | 30 min | `cargo test --lib` — 1318 pass |
| 2g. Extract slice.rs | 20 min | `cargo test --lib` — 1318 pass |
| 2h. Extract call.rs | 30 min | `cargo test --lib` — 1318 pass |
| 3. Thin dispatcher | 10 min | `cargo test --lib` — 1318 pass |
| **Total** | **~3.5 hours** | |

## Key Risk: Fallthrough Returns

Some handlers (Call, Slice, Within) rely on falling through to
`emit_expr`'s default `TypedRegister { name: v, ty: Type::Int }`.
When extracted into standalone functions, these paths must have explicit
`return` statements.

**Mitigation:** Before extracting each handler, audit it for fallthrough
paths and add explicit `return TypedRegister { name: v, ty: Type::Int };`
where needed. Test after each audit.
