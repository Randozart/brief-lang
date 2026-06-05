# Brief Compiler - Agent Guidelines

See CLAUDE.md for complete documentation. This file ensures OpenCode picks up the same guidelines.

## Quick Reference

### Commands
- **Build**: `cargo build`
- **Test**: `cargo test --lib`
- **Test backend registry**: `cargo test --lib -- backend::tests`
- **Compile RBV**: `./target/release/brief-compiler rbv <file.rbv>`
- **Benchmark**: `bash benchmarks/build_and_bench.sh` — always use this harness (nanosecond CLOCK_MONOTONIC, 5-iteration average). Ad-hoc timing produces false hangs and imprecise numbers.

### File Types
- **.bv** - Brief (standard Brief file)
- **.rbv** - Rendered Brief (Brief + View, compiles to web frontend. Like `.tsx` is to `.ts`)
- **.ebv** - Embedded Brief (oriented towards bare metal and embedded)
- **.dbv/.dbvs/.dbvl** - Data Brief (configuration with schema, think `.xml`/`.xmls`/`.jsonl`)

### Critical Philosophy

**CONTRACT-FIRST**: Contracts are the source of truth. Never weaken contracts to match lazy code.

**NO MAGIC**: Never add hardcoded Rust string matches as "built-in" functions.
- If a `.bv` file needs `is_digit`, import `char` from `"std/char.bv"` — NOT a Rust match arm.
- If a `.bv` file needs `None`, import `option` from `"std/option.bv"` — NOT pre-populating state.
- The FFI system (`frgn from "..."`) and the standard library are the transparent paths. Use them.

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

### Observability as Liveness

A program that produces no observable effect IS dead code. The compiler is correct to eliminate it.

Brief's liveness model: **a value is live if an FFI call consumes it.** Every program must interact with the world — print, file I/O, network — via `frgn` calls.

If the compiler folded your hot loop to `store i64 N`, **the compiler is right.** Your program produced no observable output. The fix is NOT liveness hacks (`x == x`, synthetic exit fields). The fix IS `frgn __print_int(result)`.

The C reference must use the SAME observable. Symmetric benchmarks, symmetric optimizations.

## Benchmark Philosophy

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

### The C reference is symmetric, always

Both get `-O3 -ffast-math` from the same clang. No `volatile`, no unused variables. Any asymmetry is a signal of a missing Brief optimization — fix the compiler, not the C code.

### Useful utilities become standard library functions

When a benchmark produces a general-purpose helper (rolling hash, vector math, frequency counting), extract it into `lib/std/`. Any function designed for a benchmark that could serve as a general-purpose utility MUST be added to `lib/std/`.

### Correct Approach
- Keep contract `[product > 0]` — fix code, not contracts
- `UndefinedForeignFunction("is_digit")` → `import char from "std/char.bv"`
- Import resolver can't find file → fix search path, not interpreter

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
| ✅ | Call, ListLiteral, ListIndex, Projection (5 targets), FieldAccess |
| ✅ | StructInstance, ObjectLiteral, PatternMatch, Concat |
| ✅ | Slice, MultiSlice, Block, Tuple, TupleDestructure, Cast, Match |
| ⚠️ | **ForAll, Exists** — REMOVED from core syntax. Stub AST nodes remain but are NOT part of the surface language. |

### Statements — All fully implemented
Assignment, Let, InlineAsm, Expression, Term (with optional swan song), TermBang (with optional swan song), Escape, Guarded, Unification, LocalTrigger.

### Known Gaps
- **Recursive defn calls**: No recursion guard or stack depth limit. Deep recursion overflows the Rust interpreter.
- **ForAll/Exists**: Removed from surface syntax.

## LLVM Backend Gaps

Additive only — never weaken existing optimization paths.

### Expressions — Stub (Returns 0 or Degraded)
| Expr | What's Missing |
|------|----------------|
| **Slice** | Only handles `start` offset. Missing `end`, `stride`, `mask`, buffer allocation + copy. |
| **MultiSlice** | Returns base pointer unchanged. Missing coordinate-based indexing. |
| **Tuple / TupleDestructure** | No LLVM struct type for user types. Returns 0. |
| **StructInstance / ObjectLiteral** | Returns 0. Missing allocation + GEP + stores. |
| **FieldAccess** | Returns object pointer as-is. Missing GEP at known field offset. |
| **ForAll** | Returns 1 always. Matches interpreter stub. |

### Top-Level — Silently Skipped
| TopLevel | Impact |
|----------|--------|
| **Struct** | No LLVM struct type generated. StructInstance/FieldAccess stubs are symptoms. |
| **Enum** | No tagged union layout. Enum constructors work via ad-hoc stack alloca + discriminant prefix. |
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

## Self-Hosting Pipeline

The Brief-in-Brief compiler lives in `lib/compiler/`. Run via `brief-compiler selfhost <file.bv>`.

**NOT currently being worked on.** Broken at parser level (multidimensional slice bug). Deferred.

**Do NOT add as built-ins**: `is_digit`, `is_alpha`, `is_alphanumeric`, `is_upper`, `is_lower`, `is_space`, `char_to_string`, `None`, `Some`, `Ok`, `Err`. These are in `lib/std/` — import them.

## Critical Context

### Already Done (Don't Redo)
- **Projection operator (`:>`)** — fully implemented, 5 targets (Size, Bytes, Ptr, Alignment, Range). `Expr::ListLen` deleted. All stdlib migrated (277 calls across 16 files).
- **`<-` arrow syntax** — first-class Expr variants for collection mutation (push/pop/insert/remove).
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

### Not a Priority
- Self-hosting pipeline (broken, deferred)
- ForAll/Exists (removed from core syntax)

### Historical Record
All optimization sprints, benchmark timing tables, bug diagnoses, and implementation phases are preserved in `AGENTS_HISTORY.md`.

### Current State
- 434 tests pass, 0 fail
- Interpreter is the reference — if it runs a program, the backend should eventually compile it
- All additions are additive (new match arms) — never modify existing optimization paths
