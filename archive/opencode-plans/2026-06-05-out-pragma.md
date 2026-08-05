# Plan: `#out` Observability Contract — 2026-06-05 23:45 UTC

## Briv's Pragma Philosophy

Documented: 2026-06-06 00:15 UTC

In other languages, pragmas exist so the programmer can feed the compiler hints to optimize better. They require deep systems-level insight — the programmer must understand what the compiler struggles with and help it along.

In Briv, the compiler is already running at full speed by default — inlining, folding, precomputing, dead-field-eliminating — with maximum zealotry. **Pragmas are the programmer's way to request the compiler calm down on a specific point.** Not "help me optimize" but "I understand you can prove this is dead, but I need it alive anyway."

Every pragma follows this pattern:
- `#out` — "Calm down, this FFI call has external effects you can't see"
- `#!out(x)` — "Calm down, this field reaches hardware/I/O you can't model"
- `#assume_event(x)` — "Calm down proof engine, trust that `x` fires"
- `#assume_shape(g, a)` — "Calm down, the guard+action contract is valid; keep the txn alive"

This is teachable in one sentence: **"Briv runs at full speed by default. A pragma is a request to the compiler to hold back its zealotry on a specific point."** The programmer holds the authority — the compiler defers.

## The Problem

After LTO merges `briv_rt.c` with the program IR, LLVM sees `fprintf(stderr, ...)` in the inlined body of `__print_int`. LLVM's GlobalOpt recognizes `fprintf` via TargetLibraryInfo as a stdio write to `stderr`. Since nothing in the merged module ever re-reads `stderr`'s buffer, LLVM proves the write is dead and eliminates the entire call chain. Same for `__print_float` and `__putchar`.

The result: benchmark binaries that should print output at exit run silently. The program is "correct" by LLVM's definition — the output is never re-read — but it's wrong by user expectation.

A pure FFI like `__sqrtf` should be inlined and optimized. An output FFI like `__print_int` must be preserved even after LTO inlining. The compiler needs a way to distinguish them.

## Existing Language Precedent

Briv already has native syntax for distinguishing call kinds:

| Syntax | Kind | Precedent |
|--------|------|-----------|
| `frgn __sqrtf(x: Float) -> Float;` | Pure foreign | Inline-able, optimizable |
| `frgn! __print_int(n: Int);` | Fire-and-forget foreign | Always alive, void return — **new** |
| `syscall SYS_WRITE(fd: Int, buf: Data, count: Int) -> Result<Int, Error>;` | Kernel syscall w/ result | Target-spec resolved |
| `syscall! SYS_EXIT(code: Int);` | Fire-and-forget syscall | Target-spec resolved, void |

The `!` suffix already means "fire-and-forget, no return, external side effect" — `term!` (program exit), `syscall!` (kernel call), and now `frgn!` (foreign call with observable effect).

## Design

### Three syntaxes, one concept

| Syntax | Sugar / Desugar | Purpose |
|--------|----------------|---------|
| `frgn __print_int(n: Int) -> Bool #out;` | Base modifier | Value-returning FFI with side effects |
| `frgn! __print_int(n: Int);` | `frgn!` → `frgn ... #out` + void | Fire-and-forget output call |
| `#!out(energy);` | Program-level pragma | Declares a field externally observable |

The `#out` modifier tells the compiler: "this FFI communicates with the outside world." Its calls are never eliminated, and values flowing into it are live.

The `#!out(x)` pragma at program level declares that field `x` is externally observable even without a consuming FFI call (needed for MMIO, hardware handoff, or fields that become outputs through future compiler passes).

### Stdlib hides it transparently

```briv
// lib/std/briv_rt.bv
frgn __print_int(n: Int) -> Bool #out;     ← one annotation in stdlib source
frgn __print_float(d: Float) -> Bool #out;
frgn __putchar(c: Int) -> Int #out;
frgn! __print_str(msg: String);
```

A Briv programmer writes:
```briv
import { __print_int } from "std/briv_rt.bv";
__print_int(checksum);  // Always alive. Always works.
```

No annotation needed at use site. If curious, the programmer reads the stdlib source and finds `#out` with a doc comment explaining it.

### What it does under the hood

Changes the LLVM declaration from:
```llvm
declare i64 @__print_int(i64) #1       // #1 = { nocallback nofree nosync nounwind willreturn }
```
to:
```llvm
declare i64 @__print_int(i64) #1       // #1 unchanged (pure FFI still gets full optimization)
```

The `#out` modifier emits `memory(write)` on the ***individual declaration***, not on the shared attribute group. This survives LTO: even after `llvm-link` merges `briv_rt.bc`, the program's declaration side keeps `memory(write)`. LLVM knows the call "might write to memory that's later read" and preserves it.

`frgn!` additionally emits a void return — matching `syscall!` and `term!` semantics.

### Values are live by definition

- `__print_int(checksum)` → `checksum` is live (consumed by `#out` call)
- `frgn! __render_frame(buffer)` → `buffer` is live (consumed by `frgn!` call)
- `#!out(energy)` → `energy` is live (declared observable via pragma)

Purity analysis, dead-field elimination, and fold optimization all respect this. `compute_effectively_pure` already checks `statement_contains_ffi` — no change needed there, since calls are already non-pure. But the `#out` modifier ensures the call itself isn't eliminated by LLVM's post-codegen optimization.

### Compiler diagnostics teach the pattern

```
warning: txn 'fan' has no observable effect — folded to pure counter
  note: writes to 'checksum' are never consumed by a #out call
  help: annotate the consuming FFI with #out:
          frgn __print_int(n: Int) -> Bool #out;
        or use fire-and-forget:
          frgn! __print_int(n: Int);
```

```
warning: field 'energy' is written but never observed by any #out call
  note: the value is computed but has no visible effect
  help: add #!out(energy) at top level if this is intentional output
```

## Implementation Phases

### Phase 1 — Parser: `#out` on `frgn`, `frgn!` token, `#!out()` pragma

**Lexer** — `src/lexer.rs`:
- `Token::FrgnBang` — new token variant mapped from `frgn!`
- `Token::Out` — new token variant mapped from `#out` (reuses `#` prefix parsing like `#assume`, `#!exit`)
- `Token::HashBangOut` — new token variant mapped from `#!out`

**Parser** — `src/parser.rs`:
- `frgn! name(args);` → parse as `ForeignSignature { name, params, ffi_kind: FrgnBang, is_out: true, return_type: Type::Void, ... }`
- `frgn name(args) -> T` followed by `#out` → parse `#out` as modifier, set `is_out: true`
- `#!out(ident);` at top level → parse as program-level observable-output pragma, store in `Program { out_pragmas: Vec<String> }`

**AST** — `src/ast.rs`:
- `ForeignSignature` gains `is_out: bool` field
- `Program` gains `out_pragmas: Vec<String>` field

### Phase 2 — LLVM codegen: `memory(write)` for `#out`, void for `frgn!`

`src/backend/llvm.rs`:

- For each foreign declaration: if `is_out`, emit `memory(write)` attribute on the declaration line in addition to the base `#1` attribute group
- For `frgn!`: emit void return type (`declare void @name(...)`)
- For `#!out(x)`: emit a volatile load-then-store sequence at the end of `main()`:
  ```llvm
  %out_x = load volatile i64, i64* @x
  store volatile i64 %out_x, i64* @x
  ```
  This forces LLVM to treat `x` as memory-mapped / externally observed.
- `attributes #1` restored to `{ nocallback nofree nosync nounwind willreturn }` — pure FFI still gets full optimization

### Phase 3 — Liveness analysis: `#out` marks values alive

`src/analysis/transition_graph.rs`:

- `compute_effectively_pure`: add check for `#out` calls in body (already handled by `statement_contains_ffi` — all calls are impure. But the `#out` guarantee also prevents the *LLVM* optimizer from eliminating the call, which is a separate concern.)
- `dead_field_elimination`: fields consumed by `#out` calls are live by definition
- `#!out(x)`: mark field `x` as live in the live-fields set

### Phase 4 — Diagnostics: dead-code warnings with hints

`src/backend/llvm.rs` (warning infrastructure already exists):

- When a txn is folded as pure-counter, check if any dead fields could be live via `#out`. If so, emit diagnostic with `#out` / `frgn!` hint.
- When a field is dead-field-eliminated, check if it could be saved by `#!out()`. Emit warning with hint.

### Phase 5 — Stdlib: add `#out` to output functions

`lib/std/briv_rt.bv`:
- `frgn __print_int(n: Int) -> Bool #out;`
- `frgn __print_float(d: Float) -> Bool #out;`
- `frgn __putchar(c: Int) -> Int #out;`
- `frgn! __print_str(msg: String);` (new fire-and-forget convenience)

`lib/std/io.bv` (if it exists or gets created):
- Same pattern for any I/O wrappers

### Phase 6 — Cleanup: revert runtime & codegen hacks

`runtime/briv_rt.c`:
- Restore `static inline __attribute__((always_inline))` on `__putchar`
- Restore `static inline` on `__print_str_len`, `__write_bytes`

`src/backend/llvm.rs`:
- Restore `attributes #1 = { nocallback nofree nosync nounwind willreturn }`
- Remove `in_main` and `returns_i64` changes? No — keep these. The `ret i32 0` vs `ret i64 0` fix is type-correctness, not a liveness hack.

### Phase 7 — Test all benchmarks

- `cancel_math`, `bit_clear`, `queue_drain`, `interval_step` — pure halting patterns, no output needed
- `fasta` — streaming `__putchar` per iteration, verify chars written
- `fannkuch_redux` — exit-only `__print_int(checksum)`, verify checksum printed
- `knucleotide` — periodic + exit `__print_int(chksum)`, verify intermediate and final output
- `mandelbrot` — periodic + exit `__print_int(escapes)`, verify output
- `nbody_sqrt`, `nbody_newton` — exit `__print_float(energy)`, verify energy value
- `cargo test --lib` — no regressions

## Files Changed Summary

| File | Change |
|------|--------|
| `src/lexer.rs` | Add `Token::FrgnBang`, `Token::Out`, `Token::HashBangOut` |
| `src/parser.rs` | Parse `frgn!`, `#out`, `#!out()` |
| `src/ast.rs` | Add `is_out: bool` to `ForeignSignature`, `out_pragmas` to `Program` |
| `src/backend/llvm.rs` | Emit `memory(write)` for `#out`, void for `frgn!`, volatile for `#!out()`. Restore `attributes #1`. |
| `src/analysis/transition_graph.rs` | `#!out(x)` marks `x` live in liveness analysis |
| `lib/std/briv_rt.bv` | Add `#out` to output function declarations |
| `runtime/briv_rt.c` | Restore `static inline` on runtime helpers |
| `benchmarks/*.bv` | Remove `frgn __print_int` declarations (now from stdlib). Clean up `io_pending` remnants. |
| `BUGS.md` | Log this plan and all prior slip-ups |