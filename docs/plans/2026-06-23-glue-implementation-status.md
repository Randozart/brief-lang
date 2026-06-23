# GLUE Implementation Status & Remaining Work

**Date:** 2026-06-23  
**Status:** Active implementation — library-mode compilation in progress  

---

## 1. What GLUE Is

GLUE (General Language Unification Engine) is a universal FFI broker built on Brief's `meld` system. Any two languages that consume LLVM-compatible object code can be linked through GLUE. Neither language knows Brief exists. Both see their own native interface. Brief compiles to native `.o`/`.a`/`.wasm` — no C compiler, no `extern "C"`, no `cc` crate needed.

### CLI Verbs

| Command | Purpose | Status |
|---|---|---|
| `brief link <path> <function>` | Analyze a foreign library, generate `.bv` with `frgn` declarations | Not started |
| `brief export <bridge.bv> <language>` | Compile to `.a`, generate native wrappers via `$!` macro | Not started |
| `glue <target> <function> <language>` | One-shot: `brief link` + `brief export` | Not started |

### Protocol Files

| File | Purpose |
|---|---|
| `glue.dbvl` | Data Brief Lines — adapter registry, one language per line |
| `glue.dbvs` | Data Brief Schema — validates `glue.dbvl` entries |
| `glue/adapters/<language>.bv` | `$!macro` that generates native wrappers for a language |

---

## 2. Implemented So Far

### 2.1 Data Brief Parser (`src/glue/`)

- **`src/glue/mod.rs`** — Module declaration
- **`src/glue/dbvl_reader.rs`** — `.dbvl` parser
  - Splits lines by commas, respecting `" "` quoted strings and `{ }` map blocks
  - `schema <path>;` directives for mid-file schema switching
  - No-schema scraping mode (returns raw string vectors)
  - `parse_map("{Int:i64 Float:f32}")` helper
  - `unquote(s)` helper
- **`src/glue/dbvs_validator.rs`** — `.dbvs` schema parser
  - `entry Name { field: Type; ... };` grammar
  - Field types: String, Int, Enum<A,B>, Optional<T>, List<T; delimeter=X>, Map<K,V; pair_separator=:; ...>
  - Angle-bracket-aware semicolon splitting (critical for Map<...; ...>)
  - `validate_fields()` for positional validation
- **14 tests** across both modules
- **`lib.rs`** — added `pub mod glue;`

### 2.2 AGENTS.md Updates

Added sections:
- **GLUE Architecture** — overview of how GLUE works
- **Correctness Over Speed** — applies to ALL development, not just GLUE
  - Use Brief's existing systems (`$!` macros, `meld`, contracts) instead of throwaway Rust infrastructure
  - Fix deprecated patterns when encountered (frgn→intrinsic, double `[true][true]`, TOML bindings)
  - NO prototyping — every commit is production-quality
- **EXECUTIVE REQUESTS ARE NOT OPTIONAL** — do not skip explicit instructions

### 2.3 frgn→Intrinsic Cleanup

Replaced `frgn` declarations with `defn` wrappers calling intrinsics:

| File | Frgns Replaced | Intrinsics Used |
|---|---|---|
| `lib/std/gpu.bv` | `get_global_id`, `get_local_id`, `get_group_id`, `get_num_groups`, `barrier` | `get_global_id#`, etc. |
| `lib/std/tty.bv` | `__tty_raw_mode`, `__tty_size`, `__tty_read_key` | `tty_raw_mode#`, etc. |
| `lib/std/ffi/out.bv` | `__print_int`, `__putchar`, `__print`, `__print_float`, `__exit` | `print_int#`, etc. (whole file rewritten, removed obsolete `sig #out`) |
| `lib/std/io.bv` | `__read_file`, `__write_file` | `read_file#`, `write_file#` |
| `lib/std/ffi/io.bv` | `__read_file`, `__write_file` | `read_file#`, `write_file#` |
| `lib/std/process.bv` | `__spawn`, `__spawn_with_output` | `spawn#`, `spawn_with_output#` (Result preserved via wrapper) |
| `lib/std/ffi/process.bv` | `__spawn`, `__spawn_with_output` | `spawn#`, `spawn_with_output#` |
| `lib/std/shm.bv` | `__shm_open`, `__shm_unlink`, `__munmap` | `shm_open#`, `shm_unlink#`, `munmap#` (Result preserved via wrapper) |
| `lib/std/ffi/shm.bv` | `__shm_open`, `__shm_unlink`, `__munmap` | `shm_open#`, `shm_unlink#`, `munmap#` |
| `lib/std/string.bv` | `__bytes` (replaced with `str_bytes#` intrinsic) | `str_bytes#` (new intrinsic) |
| `lib/std/ffi/string.bv` | `__bytes` | `str_bytes#` |
| `examples/test_ffi.bv` | `println`, `sqrt`, `sin`, `pow` | `println#`, `sqrt#`, `sin#`, `pow#` |
| `examples/test_ffi_minimal.bv` | `println`, `sqrt` | `println#`, `sqrt#` |

### 2.4 Intrinsic Renames

| Old Name | New Name | Reason |
|---|---|---|
| `Intrinsic::Bytes` (user: `bytes#`) | `Intrinsic::ByteCount` (user: `byte_count#`) | `bytes#` sounded like it converts to bytes, but actually returns byte count |
| `Intrinsic::Truncate` (user: `truncate#`) | `Intrinsic::FTruncate` (user: `ftruncate#`) | `truncate#` truncated files (POSIX truncate), not strings. Renamed to match POSIX naming. |
| Added `Intrinsic::StrBytes` (user: `str_bytes#`) | New | Converts a String to `List<Int>` of byte values (replaces `__bytes` frgn) |

Files modified: `ast.rs`, `parser.rs`, `emit_expr.rs`, `mod.rs` (LLVM backend declare), `interpreter.rs`, `typechecker.rs`

### 2.5 Double `[true][true]` Contract Fix

Fixed `examples/pipe-chain.bv` and `examples/pipe-skip.bv` which had `[true][true]` — rejected by the validator ("both precondition and postcondition are [true]"). Removed the contracts entirely (they were redundant).

### 2.6 Library-Mode Compilation (In Progress)

**Completed:**
- Added `library_mode: bool` field to `LlvmBackend` struct
- Added `with_library_mode()` builder method (next to `with_dump_layout`)
- Added `emit_library_shim()` in `emit_toplevel.rs`:
  - Emits `define dso_local i64 @__brief_init_state()` — allocates `%State`, calls init_state, returns ptr
  - Emits `define dso_local void @__glue_release(i64 %frame_tag)` — no-op placeholder
- Modified `generate()` in `mod.rs` to call `emit_library_shim()` instead of `emit_main()` when `library_mode` is true
- Added `--library` CLI flag to both subcommand handlers (`build` and `llvm`)
- Added `--layout` CLI flag to both subcommand handlers
- Added `.with_library_mode(library_mode)` to backend construction in `run_llvm_compile`
- Modified `run_llvm_compile()` signature to include `library_mode: bool` parameter
- Added `ar rcs` linking path in both linking sections of `run_llvm_compile`

**Blocked:** The other agent added `Expr::CompCall(Vec<Expr>)` to the AST, breaking 50+ match expressions. Library-mode compilation cannot be tested until this is fixed.

---

## 3. Architecture Decisions

### 3.1 Adapters are Brief `$!` Macros (Not Rust Template Engine)

Language adapters are `.bv` files containing `$!macro` definitions. The macro takes the bridge's `#export`/`frgn`/`meld` declarations at compile time and emits native wrapper source code.

The `brief export` command:
1. Compiles the `.bv` bridge to `.a` (library mode)
2. Finds the language entry in `glue.dbvl`
3. Invokes the `$!macro` for that language with the bridge's declarations
4. The macro uses `compile#()` (compile-time intrinsic) to write generated source files alongside the `.a`

### 3.2 Memory Model (Three Tiers)

| Tier | Mechanism | When |
|---|---|---|
| **Register** (Int/Float/Bool, ≤16 byte structs) | Passed in registers. Zero allocation. | Always, when types fit. |
| **Arena stacks** (Strings, large structs) | Bridge allocates in per-call arena. Caller reads then calls `__glue_release(tag)`. | Default for heap-allocated returns. |
| **Shared memory regions** | Bridge and caller negotiate during `__glue_init()`. `meld` proves both sides interpret same bytes. | Zero-copy advanced mode. |

### 3.3 Error Propagation ("Always Be Prepared for Null")

Bridge never panics. When a `frgn` inside the bridge fails:
- Returns zero value for return type (0 for Int, null for String/pointer, false for Bool)
- Caller is always responsible for null-checking
- Brief's `Result` type makes this explicit — no silent failures

### 3.4 File Extensions

| Extension | Meaning |
|---|---|
| `.bv` | Universal Brief — code and data |
| `.dbvl` | Data Brief Lines — raw data, one line per entry |
| `.dbvs` | Data Brief Schema — validates `.dbvl` and `.dbv` files |

---

## 4. Remaining Work (Priority Order)

### P0 — Fix CompCall Compilation Breakage

The other agent added `Expr::CompCall(Vec<Expr>)` to `src/ast.rs:1299`. Every `match` on `Expr` needs a `CompCall(v)` ⇒ `/* recurse on args */` arm. Estimated 50+ match sites across all files.

**Action:** Wait for the other agent to complete their fix, then test library mode.

### P1 — Verify Library-Mode End-to-End

After CompCall is fixed:

1. `cargo test --lib` — all tests pass
2. `cargo run -- brief build examples/meld-simple.bv --no-stdlib --library`
3. Check output is `meld-simple.a` not an executable
4. `ar t meld-simple.a` — verify it contains `.o` files with `__brief_init_state`
5. `nm meld-simple.a | grep export` — verify `#export` wrappers are `T` (text section) symbols

### P1 — Implement `brief export` Subcommand

New subcommand `brief export <bridge.bv> <language>`:

1. **Parse bridge.** Collect `#export` declarations, `frgn` declarations, and `meld` declarations from the `.bv` file.
2. **Compile.** `brief build bridge.bv --library` → `bridge.a`
3. **Read registry.** Parse `glue.dbvl`, find entry for `<language>`, validate against `glue.dbvs`.
4. **Load macro.** Find `glue/adapters/<language>.bv` — this contains the `$!macro` for the target language.
5. **Render.** Invoke the macro with bridge information. The macro calls `compile#()` to write generated source files.
6. **Output.** `glue/<language>-bridge/` containing:
   - `bridge.a`
   - `<language>-specific source files` (e.g., `lib.rs`, `Cargo.toml` for Rust)
   - `README.md`

### P1 — Write Rust Adapter Macro (`glue/adapters/rust.bv`)

The Rust adapter must generate:
- `Cargo.toml` — package metadata, `bridge.a` linked statically
- `build.rs` — `cargo:rustc-link-lib=static=bridge` directive
- `src/lib.rs` — safe Rust wrappers for each `#export` function
- `src/ffi.rs` — `extern "C"` block (not exposed to user)

The macro receives:
- List of export declarations (name, params, return type)
- List of frgn declarations (name, params, return type)
- Type map from `glue.dbvl` (Brief type → Rust type mapping)

### P2 — Write Additional Language Adapters

| Language | Template File | Key Challenge |
|---|---|---|
| Python | `glue/adapters/python.bv` | CPython `PyInit_` module, ctypes wrapper |
| Node | `glue/adapters/node.bv` | N-API native addon, `binding.gyp`, TypeScript types |
| WASM | `glue/adapters/wasm.bv` | `.wasm` exports, TypeScript types |

### P2 — Create `glue-ffi` Standalone Project

Standalone repository at `~/Desktop/Projects/glue-ffi/`:

```bash
glue-ffi/
  Cargo.toml
  src/
    main.rs          # CLI entry — delegates to brief build --library
  templates/
    rust/
    python/
    node/
  lib/
    glue.dbvl
    glue.dbvs
```

The standalone GLUE requires `brief` compiler on `$PATH`. It shell-executes `brief build --library` and `brief export` internally.

### P3 — Implement `brief link` Subcommand

Analyzes a foreign library:
1. Reads symbol table (via `nm`/`objdump` or TOML binding files)
2. Cross-references against `Intrinsic` enum — if `frgn` name matches an intrinsic, emit `intrinsic_call#()`
3. Generates `.bv` file with `frgn` declarations
4. Optionally generates `bindings.toml` for the FFI registry

### P3 — Implement `compile#()` Compile-Time Intrinsic

The `$!` macro system needs `compile#(filename, content)` to write generated files at compile time. Currently not implemented.

**Implementation:**
```
// In src/features/macros/template.rs or similar
fn handle_compile_intrinsic(filename: &str, content: &str) -> Result<(), Error> {
    let output_dir = determine_output_dir();
    std::fs::write(output_dir.join(filename), content)?;
    Ok(())
}
```

The intrinsic is annotated with `is_compile_time_only()` and is only available during `$!` macro expansion, not at runtime.

---

## 5. File Index

### New Files

| File | Status | Purpose |
|---|---|---|
| `src/glue/mod.rs` | ✅ Done | Module declaration |
| `src/glue/dbvl_reader.rs` | ✅ Done | `.dbvl` parser |
| `src/glue/dbvs_validator.rs` | ✅ Done | `.dbvs` parser/validator |
| `lib/glue.dbvl` | ❌ Not started | Adapter registry |
| `lib/glue.dbvs` | ❌ Not started | Registry schema |
| `glue/adapters/rust.bv` | ❌ Not started | Rust adapter macro |

### Modified Files

| File | Status | Changes |
|---|---|---|
| `src/lib.rs` | ✅ Done | Added `pub mod glue;` |
| `src/ast.rs` | ✅ Done | `Bytes` → `ByteCount`, `Truncate` → `FTruncate`, added `StrBytes` |
| `src/parser.rs` | ✅ Done | Updated intrinsic keyword mapping |
| `src/interpreter.rs` | ✅ Done | Renamed intrinsics, added StrBytes handler |
| `src/typechecker.rs` | ✅ Done | Renamed intrinsics, added StrBytes return type |
| `src/backend/llvm/mod.rs` | ✅ Done | Added `library_mode`, `dump_layout` fields, emit_library_shim call in generate() |
| `src/backend/llvm/emit_toplevel.rs` | ✅ Done | Added `emit_library_shim()` |
| `src/backend/llvm/emit_expr.rs` | ✅ Done | Renamed intrinsics, added StrBytes handler |
| `src/main.rs` | ✅ Partial | Added `--library`, `--layout` flags, `ar rcs` linking (needs testing) |
| `AGENTS.md` | ✅ Done | Added GLUE architecture, Correctness Over Speed, Executive Requests sections |

---

## 6. Test Plan

### Unit Tests (Existing)

| Test | What it validates |
|---|---|
| `dbvl_reader::tests::test_parse_simple_line` | Basic comma splitting |
| `dbvl_reader::tests::test_parse_quoted_value` | Quoted strings with commas |
| `dbvl_reader::tests::test_schema_directive` | Schema path extraction |
| `dbvl_reader::tests::test_schema_switch` | Mid-file schema changes |
| `dbvl_reader::tests::test_parse_map` | `{key:val}` map parsing |
| `dbvl_reader::tests::test_comments_and_blank_lines` | Comment skipping |
| `dbvl_reader::tests::test_no_schema_scraping_mode` | Raw mode with no schema |
| `dbvs_validator::tests::test_parse_entry_string` | Basic schema parsing |
| `dbvs_validator::tests::test_parse_entry_enum` | Enum type parsing |
| `dbvs_validator::tests::test_parse_entry_map` | Map type parsing (semicolons in angle brackets) |
| `dbvs_validator::tests::test_validate_correct_fields` | Field count validation |
| `dbvs_validator::tests::test_validate_wrong_field_count` | Wrong count rejection |
| `dbvs_validator::tests::test_parse_optional` | Optional type parsing |
| `dbvs_validator::tests::test_empty_schema` | Empty schema body |

### Integration Tests (To Write)

| Test | What it validates | Priority |
|---|---|---|
| `brief build example.bv --library` | Library mode produces `.a` not executable | P1 |
| `nm bridge.a \| grep __brief_init_state` | `__brief_init_state` symbol exported | P1 |
| `brief export example.bv rust` | Full pipeline: compile → read dbvl → invoke macro → write crate | P1 |
| Rust program linking bridge crate | End-to-end: Rust calls export, bridge calls frgn | P2 |

---

## 7. Known Issues

1. **`Expr::CompCall` breaks compilation.** The other agent's addition must be resolved before library mode can be tested.
2. **`compile#()` intrinsic not implemented.** The `$!` macro system can't write files yet. Adapter macros need this.
3. **Duplicate stdlib files.** `lib/std/ffi/*.bv` are identical copies of `lib/std/*.bv`. These should either be removed or kept as aliases.
4. **LLVM backend `ByteCount` is a stub.** `byte_count#` always returns `8` in the LLVM backend. The interpreter handles it correctly. Needs proper codegen.
5. **`__glue_release` is a no-op.** The arena-based memory model is not implemented. Currently placeholder.
