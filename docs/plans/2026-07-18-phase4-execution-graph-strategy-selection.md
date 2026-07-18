# Phase 4: Execution-Graph → Allocation Strategy Selection

**Date:** 2026-07-18
**Status:** Plan — pre-implementation
**Depends on:** Phase 2 (Alloc# intrinsic, AllocStrategy enum, strategy-aware Free#) — DONE
**See also:**
  - `docs/plans/2026-07-18-allocation-strategy-system.md` (Phase 4 section, §598–734)
  - `docs/plans/2026-07-18-ptr-level3-borrow-checking.md` (provenance tracking for escape analysis refinement)
  - `docs/plans/2026-07-18-string-encoding-alloc-and-provenance.md` (fat pointer provenance for Ptr<T>)

---

## Executive Summary

Currently `emit_alloc` **guesses** the allocation strategy at codegen time:

```
arena active?              → Arena bump (Strategy 1)
bounded + no escape?       → Alloca (Strategy 2)
otherwise                  → Malloc (Strategy 3)
```

The guess is wrong in two cases:

1. **Arena is active but result escapes the txn** → should be `Malloc`, currently picks `Arena` (dangling pointer on arena reset)
2. **Bounded scope but result escapes** → should be `Malloc`, currently picks `Alloca` (use-after-free on scope exit)

Fix: a pre-codegen analysis pass (`src/analysis/allocation.rs`) walks the AST, finds every `Alloc#()` call site, determines the correct strategy by tracing whether the result escapes the allocating scope, and annotates each call with `AllocStrategy`. Codegen reads the annotation instead of guessing.

### Design decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Pass runs before codegen | Pre-codegen AST walk | Codegen is not the right place for escape analysis |
| Output is `HashMap<String, AllocStrategy>` | Keyed by a stable expression ID | Expression hash would work but expression IDs are already available |
| Escape = stored to %State or returned | Conservative check | Traces identifiers through `let`, `Assign`, `Guard`, `Block`, `Term` |
| `is_in_bounded_scope` check stays | Codegen fallback | Analysis pass might not be available (`--no-analysis`) |
| No interprocedural analysis in v1 | `defn` boundaries treated as escape | Simplifies v1; call-graph tracing is a future refinement |

---

## Files

### New files

| File | Purpose |
|------|---------|
| `src/analysis/allocation.rs` | The analysis pass |

### Modified files

| File | Change |
|------|--------|
| `src/analysis/mod.rs` | Register `pub mod allocation` |
| `src/compile.rs` | Run analysis pass before codegen |
| `src/backend/llvm/intrinsics.rs` | `emit_alloc` reads analysis output, emits info message on promotion |
| `src/backend/llvm/mod.rs` | Add `fn current_expr_id(&self)` or equivalent for codegen-to-analysis lookup |

---

## Implementation

### Step 1: Expression ID annotation

The analysis pass needs a way to associate analysis results with codegen call sites. The simplest mechanism: **assign a unique expression ID to every `Alloc#()` call in the AST before analysis runs**.

**File:** `src/analysis/allocation.rs`

```rust
/// 2026-07-18: Expression ID annotation for Alloc# call sites.
/// Walks all TopLevel items and assigns a unique usize ID to
/// every Expr::Call("Alloc#", ...). Stores the mapping in the
/// TypeUniverse or a separate HashMap on CompilerContext.
pub fn annotate_alloc_sites(items: &mut [TopLevel]) -> HashMap<usize, &mut Expr> {
    let mut ids = HashMap::new();
    let mut counter = 0usize;
    for item in items.iter_mut() {
        walk_exprs(item, &mut |e: &mut Expr| {
            if let Expr::Call(name, _) = e {
                if name == "Alloc#" {
                    let id = counter;
                    counter += 1;
                    ids.insert(id, e as *mut Expr);  // or store index path
                }
            }
        });
    }
    ids
}
```

**Alternate approach (no raw pointers):** Add an `analysis_id: Option<usize>` field to `Expr::Call`. The parser/resolver sets this to `None`, the analysis pass assigns it.

```rust
// In ast/types.rs or ast/mod.rs:
pub enum Expr {
    Call(String, Vec<Expr>, Option<usize>),  // +analysis_id
    // ... existing variants
}
```

This is cleaner but touches every `Expr::Call` construction site. The number of such sites is manageable — I'll use a search-and-replace approach.

Since we're going for clean code (max 2 nesting, no arrow code), the `analysis_id` field on `Expr::Call` is the right choice:

```rust
// Before:
Expr::Call(name, args)
// After:
Expr::Call(name, args, analysis_id)
```

**Update pattern:** Every `Expr::Call` construction changes from:

```rust
Expr::Call("foo".to_string(), vec![arg1, arg2])
```

to:

```rust
Expr::Call("foo".to_string(), vec![arg1, arg2], None)
```

This is mechanical — most sites are in tests and parser code. The normalizer, desugarer, and other passes that construct `Expr::Call` just pass `None`.

### Step 2: The analysis pass

**File:** `src/analysis/allocation.rs`

```rust
/// 2026-07-18: Analyze Alloc# call sites and determine optimal
/// allocation strategy. Runs before codegen. The analysis:
///
/// 1. Find all Expr::Call("Alloc#", _, analysis_id) in the program
/// 2. For each, determine if the result escapes the txn scope
/// 3. Assign AllocStrategy based on scope + escape analysis
///
/// Escape definition: the Alloc# result is stored to a %State field
/// (via field_index_map), stored to a top-level let-binding that
/// outlives the txn, or returned from the txn/defn.
///
/// Conservative: if escape analysis is uncertain, choose Malloc.
pub fn analyze_alloc_strategies(
    items: &[TopLevel],
    field_index_map: &HashMap<String, usize>,
) -> HashMap<usize, AllocStrategy> {
    let mut results = HashMap::new();
    for item in items {
        let ctx = AnalysisContext::new(item, field_index_map);
        ctx.collect_sites(&mut results);
    }
    results
}
```

The `AnalysisContext` struct:

```rust
struct AnalysisContext<'a> {
    _item: &'a TopLevel,
    field_index_map: &'a HashMap<String, usize>,
    /// True if we're inside a txn body (arena scope).
    in_txn: bool,
    /// True if we're inside a bounded loop body.
    in_bounded: bool,
}
```

Detection:

```rust
fn detect_scope(item: &TopLevel) -> (bool, bool) {
    match item {
        TopLevel::Transaction(txn) => {
            let bounded = !txn.postcondition.is_empty()
                || txn.precondition.iter().any(|(expr, _)| is_counting_precondition(expr));
            (true, bounded)
        }
        TopLevel::Definition(_) => (false, false),
        TopLevel::Reactive(_) => (true, false),
        _ => (false, false),
    }
}
```

Escape detection walks the expression tree from the `Alloc#` call forward:

```rust
fn trace_escape(
    alloc_id: usize,
    stmts: &[Statement],
    field_index_map: &HashMap<String, usize>,
) -> bool {
    for stmt in stmts {
        match stmt {
            Statement::Let { name, expr: Some(e), .. } => {
                if contains_alloc(e, alloc_id) {
                    // The alloc result is bound to `name`.
                    // Trace whether `name` escapes in subsequent statements.
                    if trace_name_escapes(name, stmts, field_index_map) {
                        return true;
                    }
                }
            }
            Statement::Assign(Expr::Identifier(name), rhs) => {
                if contains_alloc(rhs, alloc_id) {
                    // Assigned to a state field = escape
                    if field_index_map.contains_key(name) {
                        return true;
                    }
                }
            }
            Statement::Return(Some(expr)) => {
                if contains_alloc(expr, alloc_id) {
                    return true; // returned from txn = escape
                }
            }
            Statement::Guarded(guard, body) => {
                if trace_escape(alloc_id, &[guard.clone()], field_index_map)
                    || trace_escape(alloc_id, body, field_index_map)
                {
                    return true;
                }
            }
            Statement::Block(body) => {
                if trace_escape(alloc_id, body, field_index_map) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}
```

### Step 3: Wire into compile pipeline

**File:** `src/compile.rs`

```rust
// After type checking, before codegen:
if !opts.no_analysis {
    let alloc_strategies = analysis::allocation::analyze_alloc_strategies(
        &program, &ctx.field_index_map,
    );
    backend.set_alloc_strategies(alloc_strategies);
}
```

The `CompilerContext` holds the analysis results:

```rust
// In LlvmBackend or CompilerContext:
pub analysis_alloc_strategies: Option<HashMap<usize, AllocStrategy>>,
```

### Step 4: Codegen reads analysis output

**File:** `src/backend/llvm/intrinsics.rs`

In `emit_alloc`, before the triple-dispatch fallthrough:

```rust
fn emit_alloc(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let size = emit_arg(backend, out, &args[0], indent);

    // Read analysis output — the analysis_id from the Expr::Call
    let call_expr = &args[0]; // Hmm, need to get the caller's analysis_id
    // ...

    // Actually, analysis_id is on the Call itself, not on args.
}
```

Wait — the analysis pass assigns IDs to `Expr::Call` nodes, but `emit_alloc` receives `args: &[Expr]`, not the full `Expr::Call`. I need to pass the `analysis_id` somehow.

**Approach:** Add an `analysis_id: Option<usize>` parameter to `emit_alloc`:

```rust
"Alloc#" => {
    let analysis_id = if let Expr::Call(_, _, aid) = &args[0] {
        *aid  // No, args[0] is the first argument, not the Call itself
    } else {
        None
    };
}
```

Actually, let me re-think. In `emit_intrinsic_call`, the match arm looks like:

```rust
"Alloc#" => return emit_alloc(backend, out, v, args, indent),
```

where `args` are the arguments to the Call (the `Vec<Expr>` part). The `analysis_id` is on the `Expr::Call` itself. I need to either:

1. **Pass it as a separate parameter** to `emit_alloc`: `emit_alloc(backend, out, v, args, analysis_id, indent)`
2. **Use a side-channel**: map `v` (the result register name) to `analysis_id` before calling `emit_alloc`

Option 1 is cleaner:

```rust
// In emit_intrinsic_call:
"Alloc#" => {
    let e = &args[0]; // No, I don't have the Call expr here
}
```

Wait, `emit_intrinsic_call` receives `name: &str` and `args: &[Expr]`. It doesn't receive the `Expr::Call` struct itself. I need to pass the `analysis_id` from the caller.

Let me look at how `emit_intrinsic_call` is called:

```rust
// In emit_expr.rs:
Expr::Call(name, args) => {
    let reg = emit_intrinsic_call(backend, out, v, &name, args, indent);
    // ...
}
```

With the new `analysis_id` field:

```rust
Expr::Call(name, args, analysis_id) => {
    let reg = emit_intrinsic_call(backend, out, v, &name, args, analysis_id, indent);
    // ...
}
```

Then in `emit_alloc`:
```rust
fn emit_alloc(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], analysis_id: Option<usize>, indent: &str,
) -> BTypedRegister {
    // Read pre-computed strategy from analysis
    if let Some(aid) = analysis_id {
        if let Some(strategy) = backend.get_analysis_strategy(aid) {
            return emit_alloc_by_strategy(backend, out, v, &size, strategy, indent);
        }
    }
    // Fallback: triple dispatch (existing)
    // ...
}
```

This is a bigger signature change but it's the clean path. Since we're already touching every `Expr::Call` site for the `analysis_id` field, adding one more parameter to `emit_intrinsic_call` is proportional.

### Step 5: Promotion info messages

When the analysis picks a strategy that differs from the default:

```rust
fn emit_info_on_promotion(
    backend: &LlvmBackend, out: &mut String,
    indent: &str, size: &str, strategy: AllocStrategy,
) {
    let default = if backend.fun.arena_slots.is_some() {
        AllocStrategy::Arena
    } else if backend.is_in_bounded_scope() {
        AllocStrategy::Alloca
    } else {
        AllocStrategy::Malloc
    };
    if strategy == AllocStrategy::Malloc && default != AllocStrategy::Malloc {
        let msg = format!(
            "Alloc#({}) promoted to heap — result escapes txn scope",
            size
        );
        eprintln!("info: {}", msg);
        writeln!(out, "; info: {}", msg).ok();
    }
}
```

Called in `emit_alloc_by_strategy` before emitting.

---

## Testing

| Test | What it asserts | How |
|------|-----------------|-----|
| `test_alloc_auto_strategy_txn` | Alloc# inside txn → analysis picks Arena | Run analysis pass, check strategy annotation |
| `test_alloc_auto_strategy_defn` | Alloc# in defn → analysis picks Malloc | Run analysis pass, check strategy annotation |
| `test_alloc_escapes_to_heap` | Alloc# result stored to state → analysis picks Malloc | Run with state field write, assert annotation is Malloc |
| `test_alloc_no_escape_stays_arena` | Alloc# local-only → analysis picks Arena | Run with let-binding only, assert Arena |
| `test_alloc_analysis_id_survives_normalizer` | analysis_id is preserved through normalization | Normalize a program, check Expr::Call has correct analysis_id |
| `test_alloc_info_message` | Promotion emits stderr + IR comment | Capture stderr, check "promoted to heap" |
| `test_alloc_no_info_no_promotion` | No promotion → no message | Run with non-escaping Alloc#, check no message |

---

## Flat control flow enforcement

Every function in the analysis pass must follow max 2 nesting depth. Specifically:

- `trace_name_escapes` — use `?`, `if let`, guard clauses
- `trace_escape` — walk statements with `?`, extract early returns for block/guarded recursion
- `contains_alloc` — simple recursive expression walk with `?` and guard clauses
- `detect_scope` — two booleans, flat if/return chains

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **analysis_id field on Expr::Call breaks pattern matches** | Certain | Medium | Mechanical update — search and replace for `Expr::Call(name, args)` → `Expr::Call(name, args, None)`. The `unused` lint catches misses. |
| **Escape analysis misses indirect escape via pointer** | Medium | Medium | Conservative: missed escape falls through to Malloc. The Ptr Level 3 provenance tracking (future) fixes this. |
| **Analysis pass costs compile time** | Low | Low | Single linear AST walk — negligible compared to type checking and codegen. |
| **Analysis and codegen disagree on scope** | Low | High | Analysis determines scope from AST. Codegen reads scope from `arena_slots`/`is_static_bound`. These must agree — defined by the same code path. If they disagree, codegen's fallback triple dispatch produces correct (conservative) output. |
