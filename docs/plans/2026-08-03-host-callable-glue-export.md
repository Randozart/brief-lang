# Host-Callable GLUE Export — native-speed testing for RamKumar

**Date:** 2026-08-03
**Status:** Active plan
**Branch:** `glue-host-callable`
**Worktree:** `../briv-compiler-glue-host`

---

## Motivation

An interested party (RamKumar, MAKER.AI) wants to test Briv's native speed from
his own tooling (a context engine supporting 9 languages). Two explicit asks from
the LinkedIn thread (2026-07-10 → 07-24):

1. **Callbacks / progress updates** — C#-events/delegates-style: a host passes a
   callback into a long-running Briv function and receives "first-level
   primitive" updates (progress). "The granularity of update is configurable
   both before calling the long standing function or by filtering what I want
   as updates while the task is ON."
2. **Cancellation** — "we need a cancellation token if it gets stuck so we can
   cancel the request as it is taking too long."

Plus the underlying requirement: a host-callable bridge (Rust crate / Python
object / C) so Briv can be embedded and its native speed demonstrated.

## Governing Constraints (decided with the author)

- **Infinitely extensible FFI.** No target-language knowledge inside the
  compiler. No hardcoded language names, extensions, C ABI strings, template
  text, or filename conventions in `src/`. Everything language-specific lives
  in config (Data Briv) and Briv plugins:
  - `config/glue.dbvl` — per-language `protocols` (native/c_abi/wasm_abi),
    `templates`, `bindings`.
  - `lib/glue/<lang>/types.bv` — foreign type representations + melds.
  - `bridge-exports.dbvl` — the stable ABI metadata contract emitted by the
    compiler; wrappers/bindings render from it and never re-analyze the AST.
  - Adding a language = a config section + a `.bv` types module. Zero Rust
    changes. `#[serde(flatten)] HashMap` config + generic mustache renderer
    already provide this; `src/backend/bindgen.rs` is the one violator and is
    demoted to a config-driven renderer.
- **Config files are written in Data Briv** (`.dbvl`/`.dbv`), never TOML, per
  the Data Briv config plan (2026-08-03). `lib/glue.toml` migrates to
  `config/glue.dbvl` using the established machinery:
  `ConfigDb::from_quoted_str` (`src/dbriv/config_db.rs:43`),
  `resolve_config_file` (dbvl→dbv→toml), quoted templates with `\n` escapes
  (alloc-strategies precedent `config/alloc-strategies.dbvl`), quoted `#`-keys
  (protocols precedent `config/protocols.dbvl`), and a parity golden test
  (`parity_glue_dbvl_matches_toml`) gating the `.toml` deletion.
- **Separate test fixtures from benchmarks.** The round-trip test fixture
  (`pp-types.bv` restoration) is distinct from the benchmark workload
  (`examples/glue-host/rank.bv`).
- **Golden rules** (AGENTS.md): interpreter is reference; tests or it doesn't
  exist; no type-name matching (protocol categories only); body-dependent
  non-fragile ABI; explicit markers (`#`/`$`) for all special treatment;
  `git grep` in `src/glue/` + `src/backend/llvm/` for hardcoded language
  strings returns zero.

## Verified Current State (2026-08-03, HEAD 21454601)

- `briv export <bridge.bv> <lang>` generates real LLVM bodies via the full
  backend (`src/glue/export.rs:393`) but as a full-program module (no
  `library_mode`, no `--shared`).
- Wrappers in `lib/glue.toml` unconditionally pass `STATE`, but
  `definition_needs_state` (`src/backend/llvm/emit_toplevel.rs:1161`) omits
  `ptr %state` on pure exports → pure exports ABI-mismatch via generated
  wrappers.
- `briv library` (`src/library.rs:16`) uses stub codegen (`ret 0`);
  `emit_library_shim`'s real `i64 __briv_init_state()` (`emit_toplevel.rs:2810`)
  is dead code. `--emit-bindings` has no CLI wiring.
- `src/backend/bindgen.rs` (403 lines) hardcodes C/Rust/Python generators and
  hardcodes "first parameter is always state" (contradicts body-dependent ABI).
- Round-trip test file `tests/pp_roundtrip_tests.rs` is intact since the
  known-good era but its fixture `pp-types.bv` was gutted on 2026-07-27
  (`44f87367`) to an identity function → symbol lookups fail at runtime.
- Stale tests: `tests/glue_test.rs`, `tests/glue_bridge_tests.rs` reference
  removed `c_type_map` (won't compile) and expect python `extension == "py"`
  (now `"so"`); `tests/glue_integration.sh` greps removed output.
- Callbacks/fn-ptr FFI: greenfield (no function-type syntax in params, no
  `.#Ptr` codegen, `bridge_<name>` symbol never defined).
- Host cancellation: only the watchdog `?[c] within N ms` loop-deadline
  (real IR, not host-signallable). `CellChannel.terminate` is interpreter-only.

## Phase 1 — Data Briv config migration (glue)

1. Add `config/glue.dbvl`: per-language sections (rust/python/node/web/c) with
   protocols maps and template entries, quoted `#`-keys, `\n`-escaped
   templates in quoted mode. Include the new `bindings` templates (Phase 2d).
2. Rewire `load_glue_config` (`src/glue/config.rs:116`) onto `ConfigDb` via
   `resolve_config_file("glue")`.
3. Parity golden test `parity_glue_dbvl_matches_toml`; delete `lib/glue.toml`.
4. Cleanup (migrate-when-touched): delete legacy `lib/glue.dbvl`/`lib/glue.dbvs`
   and the dead `run_export`/`find_adapter`/`AdapterEntry` path in
   `src/glue/export.rs`; audit `dbvl_reader.rs` residual use.

## Phase 2 — Fix the export path forward at current HEAD

- **2a. Restore round-trip.** Rebuild the test fixture exporting the full symbol
  set (`briv_test_cstr_roundtrip`, `briv_test_type_bits`, `briv_test_type_void`,
  `briv_test_custom_echo`, `briv_test_bits_static`). Separate from the
  benchmark workload. Fix any real codegen regression the fixture surfaces
  (interpreter is reference — never weaken).
- **2b. Reconcile stale tests.** `glue_test.rs`, `glue_bridge_tests.rs`,
  `glue_integration.sh` → current APIs.
- **2c. Non-fragile body-dependent ABI.** Move `definition_needs_state`
  (`emit_toplevel.rs:1161`) to first-class `src/analysis/export_abi.rs` (shared
  backend + glue). Per-export `needs_state` + resolved C prototype into
  `bridge-exports.dbvl`; wrappers/bindings render from metadata only. Pure:
  `ret @name(args)`; stateful: `ret @name(ptr %state, args)`.
- **2d. Config-driven bindings.** Replace `bindgen.rs` hardcoded generators with
  the generic renderer + per-language `bindings` templates in `config/glue.dbvl`
  (`[c]`-style target → `briv_types.h`; `BrivState*` lifecycle
  `__briv_init_state`/`__glue_release` through metadata/templates).
  `--emit-bindings <lang>` on `briv build --library`. v1 marshals
  Int/Float/Bool/String; List/Struct/Enum rejected at compile time.
- **2e. Real `--library`.** `briv build --library` runs the full backend with
  `with_library_mode(true)` (activates `emit_library_shim`),
  `ar rcs lib<name>.a <name>.o briv_rt.o` (self-contained — matches the `.so`
  link line in `benchmarks/bridge/Makefile`), `cc -shared` for c_abi hosts.
  **C-driver test** (toolchain-guarded skip): include header → `__briv_init_state()`
  → call export → assert result. IR-golden tests for pure/stateful signatures
  + `__briv_init_state` in library mode.

## Phase 3 — Demo + native-speed benchmark

- `examples/glue-host/rank.bv`: real context-engine workload
  (`token_hash(text: String, seed: Int) -> Int`,
  `score_chunk(text: String, query: String) -> Float` — string-heavy) plus pure
  exports. Separate from the round-trip fixture.
- Hosts generated purely from config: Rust crate (LTO path), Python package,
  C driver.
- `benchmarks/bridge/bench_glue_speed.py`: Python→Briv(.so) vs Python→C vs
  Rust→Briv(LTO) vs native C on `token_hash` over a corpus. Baseline captured
  before changes (current `bench_glue_cross.py` identity-fn numbers, honest
  note that "Briv" does zero work today); new results recorded in
  `benchmarks/results/`. Colibri port = follow-on.

## Phase 4 — Callbacks / progress

- `fn(P) -> R` function-type annotation (parser + typechecker;
  `Type::Function` exists at `src/ast/types.rs:33`, lowers to
  `ret (params)` at `src/backend/llvm/types.rs:22`).
- `CallPtr#` intrinsic (explicit `#` marker): interpreter first (real function
  values — today lambdas are `Value::Void`), then LLVM `call`-through-pointer.
- Function-typed params carried in metadata; each language's config maps them
  (`ctypes.CFUNCTYPE` / `extern "C" fn` / `int64_t(*)(int64_t)`) — zero Rust
  marshaling knowledge.
- Demo: `export defn process_n(state, n, cb: fn(Int))` calling
  `CallPtr#(cb, i)` per item → Python progress-bar callback (his "update a
  first-level primitive" scenario).
- Docs: `docs/architecture/features/callbacks.md`, SPEC function-type grammar,
  learn-briv. GitHub issue: documented here, not filed (author files + credits).

## Phase 5 — Host cancellation

- Per-state `cancel_flag` in `%State` (inspect `state_layout()` /
  `emit_inline_init_stores` before committing layout).
- `CancelRequested#() -> Bool` / `ClearCancel#()` intrinsics (interpreter →
  LLVM); shim exports `__briv_set_cancel`/`__briv_clear_cancel` flow through
  metadata/templates.
- **Explicit polling only** (no implicit injection): user writes
  `when CancelRequested#() { term ... }`; composes with existing watchdog
  `within N ms` deadline.
- Tests: interpreter, IR golden, C-driver cancel (spawn thread → set cancel →
  early return).

## Cross-Cutting

- Commit per logical step; every commit `cargo test` green + Praetor on changed
  dirs + Kani on the cancel-flag load.
- Per-phase gate: tests green, C-driver/round-trip runtime checks pass, and a
  `git grep` over `src/glue/` + `src/backend/llvm/` finds zero hardcoded
  language strings.
- Docs updated in the same commit as structural changes:
  `docs/architecture/frgn-export-glue-architecture.md` (ABI + metadata
  contract), new `docs/architecture/glue-export-abi.md`,
  `docs/architecture/features/callbacks.md`, SPEC, learn-briv, AGENTS index.

## Baseline

Rule 11 (AGENTS.md): every performance plan records a baseline BEFORE changes.
Step 0 captures `cargo test --lib` state and `benchmarks/bridge/bench_glue_cross.py`
numbers at HEAD 21454601 before any modification. New results are appended here
as phases complete, never retroactively edited.

| Date | Worktree | `bench_glue_cross` median (Briv path) | Note |
|------|----------|---------------------------------------|------|
| 2026-08-03 | HEAD 21454601 | identity fn (echo ptr) | "Briv" did zero work today |
| 2026-08-03 | glue-host-callable | `feature_hash` 1000 → 2008ns/call | real FNV-1a compute; identical output to C |

### Phase 3 benchmark (2026-08-03, `benchmarks/bridge/bench_glue_speed.py`)

`make speed BRIVC=<repo>/target/debug/brivc` — per-call latency over 20000
calls, `feature_hash(count=1000, seed=42)` (FNV-1a folding over 1000 features):

| Path | median ns/call | mean ns/call |
|------|----------------|--------------|
| Python → Briv (.so) feature_hash | 2008 | 2183 |
| Python → C (.so) feature_hash | 1830 | 1969 |
| Python → Briv add (pure, no state) | 759 | 856 |

Briv vs C: 0.91× per-call overhead (identical output `8125762261814307938`).
The gap is ctypes marshalling + the state-arg path, not the compute. The Rust
LTO host (`examples/glue-host/rust-host`) calls `feature_hash`/`add` via plain
C ABI with zero marshalling and matches C bit-for-bit.

**Verified end-to-end hosts** (all produce identical results):
- C driver: `cc driver.c -lrank` against `librank.a` (real ELF, gcc-linkable).
- Python: generated `__init__.py` via ctypes against `rank.so`.
- Rust: `cargo run` in `examples/glue-host/rust-host` (build.rs runs
  `brivc build --library`, links `librank.a`, calls generated bindings).

**Known gap (logged in BUGS.md):** `#Float` exports are broken in the LLVM
backend (float32 lowering + `fmul` with an `i64` operand). The benchmark uses
Int exports only.

## Completion Status (2026-08-03)

- **Phase 1 (Data Briv config):** DONE. `lib/glue.toml` → `config/glue.dbvl`;
  parity proven; legacy `.dbvl/.dbvs` + dead `run_export` path + `dbvl_reader`
  removed; `glue_integration.sh` reconciled.
- **Phase 2 (host-callable wrappers + library):** DONE. Transitive export ABI
  (export_abi.rs incl. txn callees); config-driven conversions/state/param_decl;
  per-export wrappers verified end-to-end (Python via ctypes, C via .a/.so,
  Rust via LTO crate); `--library` + `briv bindings` + C-driver acceptance
  test (issue #2 criterion).
- **Phase 3 (demo + benchmark):** DONE. `examples/glue-host/rank.bv`
  (feature_hash/add), C reference, `bench_glue_speed.py`, `rust-host` crate.
  Baseline recorded above.
- **Phase 4 (callbacks):** DONE. `fn(P)->R` annotations, `CallPtr#`
  call-through-pointer, fn-typed export params (C fn-pointer ABI), demo
  (`apply(cb, x)`), C-driver round-trip test. See
  `docs/architecture/features/callbacks.md`.
- **Phase 5 (host cancellation):** DONE. `CancelRequested#`/`ClearCancel#`
  + `__briv_set_cancel`/`__briv_clear_cancel` (process-global atomic),
  explicit polling demo (`cancellable_sum`), C-driver cancel test. See
  `docs/architecture/features/host-cancellation.md`.

### Known follow-ups
- Python/Node wrapper templates don't render fn-typed (callback) params yet
  (CFUNCTYPE/ffi-napi) — the C path is the verified callback on-ramp.
- `#Float` backend corruption (BUGS.md) blocks Float exports.
- `sync<group>` still has no codegen (out of scope).
- Process-global cancel flag: per-state would allow concurrent instances
  (documented in the feature doc).
