# Bugs & Mistakes Log

## Format
- **Date**: YYYY-MM-DD
- **Issue**: What happened
- **Root Cause**: Why it happened
- **Fix**: How it was resolved
- **Lesson**: How to avoid next time

## 2026-05-28 — Overriding `from "..."` location in typechecker

**Issue**: `__read_file` FFI call failed with `location: <profile:__read_file>` instead of `"std::fs::read_to_string"`.

**Root Cause**: Typechecker at `src/typechecker.rs:993` unconditionally overwrote `signature.location` with `<profile:{name}>` when `toml_path` was empty, even when the `from "..."` clause had already set a correct location. The parser correctly parsed `from "std::fs::read_to_string"` but the typechecker replaced it with a profile placeholder.

**Fix**: Only set the profile placeholder when `signature.location` is empty:
```diff
- signature.location = format!("<profile:{}>", name);
+ if signature.location.is_empty() {
+     signature.location = format!("<profile:{}>", name);
+ }
```

**Lesson**: Always check before overwriting. The "new FFI syntax" (direct `from "..."`) and "old profile FFI" coexist — the typechecker assumed no `from` clause existed.

## 2026-05-28 — Adding built-in string matches for stdlib functions

**Issue**: Added `is_digit`, `is_alpha`, `is_alphanumeric`, `is_upper`, `is_lower`, `is_space`, `char_to_string` as Rust string-match built-ins in the interpreter.

**Root Cause**: When `UndefinedForeignFunction("is_digit")` appeared, instead of adding `import char from "std/char.bv"` to the calling `.bv` file, I added a Rust string match in `Expr::Call` handler. Also pre-populated `None`/`Some` enum constants in `Interpreter::new()`.

**Fix**: Reverted all built-in hacks. Added `import char from "std/char.bv"` to `lib/compiler/lexer.bv`.

**Lesson**: When the interpreter can't find a function, check if it's in the standard library first. The standard library IS the dependency source. Never add Rust string-match built-ins for things the standard library provides.

## 2026-05-28 — Typechecker overwrites `from` location

**Issue**: `from "std::fs::read_to_string"` in `lib/std/io.bv` was being parsed correctly but then overwritten by typechecker.

**Root Cause**: The typechecker's `load_binding_for_frgn` function set `location = format!("<profile:{}>", name)` unconditionally when `toml_path` was empty, discarding the parser-provided location from the `from "..."` clause.

**Fix**: Added `if signature.location.is_empty()` guard before overwriting.

**Lesson**: Multiple FFI resolution paths coexist (profile-based + direct `from`). Don't assume one path overrides the other.

## 2026-05-28 — Contract-after-arrow parser bug

**Issue**: `-> Type [pre][post]` syntax caused both pre and post conditions to parse as `Expr::Bool(true)`.

**Root Cause**: The while loop in `parse_contract` (line 2879) correctly consumed `[`, but `parse_expression()` returned `BoolTrue` instead of the bracket contents, indicating a lexer state issue specific to the contract-after-arrow code path.

**Workaround**: Use contract-before-arrow syntax `[pre][post] -> Type`.

**Lesson**: The contract-after-arrow path has a subtle lexer/parser interaction bug that's not yet fully understood. Always prefer contract-before-arrow.

## 2026-05-28 — Keyword tokens can't appear in any variable position

**Issue**: `txn`, `reg`, `from` used as variable names caused parse failures across multiple `.bv` files.

**Root Cause**: 44 keyword tokens in the lexer were not accepted by `expect_identifier()` or `parse_primary_expr()`, causing keywords to fail as parameter names, `let` bindings, and `uni` pattern variables.

**Fix**: Route B: Added all 44 missing keywords to `expect_identifier()` and the `parse_primary_expr()` fallback path. Also fixed 3 `while let Some(Ok(Token::Identifier(_)))` loops to use `expect_identifier()` instead.

**Lesson**: The lexer defines ~60 keyword tokens but only 22 were handled as identifiers. When dealing with keyword-as-identifier issues, fix the parser, not the `.bv` files.

## 2026-05-28 — \u{D800} surrogate fails char::from_u32

**Issue**: `'\u{D800}'` in `lib/std/char.bv` caused parse error because logos `regex` callback returned `None` for surrogates, interpreted as a lex error.

**Root Cause**: UTF-16 surrogates (0xD800-0xDFFF) aren't valid Unicode scalar values. `char::from_u32(cp)` returns `None` for them. Logos interprets a `None` return from a regex callback as "token didn't match" (the `?` operator propagates `None` as a failed match).

**Fix**: Changed `char::from_u32(cp)` to `char::from_u32(cp).unwrap_or('?')` in the `\u{...}` escape handler.

**Lesson**: Logos regex callbacks must never return `None` for valid lexer input. Handle all failure modes of `from_u32`, including surrogates.

## 2026-05-28 — Unevaluated enum constructors (None, Some, Ok, Err)

**Issue**: Interpreter returned `UndefinedVariable("None")` and `UndefinedForeignFunction("Err")`.

**Root Cause**: After removing magic constants from `Interpreter::new()`, `load_program` didn't register enum variant names from declarations. `Result` is also an intrinsic type — no `.bv` `enum Result { Ok, Err }` declaration exists to load from.

**Fix**: 
1. `load_program` now iterates `TopLevel::Enum` and registers each variant as `Value::Enum` in state
2. `Ok`/`Err` handled as special intrinsic enum constructors in `Expr::Call` handler
3. `is_ok`, `is_err`, `unwrap`, `unwrap_err` added as Result methods in interpreter

**Lesson**: Enum constructors must be loaded from actual declarations. Intrinsic types (Result, Option) need explicit handling since they lack `.bv` enum declarations.

## 2026-05-29 — Method-call `x.foo(y)` drops all arguments except receiver

**Issue**: `UndefinedVariable("s")` when calling `output.append_str("...")`.

**Root Cause**: The parser's `parse_postfix` function (lines 4314, 4385) parsed arguments from `x.foo(y, z)` into `args` but then constructed `Expr::Call(member_name, vec![expr])` — dropping all parsed arguments and passing only the receiver `x`. The receiver was passed as `expr` which is the preceding expression. The `let mut args = Vec::new()` on the line above was parsed but the populated `args` vector was never used in the `Expr::Call` construction.

```rust
// Line 4385 (BUGGY):
expr = Expr::Call(member_name, vec![expr]);
// Should be:
let mut call_args = vec![expr];
call_args.extend(args);
expr = Expr::Call(member_name, call_args);
```

**Fix**: Changed both locations to prepend receiver `expr` to the parsed `args` vector before constructing `Expr::Call`.

**Lesson**: Mental model matched the intent ("prepend receiver to args") but code used `vec![expr]` which discarded args. Always verify that all populated variables are actually consumed, especially when refactoring from a simpler implementation.

## 2026-05-29 — Term statement inside nested blocks doesn't propagate return value

**Issue**: `compile_file` returned `Value::Void` even though `term Ok(output)` was reached inside a `uni` block.

**Root Cause**: The `call_defn` function (line ~302) handled `Statement::Term` at the top level by capturing the result, but for statements inside nested blocks (unifications, guarded blocks, etc.), `exec_stmt` processed the `Term` and **discarded the value**:

```rust
// exec_stmt (line ~462):
Statement::Term { values: outputs, .. } => {
    if let Some(first) = outputs.first() {
        if let Some(expr) = first {
            let value = self.eval_expr(expr)?;
            if value != Value::Bool(true) {}  // <-- NO-OP, value discarded
        }
    }
}
```

While `call_defn`'s top-level handler correctly stored `result`, it never checked `exec_stmt`'s nested result. After `exec_stmt` returned, `call_defn` continued to the next statement without capturing any value set by `term` inside nested scopes.

**Fix**: 
1. Added `return_value: Option<Value>` to `Interpreter` struct
2. Modified `exec_stmt`'s `Term` handler to store the value in `self.return_value`
3. Modified `call_defn` to save/restore `return_value` and break when it's set after any statement (top-level or nested)

**Lesson**: `term` in Brief is not merely a "function return" — it's a value-capture mechanism that can appear inside any nested scope (guards, unifications, blocks). The interpreter must capture ALL `term` values, not just top-level ones.

## 2026-05-29 — Result field key mismatch between constructor and consumer

**Issue**: `run_selfhost` in `main.rs` looked for field key `"result"` (from the specific Ok/Err path at line 878) but the generic enum constructor path at line 869 used field key `"value"`.

**Root Cause**: Two code paths create `Ok`/`Err` enum values:
1. Generic enum constructor path (line 867-876): used when `Ok` variant is in state (registered from `Result` enum declaration). Creates fields with key `"value"`.
2. Specific Ok/Err path (line 878-882): used when `Ok`/`Err` are NOT in state. Creates fields with key `"result"`/`"error"`.

Since `std.result` is imported, `Ok` IS in state, so path 1 always applies. But `run_selfhost` at lines 643 and 651 looked for `"result"`/`"error"` keys from path 2.

**Fix**: Changed `fields.get("result")` to `fields.get("value")` and `fields.get("error")` to `fields.get("value")` in `run_selfhost`.

**Lesson**: When multiple construction paths exist for the same enum type, their field key conventions must be consistent. The generic path always uses `"value"` for single-field enum construction, but the specific Result path had its own convention. Prefer a single consistent convention.

## 2026-05-29 — Brief-written lexer rejects all input with "Unexpected character"

**Issue**: Self-host pipeline tokenizes files via `lib/compiler/lexer.bv` (running inside the interpreter) but fails with `Lex error: Unexpected character: ` on all inputs.

**Root Cause**: The lexer's `next_token` function (line 321-489) checks for EOF at line 325-328 using `is_none(current_char(state))`. If this check somehow fails to detect `None`, line 333 unconditionally calls `unwrap(current_char(state))` — which returns `Void` for `None` input. The guards then compare `Void` against various chars (all false), eventually hitting the "Unknown character" fallthrough at line 488. `char_to_string(Void)` produces an empty string, hence the blank error message.

**Status**: Not yet fixed. Potential causes:
- `is_none` generic function may not dispatch correctly to `Option<Char>` specialization
- `current_char` may return wrong `None` representation
- The `None` variant of `Option` may not exist in the interpreter's state

**Lesson**: (pending investigation)

## 2026-05-30 — `expand_implicit_terms_txn` injects `term true;` into void-returning transactions

**Issue**: `rct txn handle_sigint [sigint] { term; };` produced `ret i64 1` in a `define void` function, causing LLVM verification to fail with "value doesn't match function result type 'void'". The `wake_triggers.bv` fixture exposed this.

**Root Cause**: `desugarer.rs:expand_implicit_terms_txn` unconditionally converted `term;` → `term true;` for all transactions with `Bool` postconditions, mirroring the defn path. But transaction functions are `define void` — they cannot return an `i64`. The `emit_stmt` handler correctly emitted `ret i64 <val>` when `values.first()` was `Some`, never reaching the `ret void` path.

**Fix**: Removed the expansion logic entirely from `expand_implicit_terms_txn`. It is now a passthrough that clones the body unchanged. Transactions return void; their contract semantics are handled by the reactive desugaring pipeline and precondition emission, not by injecting values into `term;`.

**Lesson**: `term;` in transactions means "this transaction has no return value" (void-termination). `term;` in definitions means "terminate with the default postcondition value". These are semantically different — never unify the desugaring paths.

## 2026-05-30 — `opt` new PM syntax: `-passes=verify` not `-verify`

**Issue**: Integration tests called `opt -verify` which failed silently on LLVM 18+ because the legacy pass manager was removed.

**Root Cause**: LLVM 18 defaults to the new pass manager (`-passes=...`). The old `-verify` flag syntax is recognized but no-ops without the legacy PM enabled.

**Fix**: Changed test helper to use `opt -passes=verify` and `opt -passes=default<O3>`.

**Lesson**: Always verify LLVM tooling syntax matches the installed version. The new PM syntax is now canonical for LLVM 17+.

## 2026-05-30 — `alwaysinline` must precede attribute group in LLVM 18

**Issue**: `define void @fn(...) local_unnamed_addr alwaysinline #0` was rejected by LLVM 18 verifier.

**Root Cause**: LLVM 18 requires `alwaysinline` to appear AFTER the attribute group reference, i.e. `define void @fn(...) local_unnamed_addr #0 alwaysinline`.

**Fix**: Swapped the order in `emit_transaction`: `local_unnamed_addr #0{}` with the `alwaysinline` string appended after `#0`.

**Lesson**: LLVM 18 tightened the IR syntax for `alwaysinline`. The canonical position is `#N alwaysinline`.

## 2026-05-29 — `dispatch_mode` lost during desugaring and import resolution

- **Issue**: `#pragma dispatch(parallel)` was parsed correctly by the parser but silently ignored — the LLVM backend always emitted sequential reactor code regardless of the directive.

- **Root Cause**: The `Program` struct has a `dispatch_mode: DispatchMode` field set by the parser. However, both the `Desugarer::desugar()` (desugarer.rs:345) and `ImportResolver::resolve_imports()` (import_resolver.rs:84) construct new `Program` objects with `dispatch_mode: Default::default()` (i.e., `Sequential`) instead of preserving `program.dispatch_mode` from the input program. Since the `run_llvm_compile` pipeline runs `resolve_imports` → `desugar` → `llvm_backend.generate`, the dispatch mode is lost before code generation.

- **Fix**: Changed both `desugarer.rs:345` and `import_resolver.rs:84` to use `dispatch_mode: program.dispatch_mode` instead of `Default::default()`.

- **Lesson**: Whenever a pipeline stage constructs a new `Program` from an existing one, all fields must be explicitly forwarded. This is a brittle pattern — consider a builder or `Clone` for `Program` that preserves metadata fields. Also add a test that verifies dispatch-mode propagation through the full pipeline from parse → resolve → desugar → backend.

## 2026-05-30 — Contract-after-arrow `-> Type [pre][post]` steals first bracket as `Type::ContractBound`

**Issue**: `-> Int [pre][post]` parsed both contract brackets incorrectly — `[pre]` was silently consumed as `Type::ContractBound(Int, pre)` on the output type, and `parse_contract()` only saw `[post]`, setting `pre_condition = post` and `post_condition = Bool(true)`.

**Root Cause**: The contract-after-arrow path (parser.rs:2833) calls `parse_output_types_with_names()` before `parse_contract()`. Inside that function, `parse_type()` at line 4008 greedily consumes `[expr]` as `Type::ContractBound` on *any* type, not just types where the user explicitly wrote `Int[pre]`. Since the bracket check happens after the type name is parsed and before `<` generics, it catches contract brackets that belong to the function, not the type.

**Fix**: Split `parse_type()` into `parse_type()` (public, legacy) and `parse_type_inner(allow_contract_bound)`. `parse_output_types_with_names()` calls `parse_type_inner(false)` so contract brackets are never stolen. The `ContractBound` feature is still available for cases like `Int[product > 0]` when `parse_type()` is called directly.

**Lesson**: When parsing ordered syntax (`-> Type [contract]`), each parser component must be constrained to not consume tokens meant for later components. Greedy `[` consumption in `parse_type` was correct for standalone type parsing but wrong when types and contracts appear adjacent.

## 2026-05-30 — `len()` infinite recursion in self-host interpreter

**Issue**: The self-host pipeline (`brief-compiler selfhost`) failed with a stack overflow / hang on any input. The Brief-written lexer (`lib/compiler/lexer.bv`) called `len(state.source)` which dispatched to `call_defn("len")`, causing infinite recursion through `lib/std/string.bv`'s `term s.len()`.

**Root Cause**: The interpreter's `Expr::Call` handler (interpreter.rs:963) checked user `definitions` **before** any built-in handler for `len`. Since `lib/std/string.bv` defines `defn len(s: String) { term s.len(); }`, calling `len(x)` on a `String` value found the user definition first, dispatched to `call_defn`, which evaluated `term s.len()` — which called `len` again via `Expr::Call("len", [Identifier("s")])` — ad infinitum.

**Fix**: Added a built-in `len` handler for `Value::String` and `Value::List` **before** the definitions check. This short-circuits the recursive definition and returns the correct length directly.

**Lesson**: User definitions must never shadow built-in handlers for primitive type operations. The ordering of checks in `Expr::Call` must be: (1) built-in Result methods, (2) built-in primitives like `len`, (3) user definitions, (4) FFI, (5) enum constructors.

**2026-06-05 Update**: Fully resolved by `:>` projection operator migration. The `Expr::ListLen` magic node and the UFCS `resolve_len_calls` hack are both deleted. All length queries use `x :> Size` — unique syntax, first-class `Expr::Projection` node, zero shadowing risk. `defn len(x) { term x :> Size }` is now a pure stdlib convenience wrapper that cannot recurse because `:>` is parsed directly to `Projection`, not to a `Call`.

## 2026-05-30 — Float result registers not tracked, causing compound float math to emit integer ops

**Issue**: Compound float arithmetic — `let x = 1.0 + 2.0; let z = x + y;` — silently corrupted results. The second addition emitted `add i64` instead of `fadd float` because `is_float_expr(x)` returned `false`.

**Root Cause**: `emit_binop` (llvm.rs:1442) and `Expr::Neg` (llvm.rs:872) converted float results back to i64 but never registered the output register as `Type::Float` in `self.register_types`. Subsequent `is_float_expr` for `Expr::Identifier(name)` at llvm.rs:1401-1403 looked up the register in `register_types` and got no match (or `Type::Int` from the literal insertion), returning `false`.

**Fix**: Added `self.register_types.insert(v.to_string(), Type::Float)` at the end of both float code paths in `emit_binop` and `Expr::Neg`.

**Lesson**: Every code path that produces a value of a particular type must register it in `register_types`. The existing pattern (literals do register, but compound expressions don't) is inconsistent and fragile.

## 2026-05-30 — OnExit cleanup drained on first exit point, lost on subsequent exits

**Issue**: Functions with multiple exit points (multiple `term;` paths, `Escape`, or guarded blocks) only emitted cleanup code on the first exit. All subsequent exits generated zero cleanup, leaking resources.

**Root Cause**: `Statement::Term` (llvm.rs:626) used `std::mem::take(&mut self.pending_cleanup)` which **drains** the vector, leaving it empty for all subsequent exit points. `Statement::Escape` (llvm.rs:638) didn't emit cleanup at all.

**Fix**: 
1. Changed `std::mem::take` → `.clone()` in `Statement::Term` so `pending_cleanup` is preserved for future exits.
2. Added cleanup emission to `Statement::Escape` with the same clone pattern.

**Lesson**: Shared state like `pending_cleanup` must not be mutably consumed at the first use site when multiple consumers exist. Clone the data for each consumer instead.

## 2026-06-01 — `extract_bounded_pre` drops `And` preconditions, fold limit stuck at 0

**Issue**: Benchmarks using `[io_pending && ops < N]` produced folded while-loops comparing against `add i64 0, 0` (limit = 0, loop never executes). Benchmarks hung forever doing zero iterations per tick, trapped in wake main loop.

**Root Cause**: `src/analysis/transition_graph.rs:extract_bounded_pre` matched only `Expr::Lt(var, bound)` and `Expr::Le(var, bound)` at the top level. When the precondition was `Expr::And(io_pending, Expr::Lt(ops, N))`, the `And` wrapper was never unwrapped. `emit_folded_loop` at `llvm.rs:2029` defaults the limit to `add i64 0, 0` when no total is available.

**How discovered**: `timeout 5s ./benchmarks/ring_buffer` never terminated. LLVM IR inspection showed `%lt = add i64 0, 0` instead of `load i64, i64* @N`. `strace -c` confirmed 693 `epoll_wait` calls in 5 seconds with no useful work.

**Fix**: Added recursive `And` unwrapping to `extract_bounded_pre` — decomposes compound preconditions like `trigger && counter < total`.

**Files**: `src/analysis/transition_graph.rs:88-90`, `benchmarks/ring_buffer.bv:21`, `benchmarks/async_counters.bv:17,23`

**Lesson**: Fold analysis must recurse into logical operators in the precondition tree. Non-recursive extraction silently produces valid-looking LLVM IR with zero-iteration loops — the worst kind of silent failure because the binary doesn't crash, it hangs.

## 2026-06-01 — Solo reactive txn auto-promoted to async, injects unnecessary thread pool + barrier

**Issue**: `ring_buffer.bv` with a single `rct txn work` (no `async` keyword) generated `@async_body_work`, `@llvm.thread_pool`, thread pool init, and barrier calls in the main loop — all for one transaction that does sequential work.

**Root Cause**: `src/backend/llvm.rs:388`: the async eligibility check requires ALL non-enum reactive txns to be pairwise conflict-free. For a single txn, the pairwise loop (`for i..for j>i`) never executes, leaving `is_async_eligible = [true]` via vacuous truth. Solo txns are trivially "conflict-free with all others" because there are no others.

**How discovered**: LLVM IR inspection of `ring_buffer.ll` showed `define void @async_body_work` and `@llvm.thread_pool` despite the source having no `async` keyword and only one txn.

**Fix**: Require at least 2 async candidates: `all_async_eligible` gated on `async_candidates.len() >= 2`.

**Files**: `src/backend/llvm.rs:388`, `benchmarks/ring_buffer.bv`

**Lesson**: "All elements satisfy predicate" is true for single-element collections. When "all" implies "there should be multiple things to distribute work across", the guard must explicitly check `len() >= 2`.

## 2026-06-01 — Wake hybrid programs idle forever after convergence (no exit mechanism)

**Issue**: Wake hybrid programs with `@ link` triggers never terminate after convergence. Ring buffer completes 50M iterations then spins: `__rt_wait() → tick → switch (case_1) → done → __rt_wait() → ...` forever.

**Root Cause**: The wake main loop unconditionally routes case arms to `do_wait` → `__rt_wait()` → `br label %tick`. No convergence-based exit path exists in the wake codegen paths. Compiler already computes convergence data but doesn't use it in `emit_enum_main` or `emit_main`.

**How discovered**: `timeout 5s ./benchmarks/ring_buffer` never terminated. SIGTERM is caught by `brief_rt.c` signal handler — program refuses to die. Only SIGKILL works.

**Current workaround**: `timeout --signal=KILL 30s` for benchmarks.

**Fix**: Added `#!exit <expr>;` pragma — file-level boolean expression from the source `.bv` file, evaluated every tick before `__rt_wait()`. When true, `main()` returns 0. Backed by convergence-based exit (natural death) detection.

**Files**: `src/parser.rs` (`#!exit` parsing), `src/backend/llvm.rs` (`emit_exit_check`, `emit_exit_expr`, modified `emit_main`/`emit_enum_main`), `plans/2026-06-01-exit-semantics.md`

**Lesson**: Compile-time analysis data must propagate to ALL codegen paths that can use it. Convergence data is computed once and used for Path 3 (precompute) but Path 4/5 (enum/async dispatch) need it equally. The commit that added `is_fully_precomputable` should have wired it into `emit_enum_main` at the same time.

## 2026-06-01 — `is_trigger_gated` only matches bare `Identifier`, misses `And(trigger, condition)`

**Issue**: `async_counters.bv` with precondition `[io_pending && counter < N]` was classified as async dispatch (Path 5) instead of enum dispatch (Path 4). The async path runs increments via `reactor_tick()` only, achieving ~1 increment per 100ms tick — 29 days for 25M iterations.

**Root Cause**: `src/backend/llvm.rs:is_trigger_gated()` matched only `Expr::Identifier(name)` at the top level. `And(io_pending, Lt(counter, total))` was not recognized as trigger-gated. The `enum_txn_names` set was empty for all compound preconditions, so all txns fell through to `async_candidates`.

**How discovered**: `async_counters` benchmark hung after `#!exit` was added. Even with exit condition `a == N && b == N`, the counters never reached N because the enum dispatch path (which runs folded loops) was never selected.

**Fix**: Added `Expr::And(l, r)` arm to `is_trigger_gated` that recurses into both sides.

**Files**: `src/backend/llvm.rs:138-144`

**Lesson**: When classification functions check preconditions for structural properties (trigger-gated, bounded-convergence), they must recurse into ALL expression types that can wrap the target pattern. The common pattern `trigger && counter < N` is an `And` node — a bare `Identifier` match will never see it.

## 2026-06-01 — `emit_enum_main` single-txn `graph.nodes.len() == 1 && txns.len() == 1` guard prevents multi-txn folded loops

**Issue**: Even when multiple trigger-gated txns with bounded convergence enter the enum dispatch path, only one folded loop was emitted per case arm — the one corresponding to the first (and assumed-only) transaction. Multi-txn programs like `async_counters` (inc_a + inc_b) converged zero counters per tick.

**Root Cause**: `generate()` at `llvm.rs:650-662` extracted folding params only for the single-txn guard `graph.nodes.len() == 1 && txns.len() == 1`. For multi-txn programs, `(enum_ci, enum_ti, enum_tcn) = (0, None, None)` — every folded loop got limit `add i64 0, 0`.

**How discovered**: After fixing `is_trigger_gated`, async_counters entered enum dispatch but still ran 0 iterations per tick. LLVM IR inspection showed `%lt... = add i64 0, 0` for all case arm loops.

**Fix**: 
1. Built `enum_fold_params: HashMap<String, (usize, Option<usize>, Option<String>)>` in `generate()` — per-txn folding params extracted from the transition graph.
2. Added `fold_params` parameter to `emit_enum_main`.
3. Added `emit_case_folded_loops` closure that iterates over all fold param entries and emits one folded loop per entry per case arm.

**Files**: `src/backend/llvm.rs:647-665` (enum_fold_params build), `src/backend/llvm.rs:2285-2315` (multi-txn case arm emission)

**Lesson**: The enum dispatch path was designed for single-txn programs. Multi-txn programs with multiple bounded counters need per-txn folded loops. The `graph.nodes.len() == 1 && txns.len() == 1` guard was a premature optimization assumption that excluded valid multi-txn convergence programs. Always verify classification/path-selection logic handles N>1 inputs.

## 2026-06-02 — Struct-SSA regression for non-pure bodies (Kalman filter 2× slowdown)

**Issue**: Kalman filter benchmark ran 0.28s at 10M iterations while the old code ran 0.143s (scaled from 0.716s at 50M) — exactly 2× slower. C reference ran 0.14s. Brief went from beating C to trailing by 2×.

**Root Cause**: `emit_folded_main` with `use_phi=false, body=Some(stmts)` emits a `load %State` (64 bytes), 13-element chained `extractvalue`/`insertvalue` sequence, then `store %State`. This struct-SSA pattern requires SROA (Scalar Replacement of Aggregates) to decompose into per-field scalar operations. But `llc -O2` does NOT run SROA — only `opt -O2` does. Without SROA, LLVM's backend materializes the entire 64-byte struct as a memory block, preventing per-field register promotion, phi node generation, and GVN across float operations.

The old (pre-struct-SSA) codegen used per-field `GEP + load/store` throughout, which LLVM's backend handles naturally without SROA.

**Why it affected Kalman specifically**: The Kalman filter's precondition references all 12 float fields (`x0 == x0 && x1 == x1 && ...` — a NaN-guard pattern). This makes all float fields live, so `is_effectively_pure = false`. The non-pure path takes `use_phi=false, body=Some(stmts)` which is the struct-SSA path. Pure/effectively-pure bodies (IIR, ring_buffer, async_counters) take the `use_phi=true` phi-node path and were unaffected.

**Fix**: Run `opt -O2 -S` before `llc` in `run_llvm_compile()` at `src/main.rs:1899`. `opt -O2` runs SROA, mem2reg, GVN, and constant propagation. SROA decomposes the `load %State`/`store %State` into scalar phis; GVN eliminates redundant float→i64→float round trips. The SLP hazard analyzer's `-vectorize-slp=false` flag is passed to `opt` (where SLP runs as a middle-end pass), not `llc`. Graceful fallback if `opt` is not installed.

**Result**: Kalman filter recovers from 2× regression to 0.71s at 50M vs C 0.75s (Brief beats C by ~5%, tied at worst).

**Lesson**: `llc -O2` and `opt -O2` run different pass pipelines. `llc` is the codegen backend (instruction selection, regalloc, scheduling). `opt` is the middle-end optimizer (SROA, mem2reg, GVN, loop opts, vectorization). Struct-SSA (`load %State`/`store %State` + insertvalue chains) requires SROA to decompose — always run `opt -O2` before `llc` for programs with struct values.

## 2026-06-02 — `is_trigger_gated` misses `Expr::Eq`, enum dispatch invisible for `trigger == literal` preconditions

**Issue**: Sparse dispatch benchmark preconditions like `t == 101` (Eq(Identifier, Integer)) never entered the enum dispatch optimizer path. The enum dispatch path correctly extracted keys via `extract_trigger_keys` (line 149-183) but `is_trigger_gated` (line 139-147) returned `false` for all `Expr::Eq` patterns, so no reactive txn was classified as an enum candidate.

**Root Cause**: `is_trigger_gated` at `src/backend/llvm.rs:139-147` matched only `Expr::Identifier(name)` and `Expr::And(l, r)`. An `Expr::Eq(Identifier(trigger), Integer(value))` node was never unwrapped. The function signature says it checks for "a direct reference to one of the given trigger names" — but this was too narrow: a _direct reference_ is `trigger == value` just as much as bare `trigger`, and `extract_trigger_keys` already recognized Eq patterns.

```rust
// Before (BUGGY):
fn is_trigger_gated(pre: &Expr, trigger_names: &HashSet<&str>) -> bool {
    match pre {
        Expr::Identifier(name) => trigger_names.contains(name.as_str()),
        Expr::And(l, r) => is_trigger_gated(l, trigger_names) || is_trigger_gated(r, trigger_names),
        _ => false,  // ← Expr::Eq falls through here
    }
}
```

This bug was introduced in the same commit that added `extract_trigger_keys` and `is_trigger_gated` (Step 7 / dead-field elimination). Whoever wrote `extract_trigger_keys` correctly handled `Expr::Eq` but the counterpart `is_trigger_gated` didn't get the same treatment.

**How discovered**: Writing a sparse dispatch benchmark that used `t == 101 || t == 204 || ...` preconditions. Benchmark compiled but ran through the standard `reactor_tick()` path instead of enum switch dispatch. LLVM IR inspection showed `switch` was absent — the program used the generic tick loop.

**Fix**: Added `Expr::Eq(l, r)` arm to `is_trigger_gated`:

```rust
Expr::Eq(l, r) => {
    matches!(l.as_ref(), Expr::Identifier(name) if trigger_names.contains(name.as_str()))
        || matches!(r.as_ref(), Expr::Identifier(name) if trigger_names.contains(name.as_str()))
}
```

**Files**: `src/backend/llvm.rs:139-147`

**Lesson**: Whenever a classification function (`is_trigger_gated`) and a data-extraction function (`extract_trigger_keys`) operate on the same AST nodes for the same purpose, they must recognize the same expression patterns. `extract_trigger_keys` correctly handles `Expr::Eq` — `is_trigger_gated` must too. They were written at the same time for the same optimization path; the divergence was an oversight.

## 2026-06-02 — `llvm.assume` before `br` in folded loops makes `opt` believe exit branch is dead

**Issue**: `const_heavy.bv` compiled to a binary that immediately segfaulted. The `main()` function was optimized to `unreachable` by `opt -O2`, causing LLVM to emit invalid code.

**Root Cause**: `emit_folded_loop` at `src/backend/llvm.rs:2911-2912` emitted `call void @llvm.assume(i1 %cp)` on the loop entry comparison result, then `br i1 %cp, label %body, label %done`. The `llvm.assume` tells LLVM's optimizer that the comparison is ALWAYS true. `opt -O2` then eliminated the `done` branch (the loop exit) as dead code, making the loop infinite. The `noreturn` function attribute and `unreachable` terminator produced a segfault.

```llvm
; Before (optimized to infinite loop):
%cp = icmp slt i64 %cnt, %total
call void @llvm.assume(i1 %cp)    ; tells opt: %cp is ALWAYS true
br i1 %cp, label %body, label %done  ; opt eliminates %done

; Result (via opt -O2):
define noundef i32 @main() {
  %t9.i = tail call i64 @__get_env_int(ptr @str.0)
  store volatile i64 %t9.i, ptr @global_state
  unreachable  ; <-- SEGFAULT
}
```

Three code paths in `emit_folded_loop` (phi mode, SSA mode, call mode) all emitted `llvm.assume` on the same `icmp slt` result.

**How discovered**: `const_heavy.bv` benchmark segfaulted immediately. GDB showed PC at `0x5`. Inspection of `const_heavy.opt.ll` revealed `main()` ended with `unreachable`. The `llvm.assume` was traced to the optimization sprint commit that added "`llvm.assume` on convergent preconditions" (2026-06-02).

**Fix**: Removed `llvm.assume` emission from all three code paths in `emit_folded_loop`. Removed the `proven_convergent: bool` parameter from the function signature and all callers.

**Files**: `src/backend/llvm.rs:2911-2912`, `src/backend/llvm.rs:2936-2937`, `src/backend/llvm.rs:2969-2970`

**Lesson**: `llvm.assume(i1 %cond)` tells the optimizer that `%cond` is unconditionally true. Placing it BEFORE a conditional branch (`br i1 %cond, label %exit, label %loop`) makes the optimizer eliminate the branch's continuation as dead code. If the branch controls loop convergence, the result is an infinite loop. `llvm.assume` is correct when placed after a runtime panicking branch (`br i1 %cond, label %panic, label %safe` followed by `unreachable` then `call @llvm.assume(i1 %cond)`) — never before a convergence check.
## 2026-06-04 — Exit expression Neg(Integer) not handled in emit_exit_expr

**Issue**: Program with `#!exit cr >= -200` hung — exit condition never satisfied. The LLVM IR showed `%t599 = add i64 0, 0 ; unsupported exit expr` instead of `%t599 = sub i64 0, 200`.

**Root Cause**: `emit_exit_expr` at `src/backend/llvm.rs:3558` had an early-return filter for `Expr::Integer`, `Expr::Bool`, and `Expr::Float` that delegated to `emit_expr` for constant inlining. But negative integers like `-200` are parsed as `Expr::Neg(Box::new(Expr::Integer(200)))` — the `Neg` wrapper wasn't in the filter. The expression fell through to the `_ =>` catch-all at line 3658 which returned `add i64 0, 0`.

**Fix**: Added `Expr::Neg(_)` to the early-return filter:
```diff
- Expr::Integer(_) | Expr::Bool(_) | Expr::Float(_) => {
+ Expr::Integer(_) | Expr::Bool(_) | Expr::Float(_) | Expr::Neg(_) => {
```

**Lesson**: Negative literals in Brief are `Neg(Integer(n))`, not `Integer(-n)`. Any code path that pattern-matches on `Expr::Integer` for constants must also handle `Expr::Neg(Expr::Integer(_))`. The `emit_expr` function already handles `Neg` correctly — the fix delegates to it.

## 2026-06-04 — Universal loop hangs with decreasing counter contract

**Issue**: Programs with `rct txn ... [count > 0][count == 0]` (decreasing counter) hang. Switching to `[count < N][count == N]` (increasing counter) works.

**Root Cause**: The universal loop in `emit_folded_multi_main` emits `count < N - stride` for the unrolled body4 path, computed as `adj = add i64 N, <negated_stride>`. This comparison `icmp slt count, adj` only makes sense when count increases. A decreasing counter like `count > 0` has an INVERTED direction — count starts high and decreases, so `count > 0` should become `count > stride` not `count < bound - stride`.

**Lesson**: The universal loop (unrolled fold) assumes strictly increasing counters. The `transition_graph` should detect decreasing counters and either invert the comparison in the codegen or fall back to the non-unrolled default path.


## 2026-06-04 — Decreasing counter contracts hang or fall to O(N)

**Issue**: Programs with `rct txn [count > 0][count == 0]` either hung (universal loop path) or ran O(N) tick-per-iteration (fallback path). Only `[count < N][count == N]` was fast.

**Root cause**: Three separate gaps:
1. `extract_bounded_pre` only matched `Expr::Lt`/`Expr::Le` (increasing). `Expr::Gt`/`Expr::Ge` (decreasing) fell through to `None`.
2. `detect_increments` only matched `Expr::Add` (count = count + 1). `Expr::Sub` (count = count - 1) returned `None`.
3. `emit_folded_loop` unconditionally emitted `icmp slt` (signed-less-than) for all comparisons. No branch for `icmp sgt`.

**Fix** (3 files, additive):
1. `transition_graph.rs`: Added `ConvergeDirection { Increasing, Decreasing }` enum; added `direction` + `bound_literal` fields to `BoundedPre`. Extended `extract_bounded_pre` to match `Gt`/`Ge` (set `direction: Decreasing`). Extended `detect_increments` to match `Expr::Sub` for decrementing counters.
2. `llvm.rs`: Added `FoldParam` struct carrying `counter_idx`, `bound_field_idx`, `bound_const_name`, `is_decreasing`, `bound_literal`. Threaded through `multi_fold_params` → `enum_fold_params` → `emit_folded_multi_main` → `emit_folded_loop`.
3. `emit_folded_loop` header: branched on `is_decreasing` to emit `icmp sgt` instead of `icmp slt`, and `bound + (unroll-1)` instead of `bound - (unroll-1)` for the body4 path. Added `bound_literal` priority to bound loading for literal bounds like `count > 0`.

**Tests**: Decreasing counter program (`[count > 0][count == 0]` with `count = count - 1`) compiles, emits `icmp sgt`, completes 50M iterations in <10s.

## 2026-06-05 — Unused `io_pending` import forces reactive runtime on pure-state benchmarks

**Issue**: `benchmarks/bit_clear.bv` and `benchmarks/queue_drain.bv` both imported `io_pending` from `std/brief_rt.bv` but never used it in any precondition. The import was dead weight but still triggered the reactive runtime path (`has_wake_triggers = true` → reactor with `__rt_wait()` 100ms blocking per tick), turning a 63-iteration burn (bit_clear) into a 6.3-second slog and a 10-iteration burn (queue_drain) into a 1-second slog.

**Root Cause**: The compiler treats any import of `io_pending` as evidence the program needs wake triggers, even if no precondition references it. The presence of `io_pending` in the import set sets `has_triggers = true` in the transition graph, which in turn selects the reactor codegen path instead of the pure SSA while-loop path.

**Fix**: Removed `import { io_pending } from "std/brief_rt.bv"` from both files. Neither benchmark needs external event wakeup — both converge purely on state (`reg != 0 → reg == 0`, `queue:>Size > 0 → queue:>Size == 0`).

**Lesson**: Never import `io_pending` unless the program genuinely waits for external IO. Pure-state convergence benchmarks must use the SSA while-loop path (no reactor). The `io_pending` import is a foot-gun: it looks harmless but silently activates the full reactive machinery.

**Also**: `queue_drain.bv` declared `const queue: List<Int> = [...]` but then tried to mutate it via `<- &queue`. `const` values are compile-time immutable. Changed to `let queue`.

## 2026-06-05 — Low print modulo doesn't fire on short benchmarks

**Issue**: `bit_clear.bv` used `[reg % 1000000 == 0]` as the print guard, but the benchmark only runs 63 iterations (popcount of i64::MAX). No value of `reg` is divisible by 1,000,000 in that range, so the liveness `__print_int` never fires. The program compiled and ran correctly (no fold), but produced zero observable output.

**Root Cause**: The print threshold was copy-pasted from 50M-iteration benchmarks without adjusting for bit_clear's inherently bounded iteration count (63 max).

**Fix**: Lowered threshold to `[reg % 100000 == 0]`. The values that generate output: `reg = 0` (the last iteration, always fires since `0 % 100000 == 0`). This ensures at least one observable side-effect fires.

**Lesson**: When adapting benchmark patterns to bounded-iteration designs (integer-width-bound patterns like popcount decay), verify the print guard threshold will actually fire within the available iteration space. A silent benchmark is a dead-code-elimination risk.

## 2026-06-05 — `memory(argmem: write)` on FFI declarations lets LLVM eliminate IO calls

**Issue**: `__print_int(i64)` calls inside small loops (10-iteration queue_drain) were eliminated by `opt -O3` during LTO. The binary contained a dead `__print_int` function that was never called.

**Root Cause**: `src/backend/llvm.rs:1344` declared ALL foreign functions with `attributes #1 = { ... memory(argmem: write) }`. This tells LLVM the function only writes through pointer arguments. For `__print_int(i64)`, there are no pointer arguments, so LLVM concluded the function writes NO memory. Combined with `willreturn` and the unused return value, `opt -O3` eliminated the entire call.

**Fix**: Removed `memory(argmem: write)` from attribute #1:
```
attributes #1 = { nocallback nofree nosync nounwind willreturn }
```
Without any `memory(...)` restriction, LLVM conservatively assumes the function can read/write arbitrary memory. During LTO, `opt -O3`'s FunctionAttrs pass examines `__print_int`'s actual body (merged from `brief_rt.c`), sees `fprintf(stderr, ...)` as a real global side effect, and correctly preserves the call. For `__sqrtf`, FunctionAttrs infers `readnone` from the `sqrtf` call and restores CSE/hoisting.

**Lesson**: Never assert `memory(argmem: write)` on FFI functions — the Brief compiler cannot verify this. The mathematically correct default is no memory restriction. LTO reveals actual function bodies and LLVM's FunctionAttrs pass infers correct attributes deterministically.

## 2026-06-05 — Compile-time-known list size causes precomputation (correct behavior)

**Issue**: `queue_drain.bv` used a compile-time list literal `[1..10]`. The compiler correctly precomputed all 10 iterations within the default budget (256) and emitted `main` as `xor eax; ret`. The benchmark produced zero output and ran trivially.

**Root Cause**: Not a bug — the system works as designed. When all information is known at compile time, the compiler precomputes the result. The benchmark author must make the bound runtime-determined to prevent precomputation.

**Fix**: Replaced the compile-time list with a runtime counter:
- Before: `let queue: List<Int> = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];` + drain convergence
- After: `let N: Int = __get_env_int("BOUND");` + counter convergence + push/pop side ops

**Lesson**: If a benchmark must execute at runtime, use `__get_env_int("BOUND")` for the bound. A compile-time-known bound WILL be precomputed. This is correct, not a bug. See AGENTS.md §"Precomputation is Correct, Not a Bug".

---

## 2026-06-05 — Parser fails on `term! -> swan_song;` inside guarded blocks

**Issue**: `term! -> __print_int(checksum);` inside `[count == N] { ... }` caused parse error `expected identifier, found Arrow at 66:15`.

**Root Cause**: `parse_term_outputs()` didn't check for `Token::Arrow`, so it tried to parse `->` as an expression. Additionally, swan song was parsed via `parse_statement()` which consumed the trailing `;`, but the caller's `expect(Token::Semicolon)` expected another one.

**Fix**: Added `Token::Arrow` check in `parse_term_outputs()` to return early. Changed swan song parsing from `parse_statement()` to `parse_expression()` wrapped in `Statement::Expression`, so semicolon is consumed once by the caller.

**Lesson**: New syntax needs parser checks for all possible token sequences following `term!`. The `->` arrow, term outputs, and semicolon interact in ways that are easy to miss.

---

## 2026-06-05 — LLVM `attributes #1` declared FFI functions as pure, letting optimizer eliminate I/O

**Issue**: `opt -O3` eliminated `__print_int` calls from merged bitcode. Benchmark binaries produced zero output.

**Root Cause**: `attributes #1 = { nocallback nofree nosync nounwind willreturn }` told LLVM the function had no side effects. LLVM eliminated the calls as dead code.

**Fix**: Changed to `attributes #1 = { nounwind }`. Removed `nofree`, `nosync`, `willreturn`, `nocallback` — these all signal purity.

**Lesson**: FFI function declarations must not over-promise purity. Only `nounwind` is safe to assume. `willreturn` in particular tells LLVM it can speculate calls.

---

## 2026-06-05 — `__putchar` undefined at link time despite definition in runtime

**Issue**: `__putchar` was declared in `fasta.bv` as `frgn` and present in `runtime/brief_rt.c`, but LTO link failed with `undefined reference to '__putchar'`.

**Root Cause**: Function was `static inline` with `always_inline`. Clang compiled `brief_rt.c` at `-O2`, inlined all calls within the runtime translation unit, then discarded the function body. The program IR's `declare` remained unresolved across module boundaries.

**Fix**: Changed to `__attribute__((used)) int64_t __putchar(int64_t c)`. No `static`, no `inline`. The `used` attribute forces emission even when no callers exist within the translation unit.

**Lesson**: Runtime helper functions called only from program IR (not from within the runtime itself) must not be `static`. Use `__attribute__((used))` to prevent dead-code elimination.

--- 

## 2026-06-05 — `io_pending` used as liveness workaround in benchmarks

**Issue**: Several benchmarks used `io_pending` (an FFI call) in their transaction guard to prevent pure-counter fold elimination. This was a relic from before `term! -> swan_song;` provided proper liveness semantics.

**Root Cause**: Cargo-culted pattern from older benchmarks. `io_pending` was a hack — it forced the compiler to treat the body as impure because the precondition referenced FFI.

**Fix**: Removed `io_pending` imports and guard conditions from all Tier 3 benchmarks. Guards now use `[count < N][count == N]` directly.

**Lesson**: When new language features supersede old workarounds, audit existing code for the old pattern and clean it up. Documented here to prevent future cargo-culting.

--- 

## 2026-06-05 — Accidental deletion of benchmark source files during cleanup

**Issue**: `rm -f benchmarks/fannkuch_redux*` matched and deleted both build artifacts AND source files (`fannkuch_redux.bv`, `fannkuch_redux_c.c`). Restored from git but lost uncommitted edits.

**Root Cause**: Shell glob `*` matched all files starting with the prefix. No distinction between `.bv` source, `_c.c` reference, `.o` object, `.ll` IR, and binary executable.

**Lesson**: Never glob over source files. Use explicit filenames for cleanup: `rm -f benchmarks/fannkuch_redux benchmarks/fannkuch_redux.ll benchmarks/fannkuch_redux.o`. Or better: organize build artifacts in a separate subdirectory.

---

## 2026-06-06 — Parser discards `from "..."` value in frgn declarations

**Issue**: `frgn __print_int(n: Int) -> Bool from "libruntime"` — the `"libruntime"` string was parsed but immediately thrown away. `ForeignSignature::location` was hardcoded to `String::new()` at construction.

**Root Cause**: `parser.rs:1142` used `location: String::new()` instead of `location: location.clone()`. The parsed `location` variable was never written into the struct.

**Fix**: Changed `String::new()` to `location.clone()` so the `from` value is actually stored.

**Lesson**: Always check that parsed values are actually wired into the AST node.

---

## 2026-06-06 — Hardcoded runtime declares in LLVM backend

**Issue**: `emit_declares()` unconditionally emitted `declare void @__rt_init()`, `declare void @__rt_wait()`, `declare void @__rt_poll()`, `declare void @__exit()`, `declare void @brief_thread_pool_init()`, etc. Users couldn't opt out and these symbols were never declared in user code.

**Root Cause**: The runtime functions were hardcoded in `llvm.rs:1844-1868` instead of being declared as `frgn` in `std/rt.bv` and imported by the user.

**Fix**: Removed the hardcoded runtime declares. Added a TODO to migrate the codegen call sites to use `self.frgn_map` lookups. Created `docs/learn/ffi.md` documenting the architecture.

**Lesson**: Runtime functions should be declared in standard library modules, not hardcoded in codegen.

---

## 2026-06-06 — `"None"`/`"Err"` discriminant magic in LLVM backend

**Issue**: `llvm.rs:508` hardcoded `"None" | "Err" => 0` for enum variant discriminants, assuming `None` and `Err` are always the first variant. Three other sites used `if name == "None"` as fallback logic.

**Root Cause**: Hardcoded match on variant names instead of using the enum declaration order. The `variant_disc` map was populated but then ignored in favor of name matching.

**Fix**: Changed to sequential discriminants starting at 0, based on declaration order (not name). All variant discriminant fallbacks use `unwrap_or(0)`.

**Lesson**: Dispatch on type/variant definition, not on hardcoded names.

---

## 2026-06-06 — Interpreter built-in method dispatch is still name-based magic (deferred)

**Issue**: The E1-E3 refactor consolidated 544 lines of duplicated method blocks into a single `dispatch_method_by_type` function. However, the dispatch still matches on hardcoded function name strings (`"insert"`, `"get"`, `"push"`, `"HashMap::new"`, etc.) inside type-scoped match arms.

**Root Cause**: The refactor only changed the *structure* of the dispatch (from top-level name matches to type-scoped name matches). The core problem — dispatching on function name strings instead of through the FFI registry — remains unsolved.

**Fix**: This is deferred to a follow-up session. The correct approach (Path A) is to register all built-in operations in the interpreter's FFI registry with location keys like `"std::HashMap::insert"`, then write stdlib modules (`std/hashmap.bv`, `std/stack.bv`) that declare `frgn HashMap::insert(map, key, value) -> HashMap from "std"`. The interpreter resolves through `ffi_name_to_location` → `foreign_functions` — the same path as C FFI.

**Lesson**: Structural refactoring of magic is not the same as eliminating magic.

**Issue**: Reported `fasta.bv` as hanging. The benchmark was being passed `N=100` instead of `BOUND=100`.

**Root Cause**: The benchmark reads `BOUND` from the environment, but `N` was used when testing. The benchmark uses `__get_env_int("BOUND")` to read the bound.

**Fix**: Used `BOUND=100` instead of `N=100`. The benchmark runs correctly.

**Lesson**: Always check env var names in the source code before assuming a benchmark is broken.

---

## 2026-06-07 — `term! -> swan_song` emits `ret void` inside `i32 @main` in folded loop path

**Issue**: When `term! -> __print_int(h)` is inside a guarded block in a transaction body that enters the folded struct-SSA loop path (Path 5), the backend emits `ret void` inside `define i32 @main()`. LLVM/llc rejects: "value doesn't match function result type 'i32'".

**Root Cause**: `emit_folded_main` (llvm.rs:4299) emits `define i32 @main()`, but the body emission in `emit_folded_loop` at line 4121 sets `self.returns_i64 = false`. The `term!` handler at line 2387-2401 has three branches:
- `values.first()` is Some → always emits `ret i64 %r` (still wrong for i32 main, but unreachable for our case)
- `values` is empty + `self.returns_i64 = true` → `ret i64 0` (not taken)
- `values` is empty + `self.returns_i64 = false` → `ret void` (TAKEN — crashes llc)

The `returns_i64` flag tracks whether the enclosing compute/txn function returns i64 (for transaction wrappers) or void (for `compute()`). But it doesn't account for the folded loop path where the body is inlined directly into `define i32 @main()`.

**Fix**: Add a `main_body: bool` flag to the backend struct. When set, `term!` and `term` handlers emit `ret i32 0` instead of `ret void` or `ret i64`. Set `main_body = true` in `emit_folded_main`, `emit_ssa_main`, and `emit_folded_multi_main` before emitting the loop body. Reset to false after.

**Lesson**: The `returns_i64` flag is overloaded — it really means "returns i64 specifically" but is used as "returns something" in some paths and "returns void" in others. When the body is inlined into `i32 @main`, neither value is correct.

---

## 2026-06-07 — Guarded block handler restores `self.terminated` after `term!`, emits code after `ret`

**Issue**: When `term!` fires inside a guarded block, the Guarded handler at line 2587-2604 saves `self.terminated`, sets it to false, emits the guard body (which includes `term!` that sets `terminated = true`), then RESTORES `self.terminated` to the pre-guard value (false). The caller continues emitting code after the `ret` instruction, including subsequent unrolled iterations in `emit_folded_loop`.

**Root Cause**: Line 2604: `self.terminated = prev_terminated;` unconditionally restores the flag, undoing the effect of any terminating statement (term!, escape) inside the guarded body. The `if !self.terminated` check at line 2602 correctly prevents emitting the br-to-merge when terminated, but the restoration at 2604 undermines this.

**Fix**: Replace the unconditional restore with conditional:
```rust
if !self.terminated {
    self.terminated = prev_terminated;
}
```
If a terminating statement fired inside the guarded body, leave `self.terminated = true` so callers know the block terminated. Only restore when no termination occurred.

**Lesson**: The save/restore pattern around `self.terminated` in the Guarded handler assumes the guarded body never terminates execution — it's designed for "do something then continue" guards. When a guard body contains `term!`, this assumption is violated. Any save/restore of control-flow flags must account for the possibility that the inner code changes them in a way that should persist.

---

## 2026-06-07 — `-lm` missing in compiler driver link step (FIXED)

**Issue**: `brief_rt.c` provides `float __sqrtf(float x) { return sqrtf(x); }` (line 392), which is actively used by `benchmarks/nbody_sqrt.bv` (24 call sites). The compiler driver at `main.rs:~2360` never passes `-lm` to the linker. The C reference gets `-lm` via the same clang invocation, creating asymmetry. Programs using `__sqrtf` get undefined reference at link time.

**Root Cause**: The link command at `main.rs:2359-2365` was assembled incrementally (conditionally adding `-lrt`, `-lpthread`) and `-lm` was never added. Since `sqrtf` is in libm (not libc), glibc requires explicit `-lm`.

**Fix**: Added `link_cmd.arg("-lm");` to the link command unconditionally (brief_rt.c references `sqrtf`). Also added `-lm` to the linking-failed hint message.

**Lesson**: Every library dependency of the runtime must be provided to the linker. `sqrtf` from `libm` is the runtime's dependency, not the user program's. The compiler driver is responsible for its own runtime module's link requirements.

---

## 2026-06-07 — `Statement::Guarded` is one-shot, not a loop — ~130 defns silently broken

**Issue**: Every `defn` in the standard library and compiler that uses `[guard] { ... &i = i + 1; }` for iteration only processes the first element. The guarded statement fires once, then falls through — there is no loop.

**Root Cause**: `Statement::Guarded` evaluates its condition once and executes the body zero or one times (src/interpreter.rs:842-861). The `defn` body executes as a straight-line sequence — no implicit transaction wrapping, no reactor loop. The pattern `let i = 0; [i < list :> Size] { ... &i = i + 1; }` was cargo-culted from `rct txn` bodies where the outer reactor loop provides convergence. But `defn` has no such loop.

Affected files and approximate counts:
- `lib/std/iterator.bv`: 14 defns
- `lib/std/hashmap.bv`: 3 defns
- `lib/std/hashset.bv`: 4 defns
- `lib/std/stack.bv`, `queue.bv`: 2 defns
- `lib/std/math.bv`: 3 defns
- `lib/std/string.bv`: ~38 defns (including a near-duplicate second copy of ~17 functions)
- `lib/std/io.bv`: 1 defn
- `lib/std/encoding.bv`: 5 defns
- `lib/std/json.bv`: 1 defn
- `lib/compiler/`: ~60+ defns (deferred — self-hosting already broken)

**Fix**: Replace broken `defn` iterators with callable `txn` declarations. Regular `txn` takes parameters and returns values like `defn`, but with `[pre][post]` convergence loop semantics — the body re-executes until the postcondition is met.

```brief
// BEFORE (broken — guarded fires once):
defn iter_filter<T>(list, pred) -> List<T> [true] {
    let i = 0;
    [i < list :> Size] { ... &i = i + 1; };   // one-shot!
    term result;
};

// AFTER (correct — convergence loop):
txn iter_filter<T>(list: List<T>, pred: T -> Bool)
    [i < list :> Size][i == list :> Size] -> List<T>
{
    [pred(list[i])] { &result = result.append(list[i]); };
    &i = i + 1;
    term result;
};
```

The callable `txn` is invoked via `Expr::Call` just like a `defn`. The interpreter registers non-reactive txns in a `callable_txns` map and executes them as convergent loops (evaluate pre → execute body → check post → repeat if state changed). State is cloned per-call and restored on return, providing isolation.

Concrete changes:
1. AST: Add `output_type: Option<OutputType>` and `outputs: Vec<Type>` to `Transaction`
2. Parser: Parse optional `-> ReturnType` after contract for regular `txn`s (not `rct`)
3. Interpreter: Register `txn` in `callable_txns` during init; dispatch from `Expr::Call`; convergence loop execution
4. Backends: Add stubs for callable txn dispatch returning 0
5. Stdlib: Convert ~71 broken defns in `lib/std/` to txns
6. `lib/std/option.bv`: Fix filter postcondition `[term.is_some() && pred(unwrap(opt))]` → `[!term.is_some() \|\| pred(unwrap(term))]`
7. `lib/std/string.bv`: Remove the near-duplicate second copy (~17 functions, lines 580-876)

**Lesson**: The convergence loop is the defining feature of `txn` — `defn` has straight-line execution. When writing iteration, use `txn` with `[pre][post]` contracts. `Statement::Guarded` is for one-shot conditionals only, never for loops. `[guard] { body }` inside a `txn` with `[pre][post]` is correct because the txn's outer convergence loop re-evaluates the body, not because the guarded statement loops by itself.

## 2026-06-08 (FIXED) — `<:` subtype projection parser bug

**Root Cause**: `parse_primary()` at `src/parser.rs:5168` interpreted `items { COUNT; }` as a struct literal (`identifier { field: value }`), consuming the ops block. `parse_expression()` was used for the source but it calls `parse_primary()` which treats any `{` after an identifier as a struct literal.

**Fix**: Replaced `parse_expression()` with `parse_projection_source()` in both `<:` let-statement paths (tuple and non-tuple). This new function parses an identifier + postfix operations but stops before `{` and `[`, leaving them for `parse_subtype_ops()` to handle as the ops block or MATCH bracket syntax.

## 2026-06-09 — Proof engine guard path three-bug cascade

**Symptoms**: Benchmarks with a `[guard] { __print_*(...); };` inside an `rct txn` fail P008 contract verification with 14+ identical-looking `guard` constraints in the path state.

**Root Cause**: Three interacting bugs in `enumerate_paths_recursive` and `is_truthy`:

### Bug A — Guard-taken path never reaches `term` (proof_engine.rs:883)

When `enumerate_paths_recursive` encounters a `Statement::Guarded`, the true branch recurses into `statements` (the guard body). The guard body is typically just a print expression — no `term`. So `terminated` remains `false`, and NO path is pushed for the true branch. The false branch correctly recurses into `body[1..]` (remaining body after guard, which includes `term`) and pushes a valid path. But the true branch path is lost — its continuation to `term` is never explored.

**Line 883**: `self.enumerate_paths_recursive(statements, true_state, &mut true_paths);`

**Fix**: After recursing into the guard body, check if `true_paths` is empty. If so, the guard body didn't terminate — continue exploring `body[1..]` with the `true_state` to reach `term`.

### Bug B — `eval_numeric` missing `Mod` and `Div` (proof_engine.rs:1064)

`eval_numeric` handles `Add`, `Sub`, `Mul` but falls through to `_ => None` for `Mod` and `Div`. Since guard conditions like `count % 5000000 == 0` use modulo arithmetic, `is_truthy` always returns `false` for these conditions. This makes non-negated guard constraints appear infeasible in the `implies` check.

**Fix**: Add `Expr::Mod` and `Expr::Div` cases to `eval_numeric` for concrete integer operands.

### Bug C — `format_expr` hides `is_negated` (proof_engine.rs:804)

The error printer at line 803-805 only renders `format_expr(&constraint.condition)`. The `is_negated` flag is silently dropped. Negated constraints (`!guard_condition`) print identically to non-negated ones, making the error output deeply misleading — 14 guard-looking constraints may all be different but look the same.

**Fix**: Include `is_negated` in the display: `if constraint.is_negated { "¬" } else { "" }` prefix.

**Lesson**: The third finding (Bug C) is a debugging-jit — it's serious because it hides the distinction between guard-taken and guard-not-taken paths, making P008 errors nearly impossible to diagnose from the output alone. The guard-taken path being dropped (Bug A) means the proof engine currently does NOT fully verify contracts for paths that take guarded branches — it sees only the guard-not-taken path and judges correctness on that alone. Where the guard body has side effects (like FFI calls), this is unsound.

**Fix (same commit)**: 
1. Guard-taken path now continues to remaining body after guard via `body[i+1..]` tail recursion.
2. `eval_numeric` now handles `Expr::Mod` and `Expr::Div` for concrete operands.
3. Error output shows `¬` prefix for negated path constraints.
4. Body index `body[1..]` → `body[i+1..]` using `.enumerate()` — fixes exponential path explosion when guards were followed by more guarded statements.

### Pre-existing: convergence analysis gaps (NOT caused by Pattern B)

After the above fixes, these benchmarks still fail P008 — all are convergence analysis gaps in `check_convergence` in `proof_engine.rs`, NOT regressions:

| Benchmark | Root cause | 
|-----------|------------|
| `*_runtime.bv` | Precondition is `AND(relation, relation)` — `check_convergence` expects bare `count < bound`, not `bound > 0 && count < bound`. No AND-handling exists. |
| `bit_clear.bv` | `reg = reg & (reg - 1)` is popcount decay — `check_convergence` only detects `Add`/`Sub` increments. `detect_popcount_decay` exists in `transition_graph.rs` but is never called by the proof engine. |
| `cancel_math.bv` | `count = count + (R + 1 - R)` — algebraic simplification reduces this to `count + 1` in `transition_graph.rs::simplify_expr`, but `check_convergence` scans the ORIGINAL unsimplified body. |
| `interval_step.bv` | `count = (count + R1) - R2` — outer `Sub` wraps `Add(count, R1)`. `check_convergence` only detects `Sub(count, N)` with bare count on left. |

**Fix status**: Each requires structural improvement to `check_convergence` — AND-pre extraction, integration with `detect_popcount_decay`, simplified-body scanning, and compound increment pattern matching. Deferred — not caused by Pattern B, and the non-runtime benchmarks (which are the ones `build_and_bench.sh` actually measures) all pass.

---

## 2026-06-09 — fasta LCG broken in rct txn (all output chars same)

**Issue**: `benchmarks/fasta.bv` outputs `qqqqq` instead of `xqjqf` (C reference). All iterations produce the same character `q` (ASCII 113), meaning the LCG seed never changes.

**Root Cause**: `rct txn` atomically batches all state writes until `term;`. Inside a single tick, `&seed = seed * IA` reads the original seed (42), then `&seed = seed + IC` ALSO reads the original seed (42), and `&seed = seed % IM` also reads 42. The three writes commit at `term;` — the last one wins: `seed = 42 % 139968 = 42`. Seed stays 42 forever.

The LLVM backend treats reactive writes as deferred (all reads see pre-tick state), consistent with the reactive semantics. But `fasta.bv` was written assuming sequential in-tick execution, which is incorrect for `rct txn`.

**Fix**: Convert to callable `txn` (not `rct txn`) so writes take effect immediately within the body iteration. Or restructure as a single assignment: `&seed = (seed * IA + IC) % IM;`.

**Lesson**: `rct txn` has deferred write semantics — all state reads within a tick see pre-tick values. Sequential `&field = ...` chains like `&seed = seed * IA; &seed = seed + IC; &seed = seed % IM;` do NOT accumulate — each reads the same original seed. Use callable `txn` for sequential state mutations within a single body iteration.

---

## 2026-06-09 — LLVM backend emits `constant float 0` (needs `0.0`)

**Issue**: `benchmarks/iir_filter.ll` contains `@b2 = constant float 0`. Clang rejects: `error: integer constant must have integer type`.

**Root Cause**: The float constant emission path for zero-valued floats uses `constant float 0` instead of `constant float 0.0`. LLVM's textual IR requires a floating-point literal for `float` type, not an integer `0`.

**Affected files**: LLVM backend's float constant emission (`emit_expr.rs` or `mod.rs`). All zero-initialized `Float` state fields and `const Float = 0.0` declarations.

**Fix**: Change the float literal LLVM emission to use `0.0` (or `0.0e+0`) instead of `0` when the value is zero.

**Lesson**: LLVM IR types must match their literal formats — `float` requires a floating-point literal, not an integer. The `i64` path was correct (`add i64 0, N`), but `float 0` should be `float 0.0`.

---

## 2026-06-09 — LLVM backend emits undefined `@str.0` reference

**Issue**: `benchmarks/fasta.ll` contains `getelementptr inbounds [6 x i8], [6 x i8]* @str.0` but `@str.0` is never defined. Clang rejects: `use of undefined value '@str.0'`.

**Root Cause**: The string constant emission path creates references to `@str.N` globals but doesn't emit their definitions. The `collect_strings()` function extracts string constants but the definition pass in `emit_global_strings()` (or equivalent) is missing or buggy.

**Affected files**: All benchmarks using string constants (`fasta`, `ring_buffer`, and any benchmark with `__get_env_int("BOUND")` or similar string arguments).

**Fix**: Ensure `@str.N` globals have corresponding definitions (type, initializer, alignment) emitted before their first use in the IR module.

**Lesson**: LLVM IR globals must be declared (`@str.0 = private unnamed_addr constant [6 x i8] c"BOUND\00"`) before being referenced in instructions. Use-before-definition is not forward-declared in LLVM IR — every reference must have a matching definition earlier in the module.

---

## 2026-06-09 — `precompute_sum.bv` emits infinite tick loop (no observable output)

**Issue**: `benchmarks/precompute_sum` binary never exits (timeout at BOUND=5). LLVM IR shows an infinite tick loop: `br label %tick` → `reactor_tick` → `br label %tick`. No observable side effect exists to prevent LLVM from eliminating the loop, but the loop remains because the `.o` linking path uses `cc -O2` (not `-O3`) and may not run the full SROA/mem2reg pipeline.

**Root Cause**: `const total: Int = 500` with budget=256. 500 > 256, so the compiler can't fully precompute. The `rct txn` body is pure (no FFI, no IO), so the reactor loop has no observable effect. At `-O3`, LLVM should eliminate the loop entirely, but the linking path (`cc -O2` on `.o` file) may not be aggressive enough.

**Fix**: Either (a) increase budget to >= 500, (b) decrease total to <= 256, or (c) add an FFI output to make the loop observable. The benchmark's purpose is to test compile-time precomputation — it should use `total <= 256` so the budget covers it, or use `--optimize-budget 2048`.

**Lesson**: A benchmark that tests precomputation must have its bound within the optimization budget. `const total = 500` with default budget 256 produces a silent infinite loop — the worst failure mode. Add `#!exit` or keep bounds within budget.

---

## 2026-06-10 — LLVM backend: negative float constants in init_state stored as i64 (8 bytes) instead of float (4 bytes)

**Issue**: `nbody_newton.bv` produced `-nan` instead of `-0.169203` at BOUND=5. All outer planets' Y and Z positions were zero instead of their correct negative values (by1=-1.16, bz1=-0.1036, by3=-15.11, etc.).

**Root Cause**: In `src/backend/llvm/mod.rs:2110-2117`, the `emit_init_state` function had this code for `Expr::Neg` initializers:
```rust
Some(Expr::Neg(ref inner)) => {
    let s = match inner.as_ref() {
        Expr::Float(f) => float_to_llvm_hex(-*f),
        ...
    };
    writeln!(out, "  store i64 {}, i64* {}, align {}", s, p, self.align_of("i64")).ok();
}
```
When a `let` field had a negative float literal (e.g., `let by1: Float = -1.1603`), the parser stored it as `Neg(Literal(Float(1.1603)))`. The `Expr::Neg` arm computed the correct hex bit pattern but stored it as `i64` (8 bytes) instead of `float` (4 bytes). This wrote 8 bytes of zero into a 4-byte float slot, corrupting the adjacent float field.

Additionally, the match arm only matched `Expr::Float(f)` inside Neg, but the actual AST stores `Expr::Literal(LiteralExpr::Float(f))` inside Neg. The catch-all `_` branch stored `i64 0`.

**Fix** (`src/backend/llvm/mod.rs`): Updated `Expr::Neg` arm to match both `Expr::Float` and `Expr::Literal` with `LiteralExpr::Float`, and emit `store float` (4 bytes) instead of `store i64` (8 bytes):
```rust
Some(Expr::Neg(ref inner)) => {
    match inner.as_ref() {
        Expr::Float(f) | Expr::Literal(lit) if matches!(lit.as_ref(), LiteralExpr::Float(_)) => {
            let f = /* extract value */;
            let h = float_to_llvm_hex(-*f);
            let bits_reg = format!("%ip{}b", reg - 1);
            writeln!(out, "  {} = bitcast i32 {} to float", bits_reg, h).ok();
            writeln!(out, "  store float {}, float* {}, align {}", bits_reg, p, self.align_of("float")).ok();
        }
        ...
    }
}
```

**Lesson**: Always match the correct type when storing initial values. The LLVM IR must use `store float` for float fields — `store i64` writes 8 bytes, corrupting adjacent fields. Also, verify the AST uses `Literal(Float(…))` not bare `Float(…)` — the feature dispatch layer adds an `Expr::Literal` wrapper.

## 2026-06-10 — LLVM backend: non-SSA state field loads return Type::Int for float fields

**Issue**: In non-SSA transaction mode (the default), float arithmetic on state fields used `add i64` on boxed bit patterns instead of `fadd` on native floats. This caused float computations like `sum + a + b` to produce garbage results when all operands were state fields (no Float-type constant in the expression tree).

**Root Cause**: In `src/backend/llvm/emit_expr.rs:178`, the non-SSA state field load for float types:
```rust
s if s == "float" => { 
    let i = format!("%if{}", self.txn_counter); 
    ...
    self.reg_float_cache.insert(v.clone(), ld.clone()); 
    // NO RETURN — falls through to default
}
```
...did not return a `TypedRegister`. The function's default return (line 631) treated the value as `Type::Int`:
```rust
TypedRegister { name: v, ty: Type::Int }
```
Then in `emit_binop` (emit_expr.rs:845), the check `a.ty == Type::Float || b.ty == Type::Float` was false, so it emitted `add i64` instead of `fadd fast float`. The `reg_float_cache` had the correct native float register, but `emit_binop` checked the type first and never reached the cache lookup.

This worked in `float_math.bv` because constant globals (like `A00`, `Q00`) had type `Type::Float`, so at least one operand in each arithmetic expression triggered the native float path.

**Fix** (`src/backend/llvm/emit_expr.rs`): Added `return TypedRegister { name: v.to_string(), ty: Type::Float };` to the float state field load arm.

**Lesson**: Every code path in `emit_expr` must return the correct `TypedRegister::ty`. The default `Type::Int` fallthrough is a trap for non-int types. When adding new type-aware paths, verify the return type propagates to consumers like `emit_binop`.

## 2026-06-09 — C reference benchmarks fail at BOUND=5 (exit code 6)

**Issue**: Several C reference binaries (`float_math_c`, `float_math_nonzero_c`, `const_heavy_c`) exit with code 6 and produce no output when run with `BOUND=5`.

**Root Cause**: Unknown — likely missing `-lm` link flag or a crash in the C code when `BOUND` is small. `float_math_c` uses `__print_float` which writes to stderr via the runtime; if the runtime function is `fprintf` and no `-lm` is needed, the crash may be a null pointer or assertion in the runtime init path.

**Affected**: `float_math_c`, `float_math_nonzero_c`, `const_heavy_c`. Also `precompute_sum_c` outputs nothing (expected — it has no FFI output, just internal arithmetic).

**Fix**: Investigate the C reference binaries individually. Likely solutions: add `-lm` to all C builds, or fix runtime init to handle `BOUND`=5 correctly.

**Lesson**: C reference correctness must be verified at small BOUND values before trusting them as reference outputs. A C binary that crashes on `BOUND=5` is not a valid reference.

