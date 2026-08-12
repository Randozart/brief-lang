# Accelerated Briev (`.abv`) — Native GPU Compilation Tier

**Date:** 2026-06-18 (updated 2026-06-19)  
**Status:** COMPLETED — renamed from "Graphic Briev" to "Accelerated Briev" (`.gbv` → `.abv`)  
**Context:** SPIR-V backend extended through Phases 1–7 (1028 tests). Dedicated file
extension `.abv` that always compiles to GPU, with type/intrinsic guards.

---

## Design Decisions

| Decision | Choice |
|----------|--------|
| Allowed types | `Int`, `Float`, `Bool`, `Char`, `String` (const-only), `[T; N]` |
| Banned types | `struct`, `enum`, `HashMap`, `HashSet`, `Tuple`, `Box`, `Ptr` |
| FFI (`frgn`) | Banned entirely |
| Intrinsics | Only GPU-mapped: sin, cos, pow, sqrt, fabs, ceil, floor, get_global_id, get_local_id, get_group_id, get_num_groups, barrier |
| Contracts | Optional (warn if missing) |
| GPU flag | Implicit — no `#gpu` or `--gpu-offload` needed |
| Output | Both: native binary (embedded SPIR-V) + standalone `file.spv` |
| CLI | `briev-compiler build file.abv` — auto-detect extension |
| Alternative name | "Briev Accel" |

---

## Files Modified

| File | Change |
|------|--------|
| `src/ast.rs` | Add `StrictMode::Gpu` variant |
| `src/parser.rs` | Detect `.abv` extension → `StrictMode::Gpu` |
| `src/typechecker.rs` | Validate GPU type/intrinsic restrictions |
| `src/backend/llvm/mod.rs` | `StrictMode::Gpu` → skip CPU IR, emit SPIR-V directly |
| `src/backend/llvm/gpu.rs` | `emit_standalone_spirv()` for `.spv` file output |
| `src/main.rs` | `.abv` auto-detection, standalone `.spv` emission |
| `AGENTS.md` | File types table — add `.abv` |
| `docs/architecture/features/accelerated-briev.md` | Architecture doc |

### New test files

| File | Content |
|------|---------|
| `tests/gpu_e2e.rs` | End-to-end parse → SPIR-V IR assertions |
| `benchmarks/gpu/saxpy/saxpy.bv` | GPU saxpy benchmark |
| `benchmarks/gpu/saxpy/saxpy.c` | C reference |

---

## Enforcement Details

### Type validation (in typechecker)

For `StrictMode::Gpu` files, the typechecker rejects:

```rust
fn validate_gpu_type(ty: &Type) -> bool {
    matches!(ty,
        Type::Int | Type::UInt | Type::Float | Type::Bool | Type::Char
        | Type::String  // const-only — checked at let-binding site
        | Type::Vector(..)  // fixed-size array
    )
}
```

`String` variables are only allowed with literal initializers (no runtime string
ops like `+`, `.append()`, `.slice()`). This is enforced by checking `let` +
`Statement::Assignment` for string-typed variables: the RHS must be a string
literal or a direct copy of another string variable (no function calls, concat,
or slice).

### Intrinsic validation

Only the following intrinsics are allowed in `.abv`:
```
sin, cos, pow, sqrt, fabs, ceil, floor,
get_global_id, get_local_id, get_group_id, get_num_groups, barrier
```

Any other `Expr::IntrinsicCall` produces a compile error.

### Contract warnings

When a transaction in `.abv` lacks `[pre][post]` contracts, emit:
```
warning: .abv transaction 'name' has no contracts — contracts enable GPU
         optimization. Add [pre][post] for better codegen.
```

---

## Implementation Order

| Phase | Items | Files |
|-------|-------|-------|
| **1** | `StrictMode::Gpu` + parser `.abv` detection | `ast.rs`, `parser.rs` |
| **2** | `.abv` type/intrinsic validation | `typechecker.rs` |
| **3** | Standalone SPIR-V emit + auto-detect build | `mod.rs`, `gpu.rs`, `main.rs` |
| **4** | End-to-end test | `tests/gpu_e2e.rs`, test `.abv` files |
| **5** | GPU benchmark | `benchmarks/gpu/saxpy/`, `build_and_bench.sh` |
| **6** | Architecture doc + AGENTS.md update | `docs/architecture/features/accelerated-briev.md`, `AGENTS.md` |
| **7** | Rename `.gbv` → `.abv`, "Graphic Briev" → "Accelerated Briev" | All source + docs |


---

## Test Inventory

| # | Test name | Location | Phase |
|---|-----------|----------|-------|
| 1 | `test_gpu_e2e_simple_add` | `tests/gpu_e2e.rs` | 4 |
| 2 | `test_gpu_e2e_barrier` | `tests/gpu_e2e.rs` | 4 |
| 3 | `test_gpu_e2e_invalid_frgn` | `tests/gpu_e2e.rs` | 4 |
| 4 | `test_gpu_e2e_missing_contract_warns` | `tests/gpu_e2e.rs` | 4 |
| 5 | `test_gpu_e2e_invalid_type_struct` | `tests/gpu_e2e.rs` | 4 |
