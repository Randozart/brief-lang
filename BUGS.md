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