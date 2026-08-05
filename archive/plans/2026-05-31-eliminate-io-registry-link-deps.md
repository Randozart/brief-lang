# Plan: Eliminate `io_registry.rs` — Link Dependencies via `import "link/..."`

**Date:** 2026-05-31
**Status:** In Progress

## Motivation

The `#io` pragma and `src/io_registry.rs` are the last remaining Rust-side magic in the compiler. They
hardcode a concept→symbol table (sigint→__sigint_flag, etc.) that has no business living in the compiler.
Following the NO MAGIC philosophy, the compiler should have zero knowledge of what runtime symbols exist.

The C runtime (`briv_rt.c`) is one possible provider of symbols — but the compiler should not know or
care which ones exist. The linker resolves symbols; the Briv source declares them. Nothing in between.

## Design

### Core Insight

`@ link` already works: `trg io: Bool @ link __io_pending` compiles to `@__io_pending = external global i8`
and the linker resolves it. The mechanism is sound. The only pollution is `io_registry.rs` translating
user-visible concept names into `@ link` addresses. Remove that translation layer, and users write
`@ link` directly in `.bv` library files (which they already can, as `lib/std/io.bv:52` demonstrates).

### New Mechanism: `import "link/briv_rt.o"`

```
import "link/briv_rt.o";   // tells the compiler driver: compile briv_rt.c and link briv_rt.o
```

This is a natural extension of the existing import syntax. The parser already handles
`import "path/to/thing"` (line 744). The import resolver currently skips `.dbvs` (line 62-67)
and handles `.css`/`.svg` as file-based imports (line 119-121). We add `.o`/`.a` handling:
instead of resolving as a Briv module, the compiler driver is told to include the
corresponding artifact at link time.

For `briv_rt.o` specifically, the compiler driver knows that `briv_rt.c` lives at
`runtime/briv_rt.c` (embedded via `include_str!`). It writes the C source, compiles it
with `cc`, and links the resulting `.o` into the final binary. For user-provided `.o`
files, the driver links them directly.

### Why Not `#link` or a New Keyword?

1. It mirrors `frgn ... from "..."` — the `from` clause already tells the compiler where to find things
2. It uses existing parser machinery — no new tokens, no new top-level constructs
3. The import keyword means "bring external artifacts into this compilation unit" — link deps fit that contract
4. The suffix (.o/.a) cleanly distinguishes "link this" from "import this Briv file"

## Changes

### Phase 1: AST + Parser — `LinkDependency`

**1.1 New AST node** (`src/ast.rs`)
```rust
pub struct LinkDependency {
    pub path: String,          // e.g., "briv_rt.o", "lib/mysensor.a"
    pub is_bundled_rt: bool,   // true if path == "briv_rt.o" → use BRIV_RT_SOURCE
}
```
Add `TopLevel::LinkDependency(LinkDependency)` to the `TopLevel` enum.

**1.2 Parser change** (`src/parser.rs`)
In `parse_import()` (line 697), after parsing the path, check if the last component ends in `.o` or `.a`:
```rust
let last = import.path.last().unwrap_or(&String::new());
if last.ends_with(".o") || last.ends_with(".a") {
    let is_bundled_rt = last == "briv_rt.o";
    return Ok(TopLevel::LinkDependency(LinkDependency {
        path: import.path.join("/"),
        is_bundled_rt,
    }));
}
// ... existing import handling continues
```

Note: `parse_import()` currently returns `Result<Import, SyntaxError>`. The return type must change
to `Result<TopLevel, SyntaxError>` since it can now produce either `Import` or `LinkDependency`.

### Phase 2: Delete `io_registry.rs` + `#io`

**2.1 Delete** `src/io_registry.rs` (94 lines)

**2.2 Delete** `parse_io_declaration()` (lines 2530-2608 in parser.rs, ~80 lines)

**2.3 Delete** the `#io` parsing loop (lines 496-510 in parser.rs, ~15 lines)

**2.4 Remove** `use crate::io_registry;` from `parser.rs:25`

**2.5 Remove** `pub mod io_registry;` from `lib.rs:60`

**2.6 Delete** 5 `#io` parser tests (lines 6105-6175, ~70 lines)

Total deletion: ~270 lines of Rust-side magic.

### Phase 3: Compiler Driver — Auto Link

**3.1 Collect link dependencies** (`src/main.rs`, `run_llvm_compile`)

After parsing, iterate program items and collect `TopLevel::LinkDependency` entries.
Store them with the LLVM compilation context. This replaces the `--link-rt` bool flag.

**3.2 Remove `--link-rt` CLI flag** (lines 3355, 3369-3371, 3391)

The flag becomes obsolete — the source code declares link dependencies.

**3.3 Auto-compile and link** (`run_llvm_compile`, lines 1852-1921)

For each `LinkDependency` in the program:
```
if dep.is_bundled_rt:
    fs::write(briv_rt.c, BRIV_RT_SOURCE)
    cc_status = cc -c -O2 briv_rt.c -o briv_rt.o [+ -DBRIV_THREAD_POOL]
    link_objects.push(briv_rt.o)
else:
    # User-provided .o/.a — just link it
    link_objects.push(dep.path)
```
Then link: `cc program.o link_objects... -o program`

The existing wake/thread-pool detection (checking `@llvm.wake_triggers`/`@llvm.thread_pool` in LLVM output)
remains unchanged — it's still needed for adding `-lrt -lpthread` flags.

### Phase 4: Library Rewrites

**4.1 New file: `lib/std/briv_rt.bv`**

```briv
import "link/briv_rt.o";

trg sigint:      Bool   @ link __sigint_flag;
trg sigterm:     Bool   @ link __sigterm_flag;
trg sighup:      Bool   @ link __sighup_flag;
trg stdin_ready: Bool   @ link __stdin_ready;
trg stdin_line:  String @ link __stdin_buffer;
trg io_pending:  Bool   @ link __io_pending;
trg clock_1hz:   Int    @ link __timer_1hz;
trg clock_100hz: Int    @ link __timer_100hz;
```

This is the ENTIRE replacement for `io_registry.rs` — a pure Briv file with zero compiler-side knowledge.
`import "link/briv_rt.o"` tells the compiler driver to include the runtime. The trigger declarations
are standard `@ link` entries. Wake is auto-detected (all `@ link` triggers are wake-capable by default,
parser.rs:2496).

**4.2 Rewrite `lib/std/system.bv`**

```briv
import {
    sigint, sigterm, sighup,
    stdin_ready, stdin_line,
    clock_1hz, clock_100hz
} from "std/briv_rt.bv";

// Helper transactions unchanged...
let stdin_buffer: List<String> = [];
let shutting_down: Bool = false;
let tick_count: Int = 0;

node read_stdin_line [sigint == false && stdin_ready == true] {
    &stdin_buffer = stdin_buffer + [stdin_line];
    term;
};

node handle_sigint [sigint == true] {
    &shutting_down = true;
    term;
};

node count_ticks [clock_tick_1hz > 0] {
    &tick_count = tick_count + 1;
    term;
};
```

No more `#io`. All trigger declarations come from the stdlib import chain.

**4.3 Update `lib/std/io.bv`**

```briv
// Currently: trg __io_pending: Bool @ link __io_pending;  (line 52)
// Replace with:
import io_pending from "std/briv_rt.bv";
```

Or keep the direct `@ link` declaration — it already works. The import from `briv_rt.bv` avoids
duplicate `@__io_pending` extern declarations (the linker deduplicates anyway, so either works).

### Phase 5: Benchmark Updates

**5.1 `benchmarks/ring_buffer.bv`**
```briv
import io_pending from "std/briv_rt.bv";

let ops: Int = 0;
const N: Int = 50000000;

node work [io_pending][ops == N] {
    &ops = ops + 1;
};
```

**5.2 `benchmarks/async_counters.bv`**
```briv
import io_pending from "std/briv_rt.bv";

let a: Int = 0;
let b: Int = 0;
const N: Int = 25000000;

async node inc_a [a < N][a == N] {
    &a = a + 1;
};

async node inc_b [b < N][b == N] {
    &b = b + 1;
};
```

**5.3 `benchmarks/build_and_bench.sh`**
Remove any remaining `--link-rt` handling for async_counters/ring_buffer — link deps are declared
in the source files, so the single `cargo run --bin briv-compiler -- llvm benchmarks/${name}.bv --out benchmarks` command works for all benchmarks.

**5.4 `benchmarks/iir_filter.bv` and `benchmarks/precompute_sum.bv`**
No changes needed — these benchmarks don't use triggers or runtime.

### Phase 6: Test Updates

**6.1 Delete** — 5 `#io` parser tests (parser.rs:6105-6175)

**6.2 Modify** — Wake-related LLVM tests that use `make_wake_trg_program("sig", "__sigint_flag", ...)`
These tests construct programs programmatically (not parsed from .bv files), so they use
`LinkRef::Linked("__sigint_flag")` directly. They should continue working unchanged — the
tests don't go through `#io`. The `is_wake` field on `TriggerDeclaration` is orthogonal to
the `#io` mechanism.

**6.3 Add** — New tests:
- `test_link_dependency_parsed` — verifies `import "link/briv_rt.o"` produces `TopLevel::LinkDependency` with `is_bundled_rt = true`
- `test_link_dependency_user_o` — verifies `import "link/lib/foo.o"` produces `LinkDependency` with `is_bundled_rt = false`
- `test_import_not_link_dep` — verifies `import system from "std/system.bv"` still produces `TopLevel::Import`, not `LinkDependency`

**6.4 Verify** — All 347 existing tests continue to pass (cargo test --lib)

### Phase 7: Cleanup

**7.1 Update** `specs/PLAN.md` — remove references to `io_registry.rs`, `#io`

**7.2 Update** `AGENTS.md` — update anchored summary

**7.3 Update** `docs/2026-05-31-progress-report-optimization-pragmas.md` — add this change as "Step 6"
in the pragmas-elimination series

## Summary by File

| File | Action | Lines |
|------|--------|-------|
| `src/io_registry.rs` | DELETE | -94 |
| `src/parser.rs` (parse_io_declaration) | DELETE | -80 |
| `src/parser.rs` (#io parsing loop) | DELETE | -15 |
| `src/parser.rs` (#io tests) | DELETE | -70 |
| `src/parser.rs` (use io_registry) | DELETE | -1 |
| `src/lib.rs` (pub mod) | DELETE | -1 |
| `src/main.rs` (--link-rt flag) | DELETE | -5 |
| **Total deletions** | | **-266** |
| | | |
| `src/ast.rs` (LinkDependency) | ADD | +12 |
| `src/parser.rs` (LinkDependency detection) | ADD | +15 |
| `src/parser.rs` (return type change) | MODIFY | ~3 |
| `src/parser.rs` (LinkDependency tests) | ADD | +30 |
| `src/main.rs` (collect + auto-link) | ADD | +40 |
| `src/main.rs` (run_llvm_compile signature) | MODIFY | -1 param |
| `src/import_resolver.rs` (skip link deps) | ADD | +5 |
| `lib/std/briv_rt.bv` | NEW | +15 |
| `lib/std/system.bv` | REWRITE | ~-7, +10 |
| `lib/std/io.bv` | MODIFY | ~2 |
| `benchmarks/ring_buffer.bv` | MODIFY | ~5 |
| `benchmarks/async_counters.bv` | MODIFY | ~5 |
| `benchmarks/build_and_bench.sh` | MODIFY | ~3 |
| **Total additions** | | **~145** |

Net: ~120 lines removed from compiler, functionality moved to library files.

## Edge Cases

### Multiple files import the same link dep

The import resolver caches resolved modules. Link dependencies should be deduplicated by path
in the compiler driver: "I already wrote `briv_rt.c` and compiled `briv_rt.o` — skip the second one."
A `HashSet<String>` of seen link paths is sufficient.

### Circular link deps

Not possible — link deps don't produce Briv items. They don't recurse through the import resolver.

### User links a missing .o

The compiler driver emits `error: link dependency 'foo.o' not found` if the file doesn't exist on disk.
For `briv_rt.o` specifically, the source is embedded — it always exists.

### What about selfhost compilation?

The selfhost compiler (`lib/compiler/`) doesn't use `#io`, `@ link`, or `briv_rt`. No impact.

### What about the wake/thread-pool detection for -lrt/-lpthread?

Unchanged. The compiler driver still inspects the generated LLVM IR for `@llvm.wake_triggers` and
`@llvm.thread_pool` to decide whether to add `-lrt -lpthread` to the link step. The detection is
orthogonal to how the link dep was declared.

### User types `import "link/briv_rt.o"` but also passes `--link-rt`

After removal, `--link-rt` is gone. If a user has a script with `--link-rt`, it will error
as an unknown flag. This is acceptable — the migration is documented.

## Risks

| Risk | Mitigation |
|------|-----------|
| `parse_import` return type change cascades | Only one call site (line 504 → line 509 after deletion). Minimal blast radius. |
| Stdlib file named `briv_rt.bv` but not a standard .bv library | It IS a standard library file — it imports `link/briv_rt.o` AND declares triggers. `from "std/briv_rt.bv"` works exactly like any other stdlib import. |
| Implicit wake detection on `@ link` triggers | Already correct. All `@ link` triggers default to `is_wake = true` (parser.rs:2496). No change needed. |
| `briv_rt.bv` triggers have `is_wake = true`, but `#io` also set it | Identical. Both paths produce `is_wake = true`. |
