# Master Implementation Overview — July 18, 2026

**Author:** Compiler agent
**Scope:** All 5 active plans consolidated into sequential implementation steps
**Test baseline:** `cargo test --lib` = 931 tests passing
**Status key:** ✅ Done | ⚡ In progress | ❌ Remaining

---

## How to use this document

Each section below is a self-contained implementation block. Implement them **in order** — later blocks depend on earlier ones. Each block lists:
1. **What to change** — exact files and line numbers
2. **How to test** — what `cargo test --lib` should show
3. **Rollback** — how to undo if something goes wrong

---

## Block 14: Natural Convergence Exit

**Depends on:** None (independent change).

**Goal:** When all reactive transactions have converged (their postconditions are met and
preconditions can never become true again), the program should exit naturally with
`ret i32 0` — not spin forever in the main loop.

### Problem

The runtime's `ss_main_loop` (emitted by `emit_main` in `loop_engine/mod.rs:226`)
never exits:

```llvm
.ss_main_loop:
  ; check preconditions, run active txns, then:
  br label %.ss_main_loop  ; forever
```

Even when all txns have converged (`ops == TOTAL`, `done == 1`, etc.), the loop
continues checking preconditions. No txn's precondition is true anymore, so the loop
is spinning doing nothing. The program must be killed externally.

### Solution

After the reactor tick, check whether ANY transaction's precondition is still
evaluable-as-true. If zero txns can ever activate again, exit.

**Key insight:** A transaction is *restartable* if its precondition could become
true again after the postcondition is met. This requires at least one of:
- Wake triggers (`when` on state fields that can change)
- External input (`io_pending`, foreign-triggered state changes)
- A `sync` block that another transaction signals
- An `async` timer/cycles-based wake condition

If none of these exist, the txn is **one-shot** — once the postcondition is met
and the precondition evaluates to false, it will never run again.

### Detection: one-shot vs restartable

The compiler knows at codegen time which txns are one-shot vs restartable by
inspecting the txn's metadata:

| Characteristic | One-shot | Restartable |
|---|---|---|
| Precondition | State field comparison (`ops < TOTAL`) | External input (`io_pending`) |
| Triggers | None declared | `when state.field` declared |
| Async | No | `async` keyword present |
| Cycles wake | No | `cycles <~ N` or `cyc` triggers |

Default assumption: **one-shot**. A txn is only restartable if it explicitly
declares `trg`, `async`, `cycles`, or external I/O triggers.

### Implementation

**E1: Track restartability per txn** (`src/backend/llvm/mod.rs`)

Add to `LlvmBackend`:
```rust
pub txn_is_restartable: Vec<bool>,  // one entry per reactive txn
```

Populated during codegen init: check each `rct txn` for triggers/async/cycles.
Default `false` (one-shot).

**E2: Emit active-count check** (`src/backend/llvm/loop_engine/mod.rs:226`)

In `emit_main`, after the reactor tick, emit a check that evaluates each
one-shot txn's precondition. If any is true, continue the loop. If all are false,
exit:

```rust
// After reactor_tick call:
let any_active = self.fun.gen_reg();
writeln!(out, "  {} = call i1 @llvm.wake.any()", any_active).ok();
// Only check one-shot txns if no wake triggers fired.
// If all one-shot preconditions are false, exit.
let all_done = self.fun.gen_reg();
writeln!(out, "  {} = icmp eq i32 {{precondition_count}}, 0", all_done).ok();
writeln!(out, "  br i1 %all_done, label %.end, label %.loop").ok();
```

Wait — this is too simplistic. The existing code already has `has_wake_triggers` 
and `llvm.wake.any()` logic. The fix is simpler:

**Simpler approach:** When `has_wake_triggers` is false AND no one-shot txn has
an active precondition, exit:

```llvm
  call void @reactor_tick(ptr %state)
  ; If no wake triggers and no one-shot txns are active, exit
  br label %.loop  ; existing — change to conditional
```

**E3: Conditionally branch instead of unconditional** (`loop_engine/mod.rs:253`)

Change:
```rust
writeln!(out, "  br label %.loop").ok();
```
To:
```rust
// 2026-07-18: If no txns are restartable (all converged), exit.
if !self.has_restartable_txns() {
    writeln!(out, "  %any_remaining = call i1 @check_preconditions(ptr %state)").ok();
    writeln!(out, "  br i1 %any_remaining, label %.loop, label %.end").ok();
} else {
    writeln!(out, "  br label %.loop").ok();
}
```

**E4: Add `@check_preconditions` helper** (`src/backend/llvm/mod.rs`)

New method that emits LLVM IR to evaluate all one-shot txn preconditions and OR
them together. If the result is 0, no txn will ever run again:

```rust
fn emit_check_preconditions(&mut self, out: &mut String) -> String {
    let mut regs = Vec::new();
    for (i, is_restartable) in self.txn_is_restartable.iter().enumerate() {
        if *is_restartable { continue; }
        let cond = self.emit_exit_expr(out, &self.ctx.exit_conditions[i], "  ");
        regs.push(cond);
    }
    if regs.is_empty() { return "0".to_string(); }
    // OR all precondition evaluations together
    let result = self.fun.gen_reg();
    let mut acc = regs[0].clone();
    for r in &regs[1..] {
        let or = self.fun.gen_reg();
        writeln!(out, "  {} = or i64 {}, {}", or, acc, r).ok();
        acc = or;
    }
    writeln!(out, "  {} = icmp ne i64 {}, 0", result, acc).ok();
    result
}
```

**E5: Update `emit_exit_check`** (`loop_engine/mod.rs:210`)

The existing `emit_exit_check` handles the `exit_condition` (single condition).
For convergence detection, we need aggregate checking of ALL one-shot txns. The
`emit_exit_check` already generates the `br i1 %cond, label %.end, label %.continue`
pattern. We can reuse this by adding a compound exit condition that ORs all
one-shot txn preconditions and negates the result.

### Files to modify

| File | Change |
|------|--------|
| `src/backend/llvm/mod.rs` | Add `txn_is_restartable: Vec<bool>` field + `has_restartable_txns()` method + `emit_check_preconditions()` method |
| `src/backend/llvm/loop_engine/mod.rs` | Modify `emit_main` to use conditional branch instead of unconditional `br label %.loop` when `!has_restartable_txns()` |

### Test

1. Write `.bv` file with a one-shot `rct txn [x < N][x == N] { x = x + 1; term; }`
2. Compile and run — program should exit (not hang)
3. Write `.bv` file with a restartable txn (has triggers) — program should loop

### Rollback

Revert `loop_engine/mod.rs:253` back to unconditional `writeln!(out, "  br label %.loop").ok();`.
Remove `txn_is_restartable` field from LlvmBackend.

---

## Block 1: SSO String — Phase B (Codegen)

**Depends on:** Phase A (struct fields, is_string_like) + Phase 0 (op dispatch) — both committed.

**Goal:** When `feature_sso_strings` is ON, String becomes a `{ i64, i64 }` struct with inline storage for ≤6 bytes, heap for longer.

### B1: Add feature flag

**Files:** `src/compile.rs`, `src/backend/llvm/mod.rs`

Add `pub feature_sso_strings: bool` to `BuildOptions` (default `false`). Add `pub(crate) feature_sso_strings: bool` to `LlvmBackend`. Wire a builder method `with_sso_strings(mut self, enabled: bool) -> Self` on `LlvmBackend`.

In `compile_source()` (compile.rs ~line 148), pass `opts.feature_sso_strings` through to the backend.

**Test:** `cargo test --lib` = 919 pass (flag off by default, no behavioral change).

### B2: SSO String literal emission

**File:** `src/backend/llvm/emit_expr.rs` line ~148 (string literal emission)

```rust
/// 2026-07-18: Emit a string literal as an SSO handle or heap pointer.
fn emit_string_literal(&mut self, out: &mut String, s: &str, indent: &str) -> TypedRegister {
    if !self.feature_sso_strings {
        return self.emit_heap_string_literal(out, s, indent);
    }
    if s.len() <= 6 {
        self.emit_sso_literal(out, s, indent)
    } else {
        self.emit_heap_string_literal(out, s, indent)
    }
}
```

**`emit_sso_literal`:** Pack the bytes into a u64 (little-endian). `shl 3` to leave tag bits. `or 0b001` (SSO tag). word[1] = length. Return `TypedRegister { name: v, ty: Type::string() }`.

The return type is `Type::string()` (`Custom("String")`). The codegen must produce TWO SSA values (or one `<2 x i64>`). Since `lower_custom_type` now reads `rt.fields` and returns `{ i64, i64 }`, the existing struct handling should work for the type. But the value must be a struct, not a single i64.

**Key LLVM IR pattern for SSO literal `"hi"` (2 bytes):**
```llvm
%t0 = or i64 16, 1           ; (0x0068 0x0069) << 3 | 0b001 = pack "hi" + tag
%t1 = insertvalue { i64, i64 } undef, i64 %t0, 0
%t2 = insertvalue { i64, i64 } %t1, i64 2, 1
; result is {i64 %t0, i64 2}
```

**Where to insert:** The string literal codegen is in `emit_expr.rs` around line 148, in the `Expr::Quoted(s)` match arm. Currently it emits a stack alloca + stores + ptrtoint. Replace with the SSO path when `feature_sso_strings` is ON.

```rust
// Current code at ~line 148:
Expr::Quoted(s) => {
    // ... emit heap string literal (alloca, stores, ptrtoint)
}

// After:
Expr::Quoted(s) => {
    return self.emit_string_literal(out, &s, indent);
}
```

**Test:** `test_sso_literal_short` — compile `let s: String = "hi";` with `--feature sso-strings`, check LLVM IR has `or` + `insertvalue` (no `alloca` or `malloc`).

**Test:** `test_sso_literal_empty` — `""` produces tag|0 (no nonsense).

**Test:** `test_sso_literal_6byte` — `"abcdef"` (exactly 6) produces SSO handle.

**Test:** `test_sso_literal_7byte` — `"abcdefg"` (7 bytes) falls through to heap path.

**Rollback:** Set `feature_sso_strings` to `false` — all string codegen reverts to old heap path.

### B3: State field layout — 2 slots

**File:** `src/backend/llvm/mod.rs` — `push_field_type` (line ~822)

When `feature_sso_strings` is ON and the type is `is_string_like`, push `fields.len()` slots instead of 1:

```rust
/// 2026-07-18: Push field type — if SSO strings are enabled and the type
/// is string-like, push one slot per struct field (2 × i64 for String).
pub(super) fn push_field_type(&mut self, ty: &Type) {
    if self.feature_sso_strings
        && self.ctx.type_universe.as_ref().map_or(false, |u| u.is_string_like(ty))
    {
        let n_fields = self.ctx.type_universe.as_ref()
            .and_then(|u| ty.universe_key().and_then(|k| u.get(k)))
            .map(|rt| rt.fields.len())
            .unwrap_or(1);
        for _ in 0..n_fields {
            self.ctx.field_types.push("i64".to_string());
            self.ctx.field_brief_types.push(ty.clone());
        }
    } else {
        self.ctx.field_types.push("i64".to_string());
        self.ctx.field_brief_types.push(ty.clone());
    }
}
```

**Effects:** Every state field of type String now occupies 2 × i64 in `%State` struct. `field_index_map` entries for String fields point to the first slot index; the second slot is implicitly `index + 1`. State load/store for String fields must emit `extractvalue`/`insertvalue` on the 2-slot struct.

**Add a helper** `emit_string_state_store` (in `emit_toplevel.rs` or `helpers.rs`) that extracts the two i64 values from a String handle and stores them to the two state slots.

**Test:** `test_sso_state_2slot` — Compile a txn with `state { s: String; }`, check `%State` type has 2 more i64 fields than before.

### B4: Function ABI — String as `{ i64, i64 }`

**Files:** `src/backend/llvm/emit_toplevel.rs` (emit_fn_body, boxing), `src/backend/llvm/helpers.rs` (adapt_to_i64)

When `feature_sso_strings` is ON:

- **`emit_fn_body`** — String parameters are already `{ i64, i64 }` structs. No ptrtoint boxing needed. Use `extractvalue` to read fields.
- **`adapt_to_i64`** — For String-like types, emit `extractvalue { i64, i64 } %val, 0` instead of `ptrtoint ptr %val to i64`.
- **`frgn` boundary** — String is still passed as `i8*` (C ABI). Add a shim: `extractvalue` field[0] (which is a pointer for heap strings), `inttoptr` to `i8*`.

**Test:** `test_sso_function_abi` — Compile a defn that takes a String param and returns it, check IR shows `{ i64, i64 }` in function signature, not `i64`.

### B5: Concat — SSO path for ≤6 total bytes

**File:** `src/backend/llvm/helpers.rs` line ~749 (emit_inline_concat)

When `feature_sso_strings` is ON and `a_len + b_len <= 6`:
- Read both handles
- Extract data bytes from both
- Pack into new SSO handle
- Return `<{ i64, i64 }>` with tag

When total > 6: allocate heap buffer (no 16-byte header, just raw bytes + null terminator). Copy both strings. Store pointer in handle[0] with tag `0b000` (heap), length in handle[1].

**Test:** `test_sso_concat_short` — `"a" + "b"` produces SSO handle for "ab".
**Test:** `test_sso_concat_overflow` — `"abc" + "def"` (6 bytes total) → SSO. `"abcd" + "efg"` (7 bytes) → heap.

### B6: Tag scheme — `AND -8` instead of `AND -4`

**File:** `src/backend/llvm/mod.rs` — `emit_mask_tag` helper, and `src/backend/llvm/helpers.rs` line ~919 (emit_free_temporaries)

Current tag scheme uses bits 0-1: `AND -4` to mask. SSO uses bits 0-2:

- `000` = heap pointer
- `001` = SSO inline (≤6 bytes)
- `010` = static literal
- `100` = temporary heap (allocated in txn, freed at tick end)

When `feature_sso_strings` is ON:
- `emit_mask_tag`: `AND -8` (mask lower 3 bits)
- `emit_free_temporaries`: check tag bits with `AND 7` instead of `AND 3`

**Test:** `test_sso_tag_mask` — Compile with SSO ON, check IR has `and i64 %x, -8`.

### B6 completion criteria

```
cargo test --lib  → 919 pass (flag OFF)
cargo test --lib -- --feature sso-strings  → 919 pass (flag ON, new SSO tests)
```

---

## Block 2: Alloc Strategy — Phase 3 stdlib arena files

**Depends on:** Phase 1-2 (committed), Phase 4 (analysis pass, committed)

**Goal:** Write the pure-Brief arena types to disk so they're available for import.

### Arena type

**File:** `lib/std/memory/arena.bv`

```brief
// 2026-07-18: Arena — single-direction bump allocator in pure Brief.
// Backed by Alloc# for the backing buffer.
type Arena {
    base: Ptr<Byte>;
    offset: Int;
    capacity: Int;
};

defn arena_init(cap: Int) -> Arena {
    let base: Ptr<Byte> = Alloc#(cap) as Ptr<Byte>;
    term Arena { base, offset: 0, capacity: cap };
};

defn arena_alloc(a: Arena, size: Int) -> Ptr<Byte> {
    let ptr: Ptr<Byte> = a.base + a.offset;
    a.offset = a.offset + size;
    [a.offset <= a.capacity];
    term ptr;
};

defn arena_reset(a: Arena) { a.offset = 0; };

defn arena_free(a: Arena) { Free#(a.base as Int); };
```

### Crossword arena type

**File:** `lib/std/memory/crossword.bv`

```brief
// 2026-07-18: CrosswordArena — dual-direction arena allocator.
// Slots grow from base upward, variable-length data grows from
// (base + capacity) downward. The two regions meet in the middle.
type CrosswordArena<T> <: List<T> {
    InsertAt <~ crossword_push;
    ExtractFrom <~ crossword_pop;
};
// ... (see plan doc for full ~70-line implementation)
```

### Module re-export

**File:** `lib/std/memory/mod.bv` or `lib/std/types.bv`

Add import re-export for the memory types following existing conventions.

**Test:** `test_arena_init_pure_brief` — Compile `arena_init(1024)`, run, check non-null base.
**Test:** `test_crossword_init_pure_brief` — same pattern.

---

## Block 3: Benchmark Baseline — Allocation Strategy

**Depends on:** Blocks 1-2 done, or at least Block 1.

**Files:** None — run commands only.

### Pre-implementation baseline

```bash
cargo build --release && bash benchmarks/build_and_bench.sh --runtime
```

Capture ALL output into a table:

| Benchmark | Brief (s) | C (s) | Ratio | Correctness |
|-----------|-----------|-------|-------|-------------|
| ... | ... | ... | ... | PASS/FAIL |

### Post-implementation verification

Same command after implementation. Compare. No benchmark should regress.

---

## Block 4: Ptr Level 3 — Phase 1 (Provenance Escape)

**Depends on:** Nothing — independent, small change, high impact.

**Goal:** Fix `is_local_provenance()` (currently a stub that always returns `false`).

### 4a: Change signature

**File:** `src/analysis/provenance.rs:57-70`

```rust
/// 2026-07-18: Check if a provenance refers to a local (non-state) variable.
pub fn is_local_provenance(prov: &Provenance, local_names: &HashSet<String>) -> bool {
    match prov {
        Provenance::Known(name) => local_names.contains(name),
        Provenance::FieldAccess { base, .. } | Provenance::Index { base, .. } => {
            is_local_provenance(base, local_names)
        }
        Provenance::Deref(_) => false, // Can't trace through unknown pointer
        Provenance::Unknown => false,
    }
}
```

### 4b: Update callers

**File:** `src/analysis/provenance.rs` — `check_dangling_ptrs` must now receive `local_names`.

```rust
pub fn check_dangling_ptrs(body: &[Statement], local_names: &HashSet<String>) -> Vec<String> {
    // ... use is_local_provenance with local_names
}
```

### 4c: Wire into compile pipeline

**File:** `src/compile.rs` — after typechecking, before codegen:

```rust
// 2026-07-18: Dangling pointer check
let local_names: HashSet<String> = collect_local_names(&txn.body, &txn.params);
let warnings = analysis::provenance::check_dangling_ptrs(&txn.body, &local_names);
for w in &warnings { eprintln!("{}", w); }
```

**Test:** `test_is_local_provenance_with_locals` — `Known("temp")` is local when `"temp"` in set.
**Test:** `test_dangling_warning_fires` — `&state_field = &local_var` produces warning.

---

## Block 5: Ptr Level 3 — Phase 2 (Provenance in Typechecker)

**Depends on:** Block 4 (provenance fix).

**Goal:** Thread `Provenance` through `infer_expression`, returning `(Type, Provenance)`.

### 5a: Change return type

**File:** `src/typechecker/mod.rs`

```rust
fn infer_expression(&mut self, expr: &Expr) -> Result<(Type, Provenance), TypeError> {
    match expr {
        Expr::Identifier(name) => {
            let ty = self.lookup_type(name)?;
            Ok((ty.clone(), Provenance::Known(name.clone())))
        }
        Expr::Decimal(_) | Expr::Bool(_) | Expr::Quoted(_) => {
            Ok((self.infer_literal(expr)?, Provenance::Unknown))
        }
        Expr::AddrOf(inner) => {
            let (inner_ty, inner_prov) = self.infer_expression(inner)?;
            let ptr_ty = Type::ptr(inner_ty);
            Ok((ptr_ty, inner_prov))
        }
        Expr::Deref(ptr) => {
            let (ptr_ty, ptr_prov) = self.infer_expression(ptr)?;
            let inner = pointee_type(&ptr_ty).ok_or_else(|| TypeError::InvalidDeref(ptr_ty.clone()))?;
            Ok((inner, Provenance::Deref(Box::new(ptr_prov))))
        }
        Expr::BinaryOp(_, l, r) => {
            // ... existing type inference ...
            Ok((result_ty, Provenance::Unknown))
        }
        // All other arms return Provenance::Unknown
    }
}
```

### 5b: Add `infer_type_only` wrapper

```rust
pub fn infer_type_only(&mut self, expr: &Expr) -> Result<Type, TypeError> {
    self.infer_expression(expr).map(|(ty, _)| ty)
}
```

### 5c: Update callers

Replace all existing `self.infer_expression(expr)` calls that don't need provenance with `self.infer_type_only(expr)`. For the ~5-10 callers that DO need provenance (AddrOf, Deref, Field), destructure the pair.

**Test:** All existing 919 tests pass (behavior-preserving change).

---

## Block 6: Ptr Level 3 — Phase 3 (PtrConst)

**Depends on:** Block 5 (provenance in typechecker).

### 6a: Add Type variant

**File:** `src/ast/types.rs`

```rust
pub enum Type {
    // ... existing ...
    Ptr(Box<Type>),
    PtrConst(Box<Type>),    // 2026-07-18: Read-only pointer
    // ...
}
```

Add `pub fn ptr_const(inner: Type) -> Type { Type::PtrConst(Box::new(inner)) }`.

### 6b: Update all Type match arms

Every file that matches on `Type` — this is detected by the Rust compiler exhaustiveness check. Files to fix (found by compiling after adding the variant):

- `src/ast/types.rs` — `bytes()`, `alignment()`, `Display`
- `src/ast/display.rs` — Display for PtrConst
- `src/type_universe/mod.rs` — resolve PtrConst
- `src/backend/llvm/types.rs` — `lower_type` → PtrConst maps to `"ptr"`
- `src/backend/llvm/helpers.rs` — `is_ptr_ty` includes PtrConst
- `src/backend/llvm/emit_expr.rs` — AddrOf/Deref codegen
- `src/backend/llvm/emit_stmt.rs` — assignment through PtrConst
- `src/backend/llvm/intrinsics.rs` — Deref#/Index#
- `src/backend/llvm/emit_toplevel.rs` — field declarations
- `src/interpreter/mod.rs` — type dispatch
- `src/annotator.rs` — type dispatch
- `src/ast/unify.rs` — type unification
- All files where `Type::Ptr` appears in match arms

Pattern for each: `PtrConst` is identical to `Ptr` for size (8 bytes), LLVM type (`"ptr"`), and pointee extraction. The only behavioral difference is **no write-through** (type error on `*ptr = val` when `ptr` is `PtrConst`).

### 6c: Const inference from context

**File:** `src/typechecker/mod.rs`

```rust
fn is_mutable_location(&self, expr: &Expr) -> bool {
    let Expr::Identifier(name) = expr else { return false; };
    self.is_state_field(name) || self.is_txn_variable(name)
}
```

Used in `AddrOf` inference:

```rust
Expr::AddrOf(inner) => {
    let (inner_ty, inner_prov) = self.infer_expression(inner)?;
    let ptr_ty = if self.is_mutable_location(inner) {
        Type::ptr(inner_ty)          // Ptr<T> — mutable
    } else {
        Type::ptr_const(inner_ty)    // Ptr<const T> — read-only
    };
    Ok((ptr_ty, inner_prov))
}
```

### 6d: Write-through guard

```rust
// In check_statement, for Assign(lhs, rhs):
if let Expr::Deref(ptr) = &stmt.lhs {
    if let Ok((ptr_ty, _)) = self.infer_expression(ptr) {
        if matches!(ptr_ty, Type::PtrConst(_)) {
            return Err(TypeError::WriteThroughConstPointer { ... });
        }
    }
}
```

**Test:** `test_ptr_const_inferred_for_let` — `&let_x` produces `Ptr<const Int>`.
**Test:** `test_write_through_const_pointer_errors` — `*p = 5` on `PtrConst` → type error.

---

## Block 7: Ptr Level 3 — Phase 4 (Parallel Txn Safety)

**Depends on:** Block 5 (provenance).

**File:** `src/analysis/transition_graph.rs`

Add `write_set_from_prov` helper that traces `Provenance` to determine which fields are written. In the parallel scheduler's write-set computation, check provenance before falling back to conservative.

**Test:** `test_provenance_write_set_known` — Two txns writing through `Known("counter")` conflict.
**Test:** `test_provenance_write_set_unknown` — Txn writing through `Unknown` conflicts with everything.

---

## Block 8: Ptr Level 3 — Phase 5 (SSA Fallback)

**Depends on:** Block 6 (PtrConst) + Block 5 (provenance).

**File:** `src/backend/llvm/loop_engine/` (counter.rs, ssa.rs)

Add a scan for `&state_field` before SSA codegen selection. Any borrowed field is added to `memory_mode_fields`, forcing it into memory mode (GEP + load/store) instead of SSA phi registers.

**Test:** `test_ssa_fallback_on_borrow` — Borrowed field uses GEP/load/store, program produces correct results.

---

## Block 9: Ptr Level 3 — Phase 6 (Interpreter + Proof Engine)

**Depends on:** Block 6 (PtrConst).

**Files:** `src/interpreter/value.rs`, `src/interpreter/eval.rs`, `src/symbolic.rs`

Add `Value::Ptr(Box<Value>)` and `SymbolicValue::Pointer(Box<SymbolicValue>)`. Implement `AddrOf`/`Deref` in the evaluator using these new variants.

**Test:** `test_interpreter_addrof_deref_roundtrip` — `*(&42) == 42`.

---

## Block 10: Ptr Level 3 — Phase 7 (Webstack + CIRCT)

**Depends on:** Nothing — independent.

**Files:** `src/backend/webstack.rs`, `src/backend/circt.rs`

Add `Expr::AddrOf` and `Expr::Deref` match arms. These are active backends (per AGENTS.md) — they must compile.

```rust
// Both backends:
Expr::AddrOf(inner) => {
    self.emit_expr(inner, out)?;
    Ok(())
}
Expr::Deref(ptr) => {
    self.emit_expr(ptr, out)?;
    Ok(())
}
```

---

## Block 11: Ptr Level 3 — Phase 8 (Tests + Docs)

**Runs alongside:** Blocks 4-10.

Add tests for every new feature (see per-block test sections above). Update architecture docs:

- `docs/architecture/features/ptr.md` — PtrConst + provenance
- `docs/architecture/llvm-memory-management.md` — borrowed fields → memory mode
- `docs/architecture/backend-type-dispatch.md` — PtrConst in dispatch table

---

## Block 12: Allocation Strategy — Phase 5 (Benchmark)

**Depends on:** Block 1 (SSO) and Block 2 (arena stdlib) done.

```bash
cargo build --release && bash benchmarks/build_and_bench.sh --runtime
```

Record full output. Compare against pre-implementation baseline. Any regression must be root-caused before proceeding.

---

## Quick-reference: What commits are already in

| Commit | What | Files changed |
|--------|------|---------------|
| `d829294` | Phase A: struct_fields, config encoding, is_string_like | 12 files, +518/-796 |
| `d2579a8` | Derive bytes from fields | 2 files |
| `f0fe805` | memory_spec String 24→16 | 1 file |
| `779c0ce` | Phase 0: parser op handler + bootstrap.bv | 5 files, +166/-139 |
| `7be920d` | Phase 0: typechecker wired to universe | 4 files, +33/-23 |
| `8cc150d` | Phase 0: backend uses config dispatch | 2 files, +47/-2 |
| `ebd9646` | Architecture docs updated | 2 files, +69/-16 |
| `7bae883` | Phase 4: analysis pass (allocation) | 45 files, +1613/-? |
| *(uncommitted)* | B1: Thread feature_sso_strings flag | 4 files (compile.rs, mod.rs, main.rs) |
| *(uncommitted)* | B2: SSO literal emission + llvm_type override | 4 files (emit_expr.rs, emit_toplevel.rs, intrinsics.rs, emit_stmt.rs) |
| *(uncommitted)* | B3: push_field_type 2-slot layout for String | 1 file (mod.rs) |
| *(uncommitted)* | B4: SSO function ABI + adapt_to_i64 + frgn shim | 2 files (emit_toplevel.rs, helpers.rs, emit_expr.rs) |
| *(uncommitted)* | B5: SSO concat (short inline + heap fallback) | 1 file (helpers.rs) |
| *(uncommitted)* | B6: Tag scheme AND -8 / bit 2 temp | 1 file (helpers.rs) |

## File Manifest Check

| File | Status | Purpose |
|------|--------|---------|
| `src/compile.rs` | ✅ Done | Added `feature_sso_strings` to BuildOptions |
| `src/backend/llvm/mod.rs` | ✅ Done | LlvmBackend field + builder, push_field_type 2-slot |
| `src/backend/llvm/emit_expr.rs` | ✅ Done | SSO literal (short + heap), frgn shim, lower_type→llvm_type |
| `src/backend/llvm/emit_toplevel.rs` | ✅ Done | llvm_type SSO override, param boxing SSO gate |
| `src/backend/llvm/helpers.rs` | ✅ Done | SSO concat, adapt_to_i64 SSO, mask_tag/free temp tag scheme |
| `src/backend/llvm/intrinsics.rs` | ✅ Done | lower_type→llvm_type (SSO-aware type resolution) |
| `src/backend/llvm/emit_stmt.rs` | ✅ Done | lower_type→llvm_type, dead-br-after-term fix |
| `src/backed/llvm/emit_stmt.rs` | ✅ Done | Guard/If codegen — conditional br after term/ret |
| `src/parser/definitions.rs` | ✅ Done | txn return type (-> Type) parsing |
| `src/parser/expressions.rs` | ✅ Done | Binary bitwise ops (& \| ^ << >>) |
| `src/type_universe/operators.rs` | ✅ Done | builtin_operator_binding entries for Int bitwise/shift |
| `src/analysis/provenance.rs` | ✅ Done | Fix is_local_provenance stub + collect_local_names |
| `src/analysis/transition_graph.rs` | ✅ Done | Provenance-aware write set extraction |
| `src/typechecker/mod.rs` | ✅ Done | Provenance threading, PtrConst, write-through guard |
| `lib/std/memory/arena.bv` | ✅ Done | Pure-Brief arena type |
| `lib/std/memory/crossword.bv` | ✅ Done | Pure-Brief crossword arena |
| `lib/std/types/bootstrap.bv` | ✅ Done | Utf8View, StaticString, SmallString64 type decls |
| `lib/std/types/utf8view.bv` | ✅ Done | Pure-Brief memcmp, utf8_find, utf8_validate |
| `lib/std/types/small_string.bv` | ✅ Done | SmallString64 inline buffer operations |
| `lib/runtime/brief_rt.c` | ✅ Done | __utf8_validate, __utf8_find (kept for ref, pure-Brief supersedes) |
| `BUGS.md` | ✅ Done | All findings documented |

## Benchmark Baseline (July 18, 2026)

Pre-SSO, pre-allocation-strategy baseline. Run from commit with SSO Phase B + arena stdlib changes.

### Runtime Benchmarks (all `--runtime` tag)

| Benchmark | Brief (s) | C (s) | Ratio | Correctness |
|-----------|-----------|-------|-------|-------------|
| ring_buffer | 0.0328 | 0.0346 | 0.95x | ✅ PASS |
| float_math | 0.0629 | 0.0718 | 0.88x | ✅ PASS |
| float_math_nonzero | 0.3702 | 0.1686 | 2.20x | ✅ PASS |
| sparse_dispatch | 0.0447 | 0.0635 | 0.70x | ✅ PASS |
| print_loop | 0.0663 | 0.0625 | 1.06x | ✅ PASS |
| nbody_newton | 11.935 | 8.397 | 1.42x | ✅ PASS (float epsilon) |
| nbody_sqrt | ❌ compile error | — | — | ❌ pre-existing (`@energy` global undefined) |

### Optimizer Benchmarks (all `--optimizer` tag, full compile-time folding)

| Benchmark | Brief | C | Correctness |
|-----------|-------|---|-------------|
| iir_filter | precomputed | — | ✅ MATCH |
| precompute_sum | precomputed | — | ✅ MATCH |
| const_heavy | precomputed | — | ✅ MATCH |
| async_counters_idio | precomputed | — | ❌ MISMATCH (binary fails at runtime — `__FAIL__`) |

### Notes

- **nbody_sqrt**: Pre-existing failure — `@energy` global undefined in LLVM IR. Not related to SSO or allocation changes. Likely needs a binding or embedding directive fix in `benchmarks/nbody_sqrt.bv`.
- **async_counters_idio**: MISMATCH — binary produces `__FAIL__` output (timeout or crash). Not related to SSO or allocation changes. Pre-existing.
- All benchmarks use `BOUND=50000000` via `GetEnvInt#`. 5 iterations averaged.
- System: x86_64, Linux, gcc/clang `-O3 -march=native -ffast-math`.

## Notes

- All changes are ADDITIVE — never modify existing match arms, only add new ones with `_ => return None` fallthrough preserved.
- Every edit gets a `// 2026-07-18: <why>` rationale comment.
- `cargo test --lib` after every block before committing.
- Block 1 (SSO) is the highest priority — everything else is incremental refinement.
- Block 4-11 (Ptr Level 3) is the lowest priority — the system works correctly without it, but provenance enables the Alloc# escape analysis optimization.

---

## Block 13: SVO for List\<T\> (Quick Plan Sketch)

**Goal:** Extend SSO pattern to `List<T>` — inline storage for small element counts (Small Vector Optimization). Feature-gated behind `feature_svo: bool` (default false).

### Key differences from SSO

| Aspect | SSO String | SVO List\<T\> |
|--------|-----------|---------------|
| Element type | Fixed byte | **Generic** — sizeof(T) varies |
| Handle | `{i64,i64}` = 16B | **`{i64,i64,i64,...}`** = ≥24B |
| Inline capacity | Fixed 6 bytes | `(handle_bytes - 16) / sizeof(T)` |
| State slots | 2 | **3+** per List field |
| Type detection | `is_string_like()` → CTD/encoding | New `is_vector_like()` → `op.SVO <~ N` metadata |
| Runtime | 1 func needs tag awareness | All List RT funcs (push, pop, insert, len, at, resize, etc.) |

### Architecture

`List<T>` goes from a single `i64` heap pointer to a struct with a tag scheme:

```
Tag bits (lower 2 bits of slot 0):
  00 = heap (ptr in slot 0, len in slot 1, cap in slot 2)
  01 = inline (elements in slots 0..N-2, len in slot N-1, cap = N)

Handle layout for inline (N items capacity):
  slot[0..N-2] : packed T elements (bitcast bytes into i64 slots)
  slot[N-1]    : length (upper bits) | capacity (lower bits, packed)
```

### Phase breakdown

| Phase | What | Effort |
|-------|------|--------|
| S0 | Add `feature_svo: bool` to BuildOptions + LlvmBackend | ~10 min |
| S1 | Add `op.SVO <~ N` metadata property to type declarations, `is_vector_like()` | ~30 min |
| S2 | Change List state layout: `push_field_type` pushes N+1 slots (N inline + tag/len) | ~20 min |
| S3 | SVO literal emission: empty `List<Int>()` → inline handle with 0 elements | ~1 hr |
| S4 | SVO push: for `push` with count < cap, write inline; else promote to heap | ~3 hr |
| S5 | SVO pop: read from inline buffer; if heap, use runtime func | ~1 hr |
| S6 | SVO len/at: branch on tag, read inline or heap | ~1 hr |
| S7 | SVO concat/insert: merge two inline lists or promote one | ~2 hr |
| S8 | Update `type_is_heap_allocated` → `type_maybe_heap_allocated` (tag-aware) | ~20 min |
| S9 | Tests + docs | ~1 hr |
| S10 | Flag default true + remove old paths | ~30 min |

**Total:** ~10-12 hours. Requires SSO infrastructure (tag bits, multi-slot state, `llvm_type` override, `adapt_to_i64` → `extractvalue` pattern) which is already in place.

### Open design questions

1. **Default inline capacity N**: Should it be configurable per type (`svo <~ 3`), derived from `sizeof(T)`, or fixed at 2 across all `List<T>`? Recommendation: `svo <~ 3` default (3 elements inline), configurable via metadata. This gives reasonable inline capacity for most types (3 Int = 24B, 3 Bool = 3B padded to 24B) while keeping handle size reasonable.

2. **Promotion path**: When an inline list grows beyond capacity, we allocate a heap buffer, copy inline elements, and update the tag. This is similar to SSO heap path but must handle the generic element type.

3. **Strategy dispatch**: `InsertAt`/`ExtractFrom` currently dispatches to runtime functions that expect heap ptr/len/cap layout. SVO would need the dispatch to check the tag first and handle inline elements in the compiler before falling through to runtime functions for heap elements. This could be done by modifying `emit_strategy_fn_call` to inline the tag check + inline path.

4. **Arena interaction**: SVO inline lists are stored in the state struct, not in an arena. When promoted to heap, they use the arena or Malloc# depending on strategy. This is the same pattern as SSO string promotion.

### Files to modify

| File | Phase | Change |
|------|-------|--------|
| `src/compile.rs` | S0 | Add `feature_svo` to BuildOptions |
| `src/backend/llvm/mod.rs` | S0, S2 | Add field + builder, modify `push_field_type`, modify `type_is_heap_allocated` |
| `src/backend/llvm/emit_toplevel.rs` | S0 | `llvm_type` override for `is_vector_like` |
| `src/type_universe/mod.rs` | S1 | Add `is_vector_like()` similar to `is_string_like()` |
| `src/backend/llvm/helpers.rs` | S2-S7 | `adapt_to_i64` for List struct, inline push/pop/len |
| `src/backend/llvm/emit_expr.rs` | S4-S7 | SVO literal, push, pop emit |
| `src/backend/llvm/emit_stmt.rs` | S4 | SVO-aware store for List fields |
| `lib/std/from-bits.bv` | S1 | Add `svo <~ 3` metadata to `List` type declaration |

### Acceptance criteria

```
cargo test --lib  → 919+ pass (flag OFF)
cargo test --lib -- --feature svo  → all pass (flag ON, new SVO tests)
```

---

## Block 14: Utf8View & Embedded String Types (Design Plan)

**Goal:** Add two new string-like types to Brief: `Utf8View` (borrowed, zero-allocation UTF-8 view) and `SmallString<N>` (stack-allocated inline buffer, no heap), plus `StaticString` (compile-time-known ROM data). These complete the string type family for systems, embedded, and bare-metal targets.

### Architecture

```
String types in Brief:

  ┌──────────────────────────────────────────────────────────┐
  │                    String (owned)                         │
  │  {data: Int, len: Int}                                    │
  │  SSO: ≤6 bytes inline, >6 bytes heap                     │
  │  encoding <~ "UTF-8" (default, configurable)              │
  │  tag bits: 0b001=SSO, 0b000=heap, 0b100=temp             │
  │  Owns + frees. Can be stored in state.                   │
  └──────────────────────────────────────────────────────────┘
         │  can borrow as
         ▼
  ┌──────────────────────────────────────────────────────────┐
  │                 Utf8View (borrowed view)                  │
  │  {data: Int, len: Int}                                    │
  │  encoding <~ "UTF-8" (HARDCODED — guaranteed by constr.)  │
  │  NO ownership, NO free, NO SSO, NOT stored in state      │
  │  Validated at construction boundary.                     │
  │  O(1) slice, O(n) char_len via encoding dispatch.        │
  │  Lifetimes tracked by Ptr Level 3 borrow checker.        │
  └──────────────────────────────────────────────────────────┘
         │  subtypes for embedded:
         ▼
  ┌──────────────────────────┐  ┌──────────────────────────────┐
  │  SmallString<N> (inline)  │  │  StaticString (ROM)           │
  │  buf: [u8; N]; len: Int   │  │  data: Int (→ .rodata); len  │
  │  Stack-allocated, no heap │  │  Compile-time constant data  │
  │  bytes <~ N + 8           │  │  No runtime overhead          │
  └──────────────────────────┘  └──────────────────────────────┘
```

### U1: Utf8View Type

**Definition** (`lib/std/types/utf8view.bv`):
```brief
// 2026-07-18: Utf8View — borrowed, trusted-UTF-8 view of byte data.
// Zero-allocation. Does NOT own the underlying buffer.
// encoding <~ "UTF-8" is guaranteed by construction (validated at boundary).
// Implements the same {data, len} fat-pointer format as String.
type Utf8View {
    data: Int;
    len: Int;
    encoding <~ "UTF-8";
    tbaa <~ "Utf8View";
};
```

**Semantics:**
- **NOT stored in state** — Utf8View is a borrow, cannot survive tick boundaries
- **NOT heap-allocated** — `type_is_heap_allocated` returns false
- **NOT SSO** — always a fat pointer `{ptr, len}`. Even with `feature_sso_strings=true`, Utf8View gets `{i64, i64}` LLVM type but the `data` field is always a pointer (never inline packed data)
- **NOT freed** — no `Free#` on drop
- **encoding is ALWAYS "UTF-8"** — not configurable. The contract is: if you have a Utf8View, the bytes are valid UTF-8. If you want a different encoding, use `String`.
- **Passes `is_string_like()`** — matches Phase B (2 Int fields + encoding property)

**Construction:**
```brief
// From a String (borrow, no validation needed — String is already valid)
defn Utf8View::from_string(s: &String) -> Utf8View {
    term Utf8View { data: s.data, len: s.len };
};
// From raw bytes + len (validates UTF-8 at runtime)
defn Utf8View::from_bytes(ptr: Ptr<Byte>, len: Int) -> Utf8View [len >= 0] {
    // Runtime call to UTF-8 validator in brief_rt.c
    let valid: Bool = __utf8_validate(ptr, len);
    [valid == true];  // contract: validation MUST pass
    term Utf8View { data: ptr as Int, len: len };
};
// From compile-time string literal (zero cost — data is in .rodata)
defn Utf8View::from_literal(s: String) -> Utf8View {
    // The compiler recognizes this pattern and optimizes to a direct view
    // without the String intermediate allocation.
    term Utf8View { data: __literal_ptr(s), len: s.len };
};
```

**Operations:**
```brief
// O(1) — byte length
defn Utf8View::len(v: Utf8View) -> Int { term v.len; };

// O(1) — slice returns a new view over the same underlying data
defn Utf8View::slice(v: Utf8View, start: Int, end: Int) -> Utf8View
    [start >= 0][start <= end][end <= v.len]
{
    let new_ptr: Ptr<Byte> = (v.data as Ptr<Byte>) + start;
    term Utf8View { data: new_ptr as Int, len: end - start };
};

// O(n) — codepoint count via encoding dispatch
defn Utf8View::char_len(v: Utf8View) -> Int {
    // Dispatches through config/encodings.toml ops.char_len
    term EncodingLength#(v.data, v.len);
};

// O(n) — byte offset of substring (returns -1 if not found)
defn Utf8View::find(haystack: Utf8View, needle: Utf8View) -> Int {
    term __utf8_find(haystack.data, haystack.len, needle.data, needle.len);
};

// O(n) — equality comparison (byte-level)
defn Utf8View::eq(a: Utf8View, b: Utf8View) -> Bool {
    term a.len == b.len && __memcmp(a.data, b.data, a.len) == 0;
};

// Convert to owned String (allocates)
defn String::from_utf8_view(v: Utf8View) -> String {
    let buf: Ptr<Byte> = Alloc#(v.len) as Ptr<Byte>;
    __memcpy(buf, v.data as Ptr<Byte>, v.len);
    term String { data: buf as Int, len: v.len };
};
```

**Utf8View as `&str` sugar** — The compiler can recognize `&s` where `s: String` and automatically produce a Utf8View. This is syntactic sugar for `Utf8View::from_string(&s)`.

**Compiler changes for Utf8View:**

| File | Change |
|------|--------|
| `lib/std/types/utf8view.bv` | Add Utf8View type definition |
| `lib/std/types/bootstrap.bv` | Add `import "std/types/utf8view.bv"` |
| `src/type_universe/mod.rs` | Register Utf8View as string-like (auto-detected by `is_string_like` — no change needed) |
| `src/backend/llvm/mod.rs` `type_is_heap_allocated` | Exclude Utf8View (never heap-allocated) — add explicit `!is_string_like &&` check OR add Utf8View to exclusion list |
| `src/backend/llvm/emit_toplevel.rs` `llvm_type` | Utf8View always gets `{i64, i64}` LLVM type (regardless of `feature_sso_strings`), since it's always a fat pointer. The SSO override should check for Utf8View explicitly or the `is_string_like` check should differentiate String vs Utf8View. |
| `src/backend/llvm/helpers.rs` `adapt_to_i64` | Utf8View → i64: extract handle[0] (same as SSO String) |
| `src/backend/llvm/emit_stmt.rs` | Utf8View values cannot be stored to state fields — emit compile error |
| `lib/runtime/brief_rt.c` | Add `__utf8_validate(ptr, len)` validation function |
| `config/encodings.toml` | UTF-8 ops already defined — no change needed |
| Tests | Construction, slice, find, char_len, eq, conversion from String |

### U2: SmallString\<N\> (Embedded-Stack String)

**Purpose:** Stack-allocated inline buffer with compile-time-fixed capacity. Zero heap allocation. For embedded targets (microcontroller, bare-metal) and hot paths where allocation is unacceptable.

**Definition** (`lib/std/types/small_string.bv`):
```brief
// 2026-07-18: SmallString<N> — stack-allocated inline string.
// N = maximum byte capacity (compile-time constant).
// No heap allocation, no SSO, no Free#.
// Ideal for embedded systems and hot paths.
type SmallString<@bufsize: Int> {
    buf: Ptr<Byte>;      // points to inline buffer within the struct
    len: Int;            // current byte length
    capacity: Int;       // N (compile-time, stored for runtime bounds checks)
    encoding <~ "UTF-8";
    bytes <~ N + 16;     // struct size: N bytes inline buffer + 8 len + 8 cap
};
```

**Wait — design issue**: SmallString's `buf` field is a Ptr<Byte> that points to the inline buffer WITHIN the struct itself. This means:
- The struct is self-referential (pointer points into its own bytes)
- Moving the struct invalidates the pointer
- LLVM GEP/lifetime annotations are needed to annotate the internal pointer

**Alternative design (recommended):** Avoid the self-referential pointer. Instead, store the inline buffer directly as a sequence of `Int` fields, similar to SSO but with variable capacity:

```brief
type SmallString<@bufsize: Int> {
    buf: Int;           // packed inline data (like SSO handle[0])
    len: Int;           // current byte length
    capacity: Int;      // N (compile-time, usize)
    encoding <~ "UTF-8";
};
```

Here, `buf` is treated like SSO handle[0]: N bytes are packed into the integer, shifted left by tag bits. For N ≤ 6: identical to SSO String. For N > 6: uses additional Int fields in the parent struct (requires `bytes <~` to allocate enough space, with `push_field_type` pushing the right number of slots).

But this ties SmallString's capacity to the number of i64 slots, which is architecture-dependent.

**Third alternative (recommended for MVP):** SmallString<N> is a wrapper around a fixed-size byte array in the struct, with its own LLVM type:

```brief
// 2026-07-18: SmallString<N> — inline buffer string.
// LLVM type: { [N x i8], i64, i64 } — buf, len, capacity.
// buf is NOT a pointer — it IS the inline bytes.
// The type is lowered via declare_struct_types as a custom LLVM struct.
type SmallString<@bufsize: Int> {
    encoding <~ "UTF-8";
};
```

The struct has NO explicit fields — the LLVM type is derived from the `@bufsize` parameter and `bytes <~` metadata via `declare_struct_types`. Operations access the inline buffer via GEP into the struct.

**Operations:**
```brief
defn SmallString::init<@N: Int>() -> SmallString<@N> {
    term SmallString { };  // zero-initialized: buf = zeros, len = 0, cap = N
};

defn SmallString::push_char(s: SmallString<@N>, c: Char) [s.len < N] {
    let offset: Int = s.len;
    // Store byte(s) into inline buffer at offset
    __store_byte(s as Ptr<Byte>, offset, c as Int);
    s.len = s.len + char_width(c);
};

defn SmallString::as_utf8_view(s: SmallString<@N>) -> Utf8View {
    term Utf8View { data: s as Ptr<Byte> as Int, len: s.len };
};
```

**Compiler changes for SmallString:**

| Change | Effort |
|--------|--------|
| Parser: accept integer type args in `Applied<Width(n)>` | ~1 hr |
| Normalizer: derive LLVM struct type `{[N]i8, i64, i64}` from `@bufsize` + `bytes <~` | ~2 hr |
| `declare_struct_types`: register SmallString<N> structs | ~1 hr |
| Codegen: emit inline buffer read/write via GEP | ~1 hr |
| Utf8View conversion: cast SmallString ptr to Utf8View data ptr | ~30 min |

### U3: StaticString (ROM String)

**Purpose:** Compile-time-known string data in ROM (.rodata). Zero runtime cost.

**Definition:**
```brief
type StaticString {
    data: Int;    // ptr to .rodata (known at link time)
    len: Int;     // byte length (known at compile time)
};
```

**Construction:** Automatically created by the compiler for all string literals. Every `"hello"` in source code produces a `StaticString` before being converted to `String` or `Utf8View`. The compiler:
1. Places the bytes + null terminator in `.rodata`
2. Sets `data` to the address (known at compile time via the `@` global)
3. Sets `len` to the byte count

**No compiler changes needed** — this is already how string literals work (they live in the function's entry block alloca or in `.rodata`). The StaticString type just provides a typed view of this existing behavior.

### U4: Relationship to SSO Infrastructure

Key interactions between Utf8View/SmallString and the existing SSO infrastructure:

| Concern | String | Utf8View | SmallString<N> |
|---------|--------|----------|----------------|
| `is_string_like()` | ✅ passes | ✅ passes (2 Int + encoding) | ❌ doesn't pass (different field structure) |
| `feature_sso_strings` | SSO ≤6B, heap >6B | Always fat pointer (no SSO) | Always inline (no SSO needed) |
| `push_field_type` | 2 slots (when SSO ON) | 2 slots (always) | Custom LLVM type |
| `llvm_type` | `{i64,i64}` (SSO ON) or `ptr` | `{i64,i64}` (always) | `{[N]i8,i64,i64}` |
| `type_is_heap_allocated` | true (can be heap) | false (never) | false (never) |
| State field | ✅ allowed | ❌ forbidden (borrow) | ✅ allowed (owned inline) |
| Free# on drop | ✅ yes (heap strings) | ❌ no | ❌ no |
| Tag bits | 3-bit tag scheme | Not needed (always ptr+len) | Not needed (always inline) |

**Critical cleanup**: The `is_string_like` check at `emit_toplevel.rs:331` currently has `if name == "String"` hardcoded before the `is_string_like` fallback. For Utf8View to get `{i64, i64}` even without `feature_sso_strings`, this needs to change to:

```rust
// 2026-07-18: Utf8View always uses {i64, i64} (fat pointer), regardless of SSO.
if let Type::Custom(name) = ty {
    if name == "Utf8View" || name == "String" && self.feature_sso_strings {
        return "{ i64, i64 }".to_string();
    }
}
if self.feature_sso_strings
    && self.ctx.type_universe.as_ref().map_or(false, |u| u.is_string_like(ty))
{
    return "{ i64, i64 }".to_string();
}
```

### U5: Phase Breakdown

| Phase | What | Files | Effort |
|-------|------|-------|--------|
| U1a | Add Utf8View type declaration | `lib/std/types/utf8view.bv` | ~10 min |
| U1b | Wire Utf8View into bootstrap | `lib/std/types.bv` or mod | ~5 min |
| U1c | Update `llvm_type` for Utf8View (always `{i64,i64}`) | `emit_toplevel.rs` | ~5 min |
| U1d | Exclude Utf8View from `type_is_heap_allocated` | `mod.rs` | ~5 min |
| U1e | Add `__utf8_validate` to C runtime | `lib/runtime/brief_rt.c` | ~30 min |
| U1f | Utf8View operations (slice, find, eq, char_len) | `.bv` files + encoding dispatch wiring | ~2 hr |
| U2a | Parser: accept `Type::Width` in generic type args | `src/parser/types.rs` | ~1 hr |
| U2b | Normalizer: derive SmallString LLVM struct | `src/backend/llvm/normalizer.rs` | ~2 hr |
| U2c | `declare_struct_types` for SmallString | `src/backend/llvm/types.rs` | ~1 hr |
| U2d | SmallString operations (init, push, as_utf8_view) | `.bv` files + codegen | ~2 hr |
| U2e | Write-through guard for Utf8View (no state store) | `emit_stmt.rs` | ~10 min |
| U3 | StaticString (compile-time ROM view) | `.bv` file (already works) | ~30 min |
| UT | Tests for all new types | Various test files | ~2 hr |

**Total:** ~10-12 hours of implementation work.

### Key Design Decisions for Review

1. **Utf8View `encoding` is ALWAYS UTF-8** — not configurable. This is the semantic contract: Utf8View guarantees valid UTF-8. If you need a borrowed view of a non-UTF-8 string, use the raw String type or a byte span.

2. **Utf8View NOT in state** — enforcing this at the typechecker level: any `statement::Assign` to a state field with type Utf8View is a compile error. This prevents dangling borrows.

3. **SmallString<N> uses custom LLVM struct** — not the `{i64,i64}` SSO pattern. It's a fundamentally different type: inline byte array + length + capacity. The LLVM type `{[N]i8, i64, i64}` is declared via `declare_struct_types` and not treated as string-like by the codegen.

4. **SmallString<N> `as_utf8_view()` is zero-cost** — the SmallString's inline buffer address is cast directly to Utf8View's data pointer. The Utf8View borrows from the SmallString. Lifetime must be enforced by the borrow checker (same rule: SmallString must outlive all Utf8Views that borrow from it).

5. **No heap fallback for SmallString** — unlike String (SSO → heap promotion), SmallString panics/contract-violates when `len > N`. This is intentional for embedded systems where allocation is not available.

6. **Utf8View as function parameter convention** — Standard library functions should accept `Utf8View` for string parameters where ownership is not needed. This enables callers to pass either String (borrowed) or SmallString (borrowed) or StaticString (borrowed) without conversion.

---

## Fix Block: Pure-Brief Functions + txn Return Type + .#field Access

**Motivation:** The pure-Brief `__memcmp`, `__utf8_find`, `__utf8_validate` functions in `utf8view.bv` compile but can't be tested because:
1. The parser doesn't accept `-> RetType` on `txn` declarations
2. Non-SSO String field access via `.data`/`.len` uses `extractvalue` on `ptr` type, which fails

**Changes needed:**

### F1: Parser — optional `-> Type` for `txn`

**File:** `src/parser/definitions.rs` line 268

Before `parse_block()`, parse an optional return type:
```rust
let output_type = if self.eat(&Token::Arrow) {
    Some(self.parse_type()?)
} else {
    None
};
```
Replace `output_type: None` with `output_type`.

### F2: Non-SSO `.#field` access

The `.#field` syntax (`s1.#data`, `s1.#len`) triggers `emit_layout_field_read` which reads by byte offset via `Load#(addr + offset, width)` — works for both SSO and non-SSO String because it doesn't rely on LLVM struct decomposition.

Use in tests instead of `.data`/`.len`:
```brief
let data_ptr: Int = s1.#data;
let byte_len: Int = s1.#len;
```

### F3: Correct Brief syntax in tests

- `else if` chains → `when` guard chains
- `txn -> RetType` → valid after F1
- `if/else` expressions → `if cond { expr } else { expr }` (single-level only)

### Test file: `test_utf8_pure.bv`

Inline all 3 functions, exercise `memcmp`, `utf8_find`, `utf8_validate` with known test vectors, compile and run, verify exit code = 0.

### F4: Parser — add binary bitwise ops `&` `|` `^` `<<` `>>`

**File:** `src/parser/expressions.rs`

The `BinaryOpKind` enum and LLVM config already have `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr` — but the parser never wires the tokens to these variants. Add four new parse levels between `parse_compare` and `parse_term`:

```
parse_compare:  < > <= >=  → calls parse_bitor
parse_bitor:    |           → calls parse_bitxor
parse_bitxor:   ^           → calls parse_bitand
parse_bitand:   &           → calls parse_shift
parse_shift:    << >>       → calls parse_term
parse_term:     + - ++      → calls parse_factor
```

Each token already exists in the lexer: `Pipe` (`|`), `BitXor` (`^`), `Ampersand` (`&`), `Shl` (`<<`), `Shr` (`>>`). No new tokens needed.

### F5: Convention fixes

- `__` prefix is frgn-only — `__memcmp` → `memcmp`, `__utf8_find` → `utf8_find`, `__utf8_validate` → `utf8_validate`
- `defn` cannot mutate outside state — already correct (functions mutate their parameters, which is returned via `term`)
- No `else` keyword exists — use `when` guards everywhere
- Entry point uses `rct txn` or `[#]` marker, not `defn main()`

---

## Block 15: Unified Allocation Strategy System

**Goal:** Replace the current stub escape analysis with a full DAG-based inference
system that automatically selects the best allocation strategy per `Alloc#` call site.

### Current Gaps Found

| Gap | Impact | File |
|-----|--------|------|
| `will_escape_current_allocation()` always returns `false` | Arena/Alloca paths never selected | `mod.rs:1298` |
| `is_static_bound` never set to `true` | Alloca path is dead code | `mod.rs:1290` |
| No RingBuffer strategy | Streaming patterns default to Malloc | `mod.rs:278` |
| No Inline/SOO strategy beyond SSO string | Small allocs always go to heap | `mod.rs:278` |
| No runtime fallback (try stack → heap) | Stack overflow for dynamic sizes | `intrinsics.rs:280` |
| No DAG dataflow analysis | Strategy defaults to Malloc for everything | `allocation.rs` |
| No thread-local arena | Parallel dispatch can't use arenas | `mod.rs:1177` |
| Config strategies always Free#→@free | Wrong for pool/no-free strategies | `intrinsics.rs:359` |

### Phase A1 — Extend AllocStrategy

**File:** `src/backend/llvm/mod.rs:278`

Current:
```rust
pub enum AllocStrategy { Arena, Malloc, Alloca }
```

Add:
```rust
pub enum AllocStrategy {
    Arena,                     // bump-allocated from per-txn arena
    Malloc,                    // heap via @malloc
    Alloca,                    // stack via alloca
    Inline,                    // inline in parent struct (SSO/SVO)
    RingBuffer,               // circular buffer, overwrite-oldest
    Config(String),           // named template from alloc-strategies.toml
    Custom(String),           // user-provided Brief function name
}
```

Update all match sites (in `emit_alloc`, `emit_free`, `emit_alloc_with_strategy`).

### Phase A2 — DAG-Based Strategy Inference

**File:** `src/analysis/allocation.rs` (full rewrite)

#### A2a: DAG Builder

Walk each txn/defn body, build a dataflow graph:

```rust
pub enum DagNode {
    Alloc { id: usize, size: Expr, result: Var },
    Store { target: Var, source: Var },
    Call { name: String, args: Vec<Var>, result: Var },
    Return { value: Var },
    StateWrite { field: String, value: Var },
}

pub struct DataflowEdge {
    pub from: NodeId,
    pub to: NodeId,
    pub via: Var,
}

pub struct DataflowGraph {
    pub nodes: Vec<DagNode>,
    pub edges: Vec<DataflowEdge>,
    pub root_allocations: Vec<NodeId>, // Alloc nodes at function root
}
```

Builder walks the statement list recursively:
- `Statement::Let { name, expr: Some(expr) }` → create edges from expr vars to name
- `Statement::Assign(lhs, rhs)` → create edge from rhs vars to lhs vars
- `Expr::Call("Alloc#", args, Some(id))` → create `DagNode::Alloc { id, size, result }`
- `Statement::Term(Some(expr))` → create `DagNode::Return { value }`
- `Expr::Field(_, _)` / `Expr::Identifier(_)` → resolve to Var
- `when cond { body };` → recurse into body (same scope)

#### A2b: Escape Detection

For each `Alloc` node, trace forward through all reachable edges:

```rust
fn compute_escape(graph: &DataflowGraph, alloc: NodeId) -> EscapeResult {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(alloc);

    while let Some(node) = queue.pop_front() {
        if !visited.insert(node) { continue; }
        match &graph.nodes[node] {
            DagNode::StateWrite { .. } => return Escaped,       // stored in state
            DagNode::Return { .. } => return Escaped,           // returned
            DagNode::Call { args, .. } => {
                if args.contains(&result_var) { return Escaped; } // passed to fn
            }
            _ => {
                // Follow outgoing edges
                for edge in &graph.edges {
                    if edge.from == node { queue.push_back(edge.to); }
                }
            }
        }
    }
    NotEscaped  // no escape path found
}
```

Use existing `provenance::infer_provenance` + `is_local_provenance` for assignments
to distinguish state fields from local variables.

#### A2c: Strategy Assignment

```rust
fn assign_strategy(alloc: &AllocNode, escape: EscapeResult, scope: &ScopeInfo) -> AllocStrategy {
    match escape {
        Escaped => AllocStrategy::Malloc,      // must use heap
        NotEscaped => {
            let size_const = alloc.size.as_const_u64();
            match scope {
                ScopeInfo::InArena => {
                    if size_const.map_or(false, |s| s <= 8) {
                        AllocStrategy::Inline     // tiny → struct field
                    } else {
                        AllocStrategy::Arena      // bump allocate
                    }
                }
                ScopeInfo::Bounded => {
                    if size_const.map_or(false, |s| s <= threshold) {
                        AllocStrategy::Alloca     // stack
                    } else {
                        AllocStrategy::Inline     // inline or arena fallback
                    }
                }
                ScopeInfo::Reactive => {
                    // Check ring buffer pattern
                    if is_ring_buffer_candidate(alloc, graph) {
                        AllocStrategy::RingBuffer
                    } else {
                        AllocStrategy::Malloc     // reactive → conservative
                    }
                }
                ScopeInfo::Default => AllocStrategy::Malloc,
            }
        }
    }
}
```

Ring buffer detection: allocation result written to state field, field consumed
within same tick, field overwritten every tick (no cross-tick persistence).

### Phase A3 — Runtime Fallback Checks

**File:** `src/backend/llvm/intrinsics.rs:275`

When analysis assigns `Alloca` but size is runtime-determined:

```rust
fn emit_dynamic_alloc(backend, out, v, size, threshold, indent) {
    let stack_l = format!(".stack{}", backend.fun.txn_counter);
    let heap_l = format!(".heap{}", backend.fun.txn_counter);
    let done_l = format!(".done{}", backend.fun.txn_counter);
    backend.fun.txn_counter += 1;

    writeln!(out, "  %cmp = icmp ule i64 {}, {}", size, threshold).ok();
    writeln!(out, "  br i1 %cmp, label %{}, label %{}", stack_l, heap_l).ok();
    writeln!(out, "{}:", stack_l).ok();
    writeln!(out, "  %s = alloca i8, i64 {}", size).ok();
    writeln!(out, "  %sv = ptrtoint ptr %s to i64").ok();
    writeln!(out, "  br label %{}", done_l).ok();
    writeln!(out, "{}:", heap_l).ok();
    writeln!(out, "  %h = call ptr @malloc(i64 {})", size).ok();
    writeln!(out, "  %hv = ptrtoint ptr %h to i64").ok();
    writeln!(out, "  br label %{}", done_l).ok();
    writeln!(out, "{}:", done_l).ok();
    writeln!(out, "  {} = phi i64 [ %sv, %{} ], [ %hv, %{} ]", v, stack_l, heap_l).ok();
}
```

Add `stack_threshold: u64` to `BuildOptions` (default 4096).

### Phase A4 — RingBuffer Strategy

**File:** `src/backend/llvm/intrinsics.rs`

New function:
```rust
fn emit_ring_buffer_alloc(backend, out, v, size, indent) -> BTypedRegister {
    // Ring buffer state: @ring_head (global), @ring_buf (global array)
    writeln!(out, "  %head = load i64, ptr @ring_head").ok();
    writeln!(out, "  %wrapped = and i64 %head, {} - 1", RING_SIZE).ok();
    writeln!(out, "  %slot = getelementptr i8, ptr @ring_buf, i64 %wrapped").ok();
    writeln!(out, "{}{} = ptrtoint ptr %slot to i64", indent, v).ok();
    writeln!(out, "  %next = add i64 %head, 1").ok();
    writeln!(out, "  store i64 %next, ptr @ring_head").ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}
```

### Phase A5 — Inline/SOO Strategy

**File:** `src/backend/llvm/intrinsics.rs`

For allocations marked `Inline`, the `Alloc#` becomes a no-op — the storage
is already in the parent struct. The returned "pointer" is actually a field
offset into the containing struct. Operations on it use struct field access
(extractvalue/insertvalue) rather than load/store through a pointer.

For now, wire `Inline` to the existing SSO handle pattern (used by String with
`feature_sso_strings = true`). Future: generalize to any fixed-size type.

### Phase A6 — Wire is_static_bound

**Files:** `src/backend/llvm/mod.rs:1290`, `src/backend/llvm/emit_toplevel.rs`

Set `is_static_bound` during txn emission when the contract has a bounded
pre-condition:

```rust
// In emit_toplevel.rs, where txns are emitted:
let bounded = matches!(&txn.contract.pre_condition,
    Expr::BinaryOp(BinaryOpKind::Lt | BinaryOpKind::Le | BinaryOpKind::Lt | BinaryOpKind::Ge,
        left, right))
    && matches!(left.as_ref(), Expr::Identifier(_));
self.fun.is_static_bound = bounded;
```

This enables the `Alloca` path in `emit_alloc` for bounded-counter txns like
`rct txn count [x < TOTAL][x == TOTAL] { ... }`.

### Phase A7 — Thread-Local Arena

**File:** `src/backend/llvm/mod.rs`

Add `emit_tls_arena_init` for parallel dispatch. Uses `pthread_getspecific`/
`pthread_setspecific` to give each thread its own arena. Falls back to
per-txn arena for sequential dispatch.

### Phase A8 — Config Free# Metadata

**Files:** `src/config.rs`, `config/alloc-strategies.toml`

Extend `AllocConfigEntry`:
```rust
pub struct AllocConfigEntry {
    pub template: String,
    pub free: Option<String>,   // None=@free, Some("none")=no-op, Some("fn")=custom
}
```

In `emit_free`: check `alloc_strategies` for `Config(name)`, look up name
in config, get the `free` field, dispatch accordingly.

### Phase Order

| Phase | Depends on | Effort |
|-------|-----------|--------|
| A8 | None | ~30 min |
| A6 | None | ~30 min |
| A1 | None | ~10 min |
| A2a | None | ~4 hr |
| A2b | A2a | ~3 hr |
| A2c | A2b | ~2 hr |
| A3 | A2c | ~2 hr |
| A4 | A1 | ~2 hr |
| A5 | A1 | ~1 hr |
| A7 | A3 | ~2 hr |
| AT | All | ~3 hr |

