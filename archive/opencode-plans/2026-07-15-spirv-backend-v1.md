# SPIR-V Backend — v1 Baseline

**Date:** 2026-07-15
**Status:** Active

## Summary

Wire up a minimal SPIR-V backend that compiles Briev GPU kernels
(`txn [idx < N]`) to SPIR-V binary modules using the `rspirv` crate.
6 new files, ~630 lines, 1 new intrinsic.

## Applicable AGENTS.md Standards

| Directive | Requirement |
|-----------|-------------|
| Flat Control Flow | Max 2 nesting levels. No `else if`. Guard clauses + early returns. |
| Comment the Code | `// 2026-07-15: <why>` at every change site. Intent, not mechanics. |
| Doc Comments | Every `fn`, `struct`, `enum`, `mod`. Reader knows Rust, not domain. |
| Input Validation | Check bounds before indexing. Assert invariants after construction. |
| Need-to-Know DI | Pass builder+types, not CompilerContext. |
| Behavioral Tests | Test round-trip compile + validate, not literal SPIR-V dumps. |
| Praetor | Complexity ≤ 15, lines ≤ 100, params ≤ 6. |
| Tests or It Doesn't Exist | Every path must have a test. `cargo test --lib` before every commit. |

## Files to Create

### 1. `src/backend/spirv/mod.rs` (~90 lines)

Module root. Dispatch entry `compile_spirv()`. Accepts `&[TopLevel]` + options,
outputs `Result<Vec<u8>>` (SPIR-V binary). Delegates to builder + kernel modules.

### 2. `src/backend/spirv/types.rs` (~100 lines)

`BrievType → spirv::Word` mapping. Caches in a `HashMap<TypeId, Word>`.

| Bits(N) type | SPIR-V |
|--------------|--------|
| `Bits(1)` | `OpTypeBool` |
| `Bits(N), primitive<~Int` | `OpTypeInt W=N*8 Signed=0` |
| `Bits(N), primitive<~Float` | `OpTypeFloat W=N*8` |
| `Ptr<T>` | `OpTypePointer StorageClassFunction T` |

### 3. `src/backend/spirv/builder.rs` (~120 lines)

SPIR-V module builder using `rspirv::module::Module`.

```
struct SpirvBuilder {
    module: Module,
    capabilities: Vec<Word>,
    id_counter: Word,
    type_cache: HashMap<u64, Word>,
}
fn new() → Self
fn gen_id(&mut self) → Word
fn build(&self) → Result<Vec<u8>>
```

### 4. `src/backend/spirv/kernel.rs` (~150 lines)

Extracts kernels from `TopLevel::Transaction` with `[idx < N]` precondition.
Emits the full `OpFunction` → `OpLoopMerge` → `OpBranch` structure.

```
fn emit_kernel(builder, txn, tu) -> Result<()>
fn lower_contract(builder, contract) -> Result<(Word, Word)>
// Returns: (idx_reg, N_reg) — the phi variable and bound constant
```

### 5. `src/backend/spirv/intrinsics.rs` (~80 lines)

Maps existing `#` intrinsics to SPIR-V instructions.

| Intrinsic | SPIR-V |
|-----------|--------|
| `GetGlobalId#(dim)` | `OpLoad %GlobalInvocationId` + `OpCompositeExtract dim` |
| `GetGlobalSize#(dim)` | `OpLoad %NumWorkgroups * OpLoad %WorkgroupSize` |
| `GetLocalId#(dim)` | `OpLoad %LocalInvocationId` + `OpCompositeExtract dim` |
| `WorkgroupSize#(dim)` | `OpLoad %WorkgroupSize` + `OpCompositeExtract dim` |
| `Load#(addr, bytes)` | `OpAccessChain` + `OpLoad` |
| `Store#(addr, val, bytes)` | `OpAccessChain` + `OpStore` |
| `Add#`/`Sub#`/`Mul#`/`Div#` | `OpIAdd`/`OpISub`/`OpIMul`/`OpSDiv` |
| `Lt#`/`Gt#`/`Eq#`/ etc. | `OpSLessThan`/`OpSGreaterThan`/`OpIEqual` |
| `Sqrt#` | `OpExtInst %GLSLstd450Sqrt` |
| `Sin#`/`Cos#`/`Pow#` | `OpExtInst %GLSLstd450Sin`/`Cos`/`Pow` |

### 6. `src/backend/spirv/tests.rs` (~100 lines)

Behavioral tests: compile a minimal kernel to SPIR-V, validate with
`rspirv::validate()`. No literal IR snapshots.

```rust
/// 2026-07-15: Kernel with GetGlobalId# compiles and validates.
/// Verifies: OpEntryPoint, OpFunction, OpReturn are present.
fn test_kernel_compiles() { ... }
```

## Files to Modify

### 7. `Cargo.toml`

Add `rspirv = "0.12"`.

### 8. `src/backend/mod.rs`

Add `Spirv` variant to `BackendKind`, route to `compile_spirv`.

### 9. `src/compile.rs`

Route `.abv` extension → `BackendKind::Spirv`.

## New Intrinsic: `WorkgroupSize#(dim)`

| Field | Value |
|-------|-------|
| Signature | `WorkgroupSize#(dim: Int) -> Int, observable: false` |
| Interpreter | Stub returning 64 (default workgroup size) |
| LLVM | Not needed — GPU-only intrinsic |
| SPIR-V | `OpLoad %BuiltInWorkgroupSize` + `OpCompositeExtract dim` |

## Verification Gates

1. `cargo test --lib` — 859+ pass
2. `cargo build --release` — no warnings
3. Minimal kernel `.abv` → SPIR-V binary roundtrip
4. Output validates via `rspirv::validate()`
5. Praetor complexity ≤ 15, lines ≤ 100, params ≤ 6 on all new files
6. No arrow code — max 2 nesting depth everywhere
