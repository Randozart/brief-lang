# Plan: Wire Full LlvmBackend into `build` + Fix Benchmark Script

## The Problem

The Phase 7 rework (`595a7a1`) simplified `src/main.rs` to a clean 89-line CLI
dispatcher. However, `src/compile.rs` was left with a **stub codegen** that emits
`ret 0` for every function — it never calls the real `LlvmBackend`. Separately,
the benchmark script (`benchmarks/build_and_bench.sh`) passes old-style flags
(`--llvm`, `--out`, `--optimize-budget`) that the new CLI doesn't parse,
producing `cannot read '--llvm': No such file or directory`.

## The Fix (Two Changes)

### Change A — CLI flags for `build` subcommand

Add flag parsing to `run_build` in `src/main.rs`. Introduce a `BuildOptions`
struct that carries the parsed flags down to `compile::compile_source`.

| Flag | Type | Default | Purpose |
|------|------|---------|---------|
| `--llvm` | bool | false | Emit `.ll` only; skip binary compilation |
| `--out <dir>` | `Option<String>` | Same dir as input | Output directory for `.ll` and binary |
| `--optimize-budget <N>` | u64 | 256 | Max simulation steps before runtime fallback |
| `--gpu-offload` | bool | false | Enable GPU offload codegen |

The parser:
- Iterates args with a `while i < args.len()` loop (flat, max 2 deep)
- `--llvm` / `--gpu-offload` set a bool, advance by 1
- `--out` / `--optimize-budget` consume the next arg, advance by 2
- Unknown flags return `Err("unknown flag: ...")`
- Positional args: the **first** non-flag arg is the file path; subsequent
  positional args are rejected.
- The `BuildOptions` struct is defined in `src/compile.rs` (where it's consumed).

The `--llvm` flag is kept for backward compatibility:
- The benchmark script passes `--llvm` currently; we will **remove** it (see
  Change C), but users who manually run `build --llvm foo.bv` should get IR only.

### Change B — Wire real LlvmBackend into `compile.rs`

**B1. Return `TypeUniverse` from `parse_and_check`.**

The backend needs the `TypeUniverse` (for TBAA metadata, type resolution).
Currently `check_types` creates one internally and discards it. Change the
signature:

```rust
fn parse_and_check(file_path: &str, source: &str)
    -> Result<(Vec<TopLevel>, TypeUniverse), String>
```

Then `check_types` takes `&TypeUniverse` by reference (no longer creates one).

**B2. Replace stub `codegen()` with `LlvmBackend::generate()`.**

In `compile_source`:

1. Call `parse_and_check` → get `(items, universe)`
2. Construct `LlvmBackend`:
   ```rust
   let mut backend = LlvmBackend::new()
       .with_optimize_budget(opts.optimize_budget)
       .with_type_universe(universe);
   if opts.gpu_offload {
       backend = backend.with_gpu_offload(true);
   }
   ```
3. Call `backend.generate(&items, None)` → LLVM IR string
4. Determine output path via new helper `determine_out_path()`:
   - Base name from `Path::new(file_path).file_stem()`
   - If `--out <dir>` given, use `<dir>/<base>.ll`; else same dir as input
5. Write `.ll` file
6. If NOT `--llvm`, compile to binary via `compile_to_binary()`:
   - Invoke `clang -O3 -march=native -ffast-math <ll_path> -o <binary_path> -lm`
   - On failure, return a clear error (not a silent fallback)
   - Keep the `.ll` file (useful for debugging)

**B3. Remove stub `codegen()` and `codegen_definition()`.**

These functions are replaced entirely. Remove them.

### Change C — Update benchmark script

In `benchmarks/build_and_bench.sh` line 150-151, change:

```bash
BOUND=50000000 ./target/release/briv-compiler build --llvm "benchmarks/${name}.bv" \
    --out benchmarks --optimize-budget "$budget" $gpu_flag 2>&1
```

To:

```bash
BOUND=50000000 ./target/release/briv-compiler build "benchmarks/${name}.bv" \
    --out benchmarks --optimize-budget "$budget" $gpu_flag 2>&1
```

Just remove `--llvm`. The compiler will now produce the binary directly.
The linking fallback in the script (lines 153-158) stays as a safety net
for users who run `--llvm` manually.

## Coding Standards to Follow

Every change must follow AGENTS.md:

1. **Max 2 nesting depth** — use guard clauses, `?`, `let ... else { return }`.
2. **Doc comments on every definition** — every pub fn, struct field.
3. **Rationale comments** — `// 2026-07-14: <why this exists>` at each change site.
4. **Flat control flow** — no `else if` chains beyond 1 level; use early returns.
5. **No todo!() / unreachable!()** — every path handled.

## Files Changed

| File | What |
|------|------|
| `src/compile.rs` | Return TypeUniverse, wire LlvmBackend, add `BuildOptions`, `determine_out_path()`, `compile_to_binary()`; remove stub codegen |
| `src/main.rs` | Add flag parsing in `run_build()`, import `BuildOptions`, pass to `compile_source()` |
| `benchmarks/build_and_bench.sh` | Remove `--llvm` from compiler invocation |
| `docs/plans/2026-07-14-wire-backend-cli-shim.md` | This file |

## Verification

1. `cargo build` — no warnings
2. `cargo test --lib` — all tests pass
3. Manual smoke test: `echo 'defn main() -> Int { term 42; };' > /tmp/test.bv &&
   ./target/release/briv-compiler build /tmp/test.bv` should produce `test`
   binary in `/tmp` that exits with 42 (or whatever the backend generates for
   a trivial program)

## Risk Analysis

| Risk | Mitigation |
|------|-----------|
| `LlvmBackend::generate()` panics on unfamiliar AST | The backend handles all TopLevel variants; existing tests exercise it. If a panic occurs, the error propagates up via `main.rs`'s `Err` handling. |
| `clang` not on PATH | `compile_to_binary` returns a clear error message directing user to install clang or use `--llvm`. |
| Backend produces wrong code for some construct | The benchmark script's `check_correctness` function catches output mismatches vs C reference. |
| Optimization budget not respected | The llama backend reads `self.ctx.optimize_budget` in `generate()` at line 1368. The `with_optimize_budget()` builder sets it. |
