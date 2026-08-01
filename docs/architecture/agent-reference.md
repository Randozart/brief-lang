# Agent Reference — Language Syntax, Contracts, and Backend Conventions

**2026-07-31:** Reference material moved out of `AGENTS.md` during the
guidelines rewrite (AGENTS.md is now the operating rules; `AGENTS.md.archive`
is the full pre-rewrite document). This file is the day-to-day reference for
Brief language syntax, contract/intrinsic conventions, coding standards, and
backend architecture rules.

---

## 1. Brief Language Syntax

### 1.0 Protocol variants

`#L`, `#R`, `#T` are compiler-internal positional markers for op bindings —
lexed as distinct tokens and resolved at codegen time to concrete registers.
`#Category` hashwords (`#Int`, `#Float`, `#String`, `#Bool`, `#Char`, `#Bits`)
are backend directives in op signatures; parameterized variants
(`#String<UTF8>`, `#Float<IEEE754>`) select representations; `#Link<name>`
emits `-l<name>`; `#System` is the sole bare protocol hashword.

**Width resolution** (for `WidthParametric` protocols `#Int`, `#UInt`, `#Bit`):
`!> bits: N` (exact) → `!> maxbits: N` (upper bound) → `!> minbits: N` (lower
bound) → `int_bits` (target default). `!> bits: 32` asserts the type is exactly
32 bits on every target — a hard contract, not a hint.

Well-known sub-protocols are hardcoded in the casting graph with known LLVM types
(these remove the old `disamb` metadata hack):

| Variant | LLVM type |
|---------|-----------|
| `#Float<BFloat>` | `bfloat` |
| `#Float<Half>` | `half` |
| `#Float<IEEE754>` | `float` |
| `#Float<Double>` | `double` |
| `#Float<FP128>` | `fp128` |
| `#Float<X86_FP80>` | `x86_fp80` |
| `#String<UTF8>` | `{ i64, i64 }` |
| `#String<ASCII>` | `{ i64, i64 }` |

The file extension determines the default variant (`.bv` → UTF8, `.ebv` →
ASCII); cross-variant calls need explicit disambiguation. If the compiler must
distinguish two representations of the same width, add a hardcoded protocol
variant — never a metadata key that codegen must check.

### 1.1 Naming convention

- **PascalCase**: protocol identifiers, hashwords, intrinsics
  (`#String<UTF8>`, `#Float<IEEE754>`, `Sqrt#`, `PrintInt#`, `#Int`, `#Bits`,
  `Posit32`, `CastTo(#String<UTF8>)`).
- **snake_case**: user functions in `.bv` files and Rust stdlib calls
  (`ascii_to_utf8()`, `from_utf8_lossy()`, `array_map()`).
- The dividing line: if the compiler MUST know the name to function (intrinsic
  registry, protocol hashwords) it is PascalCase; if a user could rename it and
  the compiler still works it is snake_case.

### 1.2 `<-` arrow operator

Statement-level only (breaks the expression parser — cannot write
`let x = &list <- val`):

- `&list <- val;` — push val onto list (destructive insert)
- `x <- &list;` — pop from list into x (destructive extract)
- `x <- list;` — read from list without removing (non-destructive copy)

### 1.3 `&` — pointer ref on LHS, move/copy discriminator on RHS

`&` never appears on the LHS of assignment/arrow syntax. Use plain
`i = i + 1;` / `i += 1;`. On the RHS of `<-`:

- `target <- &source;` — **consume/move**: source left empty/undefined
- `target <- source;` — **copy**: source retains the value
- `<- &source;` — **discard**: pop into void

Pointer references on the LHS use `Ptr<T>` and `.` dereference.

### 1.4 `frgn` is an import

First name after `frgn` is the C/foreign symbol, `as` gives the Brief name.
`from` is required. `from "libruntime"` is forbidden — use `from "c"` or
`from "link/brief_rt.c"`:

```brief
frgn XXH64(data_ptr: Int, len: Int, seed: Int) -> Int as frgn__xxh64 from "link/xxhash/xxhash.c" fallback 0;
```

### 1.5 Lexer / parser gotchas

- `>>` in nested generics: `Ptr<Ptr<Int> >` (space required).
- `_` discard binding: `let _ = expr;` also works in tuples
  (`let (_, value) = get_pair();`).
- Imports are flat — no `::` module paths. `loader::read_u8(x)` is invalid.
- `import "foo.bv"` is file-relative to the importing file; `"<foo>"` is a
  registry lookup.
- Tuples are heap-allocated (`(1, 2)` calls `@malloc`); SROA promotes small
  tuples in optimized builds.
- `Byte` is defined in `lib/std/types.bv` — import it or use `Int`.
- `defn main()` and bare top-level `let`/`const` bindings run via the
  flat-scripting plugin (2026-08-01): a synthesized one-shot
  `node __script_main [__script_done == false][__script_done]` executes them
  exactly once. Reactive programs start via state-space triggers on `node`
  declarations; CLI subcommands use `entry!`/`args!`.

### 1.6 Import / narrowing

- `Int` narrowing is protocol-based (`#Int`/`#UInt` membership, never type
  names). Fixed-width types (`Int8`…`Int64`) cap the floor via `bits <~ N`.

### 1.7 Types

- `type Foo: List { … }` inherits, but `Foo<Int>` is NOT automatically
  assignable to `List<Int>` — projections like `.#Size`/`foo[i]` may fail.
- No implicit `Copy` on enums containing `String` (`InsertStrategy::Custom(String)`
  requires removing `Copy`).

---

## 2. Contracts

1. **`defn` needs no contract** — straight-line translation is inherently
   provable. Add contracts only for the optimization leverage they unlock.
2. **`txn` needs at least one contract side** — `[pre][post]`, `[pre]]`, or
   `[[post]`. Convergence must be provable.
3. **Intrinsics have no body** — `Sqrt#(x)` is never declared in source; the
   compiler knows it via `get_intrinsic_signature("Sqrt#")`.
4. **`[true][true]` is rejected** — use `[[post]` or `[pre]]` sugar.
5. **`[[post]` = `[true][post]`** (postcondition-only);
   **`[pre]]` = `[pre][true]`** (precondition-only).
6. **Single-bracket `[expr]` is ambiguous** — parser rejects it.
7. **`[true][term == true || term == false]` is a useless tautology** — write a
   contract that constrains behavior.
8. **Never weaken contracts** — never change `[product > 0]` to `[true]`.

### Intrinsic conventions

- PascalCase + `#` suffix: `Sqrt#`, `Malloc#`, `PrintInt#`, `GetEnvInt#`. The
  `#` is part of the identifier lexically. No `_` prefix convention.
- No `inop` keyword — all compiler-known ops are `#` intrinsics with entries in
  `get_intrinsic_signature()` and `execute_intrinsic()`.
- Side-effecting intrinsics MUST declare `observable <~ true` (`PrintInt#`,
  `Malloc#`, `Memcpy#`, …) so DCE cannot eliminate the call.

---

## 3. Guard / Control-Flow Forms

- `[expr];` — **convergence gate** (`Statement::Gate`): compile-time assertion;
  at runtime, if false, branches back to the convergence target.
- `[expr] stmt;` — **guarded single statement**.
- `when expr { body };` — **block guarded body** (preferred for multi-statement
  guards). Guards may chain; a trailing `{ body }` without `when` is the else.
- `[cond] { body }` is **rejected** — use `when cond { body }`.
- Post-body loops `{ body; i = i + 1; } [condition];` only work in `txn`/`node`,
  NOT `defn`.

### Iteration pattern

Iteration requires `txn` with `[pre][post]` convergence, NOT `defn` + `[guard]`
(`Statement::Guarded` is a one-shot conditional):

```brief
txn iter_map<T, U>(list: List<T>, f: T -> U, result: List<U>, i: Int)
    [i < list.^Len][i == list.^Len] -> List<U>
{
    result = result.append(f(list[i]));
    i = i + 1;
    term result;
};

defn iter_map<T, U>(list: List<T>, f: T -> U) -> List<U> {
    term iter_map_loop(list, f, [], 0);
};
```

| Construct | Semantics | When to use |
|-----------|-----------|-------------|
| `defn` | Pure function, straight-line | Stateless computations, wrappers |
| `txn params [pre][post] -> Ret` | Callable convergent loop | Iteration, accumulation, recursion |
| `node [pre][post]` | Reactive, reactor-driven | State machines, event-driven |
| `[guard] { body }` | One-shot conditional | If/else inside a `txn` body |

### `type` vs `struct` vs `obj`

- `type`: protocols, operator bindings, type extensibility
  (`type Int: #Int { op Add(#Int); };`).
- `struct`: pure data, fixed layout, C-compatible, no methods/contracts
  (`struct VMStack { data: Int[1024]; len: Int; };`). Receives fixed-size
  arrays and the bracket SIMD syntax.
- `obj`: full-featured types with methods, contracts, type params, visibility.

### Bracket arrays / SIMD

`Int[1024]` is a compile-time fixed array (`[1024 x i64]` in LLVM). Slice
syntax `arr[start:end:stride]` (any component optional); `arr1 + arr2`,
`arr * 2` on `Vector<T, N>`/`Slice<T>`; contiguous slice lvalues use `memcpy`,
strided use element loops (LLVM vectorizes); `raw as Byte[8192]` is a
zero-copy view cast (validates `N * sizeof(T) == M * sizeof(U)`). `map`,
`filter`, `fold`, `any`, `all`, `sum`, `product` are regular txns in
`lib/std/array.bv`, vectorized via the `[i < N]` convergence contract.

### `op Parse` discriminators

`op Parse(Decimal, pre:"0x")`, `suf:"km"`, `reg:"[0-9a-fA-F]+"`,
`op Parse(Quoted)`, `op Parse(Bare)`. Resolution order: form → pre/suf →
regex; ambiguity = error. `sql"SELECT"` → `Expr::TaggedQuotedLiteral`;
`42km`/`3.14f` → `Expr::TaggedLiteral`.

---

## 4. Modifiers and the concurrency gate (2026-07-31)

User-facing directives are **ordinary keywords** (no `#`); they **must never
make code faster** — a modifier-beaten default is a compiler bug. All modifiers
are **prefix** (`async node`; `node async` is rejected). See
`docs/architecture/concurrency-and-modifiers.md`.

| Modifier | Meaning |
|----------|---------|
| `seq struct Name` | declared field layout preserved — `apply_field_modes` does not reorder/compact/eliminate |
| `seq txn foo` / `seq node foo` | sequential dispatch — never the parallel reactor |
| `seq Int[N]` / `seq foreach` | sequential access — `!llvm.loop.vectorize.enable = false` |
| `vol let x` | every access is `load volatile`/`store volatile` |
| `async node foo` | explicit acknowledgement of simultaneous firing (not a hint) |
| `sync<group> node foo` | group barrier — members that fire hold off finishing until all fired members have |

**The concurrency gate (NO IMPLICIT CONCURRENCY):** for reactive nodes A and B,
if the proof engine proves `pre_A ∧ pre_B` satisfiable AND there is no XOR
read-write overlap, the compiler DEMANDS `async` on both or `sync<group>` on
both — an unclassified eligible pair is a hard error.

**Delimiter semantic load:** `<>` = compile-time type specialization
(`Stack<T>`, `#String<UTF8>`, `asm<chip>`, `sync<group>`); `()` = application &
binding (`f(a)`, `Person(...)`, `op Add: func(#L,#R)`, `op Add(Float)` —
declarations take params); `[]` = containment/bound; `{}` = grouping.

## 5. Coding Standards (details)

### Doc comments on every definition

Every `fn`/`struct`/`enum`/`trait`/`type`/`const`/`mod` needs a `///` comment
explaining intent, invariants, usage. Write for a reader who knows Rust but not
the domain. Non-negotiable — reject in review.

### Input validation & defensive checks

- Check array/vector bounds before indexing.
- Assert struct invariants after construction/mutation.
- Print diagnostic context (function, values, expected vs actual) on failure.
- Check NaN/Inf at FFI boundaries.
- `debug_assert!` on hot paths; `assert!` for safety-critical invariants.

### Need-to-know dependency injection

Pass only the data a function needs, not large context structs.

```rust
// Avoid:  fn emit_binop(ctx: &CompilerContext, state: &State) -> Result<()>;
// Prefer: fn emit_binop(builder: &mut LlvmBuilder, op: BinOp, lhs: Type, rhs: Type) -> Result<String>;
```

### Metropolitan FFI / export

- `brief export` generates wrappers from `lib/glue.toml` templates — no Rust
  knows specific languages. GLUE = compile-time bridge; Metropipe =
  runtime shared-memory IPC (`src/ffi/metropipe.rs`).
- `brief export` calls `LlvmBackend::generate()` — the same path as
  `brief build --llvm`. No `ret i64 0` stubs.
- Strings in LLVM: `[i64 length][data\0]`; globals use `<{ i64, [N x i8] }>`;
  `emit_load_length` reads `handle[0]`; `brief_str_to_c` strips tag bits `& ~3`.
- Protocol paths via BFS (`find_cast_path()` from `layout_optimizer.rs`);
  fall back to `Cast(#Bits)`; `emit_protocol_chain()` emits real IR.

### HashMap iteration determinism

Every HashMap iteration that produces LLVM IR MUST be sorted by key — SipHash
seed differs per process (up to ~9% perf variation). Applies to
`field_index_map`, `phi_field_regs`, `backedge_field_regs`, `last_val_temps`,
`done_needs_fields`, `pending_phi_backedge`, `pending_phi_native_backedge`,
`vector_phi_groups`, `vector_phi_current`, etc. HashMaps used only for O(1)
lookups are fine. Reference: commit `139c345`,
`docs/plans/2026-07-06-ir-determinism-and-benchmark-strategy.md`.

---

## 5. Anti-Patterns (NEVER DO)

- Changing `[product > 0]` to `[true]` because code doesn't set product
- Generic contracts like `[true]`; postconditions that don't guarantee outcomes
- Rust string-match built-ins when stdlib/import should be used
- Pre-populating interpreter state with enum constants (None, Some, Ok, Err)
- `x == x` self-references to force liveness; synthetic exit-condition fields
- Hardcoded `from "libruntime"` (use `from "c"` / `from "link/brief_rt.c"`);
  missing `from` on `frgn`
- `#export` (use `export defn`); `#out` (use `observable <~ true`)
- Hardcoded runtime declares (`__rt_init` must be `frgn` in `std/rt.bv`)
- Name-based interpreter dispatch (dispatch on `Value::HashMap`, not names);
  `"None"`/`"Err"` discriminant magic (use declaration order); runtime type
  tags for dispatch
- Implicit coercions — all type reinterpretations explicit via `as`
- Dynamic optimization path switching — choose layouts at compile time
- Transitive compatibility inference — declare each compatibility explicitly
- Weakening existing optimization paths — new match arms only
- Blaming regressions on "system noise" / "HashMap iteration order" without a
  controlled A/B experiment (old vs new compiler, full suite, same machine)
- Old-style `Expr::Add`/`Expr::Mul` matches without
  `expr.normalize_to_old()` first — silent wrong output otherwise
  (`try_eval_cfloat` returning `None` for `4.0 * pi * pi` → `constant float 0.0`)

---

## 6. Optimization Philosophy

### Long-term best optimization

Emit the IR that produces the BEST FINAL CODE after LLVM's full pipeline
(SROA + GVN + DSE + LICM + vectorizer), not the cleanest-looking IR. Prefer
patterns LLVM recognizes (phi + icmp + add induction, extractvalue/insertvalue
struct decomposition). Check `opt -O3 -S unopt.ll` and count remaining
instructions — that is the true cost.

### Regression prevention

Every optimization decision must leave a comment: what pattern it targets, what
it gains (benchmarks, expected improvement), what it costs (IR bloat, compile
time, edge cases), why the trade-off is optimal, and what breaks if removed.
Before every commit: does this affect an existing optimization path? If yes,
verify it still fires (IR + benchmark), update comments, run tests AND
benchmarks. The cost of a missed optimization is measured in months.

### Regression watch / trade-off analysis

- Consider ALL code paths, not just the target. Identify the pattern, when it
  would hurt, and eliminate trade-offs by detecting-and-branching in the
  compiler when runtime detection is possible.
- Consult `docs/plans/`, `docs/architecture/`, `BUGS.md`, `git log` before
  attributing a regression. Never blame "noise" without a controlled A/B.
- Benchmark both paths before/after vs C; document the cost when a trade-off is
  kept.
- When a heuristic chooses a codegen strategy, record which strategy was chosen
  per transaction (a `bool` on `LlvmBackend` + `report_lines`) so regressions
  are diagnosable.

---

## 7. Backend Architecture Rules

### Context stratification (three lifetimes)

1. **CompilerContext (global)** — read-only during codegen: AST defs, FFI
   signatures, target specs, layout.
2. **FunctionContext (per-function)** — local variables, types, SSA register
   counter; must never outlive the function.
3. **LLVMBuilder (instruction builder)** — the sole writer of IR; raw
   `writeln!` formatting of standard instructions is forbidden.

Rules: no global-state pollution (no function-scoped transient vars on the
backend struct); all registers via `builder.gen_reg()` — no manual
`format!("%t{}", counter)`.

### Defensive codegen

- No untyped casts — every coercion goes through a centralized conversion
  helper.
- Unique temp filenames for `llc` (process/thread IDs) to avoid parallel
  collisions.
- Validate pointer-tagging assumptions (mask off low 2 bits of string ptrs)
  against target alignment.
- Every foreign function has an explicit LLVM declaration; resolve C-vs-LLVM
  return-size mismatches (bool/i32) with trunc/zext to avoid ABI register
  corruption.

### Dual-path / adaptive optimizations

When a feature has two implementations each better under different workloads,
support BOTH with a static compile-time decision tree (e.g., stack/arena vs
heap; folded/O(1) vs vectorized loop; enum/switch vs sequential reactor).

### Frontend constructs are abstract — backends give meaning

| Construct | Universal meaning | LLVM | SPIR-V | CIRCT |
|-----------|------------------|------|--------|-------|
| `sync(d) {}` | Atomic exec + sync | Txn ordering | `OpControlBarrier` | Handshake stall |
| `txn` | Convergent state loop | Phi + br | Work-item loop | Clock cycle |
| `let x` | Named binding | Stack/register | Register | Wire |
| `[pre][post]` | State convergence | Branch invariants | Guard predicates | Setup/teardown |

Before adding a `#` intrinsic, check if a frontend construct already carries
the semantics (`Barrier#()` was wrong — `sync` already means synchronize).

### Dead backends — zero fixes

`verilog.rs`, `vhdl.rs`, `c.rs`, `rust.rs`, `cobol.rs`, `x86_64.rs`,
`aarch64.rs`, `wasm.rs`, `tcl_generator.rs`. If a shared API change
mechanically breaks them, use `#[allow(unused_variables)]` / `_ => {}` /
`todo!()` with a `// dead backend` comment — do not implement the feature.

---

## 8. Commenting Mandate (Backend Updates)

**Never delete rationale comments when refactoring.** Every rationale comment is
institutional memory — rewrite it to explain the new structure, never delete it
silently. Every backend code change must include a comment at the site:

```
// YYYY-MM-DD: <short description of why this exists>
// <what problem it solves, what pattern it targets>
```

Trade-offs (faster path A but slower path B) must be documented with why the
chosen approach is optimal for the targeted situation.

---

## 9. Compiler Registry

`~/.brief/registry/` (or `dirs::data_dir()/brief/registry/`) is the per-user
directory for installing Brief modules and foreign sources. Managed by
`briefc registry {add,list,remove}`:

- `briefc registry add ./my-lib.bv` — copies the file (version-locked, no symlink)
- `briefc registry add ./xxhash/ --name xxhash` — copies a directory tree
- `briefc registry list` — enumerates contents
- `briefc registry remove <name>` — deletes the matching entry

Lookup order for `import <name>` / `from <name>`:
1. Project-local `.brief/registry/<name>` (if it exists)
2. User-wide `~/.brief/registry/<name>`
3. `config/module-registry.toml` (for imports)
4. Stdlib path (for `from <name>` and `import <name>` fallback)

See `docs/plans/2026-07-26-tamer-zero-c-and-static-memory.md` §1f.

## 10. LLVM Diagnostic Commands (when optimizer fails)

```bash
# SROA failures (struct not decomposed into scalars)
opt -O3 -pass-remarks-missed=sroa unopt.ll -disable-output 2>&1
# Loop vectorization failures
opt -O3 -pass-remarks-missed=loop-vectorize unopt.ll -disable-output 2>&1
# Alias analysis / GVN failures
opt -O3 -pass-remarks-missed=gvn unopt.ll -disable-output 2>&1
# All optimization remarks at once
opt -O3 -pass-remarks-missed=sroa,gvn,licm,loop-vectorize unopt.ll -disable-output 2>&1
# Inspect IR before/after
opt -S -O3 unopt.ll -o opt.ll
diff <(grep -v '^;' unopt.ll | grep -v '^$') <(grep -v '^;' opt.ll | grep -v '^$')
# Check if %State struct survived SROA
grep '%State' opt.ll
```
