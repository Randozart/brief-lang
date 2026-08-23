# Backend scaffolding foundation — uniform contract for all backends

**Date:** 2026-08-23
**Status:** active
**Sequencing:** MUST land before the four parallel per-backend plans fork into
worktrees. Owns every shared file (`src/compile.rs`, `src/backend/mod.rs`,
`config/targets.dbvl`) so the parallel branches never collide on shared code.

**Related plans (parallel after this merges):**
- `2026-08-23-vm-compile-tail-parity.md`
- `2026-08-23-spirv-kernel-emission.md`
- `2026-08-23-circt-toolchain-validation.md`
- `2026-08-23-webstack-v2-completion.md`

## Problem

The LLVM backend is mature; the other four are not scaffolded to the same
architecture. Investigation (2026-08-23) found five structural gaps:

1. **No uniform backend contract.** Three ad-hoc entry signatures:
   - `CirctBackend::generate(items) -> String` (`circt/mod.rs:78`, no universe)
   - `compile_spirv(items, "main") -> Vec<u8>` (`spirv/mod.rs:30`, entry name
     ignored at call site `compile.rs:1907`)
   - `VmBackend::generate(items, _universe)` (`vm/mod.rs:83`, universe ignored)
   - LLVM takes ~15 builder options and internally computes its own analysis.
2. **`analyze_program` runs inside `LlvmBackend::generate`**
   (`llvm/mod.rs:2053`). The other backends never see `AnalysisResults` —
   directly contradicting the frontend-driven-dispatch pillar ("the backend
   CONSUMES decisions; it does not make them").
3. **Silent drops instead of capability declaration.** Unsupported constructs
   vanish without diagnostics:
   - CIRCT: `_ => None` expression drops (`circt/mod.rs:272,477,543`),
     `"0"` fallbacks, `unreachable!()` at :418
   - VM: unsupported expressions push 0 + trap with no source mapping
     (`vm/emit_expr.rs:284-288`)
   - SPIR-V: string errors from three hand-rolled intrinsic emitters only
4. **No semantic validation anywhere outside LLVM.** CIRCT's 17 tests
   string-match their own output; SPIR-V's single test checks non-empty bytes.
5. **Doc/routing drift.** backend-strategy.md carries two contradicting `.abv`
   tables (line 58: ".abv → LLVM accel"; line 69: ".abv → SPIR-V"). Truth:
   `targets.dbvl:20` maps `.abv → spirv`; GPU offload is module metadata
   (`!> accel:`) through `BackendKind::Gpu` (LLVM emitter reuse, `ca23fac3`).

## Principle

Every backend is a first-class consumer of the same frontend decisions.
The compiler declares what each target supports; anything outside that
surface is a helpful compile-time error, never a silent drop. Shared
knowledge lives once, in the pipeline — not re-derived per backend.

## Work items

### 0.1 Hoist analysis into the pipeline

Move the `analyze_program(...)` call out of `LlvmBackend::generate`
(`llvm/mod.rs:2053`) into `compile.rs`, computed once after normalization,
before backend dispatch. Introduce:

```rust
pub struct BackendContext<'a> {
    pub items: &'a [TopLevel],
    pub universe: &'a TypeUniverse,
    pub analysis: &'a AnalysisResults,
    pub int_bits: usize,
}
```

Each backend receives `&BackendContext` (additive parameter migration;
LLVM's builder-option setters stay). LLVM behavior must be unchanged:
verify byte-identical `.ll` output across the full benchmark suite
(A/B against the pre-change build) before merging.

Note: `LlvmBackend::generate` currently takes `(items, Option<&...>)` and
tests construct backends directly (`llvm/tests.rs:93` builds analysis
independently). Keep a direct-construction path for unit tests; the pipeline
path uses `BackendContext`.

### 0.2 Capability matrix + mandatory diagnostics

Extend the `supported_hashtags` pattern (`backend/mod.rs:324`) into a
per-backend declared feature surface:

```rust
pub struct BackendCapabilities {
    pub exprs: FeatureSet,      // Expr variants emitted
    pub stmts: FeatureSet,      // Statement variants emitted
    pub intrinsics: IntrinsicSet, // by category, not name (rule 19)
    pub frgn_sources: &[&str],  // resolvable FFI languages
}
```

A shared pre-codegen validator walks the typed AST against the active
backend's capabilities and emits house-style errors (`src/errors.rs`):
what is wrong, why the target can't, the concrete fix. Every silent-drop
site found in the investigation becomes either an implementation or a
capability error:

| Site | Today | Becomes |
|------|-------|---------|
| `circt/mod.rs:272,477,543` | `_ => None` / `"0"` | capability error or impl |
| `circt/mod.rs:418` | `unreachable!()` | real arm or capability error |
| `vm/emit_expr.rs:284-288` | push 0 + trap | trap WITH source mapping (Plan 1 upgrades payload) |
| `spirv/intrinsics.rs:21` | string error | same semantics via matrix |

Rule 19 applies to the matrix itself: intrinsics declared by protocol
category / property, never `Type::Custom(name)` matches.

### 0.3 Uniform artifact contract

```rust
pub enum Artifact { Text(String), Binary(Vec<u8>) }
```

Backends return `(Artifact, Vec<Diagnostic>)`. `compile.rs:1732` dispatch
collapses to one shape; the Spirv/Vm arms stop special-casing file writes
(`compile.rs:1905-1927`). Linking steps (LLVM/Gpu/Webstack) stay in
compile.rs — they consume `Artifact::Text`.

### 0.4 Doc truth sweep

- `docs/architecture/backend-strategy.md`: fix both contradicting `.abv`
  lines; document the real routing (.abv→spirv standalone kernels,
  accel = `!>` metadata via Gpu). Add the VM charter sentence: *the VM
  exists to finish compilation on any machine with a tamer — one bounty
  archive ships everywhere; macros adapt to the target machine.*
- `docs/architecture/agent-reference.md`: point backend implementors at
  `BackendContext` + capability matrix as the required integration shape.

### 0.5 Dead weight removal

- Delete `src/backend/bindgen.rs` — zero callers (verified 2026-08-23).
- Fix or delete `MemoryOverlay` (`backend/mod.rs:638`) — doc claims use by
  archived C/Rust backends; no live callers.
- Remove unused `MetadataRegistry` construction in circt/webstack
  (constructed at `circt/mod.rs:74`, never read).

### 0.6 CIRCT toolchain install

`tools/install-circt.sh`: pinned CIRCT release built/fetched into
`tools/circt/`, plus an availability probe function mirroring the
`is_available()` pattern (`assembler/mod.rs:41`). Tests gate on the probe
until present. Record the pin hash in the script header. Local status
(2026-08-23): circt-opt absent; spirv-val/dis, wasm-ld, verilator, iverilog
present.

## Documentation maintenance

- New plan file (this document); backend-strategy.md edits in-item.
- No rationale comments removed; the `analyze_program` relocation keeps its
  comment block, updated to say "computed once in the pipeline".
- AGENTS.md Reference Index gains rows for the four per-backend plans when
  they start (added by their respective branches).

## Verification

1. `cargo test --lib` green; no new warnings.
2. Byte-identical `.ll` for all benchmark programs vs pre-change build
   (highest-care item — LLVM generate path touched).
3. Benchmark suite A/B vs baseline worktree: no regression (LLVM untouched
   semantically, but prove it).
4. `git grep "Type::Custom.*==" src/backend/llvm src/glue` still zero.
5. Praetor on changed dirs (`praetor validate --warn --target src/backend`,
   `--target src` for compile.rs).
6. Capability validator demo: one fixture per backend that previously
   silently dropped now fails with a helpful error naming the fix.
