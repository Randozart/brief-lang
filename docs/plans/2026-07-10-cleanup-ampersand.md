# Cleanup: Remove `&` From State Field Writes

## Rationale

The `&` in `&field = value` was the old `OwnedRef` syntactic marker.
With Ptr Level 3, `&` is now a real address-of operator (`Expr::AddrOf`),
so `&field = value` means `*(&field) = value` — create a transient pointer,
write through it. This is redundant for direct state field writes.

The bare `field = value` (without `&`) goes through the original
`emit_memory_field_store` codegen path — the same path that produced the
baseline benchmark numbers. Removing `&` restores the original path.

## Changes

| Pattern | Count | Replacement | Reason |
|---------|-------|-------------|--------|
| `&field = value` | 503 | `field = value` | Redundant address-of |
| `&list <- value` | 22 | `list <- value` | Redundant arrow target |
| `<- &list` | 4 | `<- list` | Redundant arrow source |
| `*(&expr)` | 3 | `expr` | No-op deref of addr-of |
| `let p = &field` | 2 | **KEEP** | Real pointer creation |
| `*ptr` | 0 | **KEEP** | Real dereference |
| `Ptr<T>` | ~80 | **KEEP** | Real type annotation |
| `&` bitwise AND | 13 | **KEEP** | Different operator |

## Verification

1. `cargo test --lib` — all tests pass
2. `bash benchmarks/build_and_bench.sh --runtime` — all benchmarks MATCH
