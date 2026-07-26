# Language Decluttering Plan

**Date:** 2026-07-07
**Status:** Draft — awaiting approval to begin
**Branch:** `feat/language-decluttering`

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Baseline Benchmarks](#2-baseline-benchmarks)
3. [Workflow Rules](#3-workflow-rules)
4. [Rust Warnings Baseline](#4-rust-warnings-baseline)
5. [Phase 0: Immediate Cleanup](#5-phase-0-immediate-cleanup)
6. [Phase 1: Annotation System Unification](#6-phase-1-annotation-system-unification)
7. [Phase 2: Bits Thesis Type System](#7-phase-2-bits-thesis-type-system)
8. [Phase 3: Intrinsic Reduction + Prelude](#8-phase-3-intrinsic-reduction--prelude)
9. [Phase 4: Documentation Overhaul](#9-phase-4-documentation-overhaul)
10. [Phase 5: Error/Warning Improvements](#10-phase-5-errorwarning-improvements)
11. [Validation Strategy](#11-validation-strategy)
12. [Rollback Plan](#12-rollback-plan)

---

## 1. Executive Summary

Brief has accumulated syntax bloat across six dimensions:

| Bloat Area | Current | Target | Effort |
|------------|---------|--------|--------|
| Type keywords (Int8..Float64, Signed, Unsigned, etc.) | ~26 tokens | ~8 tokens (`Int`, `i8`..`i64`, `u8`..`u64`, `f32`/`f64`) | High |
| Keyword aliases (sig/sign/signature, defn/def/definition, etc.) | ~60 extra token attrs | 0 extra | Low |
| UPPERCASE keyword variants (SIG, DEFN, TXN, etc.) | ~100 token attrs | 0 | Low |
| Dead code (Alka, OnExit) | ~81 match arms | 0 | Low |
| Intrinsic variants (networking, signals, IPC, threading, etc.) | ~120 | ~40 | Medium |
| Hashtag annotation system (4 prefix syntaxes, 3 values forms, scoped wrapping, fallback chains) | ~10 syntax combinations | 3 surface syntaxes, 1 internal repr | Medium |

**Cross-cutting improvements:**
- All Rust code must follow flat control flow (max 2 nesting depth). LLVM IR emission is excluded — `.ll` output must be efficient, not pretty.
- Every change must be accompanied by architectural rationale comments (`// 2026-07-07: ...`).
- Every removal must be preceded by a unit test that verifies the replacement functionality.
- Rust compiler warnings (currently 29) must be eliminated.
- Kani harnesses (`src/backend/llvm/kani.rs`) that only generate warnings without proving anything new must be removed or converted to unit tests.
- Every step that could affect benchmarks must be verified with `bash benchmarks/build_and_bench.sh`.
- Every syntax change must update `docs/learn/types.md`, `docs/learn/*.md`, `docs/reference/BRIEF_LANGUAGE_REFERENCE.md`, and `spec/SPEC.md`.

---

## 2. Baseline Benchmarks

Captured from commit `5467de8` on `main`. All benchmarks run via `bash benchmarks/build_and_bench.sh --runtime` after `cargo build --release`.

| Benchmark | Brief vs C | Brief wins by |
|-----------|-----------|---------------|
| nbody_newton | 0.70x | 30% |
| nbody_sqrt | 0.72x | 28% |
| nbody_sqrt_idio | 0.72x | 28% |
| float_math | 0.82x | 18% |
| sparse_dispatch | 0.09x | 11× (dispatch collapse) |
| fannkuch_redux | 0.99x | tied (was 2.16x slower) |
| interval_step | 0.01x | 100× (precomputed) |
| knucleotide | 0.99x | tied |
| fasta | 0.99x | tied |
| mandelbrot | 1.00x | tied |
| queue_drain | 1.00x | tied |

**Regression rule**: Any change that drops a benchmark below 0.97x of the above baseline must be reverted or fixed before proceeding. A change that improves any benchmark without regressing others is always acceptable.

**Pre-computed / optimizer-only benchmarks** (`.text` size ratio < 25% of C reference) are skipped by the harness but correctness is still verified.

**Benchmarking procedure**:
1. `cargo build --release`
2. `bash benchmarks/build_and_bench.sh --runtime`
3. Collect the `Brief vs C` ratios from the output table
4. Compare each ratio against the baseline above
5. Any ratio below 0.97x of baseline is a regression — block and revert

---

## 3. Workflow Rules

### 3.1 Git Discipline

1. Work is performed on branch `feat/language-decluttering` branched from `main` at commit `5467de8`.
2. Each logical step is committed separately with a message matching the pattern:
   ```
   declutter(phase-N): short description
   
   Longer description of what was done and why.
   ```
3. Before each commit: `cargo test --lib` and `cargo build` (no warnings).
4. After each step that could affect the emitted IR: `bash benchmarks/build_and_bench.sh --runtime` and compare against the baseline table in this document.
5. The branch is pushed periodically for backup, never force-pushed.
6. No amendments — each commit is a new checkpoint.

### 3.2 Flat Control Flow (Max 2 Nesting Depth)

Every Rust function modified in this plan MUST adhere to:

```
fn process(x: Option<Value>) -> Option<i64> {
    let val = x?;
    let result = val.as_i64()?;
    if result <= 0 {
        return None;
    }
    Some(result)
}
```

Anti-patterns that must be eliminated:
- `if let` chains deeper than 1 level — use `?` and guard clauses
- `match` inside `match` — extract inner match to helper function
- `for` inside `if` inside `match` — inline or extract

**Exception**: LLVM IR emission code (`emit_expr.rs`, `emit_stmt.rs`, `emit_toplevel.rs`, builder methods, backend `mod.rs` functions that produce `.ll` output directly). The emitted LLVM must be efficient above all else. However, helper functions called by these emitters are NOT exempt — if the emitter calls a helper, that helper MUST follow flat control flow.

### 3.3 Architectural Comments

Every file modification must include or update rationale comments:
```
// 2026-07-07: <phase> — <what and why>
// <what problem this solves, what pattern it targets>
```

Existing rationale comments must never be deleted — they are institutional memory. If a refactoring changes the structure, rewrite the comments to explain the new structure.

### 3.4 Pre-Removal Tests

Before removing ANY of the following, a unit test must be written that verifies the replacement:

- **Intrinsic variants**: Test that `name#(args)` works via the replacement `inop` declaration in the prelude, producing the same result as the old intrinsic.
- **Type variants** (`Type::Int8`, etc.): Test that `Int<8>` in source code produces equivalent type-checking and codegen outcomes.
- **Keyword tokens** (uppercase/alias variants): Test that using the canonical form still works.
- **Dead code** (Alka, OnExit): Test that programs that previously used these constructs (if any exist) produce a helpful error message.

The test must be committed BEFORE the removal commit, in the same branch.

**Test location convention**:
- Tests that verify source-syntax behavior (e.g., `Int<8>` parsing, `(a: 1) ~> [x]` parsing) go in `src/tests.rs` with the existing integration tests.
- Tests that verify a specific compiler pass (typechecker, interpreter, backend) go in the corresponding module's test section or inline `#[cfg(test)] mod tests { ... }` block.
- Pre-removal tests that verify an intrinsic replacement go in both places: (1) a compile-time test that the `inop` declaration resolves, and (2) a runtime test that the call produces the correct result (if the intrinsic can be meaningfully tested without side effects).
- Tests must use the canonical assertion pattern: `let result = compile_and_run(source); assert_eq!(result, expected);` or equivalent.
- Tests must be self-contained — no external file dependencies unless unavoidable.
- **Exception**: Intrinsics that interact with the real filesystem (e.g., `open#`, `mkdir#`, `unlink#`) cannot be tested without temp files. For these, the pre-removal test should test that the intrinsic RESOLVES correctly (name resolution + type checking) without actually executing the I/O. A throwaway temp directory (`/tmp/opencode-test-*`) may be used for the few intrinsics that genuinely need runtime execution.

### 3.5 God Function Extraction Rule

Any function exceeding ~100 lines encountered during refactoring MUST be evaluated for extraction. If it performs multiple distinct responsibilities (e.g., "parse expression, handle operator precedence, AND emit error messages"), each responsibility should be extracted into its own function or file.

**Extraction workflow**:
1. Identify the god function and its distinct sub-responsibilities.
2. Create a new file or module for each sub-responsibility.
3. Extract the code into the new file — commit as `declutter(extract): move <name> to <path>`.
4. Add flat-control-flow refactoring and unit tests for each extracted function — commit separately.
5. The extraction commit MUST preserve the original behavior exactly. No behavioral changes during extraction.
6. Only AFTER extraction, make behavioral changes (e.g., updating type dispatch to Bits).

**Example candidates** (known god functions in the codebase):

| Candidate | Location | What it mixes |
|-----------|----------|---------------|
| `parse_expression()` | `parser.rs` chain | Expression parsing for ~30+ expression types; operator precedence; error recovery |
| `emit_toplevel.rs:emit_transaction()` | ~800+ lines | Emits LLVM function, init block, convergence loop, phi nodes, term dispatch |
| `emit_intrinsic_call()` | `expr/intrinsics.rs:14` | 120+ intrinsic dispatch, argument marshalling, return type handling |
| `emit_binop()` | `helpers.rs:876` | 45+ (operator, type) dispatch pairs for arithmetic |
| `resolve_imports()` | `import_resolver.rs` | Bootstrap type injection, core module injection, prelude injection, path resolution |

Each extraction follows the same pattern: extract → commit (no behavior change) → refactor → commit (flat control flow, tests).

---

## 4. Rust Warnings Baseline

Current `cargo build` produces 29 warnings:

| Count | Warning | Source |
|-------|---------|--------|
| 23 | `unexpected cfg condition name: kani` | `#[cfg(kani)]` blocks across 23 files |
| 2 | `unexpected cfg condition value: nightly` | `src/backend/llvm/gpu.rs:1105,1107` |
| 1 | `variable N should have a snake case name` | `src/analysis/gpu_cost.rs:49` |
| 1 | `variable cost_N should have a snake case name` | `src/backend/llvm/mod.rs:999` |
| 1 | `type DbvlTableInner is more private` | `src/ast.rs:51` (interpreter::Value::DbvlTable inner type) |
| 1 | `type ChimeraInfo is more private` | `src/analysis/region.rs` (FunctionContext::chimera_map) |

Total to eliminate: **29 warnings → 0**.

### 4.1 Kani cfg warnings (23 occurrences)

The `#[cfg(kani)]` condition is unrecognized by normal `cargo build` because `kani` is not a built-in cfg key. These blocks exist in 23 files:

- `src/annotator.rs`, `src/ast.rs`, `src/backend/llvm/mod.rs`, `src/backend/webstack.rs`
- `src/features/mod.rs`, `src/features/traits.rs`, `src/features/literal.rs`
- `src/features/binary_op.rs`, `src/features/unary_op.rs`, `src/features/call.rs`
- `src/features/stmt/assignment.rs`, `src/features/toplevel/typedef.rs`
- `src/interpreter.rs`, `src/analysis/dataflow.rs`, `src/analysis/transition_graph.rs`
- `src/backend/llvm/kani.rs` (the main Kani harness file)

**Approach**: Replace `#[cfg(kani)]` with `#[cfg(feature = "kani")]` and add a `kani` feature to `Cargo.toml`. This makes the cfg recognized and eliminates the warnings without removing the Kani harnesses.

If the project has no intention of ever running Kani again, remove the harnesses entirely and convert any valuable assertions to unit tests.

### 4.2 Nightly cfg warnings (2 occurrences)

`src/backend/llvm/gpu.rs:1105,1107` — uses `cfg(nightly)`. Add `nightly` to `Cargo.toml`'s `[lints.rust.unexpected_cfgs.check-cfg]` list.

### 4.3 Snake case warnings (2 occurrences)

- `src/analysis/gpu_cost.rs:49` — rename `N` → `n`
- `src/backend/llvm/mod.rs:999` — rename `cost_N` → `cost_n`

### 4.4 Private type visibility warnings (2 occurrences)

- `src/ast.rs:51` — `DbvlTableInner` is `pub(crate)` but `Value::DbvlTable` field is `pub`. Make the inner type `pub` or the enum field `pub(crate)`.
- `src/analysis/region.rs` — `ChimeraInfo` visibility mismatch with `FunctionContext::chimera_map`.

---

## 5. Phase 0: Immediate Cleanup

### 5.1 Step 0a — Drop UPPERCASE keywords

**File**: `src/lexer.rs`

Remove every `#[token("UPPERCASE")]` variant from the `Token` enum. For each keyword, keep only the canonical lowercase form.

**Example**: Remove `#[token("SIG")]`, `#[token("SIGN")]`, `#[token("SIGNATURE")]`, keep only `#[token("sig")]`.

**Scope**: ~100 token attributes removed. Single file change.

**Pre-removal test**: Verify that lowercase keywords parse correctly (existing tests already exercise this).

**Doc update**: `docs/reference/BRIEF_LANGUAGE_REFERENCE.md` keywords table — remove the "Aliases" column for case.

**Commit**: `declutter(0a): drop UPPERCASE keyword variants`

### 5.2 Step 0b — Canonicalize keyword aliases

**File**: `src/lexer.rs`

| Token | Keep | Drop |
|-------|------|------|
| `Sig` | `sig` | `sign`, `signature` |
| `Defn` | `defn` | `def`, `definition` |
| `Const` | `const` | `constant` |
| `Txn` | `txn` | `transact`, `transaction` |
| `Unification` | `uni` | `union`, `unify`, `unionization` |
| `Resource` | `reg` | `rsrc`, `resources`, `registry` |
| `Pvt` | `pvt` | `private` |
| `Sed` | `sed` | `sedentary` |
| `Trg` | `trg` | `trigger` |
| `TrgBang` | `trg!` | `trigger!` |

**Pre-removal test**: Verify that using `uni` parses as `Unification` and `reg` parses as `Resource` (existing tests likely already exercise these).

**Impact**: ~40 token attributes removed.

**Commit**: `declutter(0b): canonicalize keyword aliases — keep shortest form`

### 5.3 Step 0c — Remove dead code: Alka + OnExit

**Files touched (active):**

| File | Alka | OnExit |
|------|------|--------|
| `src/ast.rs` | Remove variant + AlkaBlock struct | Remove variant |
| `src/parser.rs` | Remove commented-out alka handling | Remove commented-out on_exit handling |
| `src/desugarer.rs` | Remove pass-through match arm | Remove pass-through match arm |
| `src/typechecker.rs` | Remove match arm | Remove match arm |
| `src/interpreter.rs` | Remove clone + skip arms | Remove clone + skip arms |
| `src/proof_engine.rs` | Remove skip arms | Remove skip + analysis arms |
| `src/reactor.rs` | Remove skip arm | Remove skip arm |
| `src/backend/llvm/mod.rs` | Remove skip arm | Remove skip arm |
| `src/backend/llvm/helpers.rs` | Remove clone arm | Remove clone arm |
| `src/backend/llvm/emit_stmt.rs` | Remove emit arm (wrote content) | Remove cleanup registration arm |
| `src/backend/llvm/emit_toplevel.rs` | — | Remove cleanup emission arms (x3) |
| `src/backend/webstack.rs` | Remove emit arms (x2) | Remove emit arms (x2) |
| `src/analysis/region.rs` | Remove analysis arms (x4) | Remove analysis arms (x3) |
| `src/analysis/dataflow.rs` | Remove arm | Remove arm |
| `src/analysis/transition_graph.rs` | — | Remove arm |
| `src/backend/mod.rs` | — | Remove hashtag collection arm |
| `src/features/stmt/` | Delete `alka.rs` if exists | Delete `on_exit.rs` |
| `src/type_universe.rs` | — | Remove comment reference |
| `src/fuzzing/ast_generator.rs` | Remove generation arms | Remove generation arms |

**Dead backend files**: verilog, vhdl, rust, c, cobol, wasm, x86_64, aarch64 — per policy, replace match arm with `_ => {}` catch-all. Do NOT modify these files further.

**Pre-removal test**: Write a test that verifies `alka` and `on_exit` produce a helpful parse error if referenced (they shouldn't parse anymore since the parser never produces them). Verify that any existing `.bv` file that somehow references these still produces a clean error.

**Commit**: `declutter(0c): remove permanently abandoned Alka and OnExit variants`

### 5.4 Step 0d — Fix Rust compiler warnings

**5.4.1 Kani cfg (23 warnings)**

Replace `#[cfg(kani)]` with `#[cfg(feature = "kani")]` across all 23 files. Add to `Cargo.toml`:

```toml
[features]
kani = []

[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(feature, values("kani"))'] }
```

Alternatively, if Kani is not being used: remove all `#[cfg(kani)]` blocks and the `src/backend/llvm/kani.rs` file entirely. Convert any assertions from Kani harnesses into regular `#[test]` unit tests.

**Decision needed**: Keep Kani or remove?

**5.4.2 Nightly cfg (2 warnings)**

Add to `Cargo.toml`:
```toml
[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(nightly)'] }
```

**5.4.3 Snake case (2 warnings)**

- `src/analysis/gpu_cost.rs:49`: Rename `N: u64` → `n: u64`
- `src/backend/llvm/mod.rs:999`: Rename `cost_N` → `cost_n`

**5.4.4 Private type visibility (2 warnings)**

- `src/ast.rs:51`: Change `pub(crate) struct DbvlTableInner` to `pub struct DbvlTableInner` OR change `interpreter::Value::DbvlTable(Box<DbvlTableInner>)` to `pub(crate)` if the field is not accessed outside the crate.
- `src/analysis/region.rs`: Match the visibility of `ChimeraInfo` to `FunctionContext::chimera_map`'s requirement.

**Commit**: `declutter(0d): eliminate 29 Rust compiler warnings — kani cfg, snake case, visibility`

### 5.5 Phase 0 Verification

After Step 0d:
```
cargo build             # must show 0 warnings
cargo test --lib        # must pass all 1403+ tests
bash benchmarks/build_and_bench.sh --runtime   # compare against baseline table
```

If any benchmark regresses beyond 0.97x, stop and revert the offending commit.

---

## 6. Phase 1: Annotation System Unification

### 6.1 Problem Statement

The current hashtag system has 4 prefix syntaxes (`#`, `#!`, `#?`, `#[scope]`), 4 value forms (string, int, ident, fallback chain with `|`), and scoped nesting. There's no consistency — it behaves like 3rd-rate syntax despite having 1st-class effects on codegen.

### 6.2 Design

**Single internal representation:**

```rust
pub struct Annotation {
    pub name: String,         // dotted: "llvm.inline", "gpu"
    pub value: Expr,          // any expression; Bool(true) for flags
    pub mode: AnnotationMode,
    pub span: Span,
}

pub enum AnnotationMode {
    Advisory,     // was #name — try this if supported
    Mandatory,    // was #!name — error if unsupported
    Speculative,  // was #?name — try, silently skip if unsupported
}
```

**Three surface syntaxes, all desugaring to the same `Annotation` struct:**

| Syntax | Example | Desugared |
|--------|---------|-----------|
| `#name` | `#gpu` | `Annotation { name: "gpu", value: Bool(true), mode: Advisory }` |
| `#!name` | `#!gpu` | `Annotation { name: "gpu", value: Bool(true), mode: Mandatory }` |
| `#?name` | `#?gpu` | `Annotation { name: "gpu", value: Bool(true), mode: Speculative }` |
| `decl <~ (k: v, ...)` | `txn foo <~ (unroll: 4, section: "init")` | Postfix structured — values are arbitrary exprs |
| `(k: v, ...) ~> [guard]` | `(cold: true, likely: false) ~> [x > 0] { ... }` | Prefix structured on guards |

**Design decisions:**

1. **`#name` stays as surface syntax** — muscle memory preserved. All three hash forms remain valid.
2. **`#[scope] { #inner }` is removed** — replaced by dotted names in `<~`: `<~ (llvm.inline: true, x86.align: 16)`. The scope becomes a dotted prefix on the annotation name.
3. **`#!name|alt1|alt2` fallback chains are removed** — replaced by composed annotations: `<~ (alt1: true, alt2: true)`.
4. **`<~` (postfix)** — reads left-to-right as "declaree gets annotation". `txn process <~ (gpu: true)`.
5. **`~>` (prefix)** — reads left-to-right as "annotation points to guard". `(cold: true) ~> [x > 0] { body }`.
6. **Values can be ANY expression** — `(timeout: n * 1000)`, `(name: "prefix_" ++ id)`.
7. **Parentheses required** around the annotation body in both `<~` and `~>` forms — visually groups the key-value pairs.

### 6.3 Implementation

**Lexer prerequisite — Add `~>` token**

Add `~>` to `src/lexer.rs` as a new token `TildeArrowRight` (or `RArrowTilde`):

```rust
#[token("~>")]
TildeArrowRight,
```

This token is used by the prefix annotation syntax `(k: v, ...) ~> [guard]`. It reads as "annotation points right to the guarded condition." Must be lexed before `~` and `>` individually (logos handles multi-character tokens first when placed before shorter patterns).

**Commit**: `declutter(1a-lexer): add ~> token for prefix annotation syntax`

**Step 1.1 — Define `Annotation` struct + `AnnotationMode` enum**

Replace `ast.rs`'s `Hashtag` struct with `Annotation`:

```rust
// 2026-07-07: Phase 1 — unified annotation system
// Replaces Hashtag { name, value: Option<String>, mandatory, speculative, fallback, scoped }
// with a single Annotation struct. Hash forms (#, #!, #?) desugar to this.
// Structured forms (<~, ~>) allow arbitrary expression values.
pub struct Annotation {
    pub name: String,
    pub value: Expr,
    pub mode: AnnotationMode,
    pub span: Option<Span>,
}

pub enum AnnotationMode {
    Advisory,
    Mandatory,
    Speculative,
}
```

**Commit**: `declutter(1a): define unified Annotation struct replacing Hashtag`

**Step 1.2 — Rewrite parser annotation methods**

Replace `parse_hashtag_modifiers()` with:

```rust
fn parse_annotations(&mut self) -> Result<Vec<Annotation>, SyntaxError> {
    // Parse all annotation forms in sequence:
    // 1. Hash forms: #name, #!name, #?name
    // 2. Postfix forms: <~ (k: v, ...)
    // Return combined Vec<Annotation>
}

fn parse_prefix_annotation(&mut self) -> Result<Option<Vec<Annotation>>, SyntaxError> {
    // Parse (k: v, ...) ~> before a guarded statement
    // Only valid before [condition] { body }
    // Consumes TildeArrowRight token.
}
```

Key changes:
- Scoped hashes `#[scope] { #inner }` → produce error with suggestion to use `<~ (scope.inner: true)`.
- Fallback chains `#!name|alt1|alt2` → produce error with suggestion to use multiple annotations.
- Values now parse as full `Expr`, not limited to string/int/ident.
- `parse_annotations()` is called from all the same places `parse_hashtag_modifiers()` was called.

**Pre-removal test**: Write tests for:
- `(cold: true, likely: false) ~> [x > 0] { body }` parsing
- `txn foo <~ (unroll: 4, section: "init") { ... }` parsing
- `#gpu`, `#!gpu`, `#?gpu` still produce equivalent annotations
- Old `#[scope] { #inner }` produces a helpful migration error

**Commit**: `declutter(1b): rewrite annotation parser — add <~ and ~> syntax, keep hash forms`

**Step 1.3 — Update all downstream consumers**

Replace `Vec<Hashtag>` with `Vec<Annotation>` in:

| Struct | Field |
|--------|-------|
| `Statement::Assignment` | `modifiers` → `annotations` |
| `Statement::Term` | `modifiers` → `annotations` |
| `Statement::TermBang` | `modifiers` → `annotations` |
| `Statement::Let` | `modifiers` → `annotations` |
| `Statement::Foreach` | `modifiers` → `annotations` |
| `Statement::TrgBinding` | `modifiers` → `annotations` |
| `Statement::Oracle` | `modifiers` → `annotations` |
| `Statement::Await` | `modifiers` → `annotations` |
| `Statement::Async` | `modifiers` → `annotations` |
| `Statement::AsyncAwait` | `modifiers` → `annotations` |
| `Transaction` | `modifiers` → `annotations` |
| `Definition` | `modifiers` → `annotations` |
| `StructField` | `modifiers` → `annotations` |
| `EnumVariant` | `modifiers` → `annotations` |
| `TopLevel::TriggerBinding` | `modifiers` → `annotations` |

Update consumers in:
- `src/backend/mod.rs` — hashtag validation → annotation validation
- `src/backend/llvm/mod.rs` — modifier dispatch → annotation dispatch
- `src/backend/webstack.rs` — modifier dispatch
- `src/backend/circt.rs` — modifier dispatch
- `src/proof_engine.rs` — any modifier-dependent logic
- `src/analysis/region.rs` — any modifier-dependent logic

Each consumer match arm changes from:
```rust
modifiers.iter().any(|h| h.name == "inline")
// to:
annotations.iter().any(|a| a.name == "inline" && a.mode != AnnotationMode::Speculative)
```

**Commit**: `declutter(1c): update all downstream annotation consumers`

**Step 1.4 — Remove old Hashtag struct**

After all consumers are updated, remove `Hashtag` from `ast.rs`. Verify zero references remain via `git grep Hashtag`.

**Commit**: `declutter(1d): remove old Hashtag struct`

### 6.4 Phase 1 Verification

```
cargo build             # 0 warnings
cargo test --lib        # all tests pass
```

Run any tests that exercise `#gpu`, `#inline`, `#volatile`, etc. to confirm hash forms still work.

---

## 7. Phase 2: Bits Thesis Type System

### 7.1 Problem Statement

Currently `Int`, `UInt`, `Float`, `Bool`, `String`, `Char` are 14 separate `Type` enum variants with no parametric relationship. The Bits thesis states: every type is a lens over `Bits<N>`. The existing `bit_width()` and `is_signed()` helpers already abstract toward this. The type universe (`TypeUniverse`) already drives codegen via string-based lookup.

### 7.2 Design

**New type variants (additive — old variants stay):**

```rust
pub enum Type {
    // ... all existing variants stay ...
    
    /// Raw bits with explicit width and interpretation lens.
    /// The canonical form that all numeric types normalize to.
    Bits {
        width: u64,
        interpretation: Interpretation,
    },
    
    /// Type-level width literal: `Int<8>` desugars to
    /// `Applied("Int", [Width(8)])` then resolves to
    /// `Bits { width: 8, interpretation: SignedInt }`.
    Width(u64),
}

pub enum Interpretation {
    SignedInt,
    UnsignedInt,
    IEEE754Float,
    Boolean,
    UnicodeScalar,
    RawData,
}
```

**Types that are NOT subsumed by Bits** (remain as concrete variants forever):

| Variant | Reason |
|---------|--------|
| `Type::Void` | No bits — represents absence of value |
| `Type::String` | Pointer + length struct, not scalar bits |
| `Type::Data` | Opaque byte blob, not a numeric type |
| `Type::Custom(String)` | User-named type, resolved to concrete via universe |
| `Type::Union(_)` | Sum type over multiple interpretations |
| `Type::Tuple(_)` | Product type over multiple values |
| `Type::Vector(_, _)` | SIMD vector type |
| `Type::Generic(_, _)` | Type parameter variable |
| `Type::Applied(_, _)` | Applied generic type (resolved to concrete or Bits) |
| `Type::Constrained(_, _)` | Bit-range constraint over an existing type (NOT subsumed by Width — see note below). `Int @/0..7` is still `Constrained(Int, Range(0,7))`, not `Bits { 7, SignedInt }`. The range constraint is a modifier on top of the base type. |
| `Type::Enum(_)` | Discriminated union tag |
| `Type::Sig(_)` | FFI signature reference |
| `Type::LayoutPtr(_)` | Spatial memory pointer |

**Clarification on `Type::Constrained` vs `Type::Bits`**: These serve different purposes and BOTH stay in the enum.
- `Type::Bits { 8, SignedInt }` = "this IS an 8-bit signed integer" (the type itself)
- `Type::Constrained(Type::Int, Range(0, 7))` = "this is an Int but only bits 0-7 are active" (a constraint ON a type)

`Type::Width(u64)` is ONLY used as a type-level argument inside `Applied("Int", [Width(8)])`, never in standalone type positions.

**Normalization helper:**

```rust
impl Type {
    /// Canonicalize to Bits form if this type represents bits.
    /// Returns None for non-Bits types (Void, String, Data, etc.).
    ///
    /// 2026-07-07: Phase 2 — Bits thesis canonicalization
    /// Every numeric type is a lens over Bits. This helper bridges old
    /// concrete numeric variants to the new canonical form during migration.
    /// Non-numeric types (Void, String, Data, Union, Tuple, Custom, etc.)
    /// are NOT bits and remain concrete variants forever.
    pub fn to_bits(&self) -> Option<BitsInfo> {
        match self {
            Type::Int8 => Some(BitsInfo { width: 8, interpretation: SignedInt }),
            Type::Int16 => Some(BitsInfo { width: 16, interpretation: SignedInt }),
            Type::Int32 => Some(BitsInfo { width: 32, interpretation: SignedInt }),
            // NOTE: There is no Type::Int64 variant. Type::Int is the default
            // 64-bit signed integer. Type::Int64 does not exist in the AST enum.
            Type::Int => Some(BitsInfo { width: 64, interpretation: SignedInt }),
            Type::UInt8 => Some(BitsInfo { width: 8, interpretation: UnsignedInt }),
            Type::UInt16 => Some(BitsInfo { width: 16, interpretation: UnsignedInt }),
            Type::UInt32 => Some(BitsInfo { width: 32, interpretation: UnsignedInt }),
            Type::UInt => Some(BitsInfo { width: 64, interpretation: UnsignedInt }),
            Type::Float => Some(BitsInfo { width: 32, interpretation: IEEE754Float }),
            Type::Float64 => Some(BitsInfo { width: 64, interpretation: IEEE754Float }),
            Type::Bool => Some(BitsInfo { width: 1, interpretation: Boolean }),
            Type::Char => Some(BitsInfo { width: 32, interpretation: UnicodeScalar }),
            Type::Bits { width, interpretation } => Some(BitsInfo { width: *width, interpretation: *interpretation }),
            _ => None,
        }
    }
}
```

### 7.3 Implementation Strategy

The migration follows an **additive-first, then incremental** strategy to prevent regressions:

#### Step 2.0 — Pre-removal tests

Write tests that verify:
- `let x: Int<8> = 42;` compiles and type-checks to the same type as `let x: i8 = 42;`
- `let x: Int<8> = 42i8;` is valid (width equality)
- `let x: Int<8> = 42u8;` is a type error (signedness mismatch)
- `let x: UInt<8> = 42u8;` is valid
- `let x: Float<32> = 1.0f32;` is valid
- `Bits<64>` is valid as a raw data type
- `type MyWord <: Int<8>;` type derivation works

These tests are committed BEFORE any code changes.

**Commit**: `declutter(2a): add pre-removal tests for Int<N> type syntax`

#### Step 2.1 — Add `Type::Bits` and `Type::Width` variants (additive)

Add the new variants to `Type` enum. No code uses them yet. Update `to_bits()` to handle them. Update `bit_width()`, `is_signed()`, `is_integral()`, `is_float_type()`, `universe_key()` to handle them.

**Commit**: `declutter(2b): add Type::Bits and Type::Width variants (additive)`

#### Step 2.2 — Accept `Int<N>` syntax in parser

Parser accepts:
- `Int<8>`, `Int<16>`, `Int<32>`, `Int<64>` → resolve to `Type::Bits { width, SignedInt }`
- `UInt<8>`, `UInt<16>`, `UInt<32>`, `UInt<64>` → `Type::Bits { width, UnsignedInt }`
- `Float<32>` → `Type::Bits { 32, IEEE754Float }`
- `Float<64>` → `Type::Bits { 64, IEEE754Float }`
- `Bits<N>` → `Type::Bits { N, RawData }`
- `Bool<1>` → `Type::Bits { 1, Boolean }` (warning: use bare `Bool`)

Short forms still work unchanged:
- `i8` → `Type::Int8` (old variant, still valid)
- `u32` → `Type::UInt32`
- `f64` → `Type::Float64`
- `Int` → `Type::Int`
- `Float` → `Type::Float`

**Commit**: `declutter(2c): parser accepts Int<N>, UInt<N>, Float<N> generic type syntax`

#### Step 2.3 — Normalize early in pipeline

In the type-universe pass or a new desugar pass, normalize ALL concrete type variants to their canonical `Bits` form after type-checking but before codegen. This makes downstream code see the canonical form.

```rust
// In type_universe.rs or desugarer.rs:
fn normalize_type(ty: &Type) -> Type {
    match ty.to_bits() {
        Some(bits) => Type::Bits { width: bits.width, interpretation: bits.interpretation },
        None => ty.clone(), // non-numeric types pass through
    }
}
```

This is a **compile-only change** — no effect on emitted IR because `to_bits()` is a bijection.

**Commit**: `declutter(2d): add Bits normalization pass in type universe`

#### Step 2.4 — Migrate typechecker

Convert typechecker match arms from:
```rust
match (lhs, rhs) {
    (Type::Int, Type::Int) => Ok(Type::Int),
    (Type::Float, Type::Float) => Ok(Type::Float),
    // 14 more cases ...
}
```
to:
```rust
match (lhs.to_bits(), rhs.to_bits()) {
    (Some(l), Some(r)) if l.width == r.width && l.interpretation == r.interpretation => {
        Ok(Type::Bits { width: l.width, interpretation: l.interpretation })
    }
    // ...
}
```

**Test**: `cargo test --lib` — all typechecker tests pass with identical semantics.

**Commit**: `declutter(2e): migrate typechecker to Bits canonical form`

#### Step 2.5 — Migrate LLVM backend type mapping

Convert `helpers.rs`, `mod.rs` type-to-LLVM functions from:
```rust
fn fallback_llvm_type(ty: &Type) -> &str {
    match ty {
        Type::Int8 | Type::UInt8 => "i8",
        Type::Int16 | Type::UInt16 => "i16",
        // ...
    }
}
```
to:
```rust
fn fallback_llvm_type(ty: &Type) -> &str {
    if let Some(bits) = ty.to_bits() {
        match bits.interpretation {
            SignedInt | UnsignedInt => {
                // LLVM integer types are signless — i8 covers both i8 and u8
                Cow::Owned(format!("i{}", bits.width))
            }
            IEEE754Float => match bits.width {
                32 => "float",
                64 => "double",
                _ => panic!("unsupported float width: {}", bits.width),
            },
            Boolean => "i1",
            UnicodeScalar => "i32",
            RawData => Cow::Owned(format!("i{}", bits.width)),
        }
    } else {
        // Non-numeric types: String, Data, Void, layout types
        match ty {
            Type::String => "{%State}*",  // (or whatever the current mapping is)
            // ...
        }
    }
}
```

**Critical**: The LLVM backend functions (`emit_expr.rs`, `emit_stmt.rs`, `emit_toplevel.rs`) that dispatch arithmetic operations must be migrated one at a time. Each function gets a comment documenting what it was changed and why.

**Test**: After EACH file migration, run `bash benchmarks/build_and_bench.sh --runtime` and compare against baseline.

**Commits** (one per LLVM backend file):
- `declutter(2f): migrate helpers.rs type mapping to Bits`
- `declutter(2g): migrate mod.rs type mapping to Bits`
- `declutter(2h): migrate expr/identifier.rs to Bits`
- `declutter(2i): migrate expr/rest.rs to Bits`
- `declutter(2j): migrate expr/intrinsics.rs to Bits`
- `declutter(2k): migrate expr/call.rs to Bits`
- `declutter(2l): migrate builder.rs to Bits`
- `declutter(2m): migrate emit_stmt.rs to Bits`
- `declutter(2n): migrate emit_toplevel.rs to Bits`
- `declutter(2o): migrate expr/math.rs to Bits`
- `declutter(2p): migrate expr/projection.rs to Bits`
- `declutter(2q): migrate other expr/ files to Bits`

#### Step 2.6 — Migrate interpreter

Convert `src/interpreter.rs` match arms (~79 references) from concrete variant dispatch to `to_bits()`-based dispatch. Same pattern as the typechecker migration.

**Commit**: `declutter(2r): migrate interpreter to Bits canonical form`

#### Step 2.7 — Migrate remaining consumers (LLVM-only)

Per executive direction, **only the LLVM backend is fully migrated**. The other backends get minimal compatibility stubs:

- `src/proof_engine.rs` (~13 references) — fully migrate
- `src/backend/webstack.rs` — add `_ => {}` fallthrough for `Type::Bits` in existing match arms. Does NOT get a full migration.
- `src/backend/circt.rs` — same as webstack: add `_ => {}` fallthrough.
- `src/analysis/*.rs` — any type-dependent analysis gets fully migrated.
- `src/interpreter.rs` — fully migrated in Step 2.6.

**Rationale**: LLVM is the only benchmarked backend. Migrating webstack/CIRCT would risk regressions without benchmark verification. They compile and work correctly for existing code; new code using `Int<8>` syntax will fall through to default behavior in those backends. A future plan will address full migration for all backends.

**Commit**: `declutter(2s): migrate proof_engine and analyses to Bits; stub webstack/circt`

#### Step 2.8 — Remove old concrete numeric variants

Once `git grep` confirms zero references to the following (and VERIFYING they are not referenced in non-LLVM backends that rely on them):

- `Type::Int8`, `Type::Int16`, `Type::Int32`
- `Type::UInt8`, `Type::UInt16`, `Type::UInt32`
- `Type::Float64`
- `Type::Char` (subsumed by `Bits { 32, UnicodeScalar }`)

**NOT removed** (non-Bits types, remain as concrete variants):
- `Type::Int`, `Type::UInt`, `Type::Float` — these are the DEFAULT-width variants.
  They MAPPED to Bits but are NOT removed because they're referenced as default
  fallback types throughout the codebase. After migration, they are defined as:
  `Type::Int` ≡ `Type::Bits { 64, SignedInt }`
  `Type::UInt` ≡ `Type::Bits { 64, UnsignedInt }`
  `Type::Float` ≡ `Type::Bits { 32, IEEE754Float }`
  They stay in the enum for ergonomics but all code should match through `to_bits()`.
- `Type::Void`, `Type::String`, `Type::Data`, `Type::Bool`
- `Type::Constrained`, `Type::LayoutPtr`, `Type::Custom`
- `Type::Union`, `Type::Tuple`, `Type::Vector`
- `Type::Generic`, `Type::Applied`
- `Type::Enum`, `Type::Sig`

Remove the long-form keyword tokens from `lexer.rs`:
- Remove: `TypeInt8`, `TypeInt16`, `TypeInt32`, `TypeInt64` (NOT `TypeInt` — default stays)
- Remove: `TypeUInt8`, `TypeUInt16`, `TypeUInt32`, `TypeUInt64` (NOT `TypeUInt` — default stays)
- Remove: `TypeFloat32`, `TypeFloat64`, `TypeF32`, `TypeF64`, `TypeDouble` (NOT `TypeFloat` — default stays)
- Remove: `TypeSigned`, `TypeSgn`, `TypeUnsigned`, `TypeUSgn`
- Remove: `TypeChar` (Char type keyword; `Char` literal stays via `'a'` syntax)

**Note on `Int64` / `UInt64`**: These lexer tokens exist but map to `Type::Int` / `Type::UInt` in the parser (lines 6595-6601 of parser.rs). Removing them means users must write `Int` or `Int<64>` instead of `Int64`. The `Int64` keyword was always an alias for `Int`.

**Pre-removal test**: All tests from Step 2.0 must pass using only the new syntax AND the remaining short forms (`i8`, `u32`, `f64`, `Int`, `UInt`, `Float`). No test should reference `Int64`, `UInt64`, `Float32`, `Double`, `Signed`, `Unsigned`, `Char` as type keywords.

**Commit**: `declutter(2t): remove concrete type variants — Bits canonical form is now the only form`

### 7.4 Phase 2 Verification

After EACH step in the migration:
```
cargo build              # 0 warnings
cargo test --lib         # all tests pass
```

After steps 2f-2q (LLVM backend migration steps):
```
bash benchmarks/build_and_bench.sh --runtime   # compare against baseline
```

Any benchmark regression below 0.97x of baseline blocks further migration of that file.

After step 2t (final removal):
```
bash benchmarks/build_and_bench.sh --runtime   # full baseline comparison
bash benchmarks/build_and_bench.sh --correctness  # all output checks pass
```

---

### 7.5 Phase 2 ↔ Phase 3 Dependency Note

Phase 3 (intrinsic reduction) creates `std/os/*.bv` files with `inop` declarations using types like `Int`, `Ptr<Byte>`, `Bool`, etc. These types are available in BOTH the old and new type systems:

- **Before Phase 2**: `Int` is `Type::Int` (concrete variant), `Ptr<Byte>` is `Type::Applied("Ptr", [Type::Byte])`
- **After Phase 2**: `Int` is `Type::Bits { 64, SignedInt }` (via `to_bits()`), `Ptr<Byte>` still resolves the same way

The `.bv` files use source-level type syntax which is unaffected by the internal representation change. Therefore **Phase 2 and Phase 3 are independent** and can be done in any order. However, the recommended order is Phase 2 first (since the Bits migration touches more compiler internals and benefits from early validation), then Phase 3 (which relies on stable compiler behavior to verify inop replacements).

If Phase 2 is delayed, Phase 3 can proceed using the current type system — the inop signatures don't change.

---

## 8. Phase 3: Intrinsic Reduction + Prelude

### 8.1 Problem Statement

The `Intrinsic` enum has ~120 variants. Many are thin wrappers over POSIX syscalls (socket, bind, listen, pthread_create, mmap, etc.) that should be `inop` declarations in standard library modules, not compiler intrinsics.

### 8.2 What Stays (~40 variants)

These are genuinely compiler-known and cannot be expressed as inops:

**Math:**
`Sqrt`, `Fabs`, `Ceil`, `Floor`, `Ctpop`, `Ctlz`, `Cttz`, `Abs`, `Bitreverse`, `Sin`, `Cos`, `Pow`

**Collection:**
`Size`, `Pop`, `Contains`, `Keys`, `Values`, `ByteCount`, `Sort`, `Reverse`, `Range`

**String:**
`TrimLeft`, `TrimRight`, `ToLower`, `ContainsAt`, `FindFrom`, `SplitN`, `StrBytes`
`IntToStr`, `FloatToStr`, `ToStr`, `Strlen`

**Memory:**
`Memcpy`, `Memcmp`, `Memset`, `Hash`

**Compile-time:**
`Compile`, `MacroError`, `MacroWarn`, `MacroGenSym`, `EmitFile`

**GPU:**
`GetGlobalId`, `GetLocalId`, `GetGroupId`, `GetNumGroups`, `SubGroupBarrier`

**MMIO:**
`VolatileLoad`, `VolatileStore`

**Core I/O (used by internal machinery):**
`PrintInt`, `PutChar`, `PrintFloat`, `GetEnvInt`

### 8.3 What Moves to std/os/

Each group becomes a `.bv` file with `inop` declarations. Example:

```brief
// std/os/fs.bv — auto-imported via prelude
// 2026-07-07: Phase 3 — relocated from compiler intrinsic

inop open#(path: Ptr<Byte>, flags: Int, mode: Int) -> Int;
inop close#(fd: Int) -> Int;
inop read#(fd: Int, buf: Ptr<Byte>, count: Int) -> Int;
inop write#(fd: Int, buf: Ptr<Byte>, count: Int) -> Int;
inop lseek#(fd: Int, offset: Int, whence: Int) -> Int;
inop pread#(fd: Int, buf: Ptr<Byte>, count: Int, offset: Int) -> Int;
inop pwrite#(fd: Int, buf: Ptr<Byte>, count: Int, offset: Int) -> Int;
inop stat#(path: Ptr<Byte>, buf: Ptr<Byte>) -> Int;
inop fstat#(fd: Int, buf: Ptr<Byte>) -> Int;
inop ftruncate#(fd: Int, length: Int) -> Int;
inop fsync#(fd: Int) -> Int;
inop dup#(fd: Int) -> Int;
inop dup2#(oldfd: Int, newfd: Int) -> Int;
inop fcntl#(fd: Int, cmd: Int, arg: Int) -> Int;
```

Each `.bv` file corresponds to a category from the plan:

| File | Contents |
|------|----------|
| `std/os/net.bv` | socket, bind, listen, accept, connect, send, recv, sendto, recvfrom, setsockopt, getsockopt, shutdown, getaddrinfo |
| `std/os/signal.bv` | sigaction, sigprocmask, kill, signal_fd, timerfd_create |
| `std/os/ipc.bv` | pipe, shm_open, shm_unlink, sem_open, sem_wait, sem_post |
| `std/os/thread.bv` | thread_create, thread_join, thread_exit, mutex_lock, mutex_unlock, condvar_wait, condvar_signal, condvar_broadcast |
| `std/os/fs.bv` | open, close, read, write, lseek, pread, pwrite, stat, fstat, ftruncate, fsync, dup, dup2, fcntl |
| `std/os/dir.bv` | mkdir, rmdir, unlink, rename, symlink, readlink, link, getcwd, chdir, readdir, chmod, chown, umask, access |
| `std/os/process.bv` | spawn, spawn_with_output, getpid, getppid, argv, exit, abort, sleep |
| `std/os/tty.bv` | tty_raw_mode, tty_size, tty_read_key, ioctl, isatty, ttyname |
| `std/os/user.bv` | getuid, geteuid, getgid, getegid, getpwuid, getgrgid |
| `std/os/time.bv` | clock_gettime, nanosleep, time |
| `std/os/mem.bv` | mmap, munmap, mprotect, brk, mlock |
| `std/os/rand.bv` | getrandom, errno |
| `std/os/sched.bv` | sched_yield, getpriority, setpriority |
| `std/os/resource.bv` | getrlimit, setrlimit |
| `std/os/sysinfo.bv` | uname, hostname, realpath, pagesize, cpu_count, strerror, strsignal |
| `std/os/temp.bv` | mkstemp, mkdtemp |
| `std/os/dynlib.bv` | dlopen, dlsym, dlclose |
| `std/os/debug.bv` | backtrace, halt, abort |
| `std/os/ring.bv` | ring_push, ring_pop |
| `std/os/atomic.bv` | atomic_load, atomic_store, atomic_cas, atomic_xchg, atomic_add, fence, futex |
| `std/os/io.bv` | print, println, readln, get_env, set_env, unset_env, set_stdout_buf |

### 8.4 Prelude Mechanism

Modify `import_resolver.rs` to inject prelude imports after the existing bootstrap type import:

```rust
// In resolve_imports(), after the bootstrap types injection:
// 2026-07-07: Phase 3 — auto-import OS module prelude
// These inop declarations replace the old Intrinsic variants.
// Use --no-std to disable.
if !no_stdlib && !prelude_injected {
    let prelude_modules = vec![
        "std/os/io.bv",
        "std/os/fs.bv",
        "std/os/dir.bv",
        "std/os/mem.bv",
        "std/os/process.bv",
        "std/os/time.bv",
        "std/os/rand.bv",
        "std/os/net.bv",
        "std/os/signal.bv",
        "std/os/ipc.bv",
        "std/os/thread.bv",
        "std/os/tty.bv",
        "std/os/user.bv",
        "std/os/sched.bv",
        "std/os/resource.bv",
        "std/os/sysinfo.bv",
        "std/os/temp.bv",
        "std/os/dynlib.bv",
        "std/os/debug.bv",
        "std/os/ring.bv",
        "std/os/atomic.bv",
    ];
    for module in prelude_modules {
        items.push(TopLevel::Import(Import {
            path: module.to_string(),
            is_magic: true,
            alias: None,
            span: None,
        }));
    }
    prelude_injected = true;
}
```

The `is_magic: true` flag means paths resolve relative to `BRIEF_STDLIB_PATH`, same as existing `import#` behavior.

**Duplicate import protection**: Before injecting a prelude import, check if the user has already explicitly imported the same module. If `items` already contains `TopLevel::Import { path: "std/os/fs.bv", ... }`, skip injecting it. This prevents "duplicate import" errors when a user explicitly writes `import# "std/os/fs.bv"` and the prelude also provides it.

**Note on `inop!`**: The bang variant is unchanged by this plan. `inop!` stays as the side-effect indicator. Only the intrinsic-turned-inops get the `inop` keyword (side-effect-free `inop` declarations in `std/os/*.bv`). `inop!` for user-defined side-effecting operations is untouched.

### 8.5 Implementation

**Step 3.1 — Create std/os/ module files**

Create all 18+ `.bv` files under `lib/std/os/`. Each file declares `inop` with the `#` call syntax.

**Commit**: `declutter(3a): create std/os/ module with inop declarations`

**Step 3.2 — Add prelude auto-import + pre-removal tests**

Modify `import_resolver.rs` to inject the prelude imports. Add `--no-std` handling.

**Pre-removal tests** (written and committed NOW, before intrinsic removal):
Write tests that:
- Call each `name#(args)` using the inop declarations now available through the prelude
- Verify the result matches what the old intrinsic produced
- Test error cases (bad file descriptors, invalid addresses, etc.)
- Specifically: verify `open#("test.txt", 0, 0)` resolves through the prelude and works identically to the old `Intrinsic::Open`

These tests are committed AFTER the prelude exists (so they compile) but BEFORE any intrinsic variant is removed (so they verify the replacement works).

**Commit**: `declutter(3b): add auto-import prelude for std/os/ modules + pre-removal tests`

**Step 3.3 — Remove relocated Intrinsic variants (one group at a time)**

Remove intrinsic variants in groups, committing after each group:

1. Networking (`Socket`..`GetAddrInfo`)
2. Signals (`SigAction`..`TimerFdCreate`)
3. IPC (`Pipe`..`SemPost`)
4. File I/O (`Open`..`FCntl`)
5. Directory (`MkDir`..`Access`)
6. Process (`SpawnWithOutput`, `Spawn`, `GetPid`, `GetPPid`)
7. TTY (`TtyRawMode`, `TtySize`, `TtyReadKey`, `IoCtl`, `IsTty`, `TtyName`)
8. User/Group (`GetUid`..`GetGrGid`)
9. Time (`ClockGetTime`, `NanoSleep`, `Time`)
10. Memory (`Mmap`..`MLock`)
11. Scheduling (`SchedYield`..`SetPriority`)
12. Resources (`GetRlimit`, `SetRlimit`)
13. System info (`Uname`..`RealPath`)
14. Debug (`Abort`, `Backtrace`, `Halt`)
15. Temp files (`MkStemp`, `MkDtemp`)
16. Dynamic linking (`DlOpen`..`DlClose`)
17. Ring buffer (`RingPush`, `RingPop`)
18. Atomics (`AtomicLoad`..`Futex`)
19. Threading (`ThreadCreate`..`CondvarBroadcast`)
20. Core I/O (`Print`, `Println`, `Readln`, `GetEnv`..`SetEnv`, `SetStdoutBuf`)
21. String ops (`TrimLeft`, `TrimRight`, `ToLower`, `ContainsAt`, `FindFrom`, `SplitN`, `StrBytes`)
22. GPU (`GetGlobalId`..`SubGroupBarrier`) — these are NOT moved; they stay as compiler intrinsics
23. `PrintInt`, `PutChar`, `PrintFloat`, `GetEnvInt` — these stay as intrinsics

After EACH group removal:
- `cargo build` — no warnings
- `cargo test --lib` — all tests pass
- Run the pre-removal tests from Step 3.0

**Commits**: `declutter(3d-3z): remove <category> intrinsic group — replaced by std/os/<module>.bv`

**Step 3.4 — Clean up Intrinsic enum helpers**

After all removals, simplify:
- `has_side_effects()` — only matches remaining ~40 + `UserDefined`
- `Intrinsic::from_str_name()` — only matches remaining ~40 names
- Any intrinsic dispatch in backends only references remaining variants

Update `docs/architecture/intrinsics.md` to list only the remaining compiler intrinsics.

**Commit**: `declutter(3aa): clean up Intrinsic helpers after removal`

### 8.6 Phase 3 Verification

```
cargo build              # 0 warnings
cargo test --lib         # all tests pass (including new pre-removal tests)
bash benchmarks/build_and_bench.sh --runtime   # compare against baseline
bash benchmarks/build_and_bench.sh --correctness  # all output checks pass
```

The correctness check is critical — it verifies that the inop declarations produce identical behavior to the old intrinsics for every benchmark.

---

## 9. Phase 4: Documentation Overhaul

### 9.1 Files to Update

| File | What changes |
|------|--------------|
| `docs/learn/types.md` | Document `Bits` as fundamental type, `Int<N>` syntax, short forms. Remove mention of old concrete variants. |
| `docs/learn/ffi.md` | Document prelude auto-import and `std/os/` module availability. Remove mention of removed intrinsics. |
| `docs/learn/macros.md` | No changes expected. |
| `docs/reference/BRIEF_LANGUAGE_REFERENCE.md` | Major revision: annotation system, type system, intrinsic list, keyword table, prelude docs. |
| `spec/SPEC.md` | Update language spec to reflect all changes: type system, annotations, intrinsic relocation. |
| `docs/architecture/intrinsics.md` | Remove ~60 relocated intrinsic entries. List only remaining ~40. |
| `docs/architecture/bits-thesis.md` | Update to reflect completed migration from concrete variants to canonical Bits form. |
| `docs/architecture/features/is-from-like.md` | May need type normalization explanation. |
| `docs/architecture/optimization-pipeline.md` | Add Bits normalization pass. |

### 9.2 Documentation Standards

1. Every changed syntax must have at least one runnable example.
2. Error messages must be documented if they're new (e.g., `#[scope]` removal migration error).
3. The prelude must be documented with a complete list of auto-imported modules.
4. The annotation system must have a section covering all three surface syntaxes with examples for each.

**Commit**: `declutter(4): update all documentation for language changes`

---

## 10. Phase 5: Error/Warning Improvements

**Precondition for Phase 5**: Both Phase 0 (cleanup), Phase 1 (annotations), Phase 2 (Bits thesis), and Phase 3 (intrinsics) must be complete. Phase 5 adds warnings and errors for syntax and patterns that were changed or removed in earlier phases — it cannot be implemented until those changes are in place.

### 10.1 Missing Contract Warnings

If a `txn`, `defn`, or `inop` omits `[pre]` or `[post]` and the contract defaults to `[true][true]`, emit a single grouped warning listing ALL sites:

```
warning: 3 declarations with trivial contracts:
  - src/main.bv:12:1 — txn `process`
  - src/main.bv:45:1 — defn `compute`
  - src/lib/helper.bv:8:1 — inop `load`
  hint: add [pre] and [post] contracts to enable convergence proofs
```

Implementation: collect `ContractWarning` instances during type-checking, then emit them as a single grouped diagnostic at the end.

### 10.2 Deprecated Syntax Warnings

When the parser encounters:
- `""` bare string annotation values in hashes → warn to use `(name: "value")` instead
- `#[scope] { #name }` → error with suggestion:
  ```
  error: scoped hashes (#[scope] { #name }) are removed.
    --> src/main.bv:8:1
     |
  8  | #[llvm] #inline
     | ^^^^^^^
  help: use <~ (llvm.inline: true) instead
  ```
- Old long-form type keywords (`Int8`, `UInt32`, etc.) → warn:
  ```
  warning: 'Int8' is deprecated, use 'i8' or 'Int<8>'
    --> src/main.bv:3:8
     |
  3  | let x: Int8 = 0;
     |        ^^^^
  help: use 'i8' or 'Int<8>' instead
  ```

### 10.3 Helpful Parse/Resolution Errors

When the parser or resolver encounters constructs that were removed:
- **Alka**: Error: `"alka blocks have been removed (2026-07-07)"`
- **OnExit**: Error: `"on_exit blocks have been removed (2026-07-07)"`
- **Old intrinsic `name#()` calls**: When a user writes `open#(...)` and `open` is no longer a compiler intrinsic, the name resolution will fail because the prelude inop replaces it. The error message should detect this case:
  ```
  error: unresolved intrinsic: 'open#()'
    --> src/main.bv:5:3
     |
  5  | let fd = open#("file.txt", 0, 0);
     |          ^^^^^^
  help: 'open' was moved from compiler intrinsics to std/os/fs.bv (auto-imported via prelude).
        If you used --no-std, add: import# "std/os/fs.bv";
  ```
  **Detection mechanism**: In the intrinsic resolution code (`Intrinsic::from_str_name()` or the inop lookup), maintain a `REMOVED_INTRINSICS: HashMap<&str, &str>` mapping old intrinsic names to their replacement module. When a user tries to call an intrinsic that no longer exists, check this map and emit the helpful error instead of a generic "unknown intrinsic" message.

### 10.4 Implementation

**Commit**: `declutter(5a): add grouped contract warning for trivial [true][true] contracts`
**Commit**: `declutter(5b): add deprecation warnings for old type keywords`
**Commit**: `declutter(5c): add helpful parse errors for removed constructs`

---

## 11. Validation Strategy

### 11.1 Per-Commit Gate

Before every commit:

```
cargo build                        # 0 warnings
cargo test --lib                   # all tests pass
```

### 11.2 Per-Phase Gate

After every phase that affects codegen:

```
bash benchmarks/build_and_bench.sh --runtime        # compare against baseline
bash benchmarks/build_and_bench.sh --correctness    # all output checks pass
```

### 11.3 Per-Removal Gate

Before removing any Intrinsic variant or Type variant:

1. Write the pre-removal test (committed)
2. Verify the test passes with the old code
3. Make the change
4. Verify the test still passes (confirms replacement works)
5. Commit the removal

### 11.4 Final Gate

After all phases complete:

```
cargo build                        # 0 warnings
cargo test --lib                   # all tests pass
bash benchmarks/build_and_bench.sh --runtime        # full baseline comparison
bash benchmarks/build_and_bench.sh --correctness    # all output checks
bash benchmarks/build_and_bench.sh --optimizer      # optimizer benchmarks
```

Final benchmark table must show no regression below 0.97x for any runtime benchmark.

---

## 12. Rollback Plan

### 12.1 Per-Step Rollback

If a step causes a benchmark regression:
```
git revert <commit>
```

This reverts the single commit. The step can be re-attempted with a different approach.

### 12.2 Per-Phase Rollback

If an entire phase causes systematic issues:
```
git revert <phase-start-commit>..HEAD
```

This reverts all commits in the phase while preserving earlier phases.

### 12.3 Full Rollback

```
git checkout main
git branch -D feat/language-decluttering
```

Discards the entire branch. No data loss — all commits exist in the git reflog.

### 12.4 Branch Protection

The branch `feat/language-decluttering` must never be force-pushed or rebased onto main until all phases are verified. This prevents accidental loss of the working baseline.

---

## Appendix A: Flat Control Flow Checklist

Before committing any modified or new function, verify:

- [ ] No `if` deeper than 2 levels
- [ ] No `match inside match` — extract inner logic to helper
- [ ] No `for inside if inside match` — extract or flatten
- [ ] Guard clauses preferred over `else if`
- [ ] `?` operator used for early returns where applicable
- [ ] `let val = opt else { return ... }` pattern for Option unwrapping
- [ ] Function body fits in ~20-40 lines (extract helpers if longer)
- [ ] Each helper has one clear responsibility

Exception: LLVM IR emission functions may have arbitrary nesting if required for efficient codegen.

## Appendix B: Architectural Comment Template

Every file modification must include or update comments following this template:

```
// 2026-07-07: Phase <N> — <short description>
// <What changed and why. What problem it solves.>
// <What pattern it targets. What would break if this code were removed.>
```

Example:
```
// 2026-07-07: Phase 2f — migrate type mapping to Bits canonical form
// Replaced direct match on Type::Int8 etc. with to_bits() canonicalization.
// This is part of the Bits thesis migration: every type is a lens over Bits<N>.
// The LLVM type string is determined by Bits.width + Bits.interpretation,
// not by the enum variant name. If this mapping is removed, all integer
// types will fall back to i64 and all float types to double.
```

## Appendix C: Files Changed Summary

| Phase | Files | Est. Changes |
|-------|-------|-------------|
| 0a — Drop UPPERCASE | 1 (lexer.rs) | ~100 deletions |
| 0b — Keyword aliases | 1 (lexer.rs) | ~40 deletions |
| 0c — Remove Alka/OnExit | ~20 active files | ~81 match arms removed |
| 0d — Fix warnings | ~25 files | Mostly cfg attribute changes |
| 1a-1d — Annotation system | ~20 files | New struct + rename field on 15+ structs |
| 2a-2t — Bits thesis | ~40+ files | Massive — see section 7 |
| 3a-3aa — Intrinsic reduction | ~30+ files | 80 variants removed, 18 new .bv files |
| 4 — Documentation | ~10 files | Content updates |
| 5 — Error/warnings | ~5 files | New diagnostics |

**Total estimated touch points: ~100 unique files**

---

*End of plan. Ready for review.*
