# Post-Phase-7 Cleanup: Syntax Fixes, Benchmark CLI, Examples, and Regression Check

**Date:** 2026-07-15
**Status:** Active — implementation in progress
**Branch:** `main`

## Table of Contents

1. [Summary](#1-summary)
2. [Scope](#2-scope)
3. [Documentation](#3-documentation)
4. [Batch 1: Benchmark Import Syntax](#4-batch-1-benchmark-import-syntax)
5. [Batch 2: Benchmark CLI Comment Updates](#5-batch-2-benchmark-cli-comment-updates)
6. [Batch 3: README and Docs Fixes](#6-batch-3-readme-and-docs-fixes)
7. [Batch 4: Example CLI Fixes](#7-batch-4-example-cli-fixes)
8. [Batch 5: New Example Files](#8-batch-5-new-example-files)
9. [Batch 6: Benchmark Regression Check](#9-batch-6-benchmark-regression-check)
10. [Verification Gates](#10-verification-gates)

---

## 1. Summary

Phases 5-7 of the compile-time metaprogramming plan introduced new syntax
(`import <name>`, `AddressOf#`, `@ *ptr` triggers, `$` intrinsics, stage
blocks) and removed deprecated flags (`--no-stdlib`, `--plugin <path>`,
`--skip-proof`, `-o`, old `llvm`/`rbv` subcommands). After merging these
changes, ~45 existing files reference the old syntax or flags, and 4 new
syntax features lack user-facing example files.

This plan covers all cleanup work needed to bring examples, benchmarks,
README, and docs in line with the current compiler, and creates example
files for uncovered syntax features.

---

## 2. Scope

**Included:**

| Category | Count | Files |
|----------|-------|-------|
| Parenthetical import → braces | 4 | `benchmarks/ring_buffer_runtime.bv`, `nbody_newton_sym.bv`, `precompute_sum_runtime.bv`, `print_loop.bv` |
| Benchmark CLI comment updates | 24 | All `benchmarks/*.bv` files with `brief-compiler llvm` or `cargo run -- llvm` |
| README/docs/learn-brief fixes | 7 | `README.md`, `examples/README.md`, `learn-brief/11-triggers.md`, `14-bild.md`, `16-plugins.md`, `docs/architecture/overview.md`, `prelude-and-import-magic.md` |
| Example CLI flag fixes | 2 | `pipe-skip.bv`, `pipe-chain.bv` |
| New example files | 4 | `registry-import.bv`, `dynamic-trigger-trg.bv`, `stage/collect-match.bv`, `emit-error.bv` |
| Benchmark regression check | 1 run | `bash benchmarks/build_and_bench.sh --runtime --optimizer` |

**Not included:**
- Normalizer auto-annotation removal (wrong-headed per previous review)

**Late-breaking change:** `inop` and `syscall` keywords removed from language.
All `inop-*.bv` example files and `learn-brief/14-bild.md` deleted.

---

## 3. Documentation

### 3.1 Rationale comments to add/modify

Each file change below lists the rationale comment to place at the change site.

### 3.2 Architecture docs to update

- `docs/architecture/prelude-and-import-magic.md`: `--no-stdlib` → `--disable-plugin prelude`
- `docs/architecture/overview.md`: Remove `--plugin` from diagram

### 3.3 `///` doc comments to update

None — this plan changes no function signatures.

### 3.4 Preservation of existing commentary

All rationale comments in refactored code must be preserved. Since this plan
is exclusively comment/flag fixes and new files (not refactoring), no
existing rationale comments will be affected.

---

## 4. Batch 1: Benchmark Import Syntax

**Goal:** Fix 4 benchmark files that use parenthetical `import (x) from`
instead of the standard brace syntax `import { x } from`.

**Files and changes:**

| File | Line | Old | New |
|------|------|-----|-----|
| `benchmarks/ring_buffer_runtime.bv` | 11 | `import (get_env_int) from "std/env.bv"` | `import { get_env_int } from "std/env.bv"` |
| `benchmarks/nbody_newton_sym.bv` | 10 | `import (get_env_int) from "std/env.bv"` | `import { get_env_int } from "std/env.bv"` |
| `benchmarks/precompute_sum_runtime.bv` | 9 | `import (get_env_int) from "std/env.bv"` | `import { get_env_int } from "std/env.bv"` |
| `benchmarks/print_loop.bv` | 10 | `import (get_env_int) from "std/env.bv"` | `import { get_env_int } from "std/env.bv"` |

**Rationale comment:** `// 2026-07-15: Parenthetical import syntax was never standard; all other files use import { sym } from "..."`

---

## 5. Batch 2: Benchmark CLI Comment Updates

**Goal:** Update 24 benchmark files that reference the removed `llvm`
subcommand in their header comments. Two patterns:

1. `brief-compiler llvm ...` → `brief-compiler build --llvm ...`
2. `cargo run --bin brief-compiler -- llvm ...` → `cargo run --bin brief-compiler -- build --llvm ...`

Uses `replaceAll` since the pattern is uniform across all files.

**Rationale comment at each change site:** `// 2026-07-15: llvm subcommand was removed; use build --llvm`

---

## 6. Batch 3: README and Docs Fixes

### 6.1 `README.md` Quick Start (lines ~161-169)

Three broken references:
- `--strict` flag — removed in Phase 5 CLI cleanup
- `bind` subcommand — never implemented
- `lsp` subcommand — never implemented

**Fix:** Remove or replace each with correct equivalent (`check` + `build`).

### 6.2 `examples/README.md` (lines ~50, ~115)

- `brief import <name> --path <location>` — `import` subcommand does not exist
  → Replace with `brief register <name> --path <location>`
- `brief rbv component.rbv --out dist/` — `rbv` subcommand does not exist
  → Replace with `brief build --backend webstack component.rbv --out dist/`

### 6.3 `learn-brief/11-triggers.md`

- `brief llvm program.bv --link-rt` → `brief build --llvm program.bv`
- Remove `--link-rt` flag (never existed)

### 6.4 `learn-brief/14-bild.md`

**Deleted** — entire document described `inop`/BILD syntax which was removed.

### 6.5 `learn-brief/16-plugins.md`

Rewrite the opening CLI examples from `--plugin <path>` to
`--disable-plugin <name>` / `--enable-plugin <name>`. The core concepts
(plugins as compiler passes) remain valid; only the invocation changes.

### 6.6 `docs/architecture/overview.md`

ASCII diagram shows `--plugin path/to/exe` — replace with
`--enable-plugin prelude` or remove the flag reference entirely.

### 6.7 `docs/architecture/prelude-and-import-magic.md`

Three references to `--no-stdlib` → `--disable-plugin prelude`.

---

## 7. Batch 4: Example CLI Fixes

### 7.1 `examples/pipe-skip.bv` line 16

`brief-compiler rbv` → `brief-compiler build --backend webstack`

### 7.2 `examples/pipe-chain.bv` line 8

`brief-compiler rbv` → `brief-compiler build --backend webstack`

### 7.3 `examples/inop-skiplist-dispatch.bv` line 11

`--skip-proof` (removed flag) → remove from usage comment

### 7.4 `examples/inop-isr-table.bv` line 9

`--llvm` after filename (wrong position) → `build --llvm` before filename

---

## 8. Batch 5: New Example Files

### 8.1 `examples/registry-import.bv`

Demonstrates `import <name>` (angle-bracket registry syntax) and
`import { sym } from <name>`.

```brief
// 2026-07-15: Demos import <name> registry syntax and import { sym } from <name>
// Uses collections module from config/module-registry.toml

import <collections>

defn main() -> Int {
    // registry import makes all module symbols available
    term 0
}
```

Also demonstrates `import { map } from <collections>` for selective
registry imports.

### 8.2 `examples/dynamic-trigger-trg.bv`

Demonstrates `@ *ptr` dynamic trigger binding with `AddressOf#(gpio)`.
Requires LLVM backend; passes `brief check` for type safety.

```brief
// 2026-07-15: Demos @ *ptr dynamic trigger with AddressOf#(gpio) from
// config/address-map.toml
// Compile: brief-compiler build --llvm dynamic-trigger-trg.bv

import gpio from "target"

let addr: Int = AddressOf#(gpio);

node blink [true][true] {
    @ *addr -> gpio_set(1);  // dynamic trigger via pointer deref
    @ *addr -> gpio_set(0);
    term;
};
```

### 8.3 `examples/stage/collect-match.bv`

Demonstrates `Collect$` + `MatchIR$` in a `$(Back)` stage block with a
simple BEAST pattern.

```brief
// 2026-07-15: Demos Collect$/MatchIR$ BEAST pattern matching in stage blocks
// Check: brief-compiler check --emit-beast collect-match.bv

$(Back) {
    let patterns = Collect$();
    // Match for all function declarations
    let matches = MatchIR$(patterns, "(defn ?name ?params ?ret ?body)");
    // (matches are collected but this is a demo — no transformation applied)
};
```

### 8.4 `examples/emit-error.bv`

Demonstrates `EmitError$` compile-time error in a `$(Front)` stage block.

```brief
// 2026-07-15: Demos EmitError$ compile-time error
// Check: brief-compiler check emit-error.bv  (will emit error)

$(Front) {
    EmitError$("This is a compile-time error message");
};
```

---

## 9. Batch 6: Benchmark Regression Check

### 9.1 Procedure

1. `cargo build --release` (clean build)
2. `bash benchmarks/build_and_bench.sh --runtime --optimizer`
3. Collect all ratios, Brief times, C times, correctness status
4. Compare against the baseline from the Phase 7 completion commit
5. Check for any regressions in runtime benchmarks

### 9.2 Success criteria

- No runtime benchmark regresses by more than 5% in ratio
- All optimizer benchmarks remain fully folded (ratio ~0 or .text < 25% of C)
- All correctness checks pass

---

## 10. Verification Gates

Before final commit:

1. `cargo test --lib` — all tests pass
2. `cargo build` — no warnings
3. New example files pass `brief check` (typecheck only)
4. `examples/dynamic-trigger-trg.bv` passes `brief build --llvm` (codegen)
5. Run Praetor on new/changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6)
6. Benchmark regression check confirms no regressions
