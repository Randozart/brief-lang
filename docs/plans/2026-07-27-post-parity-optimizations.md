# Post-Parity Optimization: Phase Plan
## 2026-07-27

**Baseline:** `9ff835ac` (all 19 benchmarks at parity)
**Objective:** 7 phases, each independently committed and benchmarked, zero regressions.

---

## Phase A: DataLayout-Driven Integer Width + Remove `is_wasm()`

### What

Parse the `p:<ABI_width>:<Pref_width>` segment from the LLVM `target datalayout` string
to auto-set `ctx.int_bits`. Replace the hardcoded `is_wasm()` + `pointer_bytes()` +
`pointer_llvm_type()` trio with a data-layout-derived pointer width.

The data layout string for x86_64:
```
e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128
        ^^ ABI ptr size for addrspace 270 (32-bit stack)
                          ^^ ABI ptr size for addrspace 272 (64-bit default)
```

The first `p:` segment without an addrspace qualifier is the **default** address space.
For both x86_64 and WASM, this is the relevant segment:

| Target | Data layout segment | Ptr width | int_bits |
|--------|-------------------|-----------|----------|
| x86_64 | `e-m:e-p270:32:32-...p272:64:64-...` | 64 (addrspace 272) | 64 |
| WASM32 | `e-m:e-p:32:32-...` | 32 | 32 |
| WASM64 | `e-m:e-p:64:64-...` (hypothetical) | 64 | 64 |

Parsing rule: find `-p:<abi>:<pref>-` or `-p<num>:<abi>:<pref>-` and extract `<abi>`.
If no `p:` segment exists, default to 64.

On WASM, the data layout is `p:32:32` (no addrspace qualifier = default address space).
On x86_64, the default address space is `p272:64:64` (addrspace 272). The parser
should look for an unqualified `p:<abi>:<pref>` first, then fall back to finding
any `p<num>:<abi>:<pref>` and taking the largest ABI width.

### Precise file changes

**`context.rs` — Add parser + remove `is_wasm()`:**

Replace lines 173-200:
```rust
// REMOVE these 3 functions (lines 174-200):
pub fn is_wasm(&self) -> bool { ... }
pub fn pointer_bytes(&self) -> u64 { ... }
pub fn pointer_llvm_type(&self) -> &'static str { ... }
```

Replace with:
```rust
/// Parse the default pointer width (in bits) from a target data layout string.
/// Looks for `-p:<abi>:<pref>-` (unqualified) or `-p<num>:<abi>:<pref>-`
/// and returns the `<abi>` field. Falls back to 64.
pub fn parse_pointer_width(dl: &str) -> u64 { ... }

/// Derive pointer width in bytes from int_bits.
pub fn pointer_bytes(&self) -> u64 { self.int_bits / 8 }
```

Remove `is_wasm()` entirely — all callers replaced by `int_bits` comparison.

**`context.rs` — Auto-set int_bits in `new()`:**

At line 255-257:
```rust
// Before:
data_layout: Some("e-m:e-p270:32:32-..."),
int_bits: 64,

// After:
let default_dl = "e-m:e-p270:32:32-p271:32:32-p272:64:64-...";
let dl_bits = Self::parse_pointer_width(default_dl);
data_layout: Some(default_dl.to_string()),
int_bits: dl_bits,
```

**`context.rs` — Also in `with_data_layout()` override:**

When data_layout is changed via `with_data_layout()`, re-parse int_bits:
```rust
pub fn with_data_layout(mut self, dl: &str) -> Self {
    self.ctx.data_layout = Some(dl.to_string());
    self.ctx.int_bits = Self::parse_pointer_width(dl);
    self
}
```

**`mod.rs:1162-1164` — Fix `emit_ptrtoint` bug:**

Current buggy code emits `ptrtoint i64 %src to ptr` (should be `ptrtoint ptr %src to i64`):
```rust
// Current (WRONG):
let ptr_ty = self.ctx.pointer_llvm_type();
writeln!(out, "{}{} = ptrtoint {} {} to ptr", indent, dest, ptr_ty, src).ok();

// Fixed — use int_bits for the integer side:
let int_ty = format!("i{}", self.ctx.int_bits);
writeln!(out, "{}{} = ptrtoint ptr {} to {}", indent, dest, src, int_ty).ok();
```

Note: this function has ZERO callers (dead code), but it's still a bug if anyone
uses it. The builder.rs version at line 259-261 is correct.

**`mod.rs:1172-1174` — Fix `emit_inttoptr` (same pattern):**

```rust
// Fixed — use int_bits for the integer side:
let int_ty = format!("i{}", self.ctx.int_bits);
writeln!(out, "{}{} = inttoptr {} {} to ptr", indent, dest, int_ty, src).ok();
```

Both functions (emit_ptrtoint and emit_inttoptr) are public methods on LlvmBackend.
Remove the explicit `pointer_llvm_type()` calls and use `format!("i{}", self.ctx.int_bits)`.

**`compile.rs` — Remove dead duplicate Webstack arms:**

Lines 956-988 (second Webstack arm) and 994-1014 (third Webstack arm) are unreachable
— Rust warns `unreachable pattern`. Remove both arms entirely. Also remove the
hardcoded `with_int_bits(32)` from the first Webstack arm (line 925) — this is
now auto-derived from the data layout.

**`compile.rs:887-889` — For the LLVM arm, replace `opts.int_bits` with auto-derive:**

```rust
// Before:
let mut b = LlvmBackend::new()
    .with_int_bits(opts.int_bits)
    ...

// After:
let mut b = LlvmBackend::new()
    // int_bits auto-derived from data_layout in LlvmBackend::new()
    ...
```

The `opts.int_bits` CLI flag is still honored as an override — `with_int_bits()`
simply stores the value; the DataLayout parsing only applies if `with_int_bits()`
wasn't called. Add a flag `int_bits_was_set: bool` to CompilerContext to
distinguish CLI-overridden from auto-derived.

### Verification

1. `cargo test --lib` — all pass
2. `bash benchmarks/build_and_bench.sh --runtime` — no regressions
3. Check that setting `data_layout` to `"e-m:e-p:32:32-..."` produces `int_bits=32`
4. Check that default x86_64 data_layout still produces `int_bits=64`
5. grep for `is_wasm` — zero remaining occurrences

### Impact

- WASM32 backend no longer needs `--int-bits 32` or `with_int_bits(32)` — auto-derived
- Eliminates 3 functions and 1 `is_wasm()` code smell
- No performance change on x86_64 benchmarks

### Commit

```
git commit -m "2026-07-27: Phase A — DataLayout-driven int_bits, remove is_wasm()"
```

### Verification

1. `cargo test --lib` — all pass
2. `bash benchmarks/build_and_bench.sh --runtime` — no regressions
3. Check that `data_layout` with `-p:32:32` produces `int_bits=32`

---

## Phase B: `noundef` + `dereferenceable` on `ptr %state` Parameters

### What

Add `noundef` and `dereferenceable(%StateSize)` parameter attributes to every
function definition that takes `ptr %state`. Both are valid LLVM IR parameter
attributes on `ptr` types.

The `%state` pointer is always valid for the function's lifetime (it's an `alloca`
in `main()` that outlives all txn calls). `noundef` tells LLVM the pointer is
never undef. `dereferenceable(N)` tells LLVM the first N bytes are readable.

### All 11 function definition sites

Each site has the format string or writeln that emits the function header.
Change `ptr noalias nocapture align 8 %state` to
`ptr noundef dereferenceable(N) noalias nocapture align 8 %state`.

| # | Function | File:Line | Current format | Align 8? |
|---|----------|-----------|----------------|----------|
| 1 | `@init_state` | emit_toplevel.rs:864 | `"define void @init_state(ptr noalias nocapture align 8 %state)..."` | ✅ |
| 2 | User `@defn` param | emit_toplevel.rs:1144 | `"ptr noalias nocapture align 8 %state"` | ✅ |
| 3 | User `@defn` export | emit_toplevel.rs:1277 | `"ptr %state"` (bare) | ❌ |
| 4 | `@txn_*` (reactive, unfoldable) | emit_toplevel.rs:1554 | `"define void @txn_{}(ptr noalias nocapture align 8 %state)..."` | ✅ |
| 5 | `@txn_*` (reactive, outlinable) | emit_toplevel.rs:1718 | `"define void @txn_{}(ptr noalias nocapture align 8 %state)..."` | ✅ |
| 6 | `@callable_txn` | emit_toplevel.rs:2002 | `"ptr noalias nocapture align 8 %state"` | ✅ |
| 7 | `@pre_*` | emit_toplevel.rs:2252 | `"define internal i8 @pre_{}(ptr noalias nocapture align 8 %state)..."` | ✅ |
| 8 | `@async_body_*` | emit_toplevel.rs:2302 | `"define void @{}(ptr noalias nocapture align 8 %state)..."` | ✅ |
| 9 | `@emit_fused` (3 variants) | emit_toplevel.rs:2369,2383,2421 | `"define void @{}(ptr noalias nocapture align 8 %state)..."` | ✅ |
| 10 | `@cell_persistent_ticks` | emit_toplevel.rs:2549 | `"define void @cell_persistent_ticks(ptr noalias nocapture align 8 %state)..."` | ✅ |
| 11 | `@__briev_init_state` | emit_toplevel.rs:2491 | `"define dso_local void @__briev_init_state(ptr %state)..."` | ❌ |

Also fix the **3 functions missing `align 8`** (sites 3, 11 above, plus reactor_tick):

| Function | File:Line | Add |
|----------|-----------|-----|
| `@reactor_tick` (sequential) | dispatch.rs:76 | `align 8` |
| `@reactor_tick` (parallel) | dispatch.rs:365 | `align 8` |
| `@reactor_tick` (fallback) | mod.rs:3118 | `align 8` |
| User `@defn` export | emit_toplevel.rs:1277 | `noalias nocapture align 8` (bare ptr currently) |
| `@__briev_init_state` | emit_toplevel.rs:2491 | `noalias nocapture align 8` (bare ptr currently) |

The `N` value for `dereferenceable(N)` comes from `compute_state_size_bytes()`,
already called at `loop_engine/mod.rs:244`. Store the result on `CompilerContext`
as `state_size_bytes: u64`, computed once during `generate()` before any
function definitions are emitted.

### Additional fixes in this phase

- Remove `pointer_llvm_type()` usage from `emit_inttoptr` (mod.rs:1172) and
  `emit_ptrtoint` (mod.rs:1162) — replace with `format!("i{}", ctx.int_bits)`
  to unify on the DataLayout-driven width.

### Impact

~2-3% on benchmarks with large %State (nbody, ring_buffer). LLVM can eliminate
redundant null checks and optimize phi nodes for `noundef` values.

### Commit

```
git commit -m "2026-07-27: Phase B — noundef + dereferenceable on ptr %state params"
```

### Verification

1. `cargo test --lib` — all pass
2. `bash benchmarks/build_and_bench.sh --runtime` — record, no regressions
3. Check `.ll` output for `noundef dereferenceable(N)` on function headers

---

## Phase C: Persist AnalysisResults for `!prof`

### What

Currently `AnalysisResults` is created locally in `generate()` and dropped.
Only `dep_graph` is persisted via clone. Store `transition_graph` and key
`region_analyzer` data (iter_bounds, bounded_pre, increments) on `CompilerContext`
so `emit_toplevel.rs` can access them for precise `!prof` weights.

The current `!prof` Phase 2 implementation only handles the simple modulo case
(`count % N == C` with `[count == total]` postcondition). With full analysis data,
it can handle any guard pattern that constrains the induction variable.

### The gap

`AnalysisResults` is created locally in `generate()` at `mod.rs:1738`:
```rust
let mut analysis = crate::backend::analyze_program(items, false);
self.ctx.dep_graph = analysis.dependency_graph.clone();
```

Only `dep_graph` is persisted. The `transition_graph` (containing `bounded_pre`
and `increments` per txn) and `region_analyzer` (containing `iter_bounds`) are
dropped when `generate()` returns.

### Changes

| File | Change | Lines |
|------|--------|-------|
| `src/backend/llvm/context.rs` | Add `transition_graph: ReactorTransitionGraph` field to `CompilerContext` | 2 |
| `src/backend/llvm/context.rs` | Add `iter_bounds: HashMap<String, u64>` field | 2 |
| `src/backend/llvm/mod.rs` | Store analysis data on ctx at mod.rs:1739-1742 | 5 |
| `src/backend/llvm/emit_toplevel.rs` | Replace current extracted with full analysis: match guard condition against `bounded_pre.var` to detect induction variable constraints (not just modulo), compute weights from `iter_bounds` and `inc.delta` | 40 |

The `TransitionGraph` struct is defined at `src/analysis/transition_graph.rs`.
It contains `nodes: Vec<ReactorNode>`, each with `bounded_pre: Option<BoundedPre>`
(var, bound_var, direction, bound_literal) and `increments: Option<IncrementInfo>`
(var, delta). A `node_for(name)` method or similar lookup should be added.

The `RegionAnalyzer::iteration_bound_of(txn_name)` method at `region.rs:822-825`
returns `Option<u64>`. The full `region_analyzer` is too large to store, but the
`iter_bounds` HashMap can be extracted and stored separately.

### Extended `!prof` logic

```rust
// For each guard condition referencing bounded_pre.var:
//   taken_weight = iter_bounds / (inc.delta.abs() * <derived from cond shape>)
//   not_taken_weight = iter_bounds - taken_weight
//   scale to max 1000, emit !prof

match cond {
    // count % N == C → taken = ceil(iter_bound / (delta * N))
    BinaryOp(Eq, BinaryOp(Mod, Ident(v), Decimal(N)), Decimal(C))
        if v == bounded_pre.var => { ... }
    // count >= N → taken = (iter_bound - ceil(N/delta)) / delta
    BinaryOp(Ge, Ident(v), Decimal(N))
        if v == bounded_pre.var && N <= iter_bound => { ... }
    // count == N → taken = 1 if N within bounds
    BinaryOp(Eq, Ident(v), Decimal(N))
        if v == bounded_pre.var && N <= iter_bound => { ... }
    // No match → no metadata (handled by current Phase 2 code)
    _ => {}
}
```

### Impact

~2-3% on benchmarks where guard patterns aren't simple modulo (e.g., comparison
guards like `count >= threshold` that could benefit from branch weight tuning).

### Commit

```
git commit -m "2026-07-27: Phase C — persist AnalysisResults for precise !prof weights"
```

### Verification

1. `cargo test --lib` — all pass
2. `bash benchmarks/build_and_bench.sh --runtime` — no regressions
3. Check `.ll` output has `!prof` on guard branches for non-modulo patterns

---

## Phase D: `Bits` → `Bit` Rename

### What

Mechanical rename of the `Bits` type name to `Bit` across all `.rs` and `.bv`
files. Does NOT rename `bits <~ N` property declarations (those are a separate
concept — property name stays lowercase `bits`). Does NOT rename `#Bits` hashword
(the hashword stays `#Bits` — it's the protocol category, not the type name).

### Changes

| File type | Changes | Count |
|-----------|---------|-------|
| Rust `.rs` `"Bits"` string literals | `"Bits"` → `"Bit"` | 17 |
| Rust `#Bits` hashwords | No change (stays `#Bits`) | 20 |
| `.bv` `Bits` type references | `Bits` → `Bit` | 17 |
| `.bv` `bits <~ N` properties | No change | 13 |
| Glue `.bv` `#Bits` hashwords | No change (stays `#Bits`) | 5 |

### Impact

Zero performance impact. Establishes naming convention: `Bit` is the atomic type,
`#Bits` is the protocol category, `bits <~ N` is the width property.

### Commit

```
git commit -m "2026-07-27: Phase D — rename Bits type to Bit"
```

### Verification

1. `cargo test --lib` — all pass
2. `cargo build` — no warnings about unknown types
3. Quick benchmark sanity check (not full suite — no perf change expected)

---

## Phase E: Intrinsic-Based Prints

**DO NOT IMPLEMENT UNTIL USER APPROVES.**

### What

Replace `__print_int`/`__print_float`/`__print_char` FFI calls with
`PrintInt#`/`PrintFloat#`/`PrintChar#` intrinsics. Makes print guards naturally
FFI-free — `has_ffi_call` returns false, the txn gets `#11` directly from the
existing outlining analysis, no cold-path outlining needed for any print guard.

### Why this works

The print plugin currently emits `Expr::Call("__print_int", [x], None)` which is
an FFI call (ends in `int`, not `#`). Changing to `Expr::Call("PrintInt#", [x], None)`
produces a `#`-suffixed call. The `references_triggers_or_ffi_with_decls` function
at `transition_graph.rs:771` treats `Expr::Call(_, _, _) => true` for ANY call,
so `statement_contains_ffi` still returns true — but `has_ffi_call` at
`emit_toplevel.rs:1448` checks `!name.ends_with('#')` which WOULD return false
for `PrintInt#`, correctly treating it as a non-FFI intrinsic.

The backend already handles `PrintInt#` — it falls through to `emit_external_call`
at `intrinsics.rs:124`. The `bindings.dbvl` at line 17 already has a template:
```
PrintInt# -> call i32 (ptr, ...) @printf(ptr @.fmt_int, i64 %0)
```

But this template is unused because `emit_intrinsic_call` doesn't match
`"PrintInt#"` — it falls to the generic template path which returns `None`.
Adding a match arm would make it work.

### Precise changes

| File | Line | Change |
|------|------|--------|
| `src/intrinsic_signatures.rs` | ~288 | Add `"PrintInt#"`, `"PrintFloat#"`, `"PrintChar#"`, `"PrintStr#"` with `observable: true` |
| `src/backend/llvm/intrinsics.rs` | ~52 | Add `"PrintInt#"` arm in `emit_intrinsic_call` match — emit `call i64 @__print_int(i64 %0)` directly, or use the `bindings.dbvl` template |
| `src/interpreter/intrinsics.rs` | ~100 | `execute_intrinsic`: add `"PrintInt#"` via `println!`, `"PrintFloat#"` etc. |
| `src/plugin/print_plugin.rs` | ~239 | `resolve_print`: replace `"__print_int"` with `"PrintInt#"`, same for float/str/char |
| `src/backend/llvm/intrinsics.rs` | ~124 | No change needed — the `bindings.dbvl` template fallback should work after registration, but adding explicit match arms is cleaner |

### Risk

`statement_contains_ffi` treats ALL calls as FFI (including `#`-suffixed). The
current code at `emit_toplevel.rs` distinguishes them via `is_ffi_call` which
checks `!name.ends_with('#')`. So `PrintInt#` passes through as non-FFI in the
outlining analysis, but `statement_contains_ffi` (used by dead field elimination
and effect analysis) still treats it as observable — which is CORRECT because
`PrintInt#` IS an observable side effect.

### Impact

~10-12% on ring_buffer, ~5% on nbody_newton.

### Commit

```
git commit -m "2026-07-27: Phase E — intrinsic-based prints (PrintInt# replacing __print_int)"
```

---

## Phase F: Ptr Storage in %State as LLVM `ptr`

**DO NOT IMPLEMENT UNTIL PHASES A-E COMPLETE AND STABLE.**

This is the highest-risk, highest-complexity change. Eliminates **102 inttoptr +
110 ptrtoint** instructions from the emitted IR by storing `Ptr<T>` fields as
LLVM `ptr` instead of opaque `i64` in %State.

### Current architecture

The comment at `mod.rs:921-928` explicitly chose `"i64"` for ALL fields:
```rust
// 2026-07-17: ALL state fields are stored as i64 in %State, regardless
// of their Briev type (Float, Float64, Ptr, etc.). The adapt_to_i64 /
// ensure_typed_value functions handle the conversion between i64 and
// the field's natural type at load/store time.
```

Float types already break this rule — they store as `"float"` or `"double"`.
The phi backedge handles them with a `"float"/"double"` special case. Ptr types
would be the third case.

### Changes (in order)

**A) `push_field_type` (mod.rs:993-996):** Add a check for `Type::Ptr(_)` before
the universe fallback — return `"ptr"`.

**B) Phi backedge identity (counter.rs:477-480):** The `add ptr 0, %val` path
would be reached for `"ptr"` type. Pointer arithmetic in LLVM requires
`getelementptr`, not `add`. Add a `"ptr"` case:
```rust
if field_ty == "ptr" {
    // ptr identity: getelementptr i8, ptr %val, i64 0
    writeln!(out, "  {} = getelementptr i8, ptr {}, i64 0", be_f, val).ok();
} else if field_ty == "float" || field_ty == "double" {
    ...
}
```

**C) `adapt_to_i64` (helpers.rs:2722-2725):** Currently returns name unchanged
for Ptr with comment "already i64". If Ptr is now `ptr`, need to emit `ptrtoint`:
```rust
if ptr_related {
    let boxed = self.fun.gen_reg();
    writeln!(out, "{} = ptrtoint ptr {} to i64", boxed, val.name).ok();
    return boxed;
}
```

**D) `ensure_typed_value` (helpers.rs:2932):** Add `ptr ↔ i64` conversion.

**E) `emit_expr.rs` state field loads (lines 178-214):** The identifier resolution
code assumes loaded values are `i64`. The float-boxing code at lines 194-209 does
`load i64` then `trunc → bitcast to float`. For Ptr fields, the load would return
`ptr` — the `else` block at lines 210-214 handles non-float types by returning
the register with `briev_ty`. This is actually correct for Ptr — the register IS
`ptr`, and downstream code (inttoptr → GEP) would treat it as such. The `inttoptr`
in `emit_expr.rs:485` becomes a no-op.

**F) `loop_engine/mod.rs` exit loader (lines 89-118):** Already has a `ptr` case
at line 109-112 that does `load ptr, ...` → `ptrtoint ptr %l to i64`. This path
activates automatically when `field_types[idx] == "ptr"`. ✅ Already handled.

**G) Storage size:** Ptr is pointer-width (8 bytes on x86_64, 4 bytes on wasm32).
After Phase A, `int_bits/8` gives the byte size. The `push_field_type` function
uses `bytes` argument which already accounts for this.

### Risk assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Ptr backedge identity wrong | Low | Use `getelementptr i8, ptr %val, i64 0` |
| `adapt_to_i64` callers expect i64 | Medium | Verify each of the 30+ call sites |
| Dead field elimination treats ptr differently | Low | Already handles ptr type |
| Load of ptr field breaks i64 assumption | Medium | Lines 178-214 of emit_expr.rs need verification |

### Impact

~5-8% on ring_buffer, ~2-3% on other benchmarks with pointer fields.

---

## Phase G: Selective Width for SIMD/Cache Density

Research phase — no implementation plan until feasibility is proven.

### The opportunity

`binop_int_type()` at `emit_expr.rs:1795-1797` always returns `"i64"`:
```rust
fn binop_int_type(&self) -> String {
    "i64".to_string()
}
```

The docstring says "based on the function's narrowed max width" but the
implementation always hardcodes i64. The narrowing pass only affects `ret`
instructions — intermediate SSA values stay at `i64`.

The type universe already has `min_bits`/`max_bits` on every primordial:
- `Int`: min=0, max=64 (flexible)
- `Int32`: min=32, max=32 (fixed)
- `Int8`: min=8, max=8 (fixed)

For SIMD: a `<8 x i32>` vector processes 8 elements per AVX2 cycle vs
`<4 x i64>` processing 4 elements. Keeping narrowed types packed doubles
SIMD throughput.

### What the narrowing pass would need

1. **Track whether a value is part of an array/contiguous memory** — if yes,
   keep the narrower type for cache density and SIMD packing.
2. **Use `max_bits` from the type universe** as the SSA value width, not
   `int_bits`. Currently `llvm_type()` at line 305 uses `int_bits` as a floor,
   which widens everything to 64.
3. **Emit `sext`/`trunc` only at observation points** (FFI boundary, ret, store
   to %State) rather than keeping all intermediates at i64.

### Research questions

1. Does LLVM's `opt -O3` already narrow SSA values via SCCP or similar passes?
   (Check optimized IR for `i8`/`i16`/`i32` intermediate values.)
2. Which benchmarks would benefit most? (Those with narrow integer arrays.)
3. Does the existing `min_bits`/`max_bits` on `Int` `type_floor` work correctly?

Not actionable until research is complete.

---

## Rollback

Each phase is a single commit. Rollback is:

```bash
git revert <phase-commit>
bash benchmarks/build_and_bench.sh --runtime
```
