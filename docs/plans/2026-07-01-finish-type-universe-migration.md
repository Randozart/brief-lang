# Finish TypeUniverse Migration — Complete Phase 7A

## What's done

Phase 7A (commits 73cc9e5 → 248363e) replaced `llvm_type()` hardcoded match
arms with generic universe queries. But the **boxing/unboxing conversion code**
was left as hardcoded match on `Type::Char`, `Type::Bool`, etc. — creating a
gap where universe data and conversion code can disagree.

The `lib/std/types/bootstrap.bv` file already declares `box`/`unbox` intrinsics
for most types. **Char is the only built-in type missing them.**

## Root cause of the bug

`apply_binding()` (type_universe.rs:449) matched old-style `Expr::String(s)`,
but the parser produces `Expr::Literal(LiteralExpr::String(...))`. Every
bootstrap binding silently failed — `Char`'s `llvm <~ "i32"` was never applied.
The default `llvm_type = "i64"` stayed. The fix we applied (`.as_string()` etc.)
patches the symptom but doesn't prevent recurrence.

## What "finish the migration" means

Replace ALL hardcoded type-based dispatch in boxing/unboxing with
universe-driven code. When a TypeDef declares `box <~ "zext.i32.to.i64#"`,
every boxing site routes through that intrinsic name — not a Type enum match.

## Files to change

| File | What | Why |
|------|------|-----|
| `lib/std/types/bootstrap.bv` | Add `box`/`unbox` to `Char` | Completeness — every non-i64 type needs these |
| `src/backend/llvm/emit_toplevel.rs` | Parameter boxing in `emit_definition` / `emit_callable_txn` | Replace `Type::Char` match with `box_op` |
| `src/backend/llvm/expr/identifier.rs` | State field extraction | Replace `Type::Char` match with `box_op` |
| `src/backend/llvm/emit_stmt.rs` | `adapt_via_box_op` — add `zext.i32.to.i64#` | Needed for Char via universe |
| `src/backend/llvm/builder.rs` | `TypeConverter` — add `zext.i32.to.i64#` / `trunc.i64.to.i32#` | New builder needs these too |
| `src/type_universe.rs` | Post-bootstrap validation | Catch silent binding failures |

## Phases

### Phase 1 — Bootstrap + adapt_via_box_op

- `bootstrap.bv`: add `box <~ "zext.i32.to.i64#"`, `unbox <~ "trunc.i64.to.i32#"` to Char
- `emit_stmt.rs`: add `"zext.i32.to.i64#"` handler in `adapt_via_box_op`

### Phase 2 — builder.rs TypeConverter

- `builder.rs`: add `"zext.i32.to.i64#"` handler in `box_to_i64`
- `builder.rs`: add `"trunc.i64.to.i32#"` handler in `unbox_from_i64`

### Phase 3 — emit_definition / emit_callable_txn

- `emit_toplevel.rs`: Add `emit_box_param` helper that queries universe `box_op`
- `emit_toplevel.rs`: Wire it into `emit_definition` parameter loop
- `emit_toplevel.rs`: Wire it into `emit_callable_txn` parameter loop
- Remove the `param_llvm_ty == "i64"` patch (now handled by universe query)

### Phase 4 — identifier.rs state field extraction

- `identifier.rs`: Replace `match briv_ty { Type::Char => zext i32, ... }`
  with universe-driven boxing via `box_op`

### Phase 5 — Validation

- `type_universe.rs`: After `init_primitives_from_bootstrap()`, validate that
  all registered types have consistent properties: if `llvm_type != "i64"`
  and `storage == "Boxed"`, then `box_op` must be defined.

## Testing

- `cargo test --lib` after each phase
- Verify `print_loop.bv` compiles with correct `append_char` signature (i32)
- Verify `nbody.bv` produces correct Float64 handling
- Run `bash benchmarks/build_and_bench.sh --correctness`
