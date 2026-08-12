# Benchmark Regression Fix — Arena-By-Proof, ABI Coercion, and SLP Vectorization
## 2026-07-27

## Overview

Five root causes were identified from benchmark regression analysis. Each fix is ordered by priority (P0 = must-fix for correctness, P1 = must-fix for performance parity).

| Priority | Fix | Files Changed | Impact |
|----------|-----|---------------|--------|
| P0 | Arena-By-Proof: conditional malloc(64KB) only when proven needed | `allocation.rs`, `call_graph.rs`, `mod.rs`, `dispatch.rs`, `emit_toplevel.rs`, `context.rs` | Eliminates 64KB malloc from all simple benchmarks |
| P0 | ABI type coercion in `emit_direct_frgn_call` | `emit_expr.rs` | Fixes `__print_int(float)` correctness bug |
| P0 | Print plugin float type inference | `print_plugin.rs` | Fixes root cause of `__print_int(float)` |
| P1 | Remove hand-rolled SLP vector emission | `vector_codegen.rs`, `counter.rs` | Unblocks SROA, lets LLVM vectorize naturally |
| P1 | Relax `memory(readwrite)` on main loop | `mod.rs` | Enables SROA to promote state fields |

### Baseline Requirement

Before any changes, run a clean baseline:

```bash
cargo build --release
bash benchmarks/build_and_bench.sh --runtime
bash benchmarks/build_and_bench.sh --correctness
```

Record the full output table. After all fixes, run the same commands and compare. Every benchmark must either match or improve its ratio vs C. No benchmark may regress.

---

## Fix 1 — Arena-By-Proof (Transitive Allocation Analysis)

### Problem

`emit_arena_init()` at `src/backend/llvm/mod.rs:1447` emits `call ptr @malloc(i64 65536)` unconditionally for every program with reactive transactions. The arena fields (`__arena_ptr`, `__arena_end`, `__arena_base`) are injected into `%State` unconditionally at `mod.rs:1786-1804`. This wastes 64KB of heap allocation and 24 bytes of `%State` struct space when no `Alloc#` call uses the arena.

### Existing Infrastructure

1. **`src/analysis/allocation.rs`** (395 lines): DAG-based escape analysis for `Alloc#` call sites. Purely intra-procedural — creates a fresh `DataflowGraph` per function at line 159. Assigns `AllocStrategy` (Malloc/Arena/Alloca/Inline) per `Alloc#` call. `DagNode::Call` is defined at line 27 but never constructed — dead code.

2. **`src/analysis/call_graph.rs`** (306 lines): Working `CallGraph` struct with `build_from_program()` at line 29, `has_cycle()` at line 44, `extract_called_transactions()` at line 88. Currently only tracks `Transaction`→`Transaction` calls (line 35: `TopLevel::Transaction` only). Does not track `Definition` callees and is not connected to the allocation analysis.

### Changes

#### 1a. Extend `call_graph.rs` to track `Definition` callees

**File:** `src/analysis/call_graph.rs`

**Line 29-41** — Extend `build_from_program()` to also iterate `TopLevel::Definition`:

```rust
// 2026-07-27: Also track Definition callees for transitive arena-need propagation.
// Previously only tracked Transaction→Transaction edges. defn functions can call
// Alloc# and the arena must be propagated to calling txns.
pub fn build_from_program(&mut self, items: &[TopLevel]) {
    self.graph.clear();
    self.txn_names.clear();
    self.defn_names.clear();
    self.cycles.clear();

    for item in items {
        match item {
            TopLevel::Transaction(txn) => {
                self.txn_names.insert(txn.name.clone());
                let called = extract_called_functions(&txn.body);
                self.graph.entry(txn.name.clone()).or_default().extend(called);
            }
            TopLevel::Definition(defn) => {
                self.defn_names.insert(defn.name.clone());
                let called = extract_called_functions(&defn.body);
                self.graph.entry(defn.name.clone()).or_default().extend(called);
            }
            _ => {}
        }
    }
}
```

Add `defn_names: HashSet<String>` field to `CallGraph` struct (line 13-17).

Rename `extract_called_transactions` to `extract_called_functions` (line 88) — the body is identical but the name changes to reflect that it now captures all call targets, not just txns.

#### 1b. Add `needs_arena` flag to allocation analysis

**File:** `src/analysis/allocation.rs`

Add to `DagBuilder` struct (line 90-98):

```rust
// 2026-07-27: Track whether ANY Alloc# in this function uses Arena strategy.
// If so, the enclosing txn needs the arena initialized. Propagated transitively
// through the call graph.
pub needs_arena: bool,
```

Initialize to `false` at line 101.

In `walk_expr` at line 264 (`Expr::Call` with `Alloc#`), after inserting the default strategy, check if the default is `Arena`:

```rust
Expr::Call(name, args, id) if name == "Alloc#" => {
    let analysis_id = *self.counter;
    *self.counter += 1;
    *id = Some(analysis_id);
    if let Some(size_expr) = args.first() {
        self.graph.alloc_sizes.insert(analysis_id, size_expr.clone());
    }
    let producer = format!("%alloc_{}", analysis_id);
    let strategy = self.default_strategy();
    // 2026-07-27: If this Alloc# defaults to Arena, mark needs_arena.
    // Strategy may be refined later (Inline for ≤8, Alloca for bounded, etc.)
    // but the conservative initial marking catches all cases.
    if strategy == AllocStrategy::Arena {
        self.needs_arena = true;
    }
    self.result.insert(analysis_id, strategy);
    // ...
}
```

After `compute_reaching_allocs()` at line 166, also register the per-function result:

```rust
// 2026-07-27: Record per-function arena need for transitive propagation.
// After compute_reaching_allocs, mark function as needing arena if any
// allocation escapes to state/return AND was not resolved to Malloc/Inline.
for (id, strategy) in &self.result {
    if *strategy == AllocStrategy::Arena {
        self.needs_arena = true;
        break;
    }
}
```

#### 1c. New public API: `analyze_arena_need()`

**File:** `src/analysis/allocation.rs`

Add a new entry point function at the bottom (after line 327):

```rust
// 2026-07-27: Compute which transactions need arena initialization.
// Uses the per-function DAG analysis from analyze_alloc_strategies and
// propagates results transitively through the call graph.
//
// Returns a HashSet of transaction/function names that require arena init.
//
// Propagation rule: if function A calls function B, and B needs arena,
// then A also needs arena. Repeat until fixed point.
pub fn analyze_arena_need(items: &mut [TopLevel]) -> HashSet<String> {
    // Phase 1: Build call graph including defn edges.
    let mut cg = crate::analysis::call_graph::CallGraph::new();
    cg.build_from_program(items);

    // Phase 2: Run per-function allocation analysis to find direct arena needs.
    let mut direct_needs: HashSet<String> = HashSet::new();
    for item in items.iter_mut() {
        let name = match item {
            TopLevel::Transaction(txn) => Some(txn.name.clone()),
            TopLevel::Definition(defn) => Some(defn.name.clone()),
            _ => None,
        };
        if name.is_none() { continue; }
        let name = name.unwrap();

        let mut builder = DagBuilder::new(&mut 0usize, &mut HashMap::new(), false, false);
        let mut single_item = vec![item.clone()];
        // Walk just this item's body
        let top_ref = &mut single_item[0];
        match top_ref {
            TopLevel::Transaction(txn) => {
                builder.in_txn = true;
                builder.in_bounded = !matches!(txn.contract.post_condition, Expr::Bool(true));
                builder.walk_stmts(&mut txn.body);
            }
            TopLevel::Definition(defn) => {
                builder.in_txn = false;
                builder.in_bounded = false;
                builder.walk_stmts(&mut defn.body);
            }
            _ => {}
        }
        if builder.needs_arena {
            direct_needs.insert(name);
        }
    }

    // Phase 3: Propagate transitively through call graph.
    // If A calls B and B needs arena, A needs arena.
    let mut transitive_needs = direct_needs.clone();
    let mut changed = true;
    while changed {
        changed = false;
        for item in items {
            let name = match item {
                TopLevel::Transaction(txn) => Some(txn.name.clone()),
                TopLevel::Definition(defn) => Some(defn.name.clone()),
                _ => None,
            };
            let Some(name) = name else { continue };
            if transitive_needs.contains(&name) { continue; }

            // Check if any callee needs arena.
            if let Some(callees) = cg.edges_from(&name) {
                if callees.iter().any(|callee| transitive_needs.contains(callee)) {
                    transitive_needs.insert(name);
                    changed = true;
                }
            }
        }
    }

    transitive_needs
}
```

Note: The DAG building clones items because `analyze_alloc_strategies` takes `&mut [TopLevel]` and we must not interfere with its mutation. If `walk_stmts` only reads (does not mutate), change to `&[TopLevel]` instead.

#### 1d. Thread `needs_arena` through `CompilerContext`

**File:** `src/backend/llvm/context.rs`

Add to `CompilerContext` struct (around line 118):

```rust
// 2026-07-27: Set of function names that need arena initialization.
// Populated by analyze_arena_need before codegen. If empty for a given
// txn, emit_arena_init and arena fields in %State can be skipped.
pub needs_arena: HashSet<String>,
```

Initialize to empty in `CompilerContext::default()` (line 235):

```rust
needs_arena: HashSet::new(),
```

#### 1e. Gate arena field injection in `generate()`

**File:** `src/backend/llvm/mod.rs` at line 1786

Wrap the three field injections in a condition:

```rust
// 2026-07-27: Only inject arena fields if this program needs arena.
// The needs_arena set is populated by analyze_arena_need (transitive
// call-graph propagation). Benchmarks with no Alloc# calls skip these
// 3 fields, saving 24 bytes in %State and eliminating the malloc(64KB).
let txn_name = self.fun.txn_name.clone();
let needs_local_arena = self.ctx.needs_arena.contains(&txn_name);
if needs_local_arena {
    let aptr = self.ctx.field_index_map.len();
    self.ctx.field_index_map.insert("__arena_ptr".to_string(), aptr);
    // ... (existing code lines 1787-1804)
}
```

The `txn_name` field needs to be set on `FunctionContext` during transaction emission. Add it in `emit_transaction` / `emit_callable_txn`.

#### 1f. Gate `emit_arena_init()` / `emit_arena_fini()` calls

**File:** `src/backend/llvm/mod.rs` at line 1447

At the top of `emit_arena_init()`:

```rust
// 2026-07-27: Skip arena init if this function doesn't need arena.
let txn_name = self.fun.txn_name.clone();
if !self.ctx.needs_arena.contains(&txn_name) {
    return;
}
```

Same for `emit_arena_fini()` at line 1484.

**File:** `src/backend/llvm/dispatch.rs`

At the two call sites (lines 78 and 326), gate the arena init/fini calls:

```rust
// 2026-07-27: Only emit arena init/fini when this txn needs arena.
let needs_arena = self.ctx.needs_arena.contains(&self.fun.txn_name);
if needs_arena {
    self.emit_arena_init(out, indent);
}
```

**File:** `src/backend/llvm/emit_toplevel.rs`

At lines 1424 and 1495, wrap the `emit_arena_init()` call in the same guard.

#### 1g. Add `txn_name` to `FunctionContext`

**File:** `src/backend/llvm/context.rs` around line 150:

```rust
// 2026-07-27: Name of the transaction/function being compiled.
// Used to look up arena-need in CompilerContext.needs_arena.
pub txn_name: String,
```

Initialize in `FunctionContext::new()` (or wherever it's constructed):

```rust
txn_name: String::new(),
```

Set it in `emit_transaction` and `emit_callable_txn` after creating `FunctionContext`:

```rust
self.fun.txn_name = txn.name.clone();
```

#### 1h. Wire `analyze_arena_need` into the compilation pipeline

**File:** `src/backend/mod.rs` (or wherever `BackendAnalysis` is assembled)

Before `generate()` is called, run:

```rust
let needs_arena = crate::analysis::allocation::analyze_arena_need(&mut items);
backend.ctx.needs_arena = needs_arena;
```

Find the exact location by searching for where `analyze_alloc_strategies` is called or where `BackendAnalysis` is constructed.

#### 1i. Edge case: `needs_state_stores_in_body` does not imply arena need

The flag at `mod.rs:300` (`needs_state_stores_in_body`) controls whether per-iteration field values are written back to `%State` memory (for hybrid counter-phi + memory strategy). This is orthogonal to arena need — state stores don't allocate. Do NOT gate arena init on this flag.

#### 1j. Edge case: `Realloc#` and ring buffer operations

Search for `Realloc#` calls in the codebase. If any operation resizes the arena (grows via `realloc`), it implies arena need. The `analyze_arena_need` walk must also detect `Realloc#` calls:

In `walk_expr` at line 277, add:

```rust
Expr::Call(name, args, _) if name == "Realloc#" || name == "AllocArena#" => {
    self.needs_arena = true;
    for a in args.iter_mut() { self.walk_expr(a); }
}
```

---

## Fix 2 — ABI Type Coercion in `emit_direct_frgn_call`

### Problem

`emit_direct_frgn_call` at `src/backend/llvm/emit_expr.rs:1469-1488` converts arguments to match declared parameter types only for `Ptr` vs `int` patterns. All other type mismatches (float↔int, double↔int, integer width mismatches) pass through without conversion. At line 1487, the argument's own LLVM type is used in the `call` instruction, not the parameter's declared type.

This produces invalid IR like `call i64 @__print_int(float %t432)` where the LLVM verifier would (and llc will) produce garbage due to the ABI mismatch (float in XMM register, i64 expected in RDI).

### Changes

#### 2a. Add a general type coercion helper

**File:** `src/backend/llvm/emit_expr.rs`

Add a new function between line 1484 and 1485 (before the `arg_strs` construction):

```rust
// 2026-07-27: Coerce argument LLVM type to match declared parameter type.
// Emits the appropriate LLVM cast instruction (fptosi, sitofp, trunc, zext,
// bitcast) when the argument's SSA type differs from the parameter's LLVM type.
// This prevents ABI mismatches like call i64 @__print_int(float %x).
fn coerce_to_param_type(
    &mut self,
    out: &mut String,
    arg_reg: &TypedRegister,
    param_llvm_ty: &str,
    indent: &str,
) -> TypedRegister {
    let src_llvm = self.llvm_type(&arg_reg.ty);
    if src_llvm == param_llvm_ty {
        return arg_reg.clone();
    }

    // Determine Briev types for semantic conversion.
    // i64 ↔ float: bitcast via i32 bridge
    // i64 ↔ double: bitcast directly
    // ptr ↔ i64: inttoptr/ptrtoint
    // i64 ↔ iN (trunc/zext): for narrower integer params

    let result = self.fun.gen_reg();

    match (src_llvm.as_str(), *param_llvm_ty) {
        // float → i64: bitcast to i32, zext to i64
        ("float", "i64") => {
            let b32 = self.fun.gen_reg();
            writeln!(out, "{}  {} = bitcast float {} to i32", indent, b32, arg_reg.name).ok();
            writeln!(out, "{}  {} = zext i32 {} to i64", indent, result, b32).ok();
            TypedRegister { name: result, ty: Type::int() }
        }
        // double → i64: bitcast
        ("double", "i64") => {
            writeln!(out, "{}  {} = bitcast double {} to i64", indent, result, arg_reg.name).ok();
            TypedRegister { name: result, ty: Type::int() }
        }
        // i64 → float: trunc to i32, bitcast to float
        ("i64", "float") => {
            let tr = self.fun.gen_reg();
            writeln!(out, "{}  {} = trunc i64 {} to i32", indent, tr, arg_reg.name).ok();
            writeln!(out, "{}  {} = bitcast i32 {} to float", indent, result, tr).ok();
            TypedRegister { name: result, ty: Type::float() }
        }
        // i64 → double: bitcast
        ("i64", "double") => {
            writeln!(out, "{}  {} = bitcast i64 {} to double", indent, result, arg_reg.name).ok();
            TypedRegister { name: result, ty: Type::float64() }
        }
        // float ↔ double: fpext/fptrunc
        ("float", "double") => {
            writeln!(out, "{}  {} = fpext float {} to double", indent, result, arg_reg.name).ok();
            TypedRegister { name: result, ty: Type::float64() }
        }
        ("double", "float") => {
            writeln!(out, "{}  {} = fptrunc double {} to float", indent, result, arg_reg.name).ok();
            TypedRegister { name: result, ty: Type::float() }
        }
        // ptr ↔ i64: inttoptr/ptrtoint
        ("i64", "ptr") => {
            writeln!(out, "{}  {} = inttoptr i64 {} to ptr", indent, result, arg_reg.name).ok();
            // Keep original type but note the LLVM type changes
            TypedRegister { name: result, ty: arg_reg.ty.clone() }
        }
        ("ptr", "i64") => {
            writeln!(out, "{}  {} = ptrtoint ptr {} to i64", indent, result, arg_reg.name).ok();
            TypedRegister { name: result, ty: Type::int() }
        }
        // Integer widening: i8/i16/i32 → i64 (zext for unsigned, sext for signed)
        (src, "i64") if src.starts_with('i') && src.len() > 1 => {
            let bits: u32 = src[1..].parse().unwrap_or(64);
            if bits < 64 {
                writeln!(out, "{}  {} = zext {} {} to i64", indent, result, src, arg_reg.name).ok();
                TypedRegister { name: result, ty: Type::int() }
            } else {
                arg_reg.clone()
            }
        }
        // Integer narrowing: i64 → iN
        ("i64", dst) if dst.starts_with('i') && dst.len() > 1 => {
            writeln!(out, "{}  {} = trunc i64 {} to {}", indent, result, arg_reg.name, dst).ok();
            TypedRegister { name: result, ty: arg_reg.ty.clone() }
        }
        // Fallback: use a bitcast (may still be wrong, but preserves compilation)
        _ => {
            writeln!(out, "{}  {} = bitcast {} {} to {}", indent, result, src_llvm, arg_reg.name, param_llvm_ty).ok();
            TypedRegister { name: result, ty: arg_reg.ty.clone() }
        }
    }
}
```

#### 2b. Apply coercion in the argument formatting loop

Replace lines 1485-1488 in `emit_direct_frgn_call`:

```rust
// 2026-07-27: Coerce each argument to match the declared parameter type.
// This replaces the previous approach of using arg's own LLVM type, which
// produced ABI mismatches for float↔int, ptr↔i64, and width differences.
let arg_strs: Vec<String> = final_args
    .iter()
    .zip(sig.inputs.iter())
    .map(|(arg, (_, param_ty))| {
        let param_llvm = self.llvm_type(param_ty);
        let coerced = self.coerce_to_param_type(out, arg, &param_llvm, indent);
        format!("{} {}", param_llvm, coerced.name)
    })
    .collect();
```

Note: The `self.llvm_type(&reg.ty)` call at line 1487 used the register's type. The new code uses `param_llvm` (the declared parameter's LLVM type) and coerces the value to match. This is semantically correct — the frgn declaration specifies the expected C type, and we must cast the argument to match.

---

## Fix 3 — Print Plugin Float Type Inference

### Problem

`kind_from_expr` at `src/plugin/print_plugin.rs:139-151` defaults to `"Int"` for any expression that is not a literal (`Float`/`Decimal`/`Quoted`) or a named identifier with a known type annotation. Complex float-producing expressions like `x + 1.0` or function calls returning float get `__print_int`, which then triggers the ABI mismatch in Fix 2.

### Changes

#### 3a. Improve type inference in `kind_from_expr`

**File:** `src/plugin/print_plugin.rs`

Replace the `_ => "Int"` wildcard at line 150 with a recursive type inference:

```rust
// 2026-07-27: Infer print kind for complex expressions by analyzing the
// expression tree. Previously defaulted all non-trivial exprs to "Int",
// causing __print_int to be called on float values — an ABI mismatch.
_ => {
    // Walk the expression to find leaf types. If any leaf is a Float literal
    // or float-typed identifier, the expression produces a float.
    kind_from_expr_deep(expr, known_types)
}
```

Add a new helper:

```rust
// 2026-07-27: Recursive expression type inference for print dispatch.
// Walks BinaryOp/UnaryOp/Call trees to find leaf types. If any operand
// is a float literal or float-typed variable, the result is float.
fn kind_from_expr_deep(expr: &Expr, known_types: &HashMap<String, Type>) -> &'static str {
    match expr {
        Expr::Float(_) => "Float",
        Expr::Decimal(_) => "Int",
        Expr::Quoted(_) => "String",
        Expr::Identifier(name) => {
            match known_types.get(name) {
                Some(t) => kind_from_type(t),
                None => "Int",
            }
        }
        Expr::BinaryOp(_, lhs, rhs) => {
            let lk = kind_from_expr_deep(lhs, known_types);
            let rk = kind_from_expr_deep(rhs, known_types);
            // If either side is float, result is float (float arithmetic propagates).
            if lk == "Float" || rk == "Float" { "Float" }
            else { "Int" }
        }
        Expr::UnaryOp(_, e) => kind_from_expr_deep(e, known_types),
        Expr::Call(_, args, _) => {
            // Heuristic: check argument types. If any arg is float, result may be float.
            // This is conservative — some functions take float and return Int, but
            // for the print plugin, being wrong on the side of Float is safe (we'll
            // call __print_float instead of __print_int, which still prints the value).
            for arg in args {
                let ak = kind_from_expr_deep(arg, known_types);
                if ak == "Float" { return "Float"; }
            }
            "Int"
        }
        Expr::Cast(_, target) => kind_from_type(target),
        Expr::Field(_, _) | Expr::Index(_, _) => {
            // Field access / indexing — check known_types for the parent expression
            // For now, conservative: if we can't determine, check the inner expr.
            "Int"
        }
        _ => "Int",
    }
}
```

#### 3b. No change to `resolve_print` itself

The `resolve_print` function at line 178 remains structurally the same — it just gets better input from `kind_from_expr`.

---

## Fix 4 — Remove Hand-Rolled SLP Vector Emission

### Problem

`vector_codegen.rs` emits manual `insertelement`/`shufflevector`/`extractelement` chains for SLP isomorphic groups. This creates artificial dependency chains that block SROA from decomposing `%State` struct fields into SSA registers. Meanwhile, `hazard.rs` detects high cross-float density and sets `"disable-slp-vectorize"="true"` on these same functions (attribute `#4` at `mod.rs:3226`), meaning we get no benefit from either manual or automatic vectorization.

### Changes

#### 4a. Disable SLP vector emission in the loop engine

**File:** `src/backend/llvm/loop_engine/counter.rs`

At lines 615-637, the SLP vectorization dispatch in `emit_countable_body`:

```rust
// 2026-07-27: SLP vector emission disabled — the manual insertelement/
// extractelement chains create artificial dependencies that block SROA
// while LLVM's SLP vectorizer is simultaneously disabled (by hazard.rs).
// Instead, emit scalar code and let LLVM auto-vectorize naturally.
// When LLVM's SLP vectorizer sees flat scalar float operations, it can
// pack them into vector instructions on its own terms, with proper
// dependency analysis and alias information.
// Remove this block entirely. The vector_codegen module is kept for
// reference but not called.
```

Replace the entire `if let Some(ref group) = match_group { ... }` block with a fall-through to scalar emission:

```rust
// 2026-07-27: SLP vector emission disabled (see rationale below).
// Fall through to scalar emission for all groups — LLVM's auto-vectorizer
// handles this better than our manual insertelement chains.
// if let Some(ref group) = match_group {
//     let should_vec = ...  // REMOVED
// }
```

Delete the check and just fall through to the scalar `match stmt` at line 638.

#### 4b. Remove SLP hazard detection (optional but clean)

**File:** `src/backend/llvm/hazard.rs`

The SLP hazard detector that sets `disable-slp-vectorize` was a workaround for the manual vector code conflicting with LLVM's SLP. With manual vector code removed, the hazard detector is no longer needed. However, to minimize risk, keep it but invert the logic — only remove the attribute entirely (don't set `#4`/`#5` for any function). The cleanest approach:

In `mod.rs:3223-3231`, remove the conditional block that emits `#4` and `#5` attribute groups. All functions will use `#0` or `#3` regardless of SLP hazard.

```rust
// 2026-07-27: SLP hazard attribute variants removed — manual SLP vector
// emission is disabled (per Fix 4a), so there's no conflict with LLVM's
// auto-vectorizer. All functions use #0 or #3 without disable-slp.
// The hazard detection code is retained for future re-evaluation.
```

#### 4c. Keep `vector_codegen.rs` but mark as dormant

**File:** `src/backend/llvm/vector_codegen.rs`

Add a doc comment at the top:

```rust
// ── DORMANT: SLP Vector Codegen ──────────────────────────────────────
// 2026-07-27: This module is kept for reference but not actively called.
// Manual insertelement/extractelement chains created artificial dependencies
// that blocked SROA while hazard.rs simultaneously disabled LLVM's SLP
// vectorizer. The compiler now emits scalar code and lets LLVM auto-vectorize.
//
// Re-activate if: LLVM's auto-vectorizer produces worse code for a specific
// pattern than manual vectorization, AND we can emit vector code without
// disabling LLVM's own SLP pass.
```

#### 4d. Verify the impact on `slp_isomorphism.rs`

The SLP isomorphism analysis at `src/analysis/slp_isomorphism.rs` still runs and populates `slp_groups`. This is fine — we just don't use the result in codegen. The analysis itself is cheap and may be useful for future re-evaluation.

---

## Fix 5 — Relax `memory(readwrite)` on Main Loop

### Problem

`main()` functions use `attributes #3` and `#9` at `mod.rs:3219` and `mod.rs:3256-3258`, which include `memory(readwrite)`. This tells LLVM the function reads or write ANY memory (including global memory), which prevents SROA from promoting `%State` (a local `alloca`) to SSA registers. Even though `%state = alloca %State` is stack-local, `memory(readwrite)` causes the optimizer to conservatively assume the alloca may be accessed through any pointer.

### Changes

#### 5a. Give `main()` the `argmemonly` attribute

**File:** `src/backend/llvm/mod.rs`

Line 3256-3258, change `#9` from `memory(readwrite)` to `memory(argmem: readwrite)`:

```rust
// 2026-07-27: #9 = argmemonly variant for main functions.
// Main allocates %state = alloca %State and passes it as ptr %state to
// reactor_tick and each txn function. All memory access is through this
// argument pointer — no global memory (beyond @stdout for print FFI calls).
// argmem:readwrite lets LLVM SROA promote %State fields to SSA registers.
// Previously used memory(readwrite) which blocked SROA.
writeln!(out, "attributes #9 = {{").ok();
writeln!(out, "    nofree norecurse nosync nounwind memory(argmem: readwrite)").ok();
writeln!(out, "}}").ok();
```

#### 5b. Verify `nofree norecurse nosync` still applies

The `nofree` attribute means the function never calls `free`. For `main()`, this is true — arena teardown (`emit_arena_fini`) is in separate `@txn_` functions, not in `main()`. Verify this holds. If `main()` directly calls `free`, add a comment explaining the trade-off.

#### 5c. Impact on `@stdout` / `@stderr` global access

If `main()` writes to `@stdout` via `fprintf` calls (for `__print_int`), those are foreign function calls — the `fprintf` itself writes to the global, not `main()` directly. The `memory(argmem: readwrite)` on `main()` does not restrict callees. This is correct.

---

## Test Plan

### Behavioral Tests (Plan Directive #5)

Every feature must have tests that assert behavioral outcomes, not literal IR snapshots.

#### Test: Arena-by-proof

**File:** `src/backend/llvm/tests.rs` (or `src/analysis/allocation.rs` tests)

1. **No allocation benchmark**: Create a minimal `.bv` file with a `node` that has no `Alloc#` calls. Compile with `brievc`. Verify the generated `.ll` has no `call ptr @malloc(i64` in the main loop or txn functions.

2. **Arena-needed benchmark**: Create a `.bv` file with a `txn` that calls `Alloc#(64)`. Compile. Verify the generated `.ll` has `call ptr @malloc(i64 65536)` in the arena init path.

3. **Transitive propagation**: Create `txn A` that calls `defn B` which calls `Alloc#(64)`. Verify `txn A` has arena init.

4. **No transitive false positive**: Create `txn A` that calls `defn B` which calls `Alloc#(4)` (→ Inline). Verify `txn A` does NOT have arena init.

#### Test: ABI type coercion

**File:** `src/backend/llvm/tests.rs`

1. **float to __print_int**: Create a `.bv` file with `!Print(x)` where `x` is a float expression. Compile. Verify the `.ll` has `call i64 @__print_int(i64 ...)` with a `bitcast float %... to i32` / `zext i32 ... to i64` before the call.

2. **i64 to float frgn**: Create a `frgn my_func(x: Float)` and call it with an integer argument. Verify the `.ll` has a trunc+bitcast conversion.

#### Test: Print plugin float inference

**File:** `src/plugin/print_plugin.rs` tests

1. **Float binary op**: `!Print(x * 0.5)` where `x` is Int. Verify `kind_from_expr` returns `"Float"`.

2. **Int binary op**: `!Print(x + 1)` where `x` is Int. Verify `kind_from_expr` returns `"Int"`.

3. **Mixed binary op**: `!Print(x + 1.0)` where `x` is Int. Verify returns `"Float"`.

4. **Nested call**: `!Print(sqrt(x))` where `sqrt` returns float. Verify returns `"Float"`.

#### Test: Regression — run full benchmark suite

```bash
bash benchmarks/build_and_bench.sh --runtime
bash benchmarks/build_and_bench.sh --correctness
```

Compare against the pre-change baseline. Every benchmark must:
- Maintain or improve its ratio vs C
- Still be marked as MATCH (correctness preserved)
- No new MISMATCH entries

---

## Documentation

### Architecture docs to update

| Document | What to update |
|----------|----------------|
| `docs/architecture/memory-model.md` (create) | Document the Arena-By-Proof model: transitive call-graph propagation, how `needs_arena` is computed, what happens when no allocation is needed |
| `docs/architecture/txn-semantics.md` | Add section on arena lifecycle — init/reset/fini — and explain when the compiler skips it |
| `docs/architecture/backend-type-dispatch.md` | Add section on ABI coercion: how emit_direct_frgn_call converts argument types to match declared parameter types |

### Inline /// doc comments to add

| Location | Comment |
|----------|---------|
| `allocation.rs:analyze_arena_need` | `/// 2026-07-27: Compute transitive arena need across the call graph. Returns HashSet of function names needing arena init.` |
| `emit_expr.rs:coerce_to_param_type` | `/// 2026-07-27: Emit LLVM cast to coerce argument register to match declared parameter LLVM type. Handles float↔int, int width, ptr↔int conversions.` |
| `context.rs:needs_arena` | `/// 2026-07-27: Set of function names proven to need arena initialization. Populated pre-codegen by analyze_arena_need.` |
| `context.rs:txn_name` | `/// 2026-07-27: Current transaction/function name being compiled. Used as key for needs_arena lookup.` |
| `print_plugin.rs:kind_from_expr_deep` | `/// 2026-07-27: Recursive expression type inference for print dispatch. Walks expression trees to find float/string/int result types.` |

### Rationale comments at code sites (Plan Directive #2)

Every modified code site must have a `// 2026-07-27: <why>` comment. The plan above specifies exact comments for each change.

---

## Per-Commit Checklist

Before every commit:

1. `cargo test --lib` — all tests pass
2. `cargo build --release` — no warnings
3. Run benchmark baseline comparison:
   ```bash
   bash benchmarks/compare_baseline.sh <each_benchmark_name>
   ```
4. Update architecture docs if API contracts changed
5. Log bugs/gotchas in BUGS.md or `docs/architecture/praetor-log.md`
6. Run Praetor on new/changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6)
7. Verify `git grep` returns zero results for `Type::Custom.*==` in modified backend files

### Commit order

| Commit # | Files | Summary |
|----------|-------|---------|
| 1 | `allocation.rs`, `call_graph.rs` | Arena-by-proof: transitive call-graph + allocation analysis |
| 2 | `context.rs`, `mod.rs`, `dispatch.rs`, `emit_toplevel.rs` | Arena-by-proof: gate emission on needs_arena |
| 3 | `emit_expr.rs` | ABI type coercion in emit_direct_frgn_call |
| 4 | `print_plugin.rs` | Float type inference in print plugin |
| 5 | `counter.rs`, `vector_codegen.rs`, `hazard.rs`, `mod.rs` | Remove hand-rolled SLP vector emission + relax memory(readwrite) |
| 6 | `docs/architecture/` | Update architecture docs |

Each commit is individually testable and revertible.

---

## Anti-Patterns to Avoid

- **DO NOT** add `x == x` or synthetic exit fields to prevent DCE — fix the codegen, not the program
- **DO NOT** weaken existing optimization paths — additive match arms only, `_ => return None` must remain unchanged
- **DO NOT** hardcode type names (`"Int"`, `"Float"`) in backend code — use protocol membership (`is_protocol_member`)
- **DO NOT** add `#[allow(unused)]` or `#[allow(dead_code)]` — if code is unused after these changes, remove it or mark it with `_` prefix
- **DO NOT** commit `# TODO:` or `todo!()` — every feature must be fully wired

---

## Discovered Blockers (Implementation Review — 2026-07-27)

These issues were discovered during plan review and must be resolved during implementation. They would have blocked a previous agent from completing the plan.

### Blocker 1: Fix 1c — Temporary mutable references in `DagBuilder::new()`

**Plan line 165-166:**
```
let mut builder = DagBuilder::new(&mut 0usize, &mut HashMap::new(), false, false);
```

**Problem:** `&mut 0usize` and `&mut HashMap::new()` are mutable references to
temporaries. Rust's borrow checker rejects this — the temporary expires at
end-of-statement, leaving the `DagBuilder` with dangling mutable references.

**Fix:** Create named bindings for the temporaries:
```rust
let mut counter = 0usize;
let mut result_map = HashMap::new();
let mut builder = DagBuilder::new(&mut counter, &mut result_map, false, false);
```

### Blocker 2: Fix 1h — Wiring location unspecified

**Plan line 315:** "Find the exact location by searching for where
`analyze_alloc_strategies` is called or where `BackendAnalysis` is constructed."

**Problem:** `BackendAnalysis` does not exist — the analysis container is called
`AnalysisResults` (`src/backend/mod.rs:23`). The actual call site for
`analyze_alloc_strategies` is `src/compile.rs:475`. The pipeline flow is:

1. `compile.rs:475` — `analyze_alloc_strategies(&mut items)` returns `HashMap<usize, AllocStrategy>`
2. `compile.rs:562` — `codegen(items, ..., alloc_strategies, ...)` passes it to the backend
3. `compile.rs:881-909` — Builder pattern: `.with_alloc_strategies(alloc_strategies)` then `b.generate(items, None)`

**Correct approach:** Compute `analyze_arena_need` after `analyze_alloc_strategies` at
`compile.rs:475`, pass through a new parameter to `codegen()`, and add a builder
method `.with_needs_arena()` on `LlvmBackend` that stores into `CompilerContext.needs_arena`.

### Blocker 3: Fix 1a — Pre-existing `extract_called_transactions` bug

**Plan line 80:** "Rename `extract_called_transactions` to `extract_called_functions`"

**Problem:** The rename was already done — the function is named
`extract_called_functions` at `call_graph.rs:162`. However, the recursive self-call
at line 178 still uses the old name `extract_called_transactions(statements)`,
which does not exist anywhere in the codebase. This is a pre-existing compile error.

**Fix:** Change line 178 from `extract_called_transactions(statements)` to
`extract_called_functions(statements)`. The plan's Fix 1a should note this rather
than assuming a stale function name still exists.

### Blocker 4: Fix 2 — `*param_llvm_ty` deref in match arm

**Plan line 386:**
```rust
match (src_llvm.as_str(), *param_llvm_ty) {
```

**Problem:** If `param_llvm_ty` is `&str`, then `*param_llvm_ty` is `str` (unsized),
which cannot be used as a match pattern. The match arms compare against string
literals like `"i64"`, so the second element must be `&str`.

**Fix:** Use `(src_llvm.as_str(), param_llvm_ty)` and ensure match arms compare
against `&str` pattern (which work because string literal patterns match `&str`).

### Minor Issues

| Issue | Location | Detail |
|-------|----------|--------|
| Path mismatch | Fix 4 | `src/backend/llvm/counter.rs` should be `src/backend/llvm/loop_engine/counter.rs` |
| Fix 4 vague | Fix 4 | "Remove entire block" without concrete diff — actual removal is safe because fallthrough to scalar emission is the else path |
| Fix 4b vague | Fix 4b | "Remove conditional block that emits #4 and #5" without exact lines or replacement |
| Map determinism | Implicit | New `needs_arena` HashSet iteration in gating code isn't sorted — but `HashSet::contains()` is a single lookup, no iteration issue |
| `BackendAnalysis` wrong | Fix 1h | Struct is `AnalysisResults`, not `BackendAnalysis` |