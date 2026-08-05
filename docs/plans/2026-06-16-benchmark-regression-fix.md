# Benchmark Regression Fix & Intrinsic Migration Plan

**Date:** 2026-06-16  
**Author:** OpenCode  
**Status:** Plan — implementation in progress

## Root Causes Identified

### Bug 1 (Critical): `ret void` in guard else-path breaks LTO pipeline
**File:** `src/backend/llvm/emit_stmt.rs:491-502`  
**Introduced by:** Commit `392674f8` (Jun 13), worsened by `1980461e` (Jun 15)

When a guarded statement's then-path terminates (e.g., `[count == N] { term! -> __print_int(nchksum); }`),
the else-path (guard false → loop continuation) emits `ret void` instead of branching
to the loop back-edge. This is inside `define i32 @main()`, so opt/llc/llvm-as all
fail with `"value doesn't match function result type 'i32'"`. LTO falls through to
a broken `clang -O3 *.ll -o bench` fallback, which also fails. Binaries don't get
rebuilt — stale pre-regression binaries persist.

**Affected benchmarks (8 of 22):** precompute_sum, nbody_newton, nbody_sqrt,
fannkuch_redux, mandelbrot, knucleotide, and others with `[guard] { term! ... }`.

### Bug 2 (Critical): `benchmarks/briv_rt.c` writes output to stderr without flushing
**File:** `benchmarks/briv_rt.c`, `runtime/briv_rt.c`

`__print_int`, `__print_float`, `__putchar` all use `fprintf(stderr, ...)` without
calling `fflush(stderr)`. The constructor `briv_rt_ctor` sets
`setvbuf(stderr, NULL, _IOFBF, 65536)` (fully buffered 64KB). Output accumulates
in the buffer and is silently discarded on `exit()`.

The canonical `lib/runtime/briv_rt.c` writes to stdout with `fwrite` + `fflush`
and works correctly. Benchmarks that DON'T have Bug 1 (e.g., print_loop, fasta,
ring_buffer) DO link via LTO successfully — but their output disappears into
the stderr buffer void.

### Bug 3 (Configuration): `resolve_link_source` picks wrong `briv_rt.c`
**File:** `src/main.rs:1848-1853`

Search order is project-relative first. Since source files are in `benchmarks/`,
`source_dir = benchmarks/`, so `link/briv_rt.c` resolves to
`benchmarks/briv_rt.c` before `lib/runtime/briv_rt.c`. The benchmarks directory
has a stripped-down, differently-behaved runtime copy.

### Bug 4 (Design): `has_side_effects()` missing from intrinsics
**File:** `src/analysis/transition_graph.rs:740`

All intrinsics are unconditionally treated as impure (`Expr::IntrinsicCall { .. } => true`).
Pure intrinsics like `sqrt#`, `abs#`, `ctpop#` cannot be folded even when their
inputs are compile-time known. No metadata exists to distinguish pure from impure.

---

## Fix Plan — 7 Phases

### Phase 0 — Fix `ret void` in Guard Else-Path
**Scope:** 1 file, ~10 lines changed

Remove the bogus `ret void`/`ret i64 0` emission in the guard else-path when
the then-path terminates. Restore `prev_terminated` so callers continue the loop.

```rust
// Before (broken):
} else {
    self.terminated = true;
    writeln!(out, "  ret void").ok();  // kills loop in main()
}

// After (fixed):
} else {
    self.terminated = prev_terminated;  // continue loop naturally
}
```

**Verification:** `opt -O3` and `llc` pass on knucleotide.ll, nbody_newton.ll, etc.

---

### Phase 1 — Add `has_side_effects()` Metadata to Intrinsic Enum
**Scope:** 2 files, ~50 lines total

Add a method on `Intrinsic` that classifies each variant as observable
(has side effects, cannot fold) or pure (can fold safely). Use this in
`references_triggers_or_ffi` to allow folding of `sqrt#`, `abs#`, etc.

**File:** `src/ast.rs` — add:
```rust
impl Intrinsic {
    fn has_side_effects(&self) -> bool {
        match self {
            // Observable I/O — never fold
            Intrinsic::Println | Intrinsic::PutChar | Intrinsic::PrintInt
            | Intrinsic::PrintFloat | Intrinsic::Readln
            | Intrinsic::WriteFile | Intrinsic::Exit
            | Intrinsic::TtyReadKey | Intrinsic::TtyRawMode
            | Intrinsic::Spawn | Intrinsic::SpawnWithOutput
            | Intrinsic::Open | Intrinsic::Close | Intrinsic::Read
            | Intrinsic::Write | Intrinsic::Fcntl | Intrinsic::FSync
            | Intrinsic::GetEnvInt          // reads external state
            // ... all syscall-like intrinsics ...
            => true,

            // Pure/mathematical — can fold safely
            Intrinsic::Sqrt | Intrinsic::Fabs | Intrinsic::Ceil
            | Intrinsic::Floor
            | Intrinsic::Ctpop | Intrinsic::Ctlz | Intrinsic::Cttz
            | Intrinsic::Abs | Intrinsic::Bitreverse
            | Intrinsic::Bytes | Intrinsic::Size
            => false,
        }
    }
}
```

**File:** `src/analysis/transition_graph.rs:740` — update:
```rust
Expr::IntrinsicCall { intrinsic, .. } => intrinsic.has_side_effects(),
```

---

### Phase 2 — Add New Intrinsic Variants (PrintInt, PutChar, PrintFloat, GetEnvInt)
**Scope:** 8 files, ~200 lines total

Add 4 new variants to the `Intrinsic` enum, wired through parser →
typechecker → interpreter → LLVM codegen. Use direct libc calls in LLVM
(no briv_rt.c shims).

**New variants:**

| Variant | String name | Args | Return | `has_side_effects` |
|---|---|---|---|---|
| `PrintInt` | `"print_int"` | 1 Int | Bool | `true` |
| `PutChar` | `"putchar"` | 1 Char | Bool | `true` |
| `PrintFloat` | `"print_float"` | 1 Float | Bool | `true` |
| `GetEnvInt` | `"getenv_int"` | 1 String | Int | `true` |

**Parser** (`src/parser.rs`): Already works — `name#(args)` parsing is generic.
Just add names to parser tests.

**Typechecker** (`src/typechecker.rs:~1280`): Add return types:
- `PrintInt | PutChar | PrintFloat => Type::Bool`
- `GetEnvInt => Type::Int`

**Interpreter** (`src/interpreter.rs:~1475`): Add match arms:
- `PrintInt` → `print!("{}", v)` → `Bool(true)`
- `PutChar` → `print!("{}", c as u8 as char)` → `Bool(true)`
- `PrintFloat` → `print!("{:.9}", v)` → `Bool(true)`
- `GetEnvInt` → `env::var(name).ok()?.parse().unwrap_or(0)` → `Int`

**LLVM codegen** (`src/backend/llvm/emit_expr.rs`): Emit direct libc calls:
```llvm
; print_int#(n): fprintf(stdout, "%ld\n", n)
@FMT_INT = private unnamed_addr constant [4 x i8] c"%ld\0A"
%stdout_v = load ptr, ptr @stdout
%res = call i32 (ptr, ptr, ...) @fprintf(ptr %stdout_v, ptr @FMT_INT, i64 %n)

; putchar#(c): fputc(c, stdout)
%stdout_v = load ptr, ptr @stdout
%res = call i32 @fputc(i32 %c, ptr %stdout_v)

; print_float#(d): fprintf(stdout, "%.9f\n", d)
@FMT_FLOAT = private unnamed_addr constant [6 x i8] c"%.9f\0A"
%stdout_v = load ptr, ptr @stdout
%res = call i32 (ptr, ptr, ...) @fprintf(ptr %stdout_v, ptr @FMT_FLOAT, double %d)

; getenv_int#(name): { char* v = getenv(name); return v ? atol(v) : 0; }
%v = call ptr @getenv(ptr %name)
%ok = icmp ne ptr %v, null
br i1 %ok, label %ok, label %nul
nul: ret i64 0
ok: %n = call i64 @atol(ptr %v); ret i64 %n
```

**LLVM declarations** (`src/backend/llvm/mod.rs`):
```llvm
@stdout = external global ptr
declare i32 @fprintf(ptr, ptr, ...)
declare i32 @fputc(i32, ptr)
declare ptr @getenv(ptr)
declare i64 @atol(ptr)
```

**Format string pool** (`src/backend/llvm/mod.rs`): Add flags and emission:
```rust
struct LlvmBackend {
    // ...
    needs_fmt_int: bool,
    needs_fmt_float: bool,
    needs_fmt_str: bool,
}
```

Emit format strings at the end of the module header if flags are set.

---

### Phase 3 — Fix Existing Intrinsic Stubs in LLVM Backend
**Scope:** 1 file (`emit_expr.rs`), ~30 lines changed

| Variant | Current stub | New codegen |
|---|---|---|
| `Println` | `and i1 true, true` | `@fprintf(@stdout, @FMT_STR, %msg)` then `\n` |
| `Readln` | `add i64 0, 0` | `call i64 @briv_read_stdin(i64 %buf)` |
| `Exit` | `call void @__exit()` + stub | `call void @exit(i32 0)` |
| `WriteFile` | `add i64 0, 1` | delegate to `briv_write_file` or `fwrite` |
| `Sleep` | `add i64 0, 1` | `call i64 @briv_nanosleep(i64 %ms)` |
| `SpawnWithOutput` | ignores args[1]+ | merge args into shell command |

---

### Phase 4 — Type Checker Fixes (Issues 1, 3, 4, 5)
**Scope:** 1 file (`typechecker.rs`), ~20 lines changed

**Issue 1 — GetEnv return type:**  
Split current catch-all `GetEnv | SetEnv | ... => Type::Int` into:
- `GetEnv => Type::String`
- `SetEnv | UnsetEnv => Type::Bool`
- `GetPid | GetPPid | ClockGetTime | NanoSleep => Type::Int`

**Issue 3 — String indexing `s[i]`:**  
When the collection is `String`, return `Type::Char` instead of `Custom("unknown")`.

**Issue 4 — Char ↔ String casts:**  
Add cast validation rules for `(String)Char` and `(Char)Int`.

**Issue 5 — Char arithmetic:**  
Allow `Char + Int`, `Char - Int`, `Char - Char` in arithmetic expression type-checking.
Implement in interpreter (i32 math with trunc/zext) and LLVM backend (i32 arithmetic).

---

### Phase 5 — Migrate All Benchmarks to Intrinsics
**Scope:** ~22 `.bv` files in `benchmarks/`

For each file:
1. Remove `import "link/briv_rt.c";`
2. Remove `frgn __print_int(n: Int) -> Bool ;` etc.
3. Replace calls with `#` intrinsic syntax

**Migration table:**

| `frgn` call | Intrinsic replacement |
|---|---|
| `__print_int(n)` | `print_int#(n)` |
| `__print_float(d)` | `print_float#(d)` |
| `__putchar(c)` | `putchar#(c)` |
| `__get_env_int("BOUND")` | `getenv_int#("BOUND")` |
| `__print(msg)` | `println#(msg)` |
| `import { __get_env_int } from "std/env.bv"` | remove (use `getenv_int#`) |

**Benchmarks to migrate (22+):**
async_counters, async_counters_runtime, bit_clear, cancel_math, const_heavy,
fannkuch_redux, fannkuch_redux_sym, fasta, float_math, float_math_nonzero,
iir_filter, iir_filter_runtime, interval_step, kalman_filter_runtime,
knucleotide, mandelbrot, nbody_newton, nbody_sqrt, precompute_sum,
print_loop, queue_drain, queue_drain_idio, queue_drain_sym, ring_buffer,
sparse_dispatch

---

### Phase 6 — Clean Up Dead Runtime Files
**Scope:** 2 files removed

- **Remove** `benchmarks/briv_rt.c` — no longer referenced after migration
- **Remove** `runtime/briv_rt.c` — duplicate of `lib/runtime/briv_rt.c`
- **Keep** `lib/runtime/briv_rt.c` — canonical runtime for programs that LTO-link

---

### Phase 7 — Update Tests
**Scope:** 3 files, ~50 lines added/changed

**File:** `src/parser.rs` — add parser tests for new intrinsic names
**File:** `src/typechecker.rs` — fix tests that assert `GetEnv => Type::Int`
**File:** `src/backend/llvm/tests.rs` — update expected LLVM IR; add new tests
**File:** `src/interpreter.rs` — add eval tests for new intrinsics

---

## Verification

After each phase:
1. `cargo test --lib` — all tests pass
2. `cargo build --release --bin briv-compiler` — no warnings

Final verification:
3. `bash benchmarks/build_and_bench.sh --correctness` — all outputs match C
4. `bash benchmarks/build_and_bench.sh` — all 22+ benchmarks build and time
5. Compare timing against 8914ac5 results table

## Risk Register

| Risk | Impact | Mitigation |
|---|---|---|
| `fprintf`/`putchar` varargs LLVM IR wrong | Segfault or no output | Test each intrinsic in isolation; verify with strace |
| Format string pool conflicts with existing `@str.N` constants | Name collision | Use `@FMT_INT` prefix (all caps, `FMT` prefix is unique in codebase) |
| Char arithmetic changes affect non-benchmark code | Breakage in stdlib | Run `cargo test --lib` after every change |
| Stale `benchmarks/briv_rt.c` deletion breaks something | Build failure | Only delete after all benchmarks migrated and tested |
