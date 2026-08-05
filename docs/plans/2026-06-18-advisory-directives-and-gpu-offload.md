# Advisory Directives (`#?`), Optimization Remarks, and GPU Offloading

**Date:** 2026-06-18
**Status:** Draft for review
**Phase:** Architecture + multi-sprint

---

## Executive Summary

This plan introduces three interlocking features that extend Briv's
"Safety as Optimization" philosophy from a passive guarantee into an
interactive partnership between the developer and the compiler:

1. **`#?` advisory directive system** — a universal syntactic modifier
   that lets developers express *intent* ("I think this should happen")
   while the compiler evaluates the mechanical feasibility.
2. **Optimization remarks** — per-directive diagnostic feedback that
   teaches the developer *why* the compiler made its decision.
3. **GPU offloading** (`#gpu`, `#gpu?`, `--gpu-offload`) — automatic
   heterogeneous compilation with a cost-benefit analyzer.

---

## Phase 1: `#?` Advisory Directive System

### 1.1 Lexer: New Token

Add `Token::HashQuestion` matching `"#?"` in `src/lexer.rs`:

```rust
#[token("#?")]
HashQuestion,
```

**Must be placed BEFORE `Token::Hash`** in the `logos` derive — logos is
greedy and `#` is a prefix of `#?`. Placing `HashQuestion` first ensures
the longer match wins.

### 1.2 AST: `speculative` Field on `Hashtag`

Add a `speculative: bool` field to `Hashtag` in `src/ast.rs`:

```rust
pub struct Hashtag {
    pub name: String,
    pub value: Option<String>,
    pub mandatory: bool,
    pub speculative: bool,   // NEW
    pub fallback: Vec<String>,
    pub scoped: Option<String>,
}
```

Constructor helpers:
- `Hashtag::new(name)` → `speculative: false`
- `Hashtag::mandatory(name)` → `speculative: false`
- `Hashtag::speculative(name)` → `speculative: true`, `mandatory: false`

### 1.3 Parser: Handle `#?` in `parse_hashtag_modifiers`

In `src/parser.rs`, add a new arm BEFORE the `Token::Hash` arm:

```rust
Some(Ok(Token::HashQuestion)) => {
    self.advance();
    let name = /* parse identifier same as Hash arm */;
    let value = /* parse optional (value) same as Hash arm */;
    mods.push(Hashtag { name, value, mandatory: false, speculative: true, fallback: Vec::new(), scoped: None });
}
```

The existing `Token::Hash` arm produces `speculative: false`.
The `Token::HashBang` arm is unchanged (`mandatory: true`, `speculative: false`).

### 1.4 Validation: Respect Speculative Mode

In `src/backend/mod.rs`, update `validate_single_hashtag`:

- A `speculative: true` tag is **always advisory** — it maps to
  `UnsupportedAdvisory` when unsupported, never to `UnsupportedMandatory`,
  even if the tag name is unrecognized.
- A `#?gpu` on a backend that has no GPU support → warning, not error.
- A `#gpu` (imperative) on a backend that has no GPU support → error.

### 1.5 Display: Annotator

In `src/annotator.rs`, display `#?name` rather than `#name` when
`speculative: true`. This ensures the rendered AST matches source.

### 1.6 Testing

- Lexer: `"#?inline"` → `[HashQuestion, Identifier("inline")]`
- Parser: `#?vectorize` → `Hashtag { name: "vectorize", speculative: true, mandatory: false }`
- Parser: `#?gpu(threshold=1000)` → `Hashtag { name: "gpu", value: Some("threshold=1000"), speculative: true }`
- Validation: `#?unknown_tag` on LLVM backend → `UnsupportedAdvisory` (warning, not error)
- Validation: `#!unknown_tag` on LLVM backend → `UnsupportedMandatory` (error)

---

## Phase 2: Directive → LLVM Metadata Mapping

### 2.1 Hashtag Registry

Extend `supported_hashtags()` in `src/backend/mod.rs` for the `llvm` backend
to recognize the new optimization directives:

```rust
"c" | "x86_64" | "aarch64" | "llvm" => {
    vec![
        "volatile", "sfence", "lfence", "mfence", "aligned", "packed",
        "inline", "unroll", "vectorize", "gpu",
    ]
}
```

### 2.2 Directive Metadata Module

Create a new module `src/backend/llvm/directive.rs` that centralizes the
mapping from a `Hashtag` (directive) to the appropriate LLVM IR annotations.
This keeps the logic out of `emit_toplevel.rs`, `loop_engine.rs`, etc.

```rust
/// Result of resolving a directive hashtag to LLVM annotations.
pub enum DirectiveEffect {
    /// Apply a function-level attribute string (e.g. "alwaysinline").
    FunctionAttribute(String),
    /// Emit a `!llvm.loop` metadata node keyed to the current loop.
    LoopMetadata(&'static str, String),
    /// Emit a remark message (see Phase 3).
    Remark(String),
    /// No effect (directive not applicable in this context).
    None,
}

/// Resolve all relevant directives from a list of hashtags.
pub fn resolve_directives(tags: &[Hashtag], context: DirectiveCtx) -> Vec<DirectiveEffect> { ... }
```

`DirectiveCtx` enum distinguishes where the directive is applied:

```rust
pub enum DirectiveCtx {
    Transaction,    // reactive txn → function attr
    CallableTxn,    // callable txn/defn → function attr  
    Loop,           // foreach or counted loop → loop metadata
    Body,           // general guarded body
}
```

### 2.3 Mapping Table

| Directive | `mandatory` | `speculative` | Ctx | LLVM Effect |
|-----------|-------------|---------------|-----|-------------|
| `#inline` | false | false | Txn | `alwaysinline` on function |
| `#?inline` | false | true | Txn | `inlinehint` on function |
| `#!inline` | true | false | Txn | `alwaysinline` + error if cycles |
| `#unroll` | false | false | Loop | `!llvm.loop.unroll.full` |
| `#?unroll` | false | true | Loop | `!llvm.loop.unroll.enable` (heuristic) |
| `#!unroll` | true | false | Loop | `!llvm.loop.unroll.full` (fail if impossible) |
| `#vectorize` | false | false | Loop | `!llvm.loop.vectorize.enable = true` + strict alignment |
| `#?vectorize` | false | true | Loop | `!llvm.loop.vectorize.enable = true` (advisory) |
| `#!vectorize` | true | false | Loop | `!llvm.loop.vectorize.enable = true` (fail if unsafe) |
| `#gpu` | false | false | Loop | GPU kernel extraction (see Phase 4) |
| `#?gpu` | false | true | Loop | Cost-model evaluation + conditional offload |
| `#!gpu` | true | false | Loop | Force GPU kernel, fail if impossible |

### 2.4 Integration Points

#### `emit_transaction` and `emit_callable_txn` (function-level)

In `src/backend/llvm/emit_toplevel.rs`:

- After computing `alwaysinline` from cycle detection, ALSO check for
  `#inline` / `#?inline` directives.
- `#inline` (imperative) → emit `alwaysinline` regardless of cycles.
  If the call graph has cycles, emit a warning that `alwaysinline` may
  cause code bloat, but honor the directive.
- `#?inline` (advisory) → emit `inlinehint` instead of `alwaysinline`.
  Remove the `alwaysinline` condition when `#?inline` is present and
  cycles exist — `inlinehint` is safe with cycles.
- `#!inline` (mandatory) → emit `alwaysinline`. If cycles detected,
  emit error and abort compilation.

#### `foreach` and loop emission (loop-level)

In `src/features/stmt/foreach.rs` and `src/backend/llvm/loop_engine.rs`:

- Check the statement's `modifiers` for `#unroll` / `#?unroll` /
  `#vectorize` / `#?vectorize` directives.
- Emit appropriate `!llvm.loop.unroll.*` and `!llvm.loop.vectorize.enable`
  metadata on the backedge `br` instruction.
- Currently `!llvm.loop.vectorize.enable = true` is hardcoded in foreach
  (line 92). Make it conditional: emit `true` only when no `#vectorize`
  directive says otherwise, or when `#!vectorize` is present.
- Add unroll metadata:
  - `!llvm.loop.unroll.full` for `#unroll` / `#!unroll`
  - `!llvm.loop.unroll.enable` for `#?unroll` (let LLVM's cost model decide width)
  - `!llvm.loop.unroll.disable` when `#!unroll` with count=0 or explicit `nounroll`

#### Folded loops and counted transactions

In `src/backend/llvm/loop_engine.rs`, the `emit_folded_loop` function
currently uses manual software unrolling (factor 4). When `#unroll` or
`#?unroll` is present:

- `#unroll` → emit `!llvm.loop.unroll.full` on the backedge, AND keep
  software unrolling (LLVM can further unroll the software-unrolled body).
- `#?unroll` → emit `!llvm.loop.unroll.enable` on the backedge. Let LLVM
  decide the factor; remove or reduce software unrolling to avoid
  double-unrolling conflicts.

### 2.5 Testing

- `#inline` on cycle-free txn → LLVM IR contains `alwaysinline`
- `#?inline` → LLVM IR contains `inlinehint`
- `#!inline` with cycles → compile error
- `#vectorize` on foreach → `!llvm.loop.vectorize.enable = true` in IR
- `#unroll` on foreach → `!llvm.loop.unroll.full` in IR
- `#?unroll` → `!llvm.loop.unroll.enable` in IR
- No directives → existing behavior unchanged (vectorization still enabled for foreach)

---

## Phase 3: Optimization Remarks System

### 3.1 Architecture

Build a remark subsystem that the `#?` directives feed into. Each
speculative directive produces a structured remark explaining the
compiler's decision.

#### New Types in `src/backend/llvm/directive.rs` (or a new `remark.rs`):

```rust
pub struct OptimizationRemark {
    pub directive: String,       // e.g. "vectorize"  
    pub location: Span,          // source location of the directive
    pub decision: RemarkDecision,
    pub analysis: Vec<String>,   // bullet points explaining the math
    pub hints: Vec<String>,      // actionable advice
}

pub enum RemarkDecision {
    Applied { detail: String },
    Skipped { reason: String },
    Failed { error: String },
}
```

#### Storage in `LlvmBackend`:

```rust
pub(crate) remarks: Vec<OptimizationRemark>,
pub(crate) emit_remarks: bool,  // set by --remarks CLI flag
```

### 3.2 Remark Emission Points

Each directive resolution site emits a remark when `speculative: true`:

| Directive | Decision point | Example remark |
|-----------|---------------|----------------|
| `#?inline` | After cycle + cost analysis | `applied: function size 14 ≤ threshold 25, inlined successfully` |
| `#?unroll` | After trip-count analysis | `skipped: trip count 4 is below unroll profitability threshold` |
| `#?vectorize` | After dependency analysis | `failed: loop-carried dependency at line 44 prevents vectorization` |
| `#?gpu` | After cost model | `skipped: arithmetic intensity 1.2 ops/byte < minimum 8.0 ops/byte` |

### 3.3 Output Format

When `--remarks` is passed (or `--verbose` enables them), remarks are
printed after compilation:

```
remark: #?vectorize on line 42 did not vectorize
  analysis:
    - loop-carried dependency detected: data[i] -> data[i-1] at line 44
    - LLVM cannot safely execute iterations in parallel
  help:
    - Try restructuring to remove the backward data dependency
    - Use #vectorize (imperative) to force vectorization with runtime checks
```

### 3.4 CLI Flag

Add `--remarks` to the `llvm` and `build` subcommands in `src/main.rs`:

```rust
.arg(Arg::new("remarks")
    .long("remarks")
    .help("Emit optimization remarks for #? speculative directives"))
```

Propagate to `LlvmBackend` via a builder method:

```rust
.with_emit_remarks(true)
```

### 3.5 Testing

- `#?vectorize` on a vectorizable loop → remark with `Applied`
- `#?inline` on a large function → remark with `Skipped` + reason
- `#?unroll` on a known-small loop → remark with `Skipped` (below threshold)
- No `--remarks` flag → no remark output (zero overhead)
- `--remarks` without any `#?` directives → empty remark list

---

## Phase 4: GPU Offloading Infrastructure (SPIR-V + Vulkan)

This is the largest and most architecturally significant feature.
It breaks into five sub-phases.

### Design Decision: SPIR-V + Vulkan Compute

After research, the target is **SPIR-V** emitted via LLVM's built-in
`spirv64-unknown-unknown` backend, dispatched via **Vulkan compute**.

**Why SPIR-V:**
- LLVM includes a full SPIR-V backend (`llvm/lib/Target/SPIRV/`) with
  instruction selection, legalizer, and code gen — already compiled into
  the LLVM Briv depends on.
- SPIR-V is the most portable GPU IR: runs on NVIDIA (NVK), AMD (RADV),
  Intel (ANV), Apple (MoltenVK), and software (LLVMPipe/Mesa).

**Why Vulkan:**
- Modern, actively maintained by all GPU vendors via Mesa.
- OpenCL is legacy but consumes the same SPIR-V — secondary runtime
  path (`--gpu-backend vulkan,opencl`) is a compile-time flag.

**Dual compilation model:**

When `#gpu` or `--gpu-offload` is active, the compiler emits TWO outputs:
1. **CPU binary** via existing LLVM x86/ARM backend (GPU loops become
   dispatch calls with CPU fallback).
2. **SPIR-V blobs** via separate `spirv64` codegen pass, embedded in the
   executable's `.rodata` as opaque byte arrays.

At runtime, `briv_gpu_rt.c` (Vulkan compute runtime) handles:
- Vulkan instance/device creation
- SPIR-V shader module loading
- Device memory allocation + upload/download
- Compute pipeline dispatch
- Graceful CPU fallback when Vulkan is unavailable

### 4.1 Sub-phase A: Kernel Extraction

**Goal:** Extract a transaction body (or loop body) into a standalone
SPIR-V kernel function and generate host-side Vulkan dispatch calls.

**Location:** New module `src/backend/llvm/gpu.rs`.

**Kernel eligibility check** (building on existing `gpu_eligible`):

A transaction is GPU-eligible when:
1. No FFI calls in the body (purity)
2. No loop-carried dependencies (parallelizable)
3. Contiguous memory access patterns (coalesced reads/writes)
4. Bounded iteration count (known or provably finite)
5. No `term`/`term!`/`unification`/`escape` statements
6. Only operates on integer and float types (no string/struct/enum)
7. No `String`/`struct`/`HashMap`/`enum` — SPIR-V storage buffers
   support only flat data types with known byte offsets.

**Extraction algorithm:**

1. Clone the transaction body AST
2. Wrap it in a function `@kernel_<name>(i8* %state, i64 %N)`
3. Replace state field accesses with SPIR-V storage buffer accesses:
   `state.field[i]` → `%buffer_base[i * stride + field_offset]`
4. Emit as a separate LLVM module with `spirv64-unknown-unknown` triple:
   ```bash
   llc --mtriple=spirv64-unknown-unknown kernel.ll -o kernel.spv
   ```
5. In the main CPU module, replace the loop body with Vulkan dispatch:
   - `briv_gpu_init()` → one-time Vulkan instance creation
   - `briv_gpu_malloc()` → `vkAllocateMemory`

**Control flow:**

```
Original:
  [i < N] { data[i] = sin(data[i]) * cos(data[i]); }

After #gpu:
  // Host code (CPU binary):
  %gpu_ok = call i1 @briv_gpu_is_available()
  br i1 %gpu_ok, label %gpu_path, label %cpu_path

gpu_path:
  %dev = call i64 @briv_gpu_malloc(i64 %N * 8)
  call void @briv_gpu_memcpy(i64 %dev, i64 %host, i64 %N*8, i32 0)  // H2D
  call void @briv_gpu_launch(i64 @kernel_0, i32 %N, i32 256, i64 %dev)
  call void @briv_gpu_memcpy(i64 %host, i64 %dev, i64 %N*8, i32 1)  // D2H
  call void @briv_gpu_free(i64 %dev)
  br label %merge

cpu_path:
  // Straight-line CPU loop (identical to un-annotated code)
  [i < N] { data[i] = sin(data[i]) * cos(data[i]); }
  br label %merge

merge:
  ...
```

### 4.2 Sub-phase B: The Arithmetic Intensity Cost Model

**Goal:** Determine whether offloading to GPU is worth the PCIe transfer
overhead.

**Location:** `src/analysis/gpu_cost.rs` (new analysis module).

**Inputs:**
- Loop body AST (for counting operations)
- Iteration count `N` (compile-time known or runtime variable)
- Memory transfer size (bytes read + bytes written)

**Cost model:**

```rust
pub struct GpuCostEstimate {
    pub arithmetic_intensity: f64,      // ops / byte
    pub estimated_cpu_time_ns: f64,
    pub estimated_gpu_time_ns: f64,     // includes PCIe transfer
    pub crossover_point: u64,           // N where GPU becomes faster
    pub recommended: OffloadDecision,
}

pub enum OffloadDecision {
    Gpu,        // GPU is faster → offload
    Cpu,        // CPU is faster → keep on CPU
    Runtime,    // N is runtime-determined → emit dispatch branch
}
```

**When N is compile-time known:**
- Compute the exact estimates
- If GPU wins → emit GPU kernel (like `#gpu`)
- If CPU wins → emit CPU loop with a remark explaining why

**When N is runtime-determined:**
- Compute the crossover point `N_c` from the model
- Emit a dispatch branch:
  ```llvm
  %is_worth_it = icmp sgt i64 %N, <crossover_point>
  br i1 %is_worth_it, label %gpu_path, label %cpu_path
  ```

### 4.3 Sub-phase C: `#gpu` and `#gpu?` Directive Handling

**`#gpu` (imperative):**
1. Run kernel eligibility check
2. If eligible → extract kernel, emit host orchestration
3. If ineligible → compile error with detailed explanation
4. Emit remark explaining the kernel launch config (block size, shared mem)

**`#gpu?` (speculative):**
1. Run kernel eligibility check
2. Run cost model
3. If `recommended == Gpu` → emit GPU kernel (same as `#gpu`)
4. If `recommended == Cpu` → emit CPU loop, emit remark with analysis
5. If `recommended == Runtime` → emit dispatch branch + both paths

### 4.4 Sub-phase D: `--gpu-offload` Global Flag

**CLI flag:** Add `--gpu-offload` to `llvm` and `build` subcommands.

**Behavior:** Treat EVERY loop/transaction in the program as if annotated
with `#gpu?`. Run eligibility + cost model on each.

**Integration with PGO:**

If `--pgo-generate` + `--gpu-offload` are combined:
1. During PGO instrumented run, record actual loop bounds
2. In the optimization compilation, use PGO data as the iteration count
   estimate instead of worst-case analysis
3. This enables the cost model to make exactly correct offload decisions
   based on real-world data sizes

### 4.5 Sub-phase E: Runtime Support Library (Vulkan Compute)

Create `briv_gpu_rt.c` alongside `briv_rt.c` implementing a lightweight
Vulkan compute runtime:

```c
// One-time init — creates Vulkan instance, picks a compute-capable device.
int   briv_gpu_init();
// Returns 1 if Vulkan is available and a compute device was found.
int   briv_gpu_is_available();
// Allocate device memory (vkAllocateMemory).
int64_t briv_gpu_malloc(size_t bytes);
// Free device memory.
void    briv_gpu_free(int64_t handle);
// Copy host→device (vkMapMemory + memcpy or vkCmdCopyBuffer).
void    briv_gpu_memcpy(int64_t dst, int64_t src, size_t bytes, int dir);
// Load SPIR-V blob as a shader module and dispatch compute.
// kernel_idx indexes into the embedded SPIR-V array.
void    briv_gpu_launch(int kernel_idx, int grid_x, int block_x,
                         int64_t* buffer_handles, int num_buffers);
// Cleanup.
void    briv_gpu_shutdown();
```

**Linking model:**

At link time, there are three possible resolutions:
1. **Vulkan available** (`vulkan_loader`): dlopen `libvulkan.so.1` at
   runtime, resolve all vk* function pointers lazily. Graceful if missing.
2. **OpenCL fallback** (`--gpu-backend opencl`): consume the same SPIR-V
   blob via `clCreateProgramWithIL`.
3. **CPU fallback** (no GPU runtime): `briv_gpu_is_available()` returns 0.
   All GPU loops execute the CPU path. Zero additional dependencies.

For `#gpu?` runtime dispatch, this means the compiled binary works on
any machine — with or without a GPU — and transparently picks the right
path at startup.

### 4.6 Testing

- `#gpu` on pure parallel loop → LLVM IR contains kernel function + launch calls
- `#gpu` on loop with FFI call → compile error
- `#?gpu` on small loop (N=10) → CPU remains, remark explains crossover
- `#?gpu` on large loop (N=10^7) with high intensity → GPU kernel emitted
- `--gpu-offload` with mixed loops → only eligible loops get kernels
- PGO + `--gpu-offload` → PGO data used in cost model
- Cross-compilation: NVPTX target triple in separate module

---

## Phase 5: Integration and Polish

### 5.1 `#?` as Universal Speculative Modifier

All directives follow the same pattern:

| Directive family | Imperative (`#`) | Advisory (`#?`) | Mandatory (`#!`) |
|-----------------|------------------|-----------------|------------------|
| Inline | `alwaysinline` | `inlinehint` | `alwaysinline` + error-if-cycles |
| Unroll | full unroll | heuristic unroll | full unroll + fail-if-impossible |
| Vectorize | strict vectorize | advisory vectorize | strict + fail-if-impossible |
| GPU | force kernel | cost-model conditional | force + fail-if-ineligible |

### 5.2 Architecture Documentation

Create (or update) these docs:

| Doc | Location | Content |
|-----|----------|---------|
| Advisory directives | `docs/architecture/features/advisory-directives.md` | `#?` syntax, mapping table, remark system |
| GPU offloading | `docs/architecture/gpu-offloading.md` | Cost model, kernel extraction, PGO synergy |
| Optimization remarks | `docs/architecture/optimization-remarks.md` | Remark format, CLI flags, educational goals |

### 5.3 Update CLAUDE.md / AGENTS.md

Add a quick-reference entry for the `#?` syntax and `--gpu-offload` flag.

---

## Implementation Order and Dependencies

```
Phase 1 (Lexer/Parser/AST)
  │
  ├──→ Phase 2.1 (Hashtag registry)
  │       │
  │       └──→ Phase 2.2–2.4 (Directive metadata mapping)
  │               │
  │               ├──→ Phase 3 (Optimization remarks) [can start after 2.2]
  │               │
  │               └──→ Phase 4 (GPU offloading) [depends on 2.1, 2.2]
  │                       │
  │                       └──→ Phase 5 (Integration, docs, polish)
  │
  Phase 5 also wraps everything
```

### Recommended sprint breakdown:

| Sprint | Focus | Deliverables |
|--------|-------|-------------|
| **E5** | Phase 1 + 2.1 | `#?` lexer/parser/AST, hashtag registry update, tests |
| **E6** | Phase 2.2–2.4 | `directive.rs` module, `#inline`/`#?inline` in codegen, `#unroll`/`#?unroll` in loops, `#vectorize`/`#?vectorize` integration |
| **E7** | Phase 3 | Remark infrastructure, `--remarks` CLI, per-directive remarks for all Phase 2 directives |
| **E8** | Phase 4A–4B | Kernel extraction, cost model analysis |
| **E9** | Phase 4C–4E | `#gpu`/`#gpu?` handling, `--gpu-offload`, runtime library |
| **E10** | Phase 5 | Documentation, polish, edge case hardening |

---

## Testing Strategy

### Unit Tests

| Layer | What to test |
|-------|-------------|
| Lexer | `#?` token recognition, ordering with `#` |
| Parser | `Hashtag.speculative` correctly set for `#?`, `#`, `#!` |
| AST | `Hashtag::speculative()` constructor |
| Validation | Speculative tags never produce `UnsupportedMandatory` |
| `directive.rs` | Each directive→metadata mapping for all 3 modes |
| Remarks | Remark struct construction, formatting, suppression without `--remarks` |
| Cost model | Arithmetic intensity computation, crossover point calculation |
| Kernel extraction | Eligibility check, AST cloning + rewriting |

### Integration Tests

| Test | What it verifies |
|------|-----------------|
| `#?vectorize` on foreach loop | IR contains `!llvm.loop.vectorize.enable` |
| `#?unroll` on folded loop | IR contains `!llvm.loop.unroll.enable` |
| `#inline` on reactive txn | IR function has `alwaysinline` |
| `#?inline` on callable txn | IR function has `inlinehint` |
| `--remarks vectorize.bv` | Remark output matching expected format |
| `#gpu` on pure parallel loop    | IR contains kernel function + host launch |
| `#gpu` on impure loop           | Compile error |
| `#?gpu` on small loop           | CPU-only, remark explains why |
| `--gpu-offload` mixed program   | Only eligible loops get GPU kernels |
| `#!vectorize` on unsafe loop    | Compile error |

### Existing test preservation

All 911 existing tests must continue to pass after each phase.
No existing optimization path is weakened — all changes are additive
(new match arms, new fields, new modules).

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| `#?` collides with `#` in logos lexer | Silent parse errors | Place `HashQuestion` BEFORE `Hash`; add lexer test for both `#inline` and `#?inline` |
| `Hashtag` struct grows — existing code doesn't set `speculative` | Tests fail or fields default to wrong value | Default `speculative` to `false` for backward compat; audit ALL `Hashtag` construction sites (there are many: `parser.rs`, `proof_engine.rs`, tests, etc.) |
| GPU kernel extraction creates IR that LLVM cannot optimize | Silent performance degradation | Add `opt -O3 -verify` step for the SPIR-V module in tests |
| Runtime dispatch branch (`#gpu?`) causes binary bloat | Large binaries from dead paths | Emit only the needed path when N is compile-time known; use linker GC for dead sections |
| `--gpu-offload` + PGO creates complex interaction | Wrong offload decisions | Validate against synthetic benchmarks with known crossover points |
| SPIR-V backend not enabled in the build's LLVM | `spirv64` target triple fails | Make SPIR-V emission conditional: `--features spirv` enables it; fallback emits remark and CPU-only binary |
| Vulkan runtime dlopen fails | GPU path silently falls back to CPU | `briv_gpu_is_available()` returns 0; CPU path always compiled as fallback |

---

## Key Design Principles

1. **Purity is the enabler.** GPU offloading is safe only because Briv
   enforces a pure domain bounded by FFI/I/O. The cost model is meaningful
   only because the compiler can count every operation.

2. **Additive only.** No existing optimization path is modified. Every
   change is a new match arm, a new module, or a new field with safe
   defaults (`false` for `speculative`, empty vec for `remarks`).

3. **The developer's domain intent, the compiler's mechanical knowledge.**
   `#?` says "I think this is a good idea." The compiler says "Here's why
   it is (or isn't)," teaching the programmer about the hardware.

4. **Safety-first GPU.** `#gpu` fails compilation if the loop is unsafe
   for GPU execution. `#gpu?` never silently degrades — it either offloads
   or explains why not, and optionally emits a runtime dispatch branch.

5. **Remarks teach.** Every `#?` decision includes actionable analysis
   and help text. The compiler is a mentor, not a black box.
