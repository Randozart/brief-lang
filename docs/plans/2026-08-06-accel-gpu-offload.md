# `accel` Keyword + Top-Level `!>` Metadata — Verifiable GPU Deferral

**Date:** 2026-08-06
**Status:** Implemented (2026-08-06) — merged into main as `b3aff893`; nbody_newton_accel MATCH (7.47x C wins), auto-tuning probe shipped
**Branch:** `feat/accel-gpu` (worktree `../briv-compiler-accel`)
**Baseline commit:** `0461a559`
**Baseline worktree:** `../briv-compiler-baseline` (detached HEAD)
**Comparable with:** `bash benchmarks/compare_baseline.sh`
**Companion SPEC change:** `spec/SPEC.md` §8.9 (top-level metadata), §9.7 (`accel`)

---

## 0. Executive Summary

Briv's nbody benchmarks measure CPU-vs-C parity on sequential, scalar
time-stepped code. A real Briv program with many bodies would want a targeted
GPU speedup that the benchmarks are not written to expose. The historical
`#GPU` pragma was opaque and has been removed along with the rest of the
pragma system. This plan replaces it with a first-class **`accel` keyword** on
top-level bodies, plus a **top-level `!>` metadata** mechanism as a module-wide
shortcut — and, critically, GPU deferral happens **only when the compiler can
verify the speedup**.

The design is grounded in Briv's operating rules:

- **Rule 19 (MEASURE BEFORE YOU BUILD):** performance cannot be proven
  statically when the workload (`BOUND`, body count) is runtime-determined and
  the device differs between machines. Verification is therefore **empirical**:
  a minimal-overhead auto-tuning probe at program start runs the accelerated
  body on both the CPU and the GPU path, verifies output equality within
  tolerance, and commits to the faster path for the rest of the process.
- **Rule 2 (disclose special treatment):** `accel` is an ordinary keyword, not
  hidden behind ordinary-looking syntax. GPU deferral never "beats" the CPU
  default — it is gated on a verified win, so the CPU default is never the
  slower choice by construction.
- **Frontend-driven dispatch:** eligibility proof, cost model, and the
  `Gpu | Probe | Cpu` decision are computed once in `src/analysis/accel.rs`
  and stored in `AnalysisResults`, exactly like `loop_shapes` and `swan_songs`.
  The backend only consumes the decision; it never re-derives it.
- **Rule 18 (no type-name matching):** all type checks derive from the
  `TypeUniverse` (`universe_key()`, casting-graph properties), never from
  matching Briv type names.
- **Rule 1 (contract-first):** GPU eligibility is a *proof obligation*, not a
  heuristic. Ineligible or unverifiable bodies fall back to CPU silently with
  an optimization remark; contracts are never weakened.

The existing GPU pipeline is a vestigial stub: SPIR-V kernels are extracted and
embedded but **never dispatched** (`briv_gpu_launch` is never emitted), the
`#?gpu` speculative path is dead code, and the cost model runs inside the
backend, violating frontend-driven dispatch. This plan **rewrites** the pipeline
on the current architecture. SPIR-V is retained because it is vendor-portable
and every mainstream GPU API (Vulkan, OpenCL 3.0, Level Zero) consumes it;
kernel emission is
rebuilt to reuse the mature LLVM expression/statement emitter instead of the
hand-rolled duplicate emitter in the old `gpu.rs`.

The current nbody benchmarks are **not modified**. A new
`nbody_newton_accel` benchmark is added, run by itself, and compared with
tolerance against its own C reference.

---

## 1. Motivation

### 1.1 The nbody realization

`benchmarks/nbody_newton.bv` is a 5-body scalar, sequential time-step
simulation:

```briv
node simulate [count < bound][count == bound] {
    let dx01: Float32 = bx0 - bx1;
    // ... ~600 lines of scalar pair computations ...
    count = count + 1;
    when count == bound { endprogram println!(last_energy); };
    term;
};
```

Every step carries state into the next step (`vx0 = nvx0; ...`). The whole body
is loop-carried — it cannot be offloaded to a GPU as a unit. A GPU win for
nbody requires many bodies and an O(N²) per-step force computation parallelized
over work-items, with a sequential host loop over time steps. That is a
**different program structure** (arrays, SoA layout, per-firing kernels), which
is exactly what the `_accel` benchmark variant is for.

This exposes the two design requirements:

1. `accel` marks a body whose **firing** is a data-parallel map over work-items
   — the GPU can run all work-items of one firing in parallel.
2. The speedup is workload- and device-dependent. A 5-body force kernel would
   be *slower* on a GPU (launch overhead dominates). The compiler must not
   offload it. It must only offload when it can **verify** GPU wins.

### 1.2 Why not a pragma

`#GPU` was opaque: it was not type-checked, had no value vocabulary, and
duplicated `#`-marker syntax whose real meaning (intrinsic suffix, hashword
prefix) is unrelated to codegen directives. The pragma family (`#gpu`,
`#?gpu`, `#!gpu`, `--gpu-offload`) is being pruned. The replacement uses the
two mechanisms Briv already considers non-pragma:

- **Ordinary keywords** for user-facing directives (`seq`, `vol`, `async`,
  `sync<g>`), per SPEC §8.9/§12.1 — this is what `accel` is.
- **`!>` metadata bindings** (`!> key: value;`) for compiler-known,
  typed, config-driven behavior (SPEC §8.9) — this is what module-level
  `!> accel: try_all;` is.

---

## 2. Locked Decisions

| # | Topic | Decision |
|---|---|---|
| D1 | Primary trigger | `accel` keyword prefix on `node`/`txn` (per-body), same surface as `seq`/`out` |
| D2 | Module shortcut | Top-level `!>` metadata attaches `accel` policy to the **module**. Values are lowercase policy atoms (see §4.3): `try_all`, `force`, `try_all_force`. Absent key = keyword-marked bodies only |
| D3 | Metadata scope | Top-level `!>` is **module-level only** (a shortcut to attach metadata to the script), not declaration-attached |
| D4 | Candidate resolution | Two axes — *target* (all bodies vs `accel`-keyword bodies) and *mode* (try vs force). See §4.4 for the resolution matrix. Per-body `accel` keyword marks the body in every mode |
| D5 | GPU target | SPIR-V kernel emission (one blob serves every SPIR-V consumer) + device-agnostic `briv_accel_rt` glue with a pluggable driver table (Vulkan + OpenCL static; see §7) |
| D6 | Kernel model | Design A: the work-item counter is a REAL state field (`let i: Int = 0;` + `i = i + 1`). `accel` marks a native counted loop `[i < N][i == N]` as a parallel map; the compiler proves the map and coalesces the loop into one GPU dispatch (fast-forwarding the counter to N). No virtual variables |
| D7 | Speedup verification | Runtime auto-tuning probe at program start, minimal overhead, when the decision is `Probe` (runtime N). `try` modes only — `force` skips the speedup gate |
| D8 | Static decision | When N is compile-time-known and N ≥ crossover, decision is `Gpu` with no probe |
| D9 | Failure behavior | `try` modes: silent CPU fallback + optimization remark, never a compile error. `force` mode (keyword-marked bodies): ineligible = compile error, unverified speedup still offloads (developer asserts), no device available at runtime = runtime error |
| D10 | `_accel` correctness | Tolerance-based comparison vs C reference (harness epsilon override) |
| D11 | Worktree | `../briv-compiler-accel`, branch `feat/accel-gpu`, isolated from `feat/out-observability` agent |
| D12 | Removals | `#gpu`/`#?gpu`/`#!gpu` directives, `--gpu-offload` flag, `.abv: spirv; --gpu-offload` default, old `gpu.rs` emitter, backend-side `gpu_cost` invocation |
| D13 | Per-body opt-out | Deferred. v1 has no per-body opt-out (an unmarked body is never a candidate unless the mode targets all bodies). In-body `!> accel: off;` on transactions is a follow-up |
| D14 | Baseline discipline | `cargo test --lib` green per commit; baseline A/B via `../briv-compiler-baseline` + `compare_baseline.sh` before/after performance work |
| D15 | Casing | Metadata keys and values are lowercase (matching the existing `!>` vocabulary: `bits`, `overflow`, `fp_math`, ...). ALL_CAPS rejected — breaking churn for zero benefit |
| D16 | Observability axis | `!> accel_report: verbose;` is a separate key (composes with any `accel` policy value). Emits a remark for every analyzed body — offloaded, CPU-fallback, and ineligibility reasons |

---

## 3. Research Findings (Codebase Survey)

### 3.1 Metadata infrastructure is mature — reuse it

- `!>` lexes as `Token::ExclaimArrow` (`src/lexer.rs:313`). `!> key: value;`
  parses to `HashMap<String, PropertyValue>`.
- `parse_body_metadata` (`src/parser/metadata.rs:15`) parses `!>` pairs and
  `#key` annotations inside type/struct/defn bodies. `parse_metadata_value`
  (`metadata.rs:36`) handles identifier/int/bool/string/list.
- `MetadataRegistry` (`src/backend/metadata.rs:57`) is DBV-backed
  (`config/meta-vocab.dbv`): `MetaField` schema rows define typed fields;
  `BackendMapping` rows map `(backend, metadata_key, value_pattern,
  ir_attribute, scope)` → IR attribute. Lookups: `llvm_attr`, `webstack_option`,
  `circt_option` (`metadata.rs:121-133`). Consumers:
  `apply_llvm_function_metadata`, `emit_fast_math_flags`,
  `apply_webstack_metadata`, `apply_circt_metadata` (`metadata.rs:196-252`).
- **Gap:** `Transaction` (node/txn) **never** parses metadata — `metadata` is
  hardcoded `HashMap::new()` in `parse_node`, `parse_transaction`,
  `parse_sync_group` (`src/parser/definitions.rs:568,605,668`). `Definition`
  and `TypeDef` do parse it.
- **Gap:** no top-level `!>` exists.

### 3.2 Modifier keyword pattern (template for `accel`)

`seq`, `out`, `async` prefix keywords are handled in `parse_top_level`
(`src/parser/definitions.rs:40-120`): consume the keyword, parse the node/txn,
push `Annotation { name: "...", value: None }` onto `txn.modifiers`. The
backend reads modifiers, e.g. `txn.modifiers.iter().any(|m| m.name == "seq")`.

`accel` follows this exact pattern: new `Token::Accel`, a `parse_top_level`
arm, and a modifier annotation.

### 3.3 Doc-comment attach is the model for module metadata parsing

`parse_top_level` collects `Token::DocComment`/`DocCommentBang`, calls
`set_doc`, recurses, and the next item consumes via `take_doc`
(`definitions.rs:168-179`). Top-level `!>` metadata does the same *without the
attach step*: parse, accumulate into one module map, recurse.

### 3.4 The GPU pipeline is a vestigial stub — rewrite, don't reuse

- `src/backend/llvm/gpu.rs` (1620 lines): hand-rolled SPIR-V LLVM IR emitter.
  Duplicates the expression/statement emission pipeline. Kernel signature is a
  per-work-item buffer model (each work-item gets its own stride of state),
  unsuitable for shared-array nbody kernels.
- `collect_gpu_kernel` (`src/backend/llvm/mod.rs:1124`): eligibility check →
  `gpu_cost::estimate` (for "speculative") → `extract_kernel` →
  `emit_spirv_module` → `compile_to_spirv` (shells to
  `llc --mtriple=spirv64-unknown-unknown`) → embeds blob into `.rodata`
  (`emit_spirv_embeds`, mod.rs:1487).
- **Critical defect:** the CPU program never dispatches. No `briv_gpu_launch`
  call is ever emitted; the transaction body is still emitted as normal CPU
  code. The blobs are dead data.
- **Dead speculative path:** `is_speculative` checks
  `m.name == "gpu" && m.name.starts_with('?')` (`emit_toplevel.rs:2278`) —
  a modifier named `"gpu"` can never start with `'?'`. `OffloadDecision::Runtime`
  (runtime crossover dispatch) is never exercised.
- `#gpu`/`#?gpu`/`#!gpu` are parsed as `#key` annotations by
  `parse_body_metadata` (`metadata.rs:17,24-27`).
- `--gpu-offload` flag: `src/main.rs:248`, default for the `.abv` extension in
  `config/targets.dbvl:20` (`.abv: spirv; --gpu-offload; prelude;`).
- `briv_gpu_rt.c` exists (`lib/runtime/briv_gpu_rt.c`): dlopens BOTH
  `libvulkan.so.1` and `libOpenCL.so` (Vulkan first, OpenCL fallback — both
  consume SPIR-V via `clCreateProgramWithIL`), storage buffers, CPU fallback
  via `briv_gpu_is_available()`. This dual-API is the seed of the §7 driver
  table.

### 3.5 Frontend-driven dispatch is the integration point

`AnalysisResults` (`src/backend/mod.rs:26`) holds per-txn analysis keyed by
name: `loop_shapes`, `swan_songs`, `density`, `modulo_partition`,
`has_unguarded_ffi`, `inline_decisions`, `batch_shape`, `global_lifetime`,
`observable_names`. `analyze_program(items, optimize, min_width)`
(`backend/mod.rs:81`) builds them. This plan adds:

- `AnalysisResults.accel: HashMap<String, AccelDecision>` (per-txn decision).
- `AnalysisResults.module_metadata: HashMap<String, PropertyValue>` (from
  `TopLevel::ModuleMetadata`).

The signature change (`analyze_program` gains a module-metadata parameter or
reads it from the items slice) must be threaded through the eight backend
callers (`backend/wasm.rs:31`, `verilog.rs:74`, `vhdl.rs:72`, `c.rs:83`,
`cobol.rs:57`, `aarch64.rs:160`, `x86_64.rs:180`, `rust.rs:56`,
`llvm/mod.rs:1721`).

### 3.6 Language facts relevant to the design

- Arrays: `Float[64]` is `Type::Vector` with compile-time size
  (`src/parser/types.rs:71`). Runtime-sized arrays do not exist; the `_accel`
  benchmark uses a compile-time `MAXB` with a runtime `BODYCOUNT ≤ MAXB`.
- GPU intrinsics already exist: `GetGlobalId#`, `GetLocalId#` in
  `src/intrinsic_signatures.rs:120-122` and `lib/std/gpu.bv`.
- Flat-value types (`Int`/`Float`/`Bool`/`Char`) are the sole data that can
  cross the host↔device boundary in v1 (SPIR-V storage buffers of primitive
  values). Strings, structs, pointers are rejected by the eligibility proof.

### 3.7 Benchmark harness

- TAG map (`benchmarks/build_and_bench.sh:80`), `BENCHMARKS` list
  (`build_and_bench.sh:110`), `gpu_flag` for `gpu/*` (`:200-205`).
- Correctness: string match, then per-line float epsilon
  (`eps=0.00001`, `build_and_bench.sh:459-469`). `BRIV_CROSS_REF` maps
  benchmarks to foreign C references (`:384`).
- Size-gated precompute detection (`is_precompute_ok`, `:342`): an `accel`
  benchmark whose binary is GPU-dispatched must still produce an observable
  output at `BOUND=5` (the probe is also the liveness guard).
- `nbody_newton`/`nbody_sqrt` get `budget=2048` (`:194`).

### 3.8 Worktrees and concurrent agents

`git worktree list`:

```
/home/randozart/Desktop/Projects/briv-lang                [main]
/home/randozart/Desktop/Projects/brief-compiler-baseline  (detached, prunable)
/home/randozart/Desktop/Projects/brief-compiler-dogfood   [compiler-in-brief]
/home/randozart/Desktop/Projects/brief-compiler-out       [feat/out-observability]
/home/randozart/Desktop/Projects/briv-compiler-baseline   (detached)   ← A/B baseline
```

Another agent works in `brief-compiler-out` (`feat/out-observability`).
`main` also carries **uncommitted interpreter changes**
(`src/interpreter/{casts,eval,mod}.rs`) that must never be touched or
discarded (Golden Rule 7). The accel work therefore runs in a fresh worktree
`../briv-compiler-accel` on branch `feat/accel-gpu` with its own `target/`
directory (no build lock).

---

## 4. Language Design

### 4.1 `accel` keyword (per-body)

**Syntax (Design A — real counter, no virtual variables):**

```briv
let i: Int = 0;                      // work-item counter, explicit init
accel node force [i < nbodies][i == nbodies] {
    dv[i] = ...;                     // per-work-item compute
    i = i + 1;                       // native counted-loop advance
    term;
};
```

- `accel` is a prefix keyword on `node`/`txn`, parsed like `seq`
  (`definitions.rs:40-54`), producing `Annotation { name: "accel" }` on
  `txn.modifiers`.
- **Semantics:** the node is an ordinary counted loop over the counter `i`
  (precondition `[i < N]` = bound + access gate; postcondition `[i == N]` =
  goal, "loop until true"). The compiler PROVES it is a parallel map over
  work-items — `i` is the counter (incremented in the body), every write
  targets a slot affine in `i` (disjoint), reads may be shared, types are flat.
- **Dispatch:** on the GPU path, one dispatch of N work-items replaces the
  N-firing loop; the runtime launches the kernel (work-item id = counter) and
  fast-forwards the counter to N so the loop bound is met after one firing. On
  the CPU path the loop runs natively — each firing is one work-item.
- Cross-work-item data exchange is only legal through host-sequenced separate
  accel nodes, never within one firing.
- **Eligibility is a proof obligation.** If the proof fails, the body falls
  back to CPU with a remark. It is never a compile error on its own (D9).

### 4.2 Top-level `!>` metadata (module shortcut)

**Syntax:**

```briv
//! script-level metadata; accumulates into one module map
!> accel: try_all;
!> target: spirv;

node foo [pre][post] { ... };
node bar [pre][post] { ... };
```

- Top-level `!>` is **module-level only** (D3): every top-level `!>` line
  merges into `ModuleMetadata` (last-wins per key). It is a shortcut for
  attaching metadata to the script, not to the next declaration.
- Value grammar reuses `parse_metadata_value` (identifier/int/bool/string/list).
- Keys and values are **lowercase** (D15) — the entire existing `!>` vocabulary
  (`bits`, `overflow`, `fp_math`, `inline_hint`, ...) is lowercase snake_case,
  and `!> accel: TRY_ALL;` would be off-beat.
- The module map is exposed as `AnalysisResults.module_metadata` and consumed
  by any backend/plugin through `MetadataRegistry`.

### 4.3 `accel` value vocabulary (v1)

The policy is two orthogonal axes: **target** (which bodies are candidates)
× **mode** (whether GPU is required or merely tried). There is no `off` value
— absent is off.

| Value | Target | Mode | Behavior |
|---|---|---|---|
| *(absent)* | `accel`-keyword bodies | try | default: bodies carrying the `accel` keyword are candidates; speedup verified (probe), silent CPU fallback |
| `try_all` | all bodies | try | every eligible body is a candidate; verified probe per body |
| `force` | `accel`-keyword bodies | force | keyword-marked bodies MUST offload: ineligible = compile error; speedup not verified (developer asserts); no device available at runtime = runtime error |
| `try_all_force` | all bodies try + keyword bodies force | hybrid | union of `try_all` and `force` |

The three values cover the design space the developer asked for: *trying on
all bodies*, *forcing on keyword-marked bodies*, and *trying on all while
forcing on the marked ones*. Absent is the conservative default.

`!> accel_report: verbose;` (D16) is a **separate** observability key and
composes with any policy value:

```briv
!> accel: try_all;
!> accel_report: verbose;
```

`verbose` emits an optimization remark for every analyzed body (offloaded,
CPU-fallback, or ineligible, with reasons and the crossover evidence). Absent
= default remark level. It is intentionally *not* a value of the `accel` key:
verbosity is orthogonal to policy, and last-wins single-key semantics would
make `!> accel: verbose;` mutually exclusive with `try_all`/`force`.

D13 (per-body in-body `!> accel: off;`) is deferred.

### 4.4 Policy resolution (D4)

`accel.rs` resolves the module policy into an `AccelMode` once:

```rust
pub enum AccelMode {
    /// Absent: keyword-marked bodies, try mode.
    TryKeyword,
    /// `try_all`: every body, try mode.
    TryAll,
    /// `force`: keyword-marked bodies, force mode.
    Force,
    /// `try_all_force`: every body tried; keyword-marked forced.
    TryAllForce,
}
```

Per-body candidate status and mode:

```
mode = resolve(module_metadata["accel"])
is_marked(body) = body.modifiers.contains("accel")

targets(body) =
    mode ∈ { TryAll, TryAllForce }                       // all bodies
    || (mode ∈ { TryKeyword, Force, TryAllForce } && is_marked(body))

mode_for(body) =
    Force if (mode == Force || mode == TryAllForce) && is_marked(body)
    else Try
```

`Try`-mode candidates: speedup verified (probe / static crossover); any miss
⇒ silent CPU + remark (D9). `Force`-mode candidates: eligibility must prove
(compile error otherwise); speedup gate skipped; runtime errors if no device
is available (D9). Verifiable candidates get an `AccelDecision` (below).

---

## 5. Frontend Analysis — `src/analysis/accel.rs`

A first-class pass, invoked from `analyze_program`, producing
`AnalysisResults.accel`. Computed once, consumed by the backend. Mirrors the
existing pattern of `loop_shape.rs` / `swan_song.rs`.

### 5.1 Eligibility proof (correctness)

Proves, in order, for each candidate body:

1. **Bound (Design A):** the contract precondition is a comparison `i < N`
   where `i` is a **real state counter** — declared (`let i: Int = 0;`),
   incremented in the body, and never a compiler-synthesized variable. The
   analysis verifies `i` is a state field and that the body advances it
   (`i = i + 1`) so the loop terminates. Extracts `i` as the work-item index
   and `N` as the work-item count expression.
2. **Write disjointness:** every statement that writes state writes either
   (a) an array slot `a[i * stride + base]` where the index expression is
   affine in `i`, or (b) a per-work-item local (`let`/temp) that is never
   stored to shared state. Reads may be shared (all work-items read the same
   arrays). Uses the existing index-affine/alias machinery (address resolution
   in `src/address_resolver.rs`, `dependency_graph.rs`, `loop_carried.rs`).
3. **Flat types:** every touched state field and temporary resolves through the
   `TypeUniverse` (`universe_key()`, casting-graph `resolve_llvm_type`) to a
   flat scalar (`#Int`, `#Float`, `Bool`, `Char`). **Never** Briv-name matching
   (Rule 18). String/struct/ptr/collection fields reject the kernel.
4. **Purity:** no user FFI, no `term!`/`ExitProgram`/`Rollback`, no
   print/observable side effects inside the kernel statements.
5. **Partition:** split body into *kernel statements* (offloadable) and *host
   statements* (loop counters, observables, swan-song print, `count += 1`).
   Only the kernel statements enter the SPIR-V kernel; host statements stay on
   the CPU path and run after the dispatch returns.

The result is a `KernelShape` struct (per txn):

```rust
pub struct KernelShape {
    pub index_var: String,          // work-item index i
    pub count_expr: Expr,           // N (may be runtime)
    pub kernel_stmts: Vec<Statement>,
    pub host_stmts: Vec<Statement>,
    pub read_buffers: Vec<String>,  // array fields read by kernel
    pub write_buffers: Vec<String>, // array fields written by kernel
    pub scalar_ins: Vec<String>,    // read-only scalars (masses, dt)
    pub eligible: bool,
    pub reasons: Vec<String>,       // ineligibility evidence (remarks)
}
```

### 5.2 Cost model (absorbs `gpu_cost.rs`)

Move the arithmetic-intensity/crossover math out of the backend
(`collect_gpu_kernel`, `mod.rs:1150`) into the frontend. Device constants
(`PCIe latency`, `bandwidth`, GPU/CPU clock, core count) move from hardcoded
`const` in `gpu_cost.rs` into `config/targets.dbvl` / `ir-lowering.dbvl`
(measured per-device, not guessed).

Estimate:
- `total_ops` (weighted statement/expression walk),
- `total_bytes` (Σ read/write buffer sizes × element width, per firing),
- `arithmetic_intensity = ops / bytes`,
- `crossover N` where `GPU_time(N) < CPU_time(N)` accounting for launch +
  PCIe transfer overhead.

### 5.3 Decision

The per-body `AccelDecision` is the outcome of policy resolution (§4.4) +
eligibility proof (§5.1) + cost model (§5.2):

```rust
pub enum AccelDecision {
    Gpu,   // compile-time N ≥ crossover → dispatch, no probe
    Probe, // runtime N → emit both paths + auto-tuning probe
    Cpu,   // try-mode: unverifiable or ineligible → CPU + remark
}
```

- **Try mode:** N compile-time-constant (`const`/literal bound): `n >= crossover`
  ⇒ `Gpu`, else `Cpu`. N runtime (`get_env_int!`): `Probe`. Ineligible: `Cpu`
  with `reasons`. Any miss is a silent CPU fallback — never an error (D9).
- **Force mode** (keyword-marked bodies under `force`/`try_all_force`): the
  speedup gate is skipped — eligibility proven ⇒ `Gpu` unconditionally;
  eligibility unprovable ⇒ **compile error** (D9). At runtime, a missing device
  is an error, never a silent CPU fallback.

Remark emission reuses `directive::OptimizationRemark`
(`src/backend/llvm/directive.rs:179`) — e.g. *"accel 'step' kept on CPU —
intensity 0.04 ops/byte, crossover N=3.2e5"*, or the ineligibility reasons.
`!> accel_report: verbose;` (D16) emits a remark for **every** analyzed body,
not just fallbacks.

---

## 6. Backend Codegen (LLVM)

The backend is a deterministic switch over `AnalysisResults.accel` (D4, rule:
backend consumes, never decides).

### 6.1 Kernel emission — reuse the LLVM emitter (D5 sidepath)

Instead of the hand-rolled emitter in old `gpu.rs`, emit the kernel as a
normal LLVM IR function using the **existing** `emit_expr`/`emit_stmt`
machinery and the casting graph for types. The kernel differs from a normal
transaction function only in its ABI:

```llvm
define spir_kernel void @kernel_step(
    ptr readnone %in_buf,     ; packed read-only buffers
    ptr %out_buf,             ; packed read-write buffers
    i64 %N)
```

- The work-item index (the real counter `i`) maps to
  `@_Z13get_global_idj` (SPIR-V `GlobalInvocationId`).
- Array reads become `getelementptr + load` on the buffer; array writes become
  `getelementptr + store`; scalars come from a read-only uniform buffer or are
  splatted constants.
- Emit with `spirv64-unknown-unknown` triple, compile via
  `llc --mtriple=spirv64-unknown-unknown` (`gpu.rs:1023` TOCTOU-safe temp-file
  pattern is preserved), embed the blob.

This satisfies the "leverage the rich LLVM backend" sidepath: one expression
pipeline, no duplication, casting-graph-correct types.

### 6.2 Host dispatch stub

For every non-`Cpu` txn, emit a host-side stub that:

1. Marshals `KernelShape.read_buffers`/`write_buffers` into device buffers via
   the runtime's generic pack (SoA-coalesced, only touched fields — the cost
   model's byte count is derived from exactly these buffers).
2. Calls the device-agnostic `briv_accel_*` runtime dispatch (see §7).
3. Unpacks written buffers back into state.
4. Runs `host_stmts` (counters, observables, swan song).

The CPU body is **always** emitted (both as the device-absent fallback and as
the probe's CPU lane).

### 6.3 Decision wiring

- `Cpu`: normal emission (current path), plus remark.
- `Gpu`: emit kernel + dispatch stub; CPU body behind
  `briv_gpu_is_available()` gate.
- `Probe`: emit kernel + dispatch stub + CPU body + the probe prologue (§7.3).

---

## 7. Runtime — device-agnostic `briv_accel_rt` (glue rewrite)

The runtime is **device-agnostic glue**: the compiler never names a device. It
emits SPIR-V blobs + per-kernel layout descriptors + calls a stable accel ABI;
the runtime dispatches to a pluggable device-driver table. This is a
formalization of the existing `lib/runtime/briv_gpu_rt.c`, which already
dlopens BOTH Vulkan and OpenCL (Vulkan first, OpenCL fallback — both consume
SPIR-V via `clCreateProgramWithIL`). The old per-work-item buffer model
(`briv_gpu_malloc`/`memcpy`/`launch`) is replaced by the descriptor-driven
model.

### 7.1 Layering (D5, refined)

| Layer | Emits / holds | Device knowledge |
|---|---|---|
| Compiler (kernel.rs) | SPIR-V blob + layout descriptor + `briv_accel_*` ABI calls | none — one blob serves every SPIR-V consumer (Vulkan, OpenCL, LevelZero) |
| `briv_accel_rt.c` | dispatcher over the driver table; generic pack/unpack | none — layout-driven, device-independent |
| driver (`briv_dev_vulkan.c`, `briv_dev_opencl.c`) | raw device buffers, upload/launch/download | per-device transfer mechanism only |

Kernel *emission* is per device-**family**: CUDA needs a different emitter
(PTX), which is a compiler **backend** (`cuda` target), never a glue change.
Vulkan vs OpenCL need no separate emission — both consume the same SPIR-V.
This is why OpenCL 3.0 standardized SPIR-V as its IL.

Marshalling splits the same way: the buffer *layout* (which fields, element
types, offsets) is program-defined and device-independent; the *transfer
mechanism* (Vulkan `vkCmdCopyBuffer` vs OpenCL `clEnqueueWriteBuffer`) is
device-specific and lives inside each driver's `launch`. This two-tier split
(generic problem-descriptor layer + device execution layer) is the standard
pattern in CUDA/ROCm/oneDNN.

### 7.2 Driver ABI (function-pointer table)

```c
typedef struct {
    const char* name;               // "vulkan" | "opencl" | "levelzero" | ...
    uint32_t    capabilities;       // bit 0: BRIV_DEV_CAN_ZERO_COPY (SVM / unified memory)
    int  (*available)(void);        // dlopen + device present
    int  (*init)(void);
    int  (*create_kernel)(const uint8_t* spirv, size_t size, void** kernel_out);
    int  (*launch)(void* kernel, size_t global_n,
                   const BrivLayout* layout,
                   const void* state, size_t state_bytes, void* state_out);
    void (*destroy_kernel)(void* kernel);
    void (*shutdown)(void);
} BrivDeviceDriver;

extern BrivDeviceDriver briv_dev_vulkan;   // formalizes existing Vulkan path
extern BrivDeviceDriver briv_dev_opencl;   // formalizes existing OpenCL fallback
// future: briv_dev_levelzero, briv_dev_cuda (cuda needs a PTX emitter — compiler backend)
```

Drivers are **statically linked** into `briv_accel_rt.c` and selected at
runtime; a dlopen'd hot-plug driver set is a future option.

### 7.3 Stable compiler-facing ABI

The only surface emitted code touches — never names a device:

```c
int  briv_accel_init(const BrivKernelDesc* descs, uint32_t n);
int  briv_accel_launch(uint32_t idx, const void* state, uint64_t work_n, void* state_out);
int  briv_accel_available(void);
int  briv_accel_probe(...);   // §7.5
```

`BrivKernelDesc` = blob pointer/size + layout (array fields [element type,
dim], scalar fields [type], index var) — the compiler's `KernelShape` projected
into a C struct.

### 7.4 Device selection

`config/targets.dbvl` default (`vulkan`) + runtime `BRIV_ACCEL_DEVICE` env
override + fallback chain Vulkan → OpenCL → CPU. No compiler rebuild to
switch. `briv_accel_available()` reflects the winning driver.

### 7.5 Generic pack/unpack + probe

- **Pack:** `briv_accel_rt.c` packs host `%State` → flat device buffers from
  `BrivLayout` (device-independent). A driver with `BRIV_DEV_CAN_ZERO_COPY`
  may skip the copy inside its own `launch`.
- **Probe API:** `briv_accel_probe(...)` runs both lanes on a slice and
  compares wall time + output equality.

### 7.6 Probe protocol (D7, minimal overhead)

For `Probe` decisions only. Runs **once**, before the first firing of the
accelerated body (a process-global cache records the verdict):

1. **No device available** → commit `CPU` immediately. Zero probe cost.
2. **Warm-up:** one dummy launch (first device dispatch is disproportionately
   slow; excluding it keeps the measurement honest).
3. **Adaptive slice:** run `K` firings of the CPU lane and the GPU lane. Start
   `K = probe_min` (config), double until the two measurements are stable
   (ratio consistent across two doublings) or `K = probe_max`. Cap total probe
   cost at `probe_budget` ≈ 0.1% of the body's bound (config).
4. **Correctness gate:** the GPU lane's outputs must match the CPU lane's
   within the tolerance. On divergence, commit `CPU` permanently — the probe is
   also the safety net against GPU codegen bugs.
5. **Commit:** GPU iff `GPU_time × (1 + ε) < CPU_time` (`ε` = probe margin,
   config), else CPU. Cache in a process-global; every subsequent firing uses
   the committed path.

The probe is emitted in the IR as a small call sequence in the entry prologue
or before the first dispatch, guarded by a static flag so it runs once.

---

## 8. Removal of Legacy GPU Paths (D12)

- Delete `src/backend/llvm/gpu.rs` (its SPIR-V triple trick and blob embedding
  are reimplemented inside the new emitter; the TOCTOU-safe temp-file logic is
  preserved).
- Remove `#gpu`/`#?gpu`/`#!gpu` from directive resolution
  (`src/backend/llvm/directive.rs` `resolve_gpu` and `DirectiveEffect::GpuOffload`).
- Remove `--gpu-offload` flag (`src/main.rs:248`) and the `.abv` default
  (`config/targets.dbvl:20`).
- Remove `collect_gpu_kernel` and `spirv_kernels`/`spirv_blobs` from
  `src/backend/llvm/mod.rs`; the new emission is driven by
  `AnalysisResults.accel`.
- Absorb `src/analysis/gpu_cost.rs` into `src/analysis/accel.rs`.
- Update `src/backend/llvm/tests.rs` GPU tests (replace `#gpu` fixture with
  `accel` keyword / `!> accel:` fixtures).

---

## 9. Benchmark — `nbody_newton_accel`

**Not** a modification of `nbody_newton.bv`. A new benchmark run by itself.

### 9.1 Structure

```briv
!> accel: try_all;

const MAXB: Int = 4096;
let nbodies: Int = get_env_int!("BODYCOUNT");   // runtime, ≤ MAXB
let bound: Int = get_env_int!("BOUND");          // time steps

// SoA body state
let px: Float[MAXB]; let py: Float[MAXB]; let pz: Float[MAXB];
let vx: Float[MAXB]; let vy: Float[MAXB]; let vz: Float[MAXB];
let dvx: Float[MAXB]; let dvy: Float[MAXB]; let dvz: Float[MAXB];
const m: Float[MAXB];  // constant masses

// per-work-item force kernel — a native counted loop (Design A)
let i: Int = 0;
accel node force [i < nbodies][i == nbodies] {
    // O(N²) accumulation; reads all px/py/pz/m, writes dv[i] only
    i = i + 1;
    term;
};

// per-work-item integrate kernel
accel node integrate [i < nbodies][i == nbodies] {
    i = i + 1;
    term;
};

// host: sequential time loop
node step [count < bound][count == bound] {
    count = count + 1;
    when count == bound { endprogram println!(energy); };
    term;
};
```

Reactor order `force → integrate` per step (host-sequenced), `step` fires
`bound` times. On the GPU path each step's force/integrate counted loops
coalesce into one dispatch of `nbodies` work-items (counter fast-forwarded).
`BODYCOUNT` runtime exercises the `Probe` path; the auto-tuner compares CPU vs
GPU per firing and commits.

### 9.2 C reference

`benchmarks/nbody_newton_accel_c.c`: standard N-body O(N²) (benchmarksgame
style), same `BODYCOUNT`/`BOUND` env vars, same printed energy. **Never
hobbled** — plain `clang -O3 -march=native -ffast-math`.

### 9.3 Harness integration

- `TAG[nbody_newton_accel]=runtime`.
- `BODYCOUNT` env default (e.g. 2048) set in the harness alongside `BOUND`.
- Per-benchmark epsilon override (default `1e-5` stays; `_accel` may need a
  larger absolute epsilon for GPU-reduction-order differences) — extend the
  existing epsilon path (`build_and_bench.sh:459`) with an `EPS[name]` map.
- Skip timing with a clear note when Vulkan is unavailable (probe commits CPU;
  benchmark still correct, just measures CPU).
- Add `nbody_newton_accel` to `BENCHMARKS`.

The speedup story is the intended output: at large `BODYCOUNT`, the Briv GPU
path legitimately beats the C CPU reference — C is unhobbled, Briv simply
targets the better device when verified.

---

## 10. Test Plan

### 10.1 Unit tests (`cargo test --lib`)

- **Parser:** `accel node`/`accel txn` produce the `accel` modifier; `accel`
  on a non-node/txn is a syntax error; top-level `!> key: value;` produces
  `TopLevel::ModuleMetadata`; duplicate keys last-win; value grammar
  (identifier/int/bool/string/list) round-trips.
- **accel.rs analysis:**
  - bound extraction from `[i < N]` (const and runtime N);
  - write-disjointness proof accepts `a[i]`/`a[i*stride+off]` writes and
    rejects cross-work-item writes (`a[0]`, `a[j]` with free `j`);
  - flat-type acceptance (Int/Float/Bool/Char) and rejection (String/struct/
    List) via `TypeUniverse`, asserting no name matching;
  - purity rejection (FFI, `term!`, print);
  - kernel/host partition;
  - decision matrix: const-N≥crossover ⇒ Gpu; const-N<crossover ⇒ Cpu;
    runtime-N ⇒ Probe; ineligible ⇒ Cpu + reasons.
- **Cost model:** port existing `gpu_cost.rs` tests; crossover monotonicity;
  intensity thresholds.
- **Kernel emission:** emitted LLVM IR has `spirv64-unknown-unknown`,
  `get_global_id` index, buffer GEP/load/store; blob embeds.
- **Runtime:** probe API decisions (GPU/CPU), warm-up exclusion, adaptive
  slice caps, tolerance divergence ⇒ CPU.
- **Kani harnesses** for the probe decision logic and buffer marshalling
  (safety-critical: index arithmetic, no OOB, no use-after-free across
  pack/dispatch/unpack).

### 10.2 Integration (`backend::tests`, end-to-end)

- `!> accel: try_all;` + a pure parallel body compiles to a binary containing
  both CPU body and embedded SPIR-V blob.
- `accel` keyword fixture: decision `Cpu` emits no blob.
- Correctness at `BOUND=5` unchanged (probe or CPU fallback still yields the
  C-matching output).

### 10.3 Benchmark

`bash benchmarks/build_and_bench.sh` runs `nbody_newton_accel`; compare vs
`nbody_newton_accel_c`; verify GPU lane engaged (probe remark) and speedup
reported. Baseline A/B via `compare_baseline.sh`.

---

## 11. Development Process & Worktree

1. Commit the plan + SPEC changes on `main`.
2. `git worktree add ../briv-compiler-accel -b feat/accel-gpu`
3. All implementation commits happen in `../briv-compiler-accel` on
   `feat/accel-gpu`. Uncommitted interpreter changes in the `main` worktree
   are never touched (Rule 7).
4. Continuous commits after each logical step; `cargo test --lib` green before
   each commit; `cargo build` no new warnings; Praetor on changed directories
   (`praetor validate --warn --target <dir>`); Kani for new safety-critical
   code; architecture docs updated in the same commit as structural changes.
5. Baseline tables before/after performance work (Rule 11) using
   `../briv-compiler-baseline` + `compare_baseline.sh`.

---

## 12. Phases

Each phase ends in a commit with green tests.

| Phase | Deliverable | Gate |
|---|---|---|
| 1 | `accel` keyword: lexer token, `parse_top_level` arm, modifier annotation, parser tests | lexer/parser tests green |
| 2 | Top-level `!>` metadata: `TopLevel::ModuleMetadata`, module map, `analyze_program` wiring, `match TopLevel` audit (transition_graph, dependency_graph, canonical, display, LSP), `AnalysisResults.module_metadata` | unit tests green, no match-site regression |
| 3 | meta-vocab.dbv: `accel` + `accel_report` MetaFields (typed vocab; no BackendMapping rows — the backend consumes accel via analysis, not IR attributes); registry tests | registry tests green |
| 4 | `src/analysis/accel.rs`: eligibility proof, cost model (absorb gpu_cost), `AccelDecision`, `AnalysisResults.accel` | accel.rs unit tests green |
| 5 | Kernel emission via LLVM emitter reuse → SPIR-V blob; host dispatch stub; delete legacy GPU paths (`gpu.rs`, `#gpu`, `--gpu-offload`, `collect_gpu_kernel`, backend gpu_cost) | integration tests green, no legacy refs |
| 6 | `briv_accel_rt.c` rewrite: driver table (Vulkan + OpenCL, statically linked), `briv_accel_*` ABI, generic pack/unpack from layout descriptor, `BRIV_ACCEL_DEVICE` env + config default + fallback chain, probe API | runtime smoke test, Kani |
| 7 | Probe machinery: prologue wiring, adaptive slice, correctness gate, commit cache; config tunables (`probe_budget`, `probe_min/max`, `probe_margin`, device constants) | probe unit tests, Kani |
| 8 | `nbody_newton_accel.bv` + `_accel_c.c` + harness (TAG, EPS map, BODYCOUNT) | benchmark runs, GPU lane engaged, A/B vs baseline |
| 9 | Docs: `spec/SPEC.md` (§8.9, §9.7, §4.1), `learn-briv/`, rewrite `docs/architecture/gpu-offloading.md`, reconcile `docs/architecture/gpu-model.md`, `AGENTS.md` index, syntax highlighter; plan doc finalized | doc review |

---

## 13. Documentation Updates

- `spec/SPEC.md` (authoritative): §4.1 keyword note, §8.9 top-level metadata,
  new §9.7 `accel` semantics.
- `docs/architecture/gpu-offloading.md`: rewrite for the new frontend-driven,
  probe-verified design.
- `docs/architecture/gpu-model.md`: reconcile (borrowing-no-barriers thesis
  unchanged; update the work-item contract `[i < N]` wording and the "no
  syntactic difference" claim now that `accel` marks the map).
- `docs/architecture/hash-words.md`: remove `#gpu` references.
- `docs/architecture/backend-type-dispatch.md` / `backend-architecture.md`:
  note the kernel-emission reuse path.
- `learn-briv/`: `accel` tutorial page.
- `AGENTS.md`: command/index updates, benchmark table entry.
- Historical docs (`docs/plans/2026-06-18-graphic-briv.md`, GPU-era plans) are
  records — reference, never retroactively edit.

---

## 14. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| GPU path wrong results | Probe correctness gate (output equality) disables GPU; tolerance check at benchmark level |
| Probe overhead | Budget capped ~0.1% of bound; one-shot; warm-up excluded; `Gpu` static path skips probe entirely |
| Runtime-N benchmark can't be statically decided | `Probe` decision is first-class, not an error |
| Buffer marshalling bugs | Kani harnesses; single pack/dispatch/unpack path shared by probe + steady state |
| `match TopLevel` sites break on new variant | Phase 2 audit; explicit arms or verified `_` fallthrough |
| Parallel agent in `feat/out-observability` conflicts | Separate worktree; isolated `target/`; `main` untouched by implementation commits |
| Benchmarks regress | Baseline A/B (Rule 11) before/after; additive-only changes |
| `llc` SPIR-V target unavailable | Keep TOCTOU-safe shell-out with graceful warning; CPU fallback remains correct |
| fp nondeterminism on GPU reduction | v1 keeps per-work-item writes (no cross-item reductions) → deterministic within a lane; tolerance absorbs CPU-vs-GPU differences |

---

## 15. Open Items

- **Sync-group with mismatched member firing schedules blocks** (found in
  Design A validation, BUGS.md): a `sync<group>` whose members fire different
  numbers of times emits an empty `reactor_tick`. Pre-existing, independent of
  accel. Phase 8 must structure the nbody group so members fire in lockstep
  (or use `async` + a sequenced reset).
- **Precompute fold evaluates Float-array indexed observables as 0**
  (BUGS.md): pre-existing; affects plain and accel identically. Fold model
  does not track Float array writes into the observable's value.
- Whether `N` for nbody derives from the `[i < nbodies]` precondition or from
  `GetGlobalSize#` — resolved in Phase 4 against `parse_contract` capabilities.
- Per-body in-body opt-out `!> accel: off;` (D13) — deferred.
- Device-constant calibration procedure for `config` (measured once per
  machine, documented in the config audit trail).
- Library (`--library`) builds do not yet package `briv_accel_rt.c` (Phase 6b
  wires the executable link only).
