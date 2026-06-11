<!-- 2026-06-11 -->

# Brief Compiler Glossary

| Term | Definition |
|------|------------|
| **Pattern B** | Struct-Variant Delegation architecture. Each AST construct is a struct in its own file with co-located parse/typecheck/eval/codegen. |
| **Feature file** | A file in `src/features/` containing one coherent language construct group, its struct definition, and all pass implementations. |
| **Router** | A thin dispatch function in a main pass file (e.g., `infer_expr` in `typechecker.rs`) that matches on an enum variant and delegates to the corresponding feature struct method. |
| **ExprDispatch** | A handle passed to Expr feature struct methods so they can recursively dispatch sub-expressions back through the router. |
| **StmtDispatch** | Handle for recursive sub-statement dispatch. |
| **Type-Universe** | Pass 1: collects/resolves/freezes type declarations. |
| **Praetor** | LSP enforcing complexity limits (cyclomatic ≤ 15, cognitive ≤ 15, lines ≤ 100, params ≤ 6, nesting ≤ 6). |
| **Feature struct** | Rust struct in a feature file representing one AST construct. Implements the relevant traits. |
| **DirectSSA** | Codegen strategy that emits a tight `while` loop with phi nodes instead of `@reactor_tick`. Used when no async/MMIO triggers exist. |
| **ReactorTick** | Codegen strategy that emits a `@reactor_tick` function for programs with async triggers or MMIO. |
| **compute_peak_live_floats** | Liveness-interval analysis for SLP hazard. Counts float temps whose def/last-use intervals overlap, not all float temps. |
| **loop_exit_label** | `Option<String>` on `LlvmBackend` (set to `"done"` in direct SSA loops). `term!` inside reactive loops emits `br %label` instead of `ret`, enabling LLVM loop unrolling. |
| **synthesize_init_txn** | `Program` method that collects all `TopLevel::Statement` items and wraps them in a synthesized `rct txn __init`. |
| **decompose_atomic_to_chars** | Converts Int/Float/Bool/Char to `Vec<char>` of their visual representation for Universal Bracket operations. |
| **reconstruct_from_chars** | Parses filtered `Vec<char>` back to the original atomic type. |
| **eval_mask_condition** | Evaluates a `BracketOp::Mask` expression against an item. Handles `Value::Bool`, `Value::Regex`, and `Value::String`. |
| **RegexPattern** | Compiled DFA from `analysis/dfa.rs`. Produced by `compile_to_dfa()`, consumed by `execute_dfa()` for O(n) zero-allocation regex matching. |
| **BracketOp::Mask** | Bracket operation that filters elements by evaluating a predicate. `_` is bound as the current element. Now supports Bool, Regex, and String (compiled to DFA on the fly). |
| **BracketOp::Coord** | Bracket operation for coordinate indexing into a value. On atomic types with a string expression, desugars to a regex filter. |
| **BracketOp::Stride** | Bracket operation that takes every Nth element. |
| **Prior-state semantics** | In `rct txn` bodies, all reads see the state as it was at the BEGINNING of the tick. Chained `&field = ...` does NOT accumulate within a tick. |
| **Equality Saturation** | 5-pass fixpoint rewrite engine with 9 rules (add-zero, mul-one, sub-self, etc.) that runs before codegen. |
| **SLP hazard** | Analysis that estimates register pressure from SLP vectorization candidates and disables SLP when spills would occur. |
| **Precomputation budget** | `--optimize-budget` flag (default 256). Programs with all-const inputs are fully precomputed up to this budget. `--prod` sets budget to `u64::MAX`. |
| **TermBang (term!)** | Program exit statement. With swan song: `term! -> fn();`. In reactive loops, emits `br %done` (not `ret`), enabling LLVM loop unrolling. |
