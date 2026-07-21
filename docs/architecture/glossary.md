<!-- 2026-06-18 -->

# Brief Compiler Glossary

| Term | Definition |
|------|------------|
| **Pattern B** | Struct-Variant Delegation architecture. Each AST construct is a struct in its own file with co-located parse/typecheck/eval/codegen. |
| **Feature file** | A file in `src/features/` containing one coherent language construct group, its struct definition, and all pass implementations. |
| **Router** | A thin dispatch function in a main pass file (e.g., `infer_expr` in `typechecker.rs`) that matches on an enum variant and delegates to the corresponding feature struct method. |
| **ExprDispatch** | A handle passed to Expr feature struct methods so they can recursively dispatch sub-expressions back through the router. |
| **StmtDispatch** | Handle for recursive sub-statement dispatch. |
| **Type-Universe** | Pass 1: collects/resolves/freezes type declarations. |
| **Lens Operator** | A collective term for `<:` (Derivation) and `:>` (Projection). These operators establish the relationship between a raw bit layout and its high-level meaning. `<:` restricts what can conform to a type (derivation); `:>` reveals meaning through a semantic lens (projection). |
| **Derivation Operator (`<:`)** | Defines a type as a structural subset of another: `type Email <: String`. Constrains the value space — only valid bit patterns are accepted. A Lens Operator. |
| **Projection Operator (`:>`)** | Extracts compile-time-known metadata or applies a type-defined lens: `list :> Size`. Reveals meaning from a bit layout. A Lens Operator. |
| **Partition Operator** | Bracket syntax `[]` and bit-anchor `@/` that partition a memory layout into addressable sub-ranges. `list[3]` selects the 4th element; `bits @/0..3` selects bits 0-3. |
| **Transfer Operator** | The arrow syntax `<-` for directional data movement — pushes, pops, and transfers values across layout boundaries. `&list <- x` pushes x into list. |
| **Anchor (`@`)** | The universal symbol of spatial/temporal location. Anchors a value to a position: `@"..."` (string literal to memory slot), `@/N..M` (field to bit position), `@ link` (timer to hardware resource), `@x` (prior-state reference). |
| **Praetor** | LSP enforcing complexity limits (cyclomatic ≤ 15, cognitive ≤ 15, lines ≤ 100, params ≤ 6, nesting ≤ 6). |
| **Feature struct** | Rust struct in a feature file representing one AST construct. Implements the relevant traits. |
| **DirectSSA** | Codegen strategy that emits a tight `while` loop with phi nodes instead of `@reactor_tick`. Used when no async/MMIO triggers exist. |
| **ReactorTick** | Codegen strategy that emits a `@reactor_tick` function for programs with async triggers or MMIO. |
| **compute_peak_live_floats** | Liveness-interval analysis for SLP hazard. Counts float temps whose def/last-use intervals overlap, not all float temps. |
| **loop_exit_label** | `Option<String>` on `LlvmBackend` (set to `"done"` in direct SSA loops). `term!` inside reactive loops emits `br %label` instead of `ret`, enabling LLVM loop unrolling. |
| **synthesize_init_txn** | `Program` method that collects all `TopLevel::Statement` items (top-level executable statements) and wraps them in a synthesized `node __init [!__booted_N][__booted_N]` that fires once at startup. Called after import resolution in `run_llvm_compile`, `run_check`, and `run_rbv`. The `!__booted_N` / `booted = 1` pattern avoids counter-loop folding paths (correct for a one-shot init). Subject to normal precomputation: pure const scripts may be precomputed; FFI calls emit a runtime loop. |
| **decompose_atomic_to_chars** | Converts Int/Float/Bool/Char to `Vec<char>` of their visual representation for Universal Bracket operations. |
| **reconstruct_from_chars** | Parses filtered `Vec<char>` back to the original atomic type. |
| **eval_mask_condition** | Evaluates a `BracketOp::Mask` expression against an item. Handles `Value::Bool`, `Value::Regex`, and `Value::String`. |
| **RegexPattern** | Compiled DFA from `analysis/dfa.rs`. Produced by `compile_to_dfa()`, consumed by `execute_dfa()` for O(n) zero-allocation regex matching. |
| **BracketOp::Mask** | Bracket operation that filters elements by evaluating a predicate. `_` is bound as the current element. Now supports Bool, Regex, and String (compiled to DFA on the fly). |
| **BracketOp::Coord** | Bracket operation for coordinate indexing into a value. On atomic types with a string expression, desugars to a regex filter. |
| **BracketOp::Stride** | Bracket operation that takes every Nth element. |
| **Prior-state semantics** | In `node` bodies, all reads see the state as it was at the BEGINNING of the tick. Chained `field = ...` does NOT accumulate within a tick. |
| **Equality Saturation** | 5-pass fixpoint rewrite engine with 9 rules (add-zero, mul-one, sub-self, etc.) that runs before codegen. |
| **SLP hazard** | Analysis that estimates register pressure from SLP vectorization candidates and disables SLP when spills would occur. |
| **Precomputation budget** | `--optimize-budget` flag (default 256). Programs with all-const inputs are fully precomputed up to this budget. `--prod` sets budget to `u64::MAX`. |
| **TermBang (term!)** | Program exit statement. With swan song: `term! -> fn();`. In reactive loops, emits `br %done` (not `ret`), enabling LLVM loop unrolling. |
| **ReactiveTransaction** | Struct representing a single reactive transaction in the reactor. Contains `name`, `contract`, `body`, `is_async`, `reactor_speed`, `dependencies`. Built from `TopLevel::Transaction` at reactor construction time. |
| **Reactor** | Event-driven execution engine for reactive transactions. Maintains `transactions` list, `dirty_preconditions` set, `dependency_map`, `triggers` map, and `last_fired` timestamps. |
| **fire_due_async_txns** | Reactor method that fires transactions whose `@Hz` interval has elapsed. Checks `is_async && reactor_speed.is_some()`, compares `last_fired[idx]` elapsed against `1000/hz` ms. Only fires in continuous mode. |
| **run_reactor_continuous** | Event-driven loop that interleaves responsive convergence (Reactor::run) with polled async firing (fire_due_async_txns). Sleeps 1ms between cycles. Used when any transaction has `is_async = true` or `reactor_speed` is set. |
| **Responsive transaction** | `node` — fires immediately when its preconditions become dirty. No tick, no timer. Existing convergence loop behavior. |
| **Polled transaction** | `rct async txn @NHz` — fires at N Hz regardless of dirty state. The `@Hz` annotates a timer-backed trigger. Pre/post conditions still enforced. |
| **Event-driven trigger** | `trg name: Type @ link <ffi_fn>` — FFI-backed trigger. When the linked FFI function returns a non-void value, the trigger variable is marked dirty and convergence runs. |
| **`pvt`** | Struct-private visibility keyword. Fields marked `pvt` are accessible only from within the struct's own transactions and definitions. Enforcement stubbed (needs `current_struct` tracking). |
| **`sed`** | File-private visibility keyword. Top-level `sed` items cannot be imported from other files. Struct fields marked `sed` trigger a `TypeMismatch` error when accessed from a different file. |
| **`<:`** (struct derivation) | Single-inheritance syntax: `struct Child <: Parent { extra_field; };`. Parent fields are flattened into the child at the desugarer level. Upcast (child → parent) is implicitly allowed by the typechecker. |
| **Struct derivation** | The mechanism by which a struct inherits fields from a parent struct. Resolved recursively (chain inheritance supported). Field name collisions produce a compile error. |
| **Upcast** | Implicit type conversion from a derived struct to its parent type. Validated by `is_derived_from()` which walks the parent chain. |
| **Call argument type checking** | Phase 1 feature. `check_call_argument_types()` validates argument types against parameter types at call sites for `defn`/`txn`/`sig`/`frgn` targets. Unknown functions are silently skipped. |
| **Bits Thesis** | The core language philosophy: every type is a lens over `Bits`. Type operators (`<:`, `:>`, `@/`, `[]`, `<-`) are spatial layout operations, not nominal abstractions. See `lib/std/from-bits.bv`. |
| **Fast Path** | Compile-time shape recognition that bypasses generic projection evaluation. When a `UserDefinedWithArg("Add", rhs)` matches a known (type, operator) pair, the backend emits native `add i64` instead of interpreting the binding expression. |
| **Silent Defaults** | Metadata properties like `Bytes`, `Alignment`, `Endian` that the compiler infers from the `@/` bit range. Users don't need to write them for standard types. |
| **Lazy Projection** | A projection whose cost is deferred until explicitly queried. E.g., `CString :> Size` runs `strlen` only when `Size` is queried; `CString :> At(i)` never calls strlen. |
| **#export** | Sig modifier that marks a transaction for cross-language export. The backend emits a globally-visible symbol with the target language's calling convention and name mangling. |
| **Autogenous Binding** | Auto-generated FFI headers (`.h`, Rust crate, Python stub) produced by the compiler alongside the `.o` library, derived from `ResolvedType` layout info. |

(End of file - total 38 lines)
