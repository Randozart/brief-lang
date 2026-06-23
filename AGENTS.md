# Brief Compiler - Agent Guidelines

See CLAUDE.md for complete documentation. This file ensures OpenCode picks up the same guidelines.

## Quick Reference

### Philosophy (One Sentence)

Brief's contract system (`[pre][post]`) is not a correctness tax — it is
information the compiler uses to optimize harder. Safety IS the
optimization enabler. Full machine access is available through contracts
proven at compile time, not `unsafe` blocks. See
`docs/architecture/philosophy.md` for the full thesis.

### GLUE — General Language Unification Engine

GLUE is a universal FFI broker built on Brief's `meld` system. Any two
languages that consume LLVM-compatible object code can be linked through GLUE.
Neither language knows Brief exists. Both see their own native interface.
Brief is the invisible translator — `meld` proves type compatibility at
compile time, `frgn` declares calls into the target language, `#export`
exposes functions to the caller.

**The bridge is native object code.** No C compiler, no `extern "C"`, no `cc`
crate. Brief emits LLVM IR → native `.o`/`.a`/`.wasm`. The foreign language's
linker consumes it directly.

**GLUE adapters use Brief's `$!` macro system**, not a separate template engine.
A `$!macro` takes the bridge's `#export`/`frgn`/`meld` declarations at compile
time and emits native wrapper source code for the target language. Adding a
language = writing one `.bv` macro file.

**Key directives:**
- `brief link <path> <function>` — analyzes a foreign library, generates a `.bv`
  with `frgn` declarations. Cross-references against the `Intrinsic` enum in
  `src/ast.rs` — if a `frgn` name matches an intrinsic, emit `intrinsic_call#()`
  instead. This replaces the old TOML binding system.
- `brief export <bridge.bv> <language>` — compiles to `.a` (library mode, no
  `main`), reads `glue.dbvl` to find the adapter entry for `<language>`, invokes
  the `$!` macro for that language, generates native wrappers.
- `glue <target> <function> <language>` — one-shot wrapper: `brief link` + `brief export`.

**GLUE protocol files:**
- `glue.dbvl` — Data Brief Lines adapter registry (one language per line)
- `glue.dbvs` — Data Brief Schema that validates `glue.dbvl` entries
- Adapter macros live in `glue/adapters/<language>.bv`

**Data Brief naming:**
- `.dbv` — Data Brief, universal data extension. The `V` stands for "Brief."
- `.dbvl` — Data Brief Lines. Raw data, one line per entry.
- `.dbvs` — Data Brief Schema. Validates `.dbvl` and `.dbv` files.

### Correctness Over Speed (All Development)

The Brief compiler is built to be correct, not to ship an MVP as fast as
possible. Being able to test between changes is important, but test with code
that won't need rewriting immediately.

**This applies to every feature, not just GLUE.** Before any implementation:
1. Understand WHY the approach is correct (not just THAT it works)
2. Document the reasoning in the commit
3. Use Brief's existing systems (`$!` macros, `meld`, contracts) rather than
   building parallel Rust infrastructure that will be thrown away during
   self-hosting. Every throwaway Rust template engine is technical debt that
   must be rewritten in Brief later.
4. When you encounter a deprecated pattern (frgn where intrinsic exists,
   double `[true][true]` contracts, TOML bindings for intrinsics), fix it
   rather than propagating it. The codebase is a living artifact — clean as
   you go.

**NO prototyping — build clean.** If a feature needs experimentation, do it in
a sandbox file, not in the main codebase. Every commit should be production-
quality code that could ship as-is. Stubs, `todo!()`, and `unreachable!()` in
committed code are bugs.

### Commands
- **Build**: `cargo build`
- **Test**: `cargo test --lib`
- **Test backend registry**: `cargo test --lib -- backend::tests`
- **Compile RBV**: `./target/release/brief-compiler rbv <file.rbv>`
- **Benchmark**: `bash benchmarks/build_and_bench.sh` — always use this harness (nanosecond CLOCK_MONOTONIC, 5-iteration average). Ad-hoc timing produces false hangs and imprecise numbers.

### Examples Library

Whenever you encounter an obscure or under-documented Brief syntax pattern, create
a minimal self-contained example in `examples/` and mention it in the relevant
architecture doc. Name the file after the pattern (e.g., `examples/wasm-import.rbv`).

This builds a living library of real syntax usage. If an example already exists
for a pattern, add a comment referencing it rather than duplicating.

### File Types
- **.bv** - Brief (standard Brief file, cosmopolitan tier — any FFI, any language, OS assumed)
- **.sbv** - Strict Brief (full contracts required, no sugar defaults)
- **.abv** - Accelerated Brief (native GPU compilation — always compiles to SPIR-V, no FFI, restricted types, GPU intrinsics only. Also known as "Brief Accel")
- **.rbv** - Rendered Brief (Brief + View, compiles to web frontend. Like `.tsx` is to `.ts`. Also known as "Brief Render")
- **.srbv** - Strict Rendered Brief (full contracts required in web target)
- **.ebv** - Embedded Brief (bare metal — no OS, no GC. C/Rust FFI allowed but Python/Java warned. Also known as "Brief Embed")
- **.sebv** - Strict Embedded Brief (full contracts required, bare metal)
- **.cbv** - Circuit Brief (pure logic graph — no FFI, no external deps, only synthesizable types. Contracts must be total. Outputs Verilog/VHDL/SV. Also known as "Brief Circuit"; formerly Hardware Embedded Brief / `.hebv`)
- **.dbv/.dbvs/.dbvl** - Data Brief (configuration with schema, think `.xml`/`.xmls`/`.jsonl`. Also known as "D-Brief" or "Brief Data")

### Contract Sugar Syntax

Brief provides sugar for single-sided contracts. Use these where possible in the stdlib
to teach readers the pattern:

| Syntax | Precondition | Postcondition | Meaning |
|---|---|---|---|
| `[pre][post]` | `pre` | `post` | Full contract (both sides) |
| `[[post]` | `true` (omitted) | `post` | Postcondition only, no guard. **The opening `[[` means the precondition was omitted.** |
| `[pre]]` | `pre` | `true` (omitted) | Guard only, no guarantee. **The closing `]]` means the postcondition was omitted.** |

Memory aid: the left bracket `[` is always the precondition. `[[` = two left brackets = the
first one opens an empty precondition (defaults to `true`), the second opens the postcondition.
`]]` = two right brackets = the first closes the precondition, the second closes an empty
postcondition (defaults to `true`).

**Banned in**: `.sbv`, `.srbv`, `.sebv`, `.cbv` (strict tiers require explicit both-sided contracts).
**Allowed in**: `.abv`, `.bv`, `.ebv`, `.rbv` (sugar is the recommended style).

### Pipe Chaining Sugar

Brief provides pipe chaining (`|>`) as a syntactic sugar that desugars to
flat let-bindings before typechecking. All three active backends see only
the desugared form — zero runtime overhead.

```brief
x |> f()            // f(x) — pipeline value prepended as first arg
x |> f() |> g()     // g(f(x)) — multi-step chain
x |> f() .|> g()    // .|> reads from 1 position back in pipeline stack
x |> f() .2|> g()   // .N|> reads from N positions back
x |> f              // auto-wrapped: f(x)
f() |> g()          // chain starts with function call (no initial input)
```

See `docs/architecture/features/pipe.md` for full documentation.

### Critical Philosophy

**CONTRACT-FIRST**: Contracts are the source of truth. Never weaken contracts to match lazy code.

**NO MAGIC**: Never add hardcoded Rust string matches as "built-in" functions.
- If a `.bv` file needs `is_digit`, import `char` from `"std/char.bv"` — NOT a Rust match arm.
- If a `.bv` file needs `None`, import `option` from `"std/option.bv"` — NOT pre-populating state.
- The FFI system (`frgn from "..."`) and the standard library are the transparent paths. Use them.

**INTRINSICS BEFORE FRGN**: Before reaching for a `frgn` declaration, check if
an `Intrinsic` variant already exists that does the same thing. This is
especially critical in `.abv` (Accelerated Brief) files, where `frgn` is banned:
  - Need to print? → `print_int#`, `print_float#`, `put_char#` (already exist)
  - Need input? → `get_env_int#`, `read_stdin#` (already exist)
  - Need GPU thread ID? → `get_global_id#`, `get_local_id#` (already exist)
  If no suitable intrinsic exists, add one to `src/ast.rs` — never add `frgn`.

**SELF-DOCUMENTING FAILURE**: Before fixing any issue:
1. Understand WHY the fix works (not just THAT it works)
2. Document the root cause in BUGS.md
3. Ensure the fix doesn't violate Contract-First or No Magic

### Anti-Patterns (NEVER DO)
- Changing `[product > 0]` to `[true]` because code doesn't set product
- Using generic contracts like `[true]` that pass everything
- Adding postconditions that don't guarantee specific outcomes
- Adding Rust string-match built-ins when the standard library or import system should be used
- Pre-populating interpreter state with enum constants (None, Some, Ok, Err)
- Adding `x == x` self-references in preconditions to force liveness
- Adding synthetic exit-condition fields solely to prevent dead-field elimination
- **Hardcoded `from` strings**: `from "libruntime"` is magic — parsed and discarded. Use `from "c"` or omit `from` entirely (symbol resolves from `import "link/..."` targets).
- **Hardcoded runtime declares**: `__rt_init`, `__rt_wait`, etc. must be declared as `frgn` in `std/rt.bv` and imported by the user — never hardcoded in `emit_declares()`.
- **Name-based interpreter dispatch**: Matching on `fn_name == "insert"` instead of dispatching on `Value::HashMap` — dispatch on the type, not on a string.
- **`"None"`/`"Err"` discriminant magic**: Never match on variant names for discriminants. Use the enum declaration order.
- **Type-based dispatch**: In the interpreter, dispatch on `Value` variant, not on string-matching the function name.
- **Runtime type tags for dispatch**: Never add a runtime tag to a value to determine "which type it really is." The type is determined statically at each site by the expression's type annotation (`TypedRegister.ty`). Runtime tags add overhead to every operation and contradict the zero-cost abstraction goal.
- **Implicit coercions between types**: Never allow `let x: B = a_val` where `a_val: A` to silently reinterpret the bits. All type reinterpretations must be explicit via `as` casts. Implicit coercions violate the principle of least surprise and make control flow harder to reason about.
- **Dynamic optimization path switching**: Never switch between memory layouts or optimization strategies at runtime based on usage counters. All optimization paths (short, hot dual, unpacked) must be chosen statically at compile time based on liveness evidence. Wrong predictions mean wasted memory, never incorrectness.
- **Transitive compatibility inference**: If `A` is compatible with `B` and `B` is compatible with `C`, never infer that `A` is compatible with `C`. Each compatibility relationship must be explicitly declared. Inference across the graph introduces invisible couplings that break with non-trivial layouts.
- **Weakening existing optimization paths for new features**: Every addition to optimization passes (fast-path registry, projection dispatch, etc.) must be an **additional match arm**. Never modify existing arms. The `_ => return None;` fallthrough must remain unchanged — non-feature types must continue to work exactly as before.

### Observability as Liveness

A program that produces no observable effect IS dead code. The compiler is correct to eliminate it.

Brief's liveness model: **a value is live if an FFI call consumes it.** Every program must interact with the world — print, file I/O, network — via `frgn` calls.

If the compiler folded your hot loop to `store i64 N`, **the compiler is right.** Your program produced no observable output. The fix is NOT liveness hacks (`x == x`, synthetic exit fields). The fix IS `frgn __print_int(result)`.

The C reference must use the SAME observable. Symmetric benchmarks, symmetric optimizations.

#### `term! -> swan_song` is the correct liveness pattern for terminal programs

When a program must run a specific FFI call (print, write) as its final act before exiting,
use `term! -> frgn_call(args);`. The `term!` emits `ret` — a function terminator that the
optimizer cannot eliminate. The swan song runs as a statement before `ret`, so the FFI call
is structurally live by construction.

**Do NOT:**
- Use `io_pending` or other opaque triggers purely to prevent fold elimination
- Add `#!exit` pragmas when `term!` already terminates the program
- Add synthetic exit-condition fields or `x == x` self-references
- Complain that `main` is just `ret` — if your program produces no observable output,
  the compiler is RIGHT to eliminate it. The fix is `frgn __print_int(result)`, not hacks.

**The correct pattern:**
```brief
frgn __get_env_int(name: Ptr<Byte>) -> Int ;
frgn __print_int(n: Int) -> Bool ;
frgn XXH64(data: Int, len: Int, seed: Int) -> Int ;

let N: Int = __get_env_int("BOUND");   // runtime-determined — prevents precomputation
let done: Int = 0;
let result: Int = 0;

rct txn compute [done < N][done == N] {
    [done == N - 1] {
        &result = XXH64(addr, len, 0);
        term! -> __print_int(result);   // program exit, swan song runs before ret
    };
    &done = done + 1;
    term;
};
```

**Every tier (interpreter, LLVM backend) must handle `term! -> swan_song` identically.**
If adding a new backend, implement `Statement::TermBang` with swan song as a blocker
before the backend ships — it is the canonical way to produce observable terminal output.

## Benchmark Philosophy

See `docs/architecture/benchmark-strategy.md` for the full benchmark design —
tag system, size-gated detection, correctness verification, and tagging
conventions. This section summarizes the key rules.

### Benchmarks test semantic goals, not syntactic features

Brief benchmarks answer: **"Can Brief compute X with competitive performance vs C?"** — not "Does Brief have feature Y?" Implement the semantic goal using Brief's idioms, not a line-by-line port.

### Benchmarks exist to find flaws in Brief

A benchmark that fails (won't compile, hangs, wrong output) tells you something is missing. A benchmark that is "too good to be true" (0.001s for real work) tells you the compiler folded your dead code. Both are diagnostic signals.

If Brief beats C by an implausible margin, suspect the C reference has been hobbled (volatile, unused return). Fix the C reference — the only valid victory is symmetric, structurally-live programs.

### When a benchmark can't be implemented as-is: find the isomorphism

| C pattern | Brief-idiomatic equivalent |
|-----------|---------------------------|
| `malloc` + pointer navigation | Contract-proven struct arrays + index traversal |
| `double u[N]` (runtime-sized) | Contract-proven compile-time bound + `<-` push |
| `HashMap<String, Int>` | Integer-encoded keys + flat field lookup |
| `for (i=0; i<N; i++)` loop | Convergent contract `[count < N][count == N]` + straight-line body |
| `while (true)` + `break` | Reactive transaction with natural death |
| Recursive `enum Tree` | Flat struct pool with index navigation |

### Each benchmark teaches one compiler lesson

| Benchmark | Lesson |
|-----------|--------|
| fasta | FFI output in hot loop prevents fold elimination |
| fannkuch-redux | 12-field rotation exercises SROA scalar decomposition |
| mandelbrot | Complex arithmetic + escape tracking = guarded integer pipeline |
| knucleotide | 64-field guarded dispatch = compiler switch-gen vs C array indexing |
| spectral-norm | Float arrays at contract-proven scale = allocation strategy |
| binary-trees | Struct pool allocation + index-based tree walk = memory model |

### Two benchmark categories — runtime vs optimizer

Every benchmark is tagged as either `--runtime` or `--optimizer` in the harness.

| Category | Tag | What it measures | Criteria |
|----------|-----|------------------|----------|
| **Runtime** | `--runtime` | Throughput of compiled code | FFI call in the hot loop body. LLVM cannot eliminate the loop. |
| **Optimizer** | `--optimizer` | Compile-time folding power | All `const` inputs + no FFI in hot loop. LLVM may eliminate the loop. |

A benchmark cannot be both. If it has no observable side effects in its hot loop, it is an optimizer benchmark — runtime timing is meaningless.

The harness detects precomputed binaries by `.text` size ratio (< 25% of C → `precompute_ok`, skip timing). Correctness (same input → same output) is checked for all benchmarks.

`bash benchmarks/build_and_bench.sh --runtime` to test only runtime benchmarks.
`bash benchmarks/build_and_bench.sh --optimizer` to test only optimizer benchmarks.
`bash benchmarks/build_and_bench.sh --correctness` to verify output only.

### The C reference is symmetric, always

Both get `-O3 -ffast-math` from the same clang. No `volatile`, no unused variables. Any asymmetry is a signal of a missing Brief optimization — fix the compiler, not the C code.

### Useful utilities become standard library functions

When a benchmark produces a general-purpose helper (rolling hash, vector math, frequency counting), extract it into `lib/std/`. Any function designed for a benchmark that could serve as a general-purpose utility MUST be added to `lib/std/`.

### Correct Approach
- Keep contract `[product > 0]` — fix code, not contracts
- `UndefinedForeignFunction("is_digit")` → `import char from "std/char.bv"`
- Import resolver can't find file → fix search path, not interpreter

### Symmetric by Default

Every Brief benchmark must compute the **same output** as its C reference for
the same input. If Brief's idiomatic approach differs fundamentally from C's
(different data structures, control flow, or algorithm), create **two**
benchmarks:

| Variant | Intent |
|---------|--------|
| **Symmetric** (`_sym`) | Mirrors C step-for-step using Brief features. Answers: "Given the same algorithm, does Brief's throughput match C's?" |
| **Idiomatic** (`_idio`) | Uses Brief-native patterns (contract-proven loops, reactive transactions) for the same semantic result. Answers: "Can Brief's optimizer find a better path?" |

Both must produce identical output for the same input. Neither claims to be
the single "fair" comparison. When fixing a broken benchmark (wrong output,
wrong algorithm), fix the bug — do not split into two variants unless the
approaches genuinely differ.

See also: Hillel Wayne's observation about `queue_drain` — C and Brief
were computing the same result through different algorithms. The fix is
not to hobble either version but to create a symmetric pair.

### Precomputation is Correct, Not a Bug

If the compiler folds your entire hot loop to `store i64 N, main` is `ret`, **the compiler is right.** It had all information at compile time and correctly precomputed the result.

This happens when the loop bound is compile-time known (e.g., `const N: Int = 10` or a fixed-size list literal `[1..10]`). The compiler proves the bound within the `--optimize-budget` and precomputes all iterations.

**Fighting precomputation is wrong.** Do not:
- Add `x == x` self-references to force liveness
- Add synthetic exit-condition fields
- Add `#!exit` conditions referencing dead fields
- Complain that `main` is just `ret`

**If a benchmark must run at runtime**, make the bound runtime-determined:

```
let N: Int = __get_env_int("BOUND");      // ✓ runtime — not precomputable
const N: Int = 50000000;                   // ✗ compile-time — precomputable
```

The `--optimize-budget` flag controls how many transactions the compiler will simulate. Default is 256. Bounds below the budget are precomputed; bounds above emit a runtime loop.

If the compiler precomputes your benchmark, **increase the budget or make the bound runtime-determined.** Never weaken the contract or add hacks. The system works as designed.

## Language Architecture

Brief is a **general-purpose programming language**. The computational primitive is the **reactive transaction** (`rct txn`):
- **Precondition** (guard): `[x > 0 && y < N]`
- **Postcondition** (contract): `[x == N]`
- **Body**: `{ &x = x + 1; &y = y * 2; }`

Loops are transactions with bounded convergence. Recursion is a transaction chain with proved termination. Every optimization (purity folding, dead-field elimination, SROA, SLP) applies because contracts give the compiler enough information.

### Misconceptions to Avoid

| Wrong | Correct |
|-------|---------|
| "Brief is a reactive state machine DSL" | Brief is general-purpose. Transactions ARE loops, iteration, and recursion. |
| "Brief has no arrays/strings/collections" | Interpreter supports `List<T>`, `String`, `HashMap`, `HashSet`, `Stack`, `Queue`, `StringBuilder`. Stdlib has 26 modules. |
| "Brief can't do tree/heap benchmarks" | Interpreter supports recursive enums, structs, field access, match. |
| "Brief needs malloc/FFI for buffers" | Compiler proves bounds from contracts, allocates accordingly. |
| "The LLVM backend is the language" | Interpreter is the reference. Backend is an optimization pass. |

### Two-Layer Architecture

1. **Interpreter** — reference implementation. Validates EVERYTHING before any codegen.
2. **LLVM Backend** — compiles to LLVM IR with optimizations. Never weakens existing optimization paths.

## Interpreter Completeness

### Expressions — Except where noted, all fully implemented
| Status | Variants |
|--------|----------|
| ✅ | Integer, Float, String, Char, Bool, Term, Identifier, OwnedRef, PriorState |
| ✅ | Add, Sub, Mul, Div, Mod, Eq, Ne, Lt, Le, Gt, Ge, Or, And, Not |
| ✅ | Neg, BitNot, BitAnd, BitOr, BitXor, Shl, Shr |
| ✅ | Call, ListLiteral, ListIndex, Projection (18 targets), FieldAccess |
| ✅ | StructInstance, ObjectLiteral, PatternMatch, Concat |
| ✅ | Slice, MultiSlice, Block, Tuple, TupleDestructure, Cast, Match |
| ✅ | ArrowMut, ArrowDiscard, ArrowTransfer (dispatch on Value type, not string names) |
| ✅ | MapLiteral, SetLiteral (evaluate to Value::HashMap, Value::HashSet) |
| ⚠️ | **ForAll, Exists** — FULLY REMOVED from AST, parser, lexer, and all match arms. |

### Statements — All fully implemented
Assignment, Let, InlineAsm, Expression, Term (with optional swan song), TermBang (with optional swan song), Escape, Guarded, Unification, LocalTrigger, SyncBlock.

### Known Gaps
- **Recursive defn calls**: No recursion guard or stack depth limit. Deep recursion overflows the Rust interpreter.
- **ForAll/Exists**: Removed from surface syntax.
- **Interpreter built-in method dispatch**: `dispatch_method_by_type` still matches on function name strings. Deferred — should use FFI registry (Path A: register all operations under `"std::HashMap::insert"` etc., resolve through `ffi_name_to_location`).
- **LLVM backend**: Slice/MultiSlice/Tuple/MapLiteral/SetLiteral/ArrowTransfer/projection stubs remain (see Backend Gaps below).

### Feature Modules (`src/features/`)

New features follow the Pattern-B convention: a single directory with `mod.rs` + per-aspect files implementing the trait dispatch system.

| Feature module | Files | Status |
|----------------|-------|--------|
| `literal/` | `mod.rs` | ✅ |
| `binary_op/` | `mod.rs` | ✅ |
| `call/` | `mod.rs` | ✅ |
| `projection/` | `mod.rs` | ✅ |
| `collection/` | `mod.rs` | ✅ |
| `tuple/` | `mod.rs` | ✅ |
| `field/` | `mod.rs` | ✅ |
| `pattern/` | `mod.rs` | ✅ |
| `block/` | `mod.rs` | ✅ |
| `arrow/` | `mod.rs` | ✅ |
| `subtype/` | `mod.rs` | ✅ |
| `sigcall/` | `mod.rs` | ✅ |
| `dbvl/` | `mod.rs` | ✅ |
| `ellipsis/` | `mod.rs` | ✅ |
| `stmt/` | `mod.rs` | ✅ |
| `toplevel/` | `mod.rs` | ✅ |
| `macros/` | `context.rs`, `expand.rs`, `template.rs`, `hygiene.rs`, `macro_.rs` | ✅ |

Note: Rust 2021 reserves `macro` as a keyword, so the directory is `macros/` (plural) and the macro-specific expansion file is `macro_.rs` (trailing underscore).

## LLVM Backend — All Gaps Closed (2026-06-21)

Additive only — never weaken existing optimization paths.

All expression types from the original gaps list (`Slice`, `MultiSlice`,
`Tuple`, `StructInstance`, `ObjectLiteral`, `FieldAccess`, `MapLiteral`,
`SetLiteral`, `ArrowTransfer`, `<-` push/pop/discard, and all projection
operators including `Keys`, `Values`, `Contains`, `Pop`, `Index`) have been
**fully implemented** in `emit_expr.rs`. `ForAll`/`Exists` were **removed**
from the AST entirely. As of 2026-06-21, there are **no known stub or
degraded expression paths** in the LLVM backend.

### Expressions — All Fixed (2026-06-21)

The following expression codegen gaps were fixed:

**Slice stride/mask** — Stride is now used in the copy loop (`src[start + i*stride]`)
with ceiling-division count. Mask applies a second-pass filter loop that binds `_`
to each element and evaluates the mask expression. Both implemented inline with
LLVM IR loops.

**MultiSlice stride/mask/range** — `Coord(Range)` allocates a new list and copies
the sub-range `[lo..hi)`. `Stride` emits a step-by copy loop. `Mask` emits a
filter loop with `_` binding. All three produce native LLVM IR loops without
runtime library calls.

**`FloatToStr`/`ToStr` working paths** — Replaced buggy `@__snprintf__`-based
implementation with `@__float_to_str` / `@__to_str` C runtime functions.
Return type changed from `i8*` to `i64` to match C functions.

**`bytes` projection for struct types** — Now computes `fields.len() * 8` when
the source type is `Type::Custom(name)` and the struct is in `struct_types`.

**`FieldAccess` field not found** — Now emits `call void @llvm.trap()` instead
of silent `add i64 0, 0 ; field`.

**`UserDefined`/`UserDefinedWithArg` projections** — Now emit `@llvm.trap()`
when `try_projection_fast_path` fails (instead of silent `add i64 0, 0`).

**Missing declares** — 8 `declare` statements for runtime functions
(`__trim_left__`, `__trim_right__`, `__to_lower__`, `__contains_at__`,
`__find_from__`, `__splitn__`, `__float_to_str`, `__to_str`) were added to
`emit_toplevel.rs`.

### Error-Guard Stubs — All Fixed (2026-06-21)

The following error-guard stubs previously emitted `add i64 0, 0` silently;
all now emit `call void @llvm.trap()` before the zero return:

**Intrinsic error-guards (wrong arg count)**: `sort`, `reverse`, `range`,
`trim_left`, `trim_right`, `to_lower`, `contains_at`, `splitn`, `int_to_str`,
`strlen`, `float_to_str`, `to_str`, `size`, `pop`, `contains`, `keys`/`values`,
`read_file`.

**Projection error-guards (unrecognized field/type)**: `Expr::Identifier` not
found, `ProjectionTarget::Bytes` for unknown type, `ProjectionTarget::UserDefined`
and `UserDefinedWithArg` fallthrough, `Expr::FieldAccess` field not found.



### Top-Level — Struct/Enum Layout
| TopLevel | Notes |
|----------|-------|
| **Struct** | No LLVM struct type generated. StructInstance/FieldAccess use field-offset arithmetic (GEP), not LLVM struct types. No TBAA on struct fields. |
| **Enum** | Tagged union layout via ad-hoc stack alloca + discriminant prefix. No LLVM struct type. |
| Signature, Import, LinkDependency | Correctly skipped — frontend-only. |

## Key Philosophy for Backend Work

### Never Weaken Optimizations for New Features
Existing optimization paths MUST NOT regress. All additions are additive — new match arms only, no touching existing fold/precompute/dispatch paths.

### The Interpreter is the Source of Truth
If the interpreter produces the correct result, the LLVM backend must compile it. Fix the codegen, never the interpreter.

### Contracts Enable Optimizations
Preserve contract information in codegen so the optimizer can reason about it.

## For OpenCode

1. Read CLAUDE.md and this file for full context
2. Follow Contract-First Philosophy
3. Never weaken contracts - fix code instead
4. Test with `cargo test --lib` before committing
5. Document bugs and root causes in BUGS.md
6. Never add Rust built-ins for things the standard library should provide
7. **No prototyping — build clean**: Every optimization in its proper module. Never inline new analysis into codegen.
8. **Never weaken C benchmarks**: Fix Brief to match or beat C, not hobble C.
9. **Interpreter IS the reference**: Add to interpreter first, then codegen.
10. **Benchmarks on our own terms**: End-to-end results. Features for benchmarks must add language value.
11. **NEVER discard staged or uncommitted work without asking.** The git index (staging area) holds work-in-progress from prior sessions that may be uncommitted but critical. Before any destructive action (`git checkout --`, `git restore`, `rm -f`, `git reset --hard`), inspect everything that will be destroyed. If in doubt, `git stash` instead of discard — stashes are recoverable, `git checkout --` is not. A single `git restore --staged .` followed by `git checkout -- <files>` can erase hours of uncommitted work with no recovery path.
12. **Plan files**: Every plan-driven session writes a `docs/plans/YYYY-MM-DD-<topic>.md` with datetime stamp before starting work. The plan is committed alongside the implementation code.
13. **main.rs edit safety**: `main.rs` has many call sites with long arg lists. When adding or removing function parameters, anchor `oldString` to the exact `)` boundary — never match multi-line blocks that could capture beyond the intended scope. A single `oldString` spanning 50+ lines can accidentally delete half the file if the file state is inconsistent.
13. **Architecture docs**: Update `docs/architecture/` in the same commit as structural changes.
14. **Kani**: Add proof harnesses for all new safety-critical code.
15. **Praetor**: Run on new/changed files; verify complexity ≤ 15, lines ≤ 100, params ≤ 6.
16. **ALWAYS FINISH WHAT YOU START**: Never leave stubs, placeholders, `todo!()`, `unreachable!()`, or `; TODO:` comments in committed code. Every feature must be fully wired through the entire pipeline — parser → AST → analysis → codegen → tests. If a function is too complex to finish in one session, break it into concrete sub-functions and implement each one. If a module is added, it must be called from at least one real code path. Stubs in committed code are bugs.

## Self-Hosting Pipeline

The Brief-in-Brief compiler lives in `lib/compiler/`. Run via `brief-compiler selfhost <file.bv>`.

**NOT currently being worked on.** Broken at parser level (multidimensional slice bug). Deferred.

**Do NOT add as built-ins**: `is_digit`, `is_alpha`, `is_alphanumeric`, `is_upper`, `is_lower`, `is_space`, `char_to_string`, `None`, `Some`, `Ok`, `Err`. These are in `lib/std/` — import them.

## Optimization Design

See `docs/design/optimization-decision-tree.md` for the full decision tree — precomputation → enum dispatch → async → folded struct-SSA → fallback — and the rationale for each path (phi reduction, SROA pipeline, why struct phis were eliminated, cross-cutting optimizations).

## Critical Context

### Already Done (Don't Redo)
- **Projection operator (`:>`)** — fully implemented, 8 targets (Size, Bytes, Ptr, Alignment, Range, Popcount, LeadingZeros, TrailingZeros, Absolute, BitReverse, Type, Ptr!, Match, Keys, Values, Contains, Pop, Index). `Expr::ListLen` deleted. All stdlib migrated.
- **`<-` arrow syntax** — fully implemented for List, HashMap, HashSet, Stack, Queue via `ArrowMut`/`ArrowDiscard`/`ArrowTransfer`. Dispatch on Value type, not string names.
- **`->` vs `<-` convention** — `->` reserved for return types and swan songs; `<-` exclusively for collection mutation (`&` sigil marks mutated operand).
- **`term -> swan_song;`** (commit action) and **`term!`** (program exit) — both implemented in interpreter + LLVM backend.
- **`#assume_event(name)`** and **`#assume_shape(guard, action)`** — pragma infrastructure in parser, analysis, LLVM.
- **`#` prefix for all directives** — reuses Hashtag/Attribute parsing.
- **Dead-field elimination** — liveness analysis drops stores to unobserved fields.
- **Dispatch-chain collapse** — preconditions evaluate against pre-tick state.
- **Thread pool async dispatch** — portable barrier + auto-inference of conflict-free txns.
- **SLP hazard analyzer** — disables SLP when peak register demand exceeds hardware.
- **Equality saturation** — lightweight recursive simplification (5-pass fixpoint, 9 rewrite rules).
- **Compile-time PGO** — interpreter profiling guides LLVM branch weights.
- **LTO pipeline** — merges `brief_rt.c` bitcode with program IR.
- **MMIO / DBVS / hardware handoff** — address plumbing, schema validation, Vivado XSA extraction.
- **alka/on_exit disabled** — parser paths commented out, code left for future revisit.
- **`__rt_poll()`** — non-blocking event drain at main() entry.
- **Sync domains (Phase 11)** — `sync(domain)` prefix on `txn`/`defn`, `TopLevel::SyncGroup`, `Statement::SyncBlock`.
- **BracketOp (MultiSlice refactor)** — flat `Vec<BracketOp>` replaces `coordinates`+`mask`. Ops: `Coord`, `Mask`, `Stride` in any order.
- **MapLiteral / SetLiteral** — `{"a": 1}` evaluates to `Value::HashMap`, `{1, 2, 3}` to `Value::HashSet`. ObjectLiteral `{field: val}` preserved.
- **Value::Tuple** — true distinct variant. `Expr::Tuple` evaluates to `Value::Tuple`. Tuple destructure handles both `List` and `Tuple`.
- **ProjectionTarget::Index(usize)** — tuple indexing via `pair :> 0`.
- **`$`/`$!` macro system (Phase 1a/1b)** — `$` for hygienic templates, `$!` for high-power macros. `quote { }` with `@`-interpolation. `compile#()`/`error#()`/`warn#()`/`gensym#()` compile-time intrinsics with `is_compile_time_only()` annotation. Phase 1a (template) → Phase 1b (macro) → re-expand 1a → TypeChecker. Gensym hygiene for local `let` bindings (`__gensym_N`). Three canonical flags: `--macro-budget`, `--unlimited-macros`, `--safe-compile`.
- **MultiSlice mask/stride evaluation** — `BracketOp::Mask` and `BracketOp::Stride` ops now evaluated in interpreter. `_` bound as implicit element variable. `Expr::Slice.mask` also implemented. ArrowTransfer filter implemented with same `_`-binding pattern.

### Not a Priority
- Self-hosting pipeline (broken, deferred)
- ForAll/Exists (removed from core syntax)

### Historical Record
All optimization sprints, benchmark timing tables, bug diagnoses, and implementation phases are preserved in `AGENTS_HISTORY.md`.

### Current State
- 1162 tests pass, 0 fail
- **Constraint unification (B1/B2/B3)** complete: `RangeConstraint` + `Type::ContractBound` removed; `Statement::Let.constraint` + `StateDecl.constraint` unified to `Option<Box<Expr>>`; `_`-binding in `eval_constraint()`/`emit_guard_check()`; TypeDef body guards in `ResolvedType.guards`; LLVM constraint codegen with `@llvm.trap()` + `unreachable`
- **Phase 3.5 (Backend Fast-Path Registry)** complete:
  - TypeUniverse wired into main.rs pipeline (constructed after desugar, passed to typechecker + LLVM backend)
  - LLVM fast-path: `try_projection_fast_path()` emits native IR for 45+ (type, operator) pairs
  - Typechecker: `resolve_user_projection_type()` resolves ~25 well-known operator return types, falls back to TypeUniverse
  - Interpreter: `eval_user_projection_fast_path()` handles all operator names on Int/Float/Bool values
  - UserDefined (unary) projections: Neg/Not/BitNot evaluated directly
- **trg reactive dirty-flag architecture** complete (Phases 1–6):
  - Phase 1: `DependencyGraph` — variable-level DAG, Kahn's sort, cycle detection
  - Phase 2: `DirtyFlags(u64)` — bitmask with mark/clear/merge/any/none
  - Phase 3: LLVM `@step(%State*, i64)` — volatile trigger loads, dirty-flag recomputation
  - Phase 4: CIRCT backend (`circt.rs`) — HW+Comb MLIR emission, trg→input ports
  - Phase 5: Webstack `step_triggers()` — dirty-signal propagation in generated Rust
  - Phase 6: Removed `__trg_stdin_read` polling; deprecated timerfd/signalfd polling
- **SSA phi dominance** fixed (6 root causes: nested guard predecessor, let_binding_types save/restore, Unification/Match leaks, terminated reset, stale old-value caches)
- **foreach** complete: LLVM loop IR via alloca-based index, `!llvm.loop.vectorize.enable` SIMD metadata, feature file migration to `src/features/stmt/foreach.rs`, docs
- **`?#` proof oracle** complete: AST/parser, interpreter with fuel injection + state rollback + handler, structural recursion checker (P021), all match arms
- **Instruction reordering** complete: read/write set analysis, dependency DAG, Kahn's topological sort ILP optimization
- **Variadic `fprintf` syntax** fixed: 3 call sites now use `(ptr, ptr, ...)` prototype (loop_engine.rs:872, emit_expr.rs:1747, emit_expr.rs:1769)
- **TBAA metadata** implemented: 6-node type tree (Brief/Int/Bool/Char/String/Float), annotated on all state field loads and stores + struct FieldAccess. Enables LLVM type-based alias analysis for i64-boxed types.
- **`!range` metadata** implemented: replaces `@llvm.assume` for simple `[x < N]` precondition patterns with `!range !{ 0, N }` on the field load.
- **Webstack backend gaps closed** (2026-06-21): ARM bare-metal codegen path no longer emits `true` placeholder — `statement_to_rust()`/`expr_to_rust()` cover all Statement and Expr variants with native Rust codegen. TS path: intrinsic handler expanded from 4 to 25+ variants (Math.*, String(), Date.now(), etc.), all Statement types (Foreach, SyncBlock, OnExit, Oracle, Await, Async, AsyncAwait, Unification, InlineAsm) emit real TS code instead of `// statement omitted`. 9 new tests.
- **CIRCT backend gaps closed** (2026-06-21): `Expr::Call` emits `hw.instance` submodule instantiation instead of returning `None`. `Expr::IntrinsicCall` handles Abs (comb.neg+comb.mux), Ctpop/Ctlz/Cttz (comb.*), Bitreverse (comb.rev), Size, Sqrt/Fabs/Ceil/Floor/Sin/Cos/Pow (comb.*f f64). Fixed duplicate trigger processing bug. 5 new tests.
- **Pattern B AssignmentStmt** (2026-06-21): `StmtEval` trait impl handles Identifier, ListIndex, TupleDestructure LHS forms. 3 new tests. StmtTypecheck/StmtCodegen stubs ready for Phase 4.
- **`$!` macro expansion wired** (2026-06-21): `macro_.rs::expand_macro_call()` now delegates to `template::expand_macro()`. `collect_macro_defs`/`expand_macro_calls_in_items` made `pub(crate)`. 3 new E2E tests (parse→expand→interpret).
- **Crypto/HTTP FFI implemented** (2026-06-21): md5 (`md-5` crate), sha1 (`sha-1`), sha256/sha512 (`sha2`), uuid v4 (`uuid`), HTTP GET/POST (`ureq`). All handle String and Data inputs. 9 new tests with known test vectors.
- **`bytes` projection extended** (2026-06-21): Intrinsic::Bytes and ProjectionTarget::Bytes now handle Data, Instance, Tuple, Stack, Queue, StringBuilder. Feature module and interpreter stay in sync. 5 new tests.
- **GPU intrinsics dimension validation** (2026-06-21): `get_global_id#`, `get_local_id#`, `get_group_id#`, `get_num_groups#` validate dimension arg (0 ≤ d < 3). `barrier#` validates no args. 7 new tests.
- **Void intrinsic stubs → `undef`** (2026-06-21): AtomicStore, Fence, ThreadExit, Halt emit `add i64 undef, 0` instead of `add i64 0, 0`. 2 new tests.
- **Exit expression stubs → `llvm.trap()`** (2026-06-21): Unknown field types, missing triggers, unknown identifiers, and unsupported exit exprs in exit conditions now emit `call void @llvm.trap()` instead of silently returning 0.
- **`<-` arrow push** implemented for `List<T>`: reads header, allocates new buffer, copies elements, appends value, writes back to state field. No longer returns 0.
- **`<-` arrow pop** implemented for `List<T>`: reads header, loads element at index, allocates buffer with len-1, copies before/after elements, writes back to state.
- **`<-` arrow discard** implemented for `List<T>`: removes element at index, allocates buffer with len-1, stores updated list back.
- **`<-` arrow transfer** implemented for `List<T>`: moves all elements from source to dest, stores both updated lists back.
- **String/char escape sequences** fully implemented: `\0`, `\n`, `\t`, `\\`, `\'`, `\"`, `\xHH`, `\u{...}` all handled in both string and char literals.
- Three canonical backends: LLVM (native), Webstack (WASM+JS), CIRCT (MLIR→Verilog)
- All other backends are dead code — zero fixes
- Kani: 14 fast-group harnesses proven (2.5s), 96 full-group pass with `--features kani_full`
- Interpreter is the reference — if it runs a program, the backend should eventually compile it
- All additions are additive (new match arms) — never modify existing optimization paths

### Roadmap — Next Work Items

See `docs/plans/2026-06-15-trinity-work-items.md` for the full plan. Summary:

**Critical — officina-cli blockers:**
1. SSA phi dominance — 17 "Instruction does not dominate all uses" errors in `loop_engine.rs` general loop emission ✅

**Core feature — `foreach` completion (AST exists, backends are stubs):**
2. LLVM backend: emit real loop IR (phi indvar, list load, element bind, body, back-edge) ✅
3. SIMD vectorization: wire `check_list_simd_lengths` → `!llvm.loop.vectorize.enable` metadata ✅
4. Feature file migration: `src/features/stmt/foreach.rs` following `sync_block.rs` pattern ✅
5. Documentation: update `statement.md` with LLVM IR and SIMD lowering ✅

**New feature — `?#` proof oracle:**
6. Structural recursion checker (SPARK-style decreasing variant) ✅
7. `?#` AST / parser / desugaring (fuel injection + rollback + handler) ✅
8. Proof engine dispatch: bounded counter, structural recursion, SMT, fuel fallback ✅
9. Runtime fuel counter + state rollback + handler emission ✅

**Optimization:**
10. Transaction body instruction reordering — reorder for ILP, emit `noalias` GEP annotations ✅

## Iteration Pattern

**Iteration requires `txn` with `[pre][post]` convergence, NOT `defn` + `[guard]`:**

`Statement::Guarded` is a **one-shot conditional** — it evaluates the guard once and executes the body zero or one times. It does NOT loop. A `defn` body executes as a straight-line sequence with no implicit transaction wrapping.

The correct pattern for iteration in Brief is a **callable `txn`** (not `rct txn`). A regular `txn` takes parameters and returns values like a `defn`, but its body executes in a convergence loop: evaluate precondition → execute body → check postcondition → repeat if precondition still holds. The precondition becoming false is the convergence signal.

```brief
// CORRECT — convergence loop via txn + [pre][post]:
txn iter_map<T, U>(list: List<T>, f: T -> U, result: List<U>, i: Int)
    [i < list :> Size][i == list :> Size] -> List<U>
{
    &result = result.append(f(list[i]));
    &i = i + 1;
    term result;
};

defn iter_map<T, U>(list: List<T>, f: T -> U) -> List<U> {
    term iter_map_loop(list, f, [], 0);
};
```

| Construct | Semantics | When to use |
|-----------|-----------|-------------|
| `defn` | Pure function, straight-line | Stateless computations, wrappers |
| `txn params [pre][post] -> Ret { body }` | Callable convergent loop | Iteration, accumulation, recursion |
| `rct txn [pre][post] { body }` | Reactive, reactor-driven | State machines, event-driven |
| `[guard] { body }` | One-shot conditional | If/else, conditional execution inside a `txn` body |

Evolution: The old pattern `[guard] { &i = i + 1; }` inside `defn` bodies was cargo-culted from `rct txn` internals, where the outer reactor loop provides convergence. But `defn` has no such loop — the guarded statement fires once and falls through. ~130 defns in `lib/` were silently broken. All have been migrated to callable `txn`s.

## Testing Mandate

**Every new feature, every code path, every match arm must have corresponding tests.** No exceptions.

- **Interpreter changes**: Add direct AST-construction tests in `src/interpreter.rs` that exercise every branch of the new code.
- **Parser changes**: Add source-text parsing tests in `src/parser.rs` that verify the parsed AST structure.
- **Backend changes**: Ensure existing tests still pass (`cargo test --lib`). For non-trivial codegen, add LLVM IR string-assertion tests.
- **Legacy code**: Changing old code paths (destructuring, field access in backends) does not require new tests for each backend — but the compiler must build and all existing tests must pass.

Run `cargo test --lib` before every commit. If a change has no test, it does not exist.

## `--dev` / `--prod` / `--release` Optimization Flags (2026-06-13)

The compiler has three optimization modes with two additional controls:

| Flag | Budget | Simplify | Use Case |
|------|--------|----------|----------|
| `--dev` (default) | 256 | OFF | Fast compilation, development |
| `--prod` / `--release` | `u64::MAX` | ON (`u64::MAX` nodes) | Full optimization, production |
| `--optimize-budget <N>` | `N` | per mode | Override budget (region analyzer) |
| `--simplify-budget <N>` | per mode | ON with cap `N` | Override simplify nodes |
| `--no-simplify` | per mode | OFF | Disable expression simplification |

**Budget**: Controls how many transaction iterations the interpreter will
simulate during precomputation analysis. Lower values compile faster but
may produce runtime loops for large bounds.

**Simplify**: The expression simplification pass (`equality_saturation.rs`)
rewrites algebraic identities (`x+0→x`, `!!x→x`) bottom-up O(n) using a
hash-cons cache. Enabled in `--prod`/`--release`. The `--simplify-budget`
flag caps total nodes processed before bailing out.

**--optimize-budget <N>** overrides both `--dev` and `--prod` budgets.
**--simplify-budget <N>** enables simplify with a node cap regardless
of mode. **--no-simplify** disables it regardless.

Implementation: `main.rs` — flags parsed in `build` and `llvm` subcommands,
passed through `run_build` → `run_llvm_compile`. `--prod`/`--release` also
enable the A005b linearity-memory path when guards aren't provably linear.

See `docs/architecture/features/backend-dispatch.md` for the full dispatch
decision tree.

## Architecture Documentation (Permanent Practice)

Maintain `docs/architecture/` as a living record of the compiler's design.
Updated in the same commit as any API or structural change.

### Directory structure

```
docs/architecture/
  overview.md              # System architecture, module responsibilities, data flow
  features/                # One file per feature group
    literal.md
    call.md
    projection.md
    ...                    # Updated when new features are added
  optimization-pipeline.md # Decision tree, folded loop, SSA, SLP hazard
  backend-strategy.md      # Per-backend design notes (LLVM, VHDL, Webstack, etc.)
  channel-map.md           # Data flow: parse → resolve → desugar → typecheck →
                           #   proof → analyze → codegen
  praetor-log.md           # Running log of diagnostics found/resolved (datestamped)
  kani-harnesses.md        # Inventory of formal verification proofs
  glossary.md              # Brief-specific terminology
```

### Rules

1. Every new feature file (`features/*.rs`) gets a corresponding doc entry when created.
2. **Doc-per-cycle**: Every migration cycle ships its architecture doc in the same commit
   as the code change. The doc is written immediately after the code, while it's fresh.
   No batch documentation phases — they drift from reality.
3. Architecture changes are documented in the same commit that makes them.
4. Praetor violations discovered during development are logged in `praetor-log.md` with
   datetime, file, root cause, and resolution.
5. Any commit that changes an API contract between passes must update `channel-map.md`.

### Coordinator docs

Coordinator files (interpreter, typechecker, parser, proof_engine, backends) get their
own architecture doc as they shrink toward their target size (1,000–2,000 lines). Each
doc explains: how the dispatcher works, what stays centralized, error handling, and
interaction patterns.

### What each feature doc covers

| Section | Content | Length |
|---------|---------|--------|
| Header | Purpose, date added, phase | 2 lines |
| Syntax | Brief syntax for the construct with examples | 10–30 lines |
| Typechecking | How types are inferred/checked | 5–15 lines |
| Evaluation | How it evaluates in the interpreter | 5–15 lines |
| Codegen | Per-backend notes (LLVM, VHDL, Webstack) | 10–30 lines |
| Kani/Praetor | Special considerations | 3–5 lines |

Feature docs target 50–150 lines — compact enough to fit in working memory.

## Formal Verification with Kani

Integrate AWS's `kani` bounded model checker as a permanent part of the development
workflow. All new safety-critical code must include Kani proof harnesses.

### Rules

1. **All new modules** created during the refactor must include Kani proof harnesses
   for any unsafe code, FFI boundary code, or functions with non-trivial safety
   invariants.
2. **Targets**: `ffi/native_mapper.rs` (byte slicing, endian conversion) and
   `reactor.rs` (state rollback, step counter) — the two most safety-critical modules
   today. Expand to all new safety-critical code going forward.
3. **Harnesses** live in `#[cfg(kani)] mod kani_tests {}` blocks at the bottom of each
   module file, co-located with unit tests.
4. **Proof goals**: Prove absence of panics, overflows, out-of-bounds access, and
   undefined behavior under all possible symbolic inputs.
5. **CI-gated**: `cargo kani` must pass before merging.
6. **Coverage requirement**: Every function modified during refactoring must have a
   Kani proof harness, regardless of whether it is "safety-critical." The refactor
   touches code across the entire compiler — Kani verifies that the new routing
   logic, enum variant conversions, and helper methods are correct under all
   possible symbolic inputs. `unsafe`-free code can still overflow, panic on
   `unreachable!()`, or miss edge cases in match arms. Proof harnesses catch these.

### Kani Harness Requirements (never hang again)

A Kani harness MUST only contain:

1. **Pure match dispatch only** — `match self { A => B, C => D }` returning a concrete result
2. **Concrete inputs only** — no `kani::any()`, no symbolic values (they trigger unbounded exploration)
3. **No formatting** — no `.to_string()`, `format!()`, `writeln!()`, string concatenation, or any `Display` impl
4. **No heap allocation** — no `Box::new()`, `Vec::new()`, `String::new()`, `HashMap::new()`
5. **No struct construction** unless the struct has ≤ 3 fields and no heap-allocated fields
6. **No loops or recursion** in the function being verified OR any function it transitively calls

A harness is **unprovable** (will timeout) if it transitively calls ANY function that:
- Converts integers to strings (`.to_string()`, `format!("{}", n)`) — **division loop**
- Formats output (`format!`, `writeln!`) — **allocation + formatting loop**
- Constructs `Box`, `Vec`, `String`, `HashMap`, `HashSet` — **heap allocation path explosion**
- Constructs any struct with >3 fields — **state space explosion**
- Iterates with loops or recurses — **unbounded path exploration**

**Fast group** (`#[cfg(kani)] mod kani_tests`): only provable harnesses per above rules. Runs in <5s.

**Full group** (`#[cfg(all(kani, feature = "kani_full"))] mod kani_full_tests`): anything that relaxes these rules (formatting, allocation, loops). Runs on CI only with `--features kani_full`.

### Reference harness patterns

Fast (provable match dispatch):
```rust
#[cfg(kani)]
mod kani_tests {
    use super::*;

    #[kani::proof]
    fn verify_as_integer_dual_path() {
        let old = Expr::Integer(42);
        let new = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        assert_eq!(old.as_integer(), new.as_integer());
    }
}
```

Full (uses formatting — `kani_full` feature only):
```rust
#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_literal_format_no_panic() {
        let lit = LiteralExpr::Integer(42);
        let s = lit.format();
        assert!(!s.is_empty());
    }
}
```

## Three Canonical Backends

Only three backends are actively developed. All others are **dead code** —
preserved in tree but receiving zero fixes, zero features, zero attention.

| Backend | Target | Status |
|---------|--------|--------|
| **LLVM** (`src/backend/llvm/`) | Native binary (`.ll` + `llc`) | **Active** — canonical OS target |
| **Webstack** (`src/backend/webstack.rs`) | WASM + JS glue | **Active** — canonical web target |
| **CIRCT** (`src/backend/circt.rs`) | Hardware (`.mlir` + `circt-opt` + `circt-translate`) | **Active** — canonical hardware target |

### Dead Backends — Zero Fixes

The following backends are dead. Do not modify them for any reason,
not even for compilation fixes. If a dead backend fails to compile,
delete its broken code paths or mark them with `#[allow(...)]` —
do not invest time fixing them.

`verilog.rs`, `vhdl.rs`, `c.rs`, `rust.rs`, `cobol.rs`, `x86_64.rs`,
`aarch64.rs`, `wasm.rs`, `tcl_generator.rs`

The only exception: if a change to a shared API (e.g. `Statement::LocalTrigger`
gets a new field or a variant is removed from an enum) mechanically breaks
a dead backend, use `#[allow(unused_variables)]`, match with `_ => {}`, or
`todo!()` with a comment `// dead backend` — do not implement the feature.

## Per-Commit Checklist

Before every commit:
1. `cargo test --lib` — all tests pass
2. `cargo build` — no warnings
3. Run Praetor on new/changed files — verify complexity ≤ 15, lines ≤ 100, params ≤ 6
4. Update architecture docs if API contracts changed
5. **LLVM Diagnostic Commands** — use these to identify why the optimizer is failing:

   ```bash
   # SROA failures (struct not decomposed into scalars)
   opt -O3 -pass-remarks-missed=sroa unopt.ll -disable-output 2>&1

   # Loop vectorization failures (trip count, reductions, dependencies)
   opt -O3 -pass-remarks-missed=loop-vectorize -pass-remarks-analysis=loop-vectorize unopt.ll -disable-output 2>&1

   # Alias analysis / GVN failures (loads not eliminated)
   opt -O3 -pass-remarks-missed=gvn unopt.ll -disable-output 2>&1

   # All optimization remarks at once
   opt -O3 -pass-remarks-missed=sroa,gvn,licm,loop-vectorize unopt.ll -disable-output 2>&1

   # Full pipeline with remarks
   opt -O3 -pass-remarks=loop-vectorize -pass-remarks=sroa -pass-remarks=gvn unopt.ll -disable-output 2>&1

   # Inspect IR before/after opt
   opt -S -O3 unopt.ll -o opt.ll
   diff <(grep -v '^;' unopt.ll | grep -v '^$') <(grep -v '^;' opt.ll | grep -v '^$')

   # Check if %State struct survived SROA (bad sign)
   grep '%State' opt.ll
   ```

 6. **Doc-per-cycle**: If this commit includes a new or migrated feature, write/update
    `docs/architecture/features/<name>.md` in the same commit. Never batch documentation.
 6. Log bugs/gotchas in BUGS.md or docs/architecture/praetor-log.md
 7. Add Kani harnesses for all newly written or modified functions

## Known Bugs Fixed

### 2026-06-17: String state initializers store null instead of string constant

**Root cause**: `emit_inline_init_stores` in `emit_toplevel.rs:468` had a special case
that matched `Some(Expr::String(_))` and stored `i8* null` instead of the actual
string constant pointer. All string state variables (e.g. `current_input: String = ""`,
`target_os: String = "linux"`) were initialized as null pointers. The first tick
that read any string field dereferenced null → **SIGSEGV**.

**Fix**: Replace `null` with a `bitcast` of `@str.N` to `i8*`, identical to what
`Expr::String` already emits in `emit_expr.rs:32`.

**Lesson**: Every `Expr` handler that evaluates to a pointer must store the actual
pointer, not a sentinel/placeholder. Unit test `test_string_state_init` added to
verify string state fields get non-null initial values.

### 2026-06-17: Wrong TFD_NONBLOCK / SFD_NONBLOCK constants in trigger init

**Root cause**: `emit_toplevel.rs:104-105` had `tfd_nonblock = 0x400` and
`sfd_nonblock = 0x400`. These should be `0x800` (same as `O_NONBLOCK` on Linux
x86_64). The values `0x400` are `FD_CLOEXEC`, not `O_NONBLOCK`. The intrinsics
themselves (`timerfd_create`, `signalfd`) already use the correct value `2048`
(`0x800`) from the migration; only the trigger-init constants were wrong.

**Fix**: Change both to `0x800`.

**Lesson**: Hardcoded platform constants should be cross-referenced against kernel
headers (`/usr/include/asm-generic/fcntl.h`). Use `O_NONBLOCK = 0o0004000 = 2048`
for all `*_NONBLOCK` constants.

### 2026-06-17: `read_file#` returns null instead of error — FFI must use `Result<T, E>`

**Root cause**: `read_file#` returned `i8*` — either a valid C string or NULL if
the file didn't exist. The Brief type system has no notion of "nullable pointer".
When the file was missing, the code dereferenced a null string → **SIGSEGV**.

**Fix**: Changed `read_file#` to return `Result<String, String>` — `Ok(contents)`
on success, `Err("file not found")` on failure. The LLVM backend constructs a
heap-allocated `Result` enum (malloc + discriminant + payload), consistent with
the existing enum construction path at `emit_expr.rs:428-463`.

**Architectural rule**: Every Brief `#`-intrinsic that can fail MUST return
`Result<T, E>` where `E` describes the failure. Raw `String` returns that can
be null are forbidden — they subvert the type system.

Before:
```
read_file#(path) → String   // null on failure, cannot distinguish
```
After:
```
read_file#(path) → Result<String, String>  // Ok(contents) | Err(reason)
```

### 2026-06-17: `tfd_nonblock` / `sfd_nonblock` constants — typo fix

### 2026-06-17: `is_string_chain` missing `Expr::Call` arm — officina SIGSEGV

**Root cause**: `is_string_chain` in `emit_expr.rs:2763` detects string `+`
for inline concat but does not handle `Expr::Call`. When `int_to_str(2) + int_to_str(3)`
(the `n >= 10` arm) is compiled, both operands are `Expr::Call`. `is_string_chain`
returns `false`, and the backend emits `add i64` (adding two struct pointers
together) instead of allocating a buffer and copying characters. The returned
garbage value is dereferenced in `draw_prompt` → SIGSEGV.

**Fix**: Added `Expr::Call(name, _)` arm checking `defn_return_types` for
`String`/`Data` return type.

**File**: `src/backend/llvm/emit_expr.rs:2777-2783`

### 2026-06-17: `\0` char escape not handled in lexer

**Root cause**: `src/lexer.rs:371-382` handles `\n`, `\t`, `\\`, `\'`,
and `\u{...}` escape sequences in char literals, but NOT `\0` (null).
`'\0'` falls through to `inner.chars().next()` → `\` (backslash, ASCII 92).
The precondition `keypress != '\0'` compiled as `keypress != 92`, so
`process_input` fired on every tick even when `keypress` was null (0).

**Fix**: Added `if inner == "\\0" { return Some('\0'); }` before the other
escape checks.

**File**: `src/lexer.rs:371-374`

### 2026-06-17: `done_{name}` SSA dispatch branches to exit instead of next txn

**Root cause**: `src/backend/llvm/loop_engine.rs:778`: the `done_l` label
(emitted when a txn's precondition is false) unconditionally branches to
`%done` (program exit) instead of `%{skip_l}` (next txn's skip label).
The FIRST txn whose precondition is false causes an early return from
`main()`, never reaching subsequent txns. The June-14 fix claimed to
address this but only covered `done_boot` while the template kept
`br label %done` for all txns.

**Fix**: Changed `br label %done` to `br label %{skip_l}`.

**File**: `src/backend/llvm/loop_engine.rs:778`

### 2026-06-17: TBAA metadata tree for `i64`-boxed types

**Implementation**: Added 6-node TBAA metadata tree (Brief root + Int, Bool,
Char, String, Float sub-types). Annotated ALL state field loads
(`pre_load_all_fields`), non-SSA state stores, and struct `FieldAccess`
loads with `!tbaa !N` metadata. This enables LLVM's type-based alias
analysis to disambiguate accesses at different type roots even though all
values are stored as `i64` at the IR level.

**Nodes defined**: `mod.rs:1669-1675`
- `!0 = !{!"Brief"}`
- `!1 = !{!"Int", !0}`   — i64-stored values
- `!2 = !{!"Bool", !0}`  — i8-stored Bool
- `!3 = !{!"Char", !0}`  — i32-stored Char
- `!4 = !{!"String", !0}` — i8*-stored String
- `!5 = !{!"Float", !0}` — float-stored Float

**Helper function**: `pub(super) fn tbaa_node(ty_str: &str) -> i32` in
`mod.rs:412-424`.

### 2026-06-17: `!range` metadata replaces `@llvm.assume` for simple patterns

**Implementation**: `emit_precondition_check` in `emit_toplevel.rs:820-856`
detects simple `[x < N]` precondition patterns (`Expr::Lt(Expr::Identifier, Expr::Integer)`)
and emits a re-load of the field with `!range !{ 0, N }` metadata instead
of `call void @llvm.assume(i1 %cond)`. Complex patterns keep `@llvm.assume`.

**File**: `src/backend/llvm/emit_toplevel.rs:850-856`

### 2026-06-17: Variadic `fprintf` missing `(ptr, ptr, ...)` prototype

**Root cause**: Three `fprintf` call sites in the LLVM backend omit the
explicit variadic function type `(ptr, ptr, ...)` from the `call`
instruction. LLVM requires the call type to match the declare
(`declare i32 @fprintf(ptr, ptr, ...)`). These would fail the LLVM
verifier if exercised by any program using `PrintInt` or `PrintFloat`
intrinsics.

**Fix**: Added `(ptr, ptr, ...)` to all three call sites.

**Files**:
- `src/backend/llvm/loop_engine.rs:872` (print_int in emit_post_print)
- `src/backend/llvm/emit_expr.rs:1747` (Intrinsic::PrintInt)
- `src/backend/llvm/emit_expr.rs:1769` (Intrinsic::PrintFloat)
