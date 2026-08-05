# Stdlib Frgn Cleanup — Comprehensive Plan

**Date:** 2026-07-22
**Status:** Committed (2026-07-22)
**Depends on:** Phases 0-7 of `2026-07-22-frgn-export-glue-pipeline.md` (completed)

---

## Motivation

The stdlib accumulated several outdated patterns during rapid iteration:

1. **inop keyword** — Was removed from the language spec but still used in 5 stdlib files + 1 example. Every `inop` has a `defn` + intrinsic equivalent.

2. **sig #out modifier** — Predates the `observable <~ true` metadata system. Now redundant; metadata handles observability.

3. **Inconsistent frgn naming** — ~180 frgn declarations use a `__` prefix (`__to_upper`, `__now`), but some use nothing (`XXH64`, `XXH32`). No consistent convention for distinguishing "raw FFI" from "Briv wrapper" at call sites.

4. **No fallback clauses** — The new frgn syntax supports `fallback <expr>` but no stdlib frgn uses it. Every FFI call silently fails on unavailable libraries.

5. **No from clauses** — Most frgns omit `from`, meaning the compiler can't determine which foreign module provides the symbol. This blocks module-level compilation and provenance tracking.

6. **from pointing to .bv files** — Some frgns use `from "std/briv_rt.bv"` but a `.bv` file is a Briv source, not a foreign module. Frgn should always point to foreign code — `.c`, `.so`, `link/...`, or similar.

7. **Duplicate std/ffi/ directory** — `lib/std/X.bv` and `lib/std/ffi/X.bv` are identical copies that will drift independently. One should be canonical, the other should import.

8. **Missing semicolons** — 2 frgn declarations in `encoding.bv` lack terminating `;`.

9. **#export modifier in examples** — Still uses `#export defn` instead of the standard `export defn`.

10. **ForeignBinding field semantics reversed** — `name` stores the Briv name and `as_name` stores the foreign/C symbol, but the import analogy demands the opposite: the first name after `frgn` is the foreign symbol, and `as` provides the Briv renaming.

---

## Architecture: Frgn Declarations as Imports

A `frgn` declaration IS an import statement. It imports a foreign function into Briv's namespace.

```briv
frgn XXH64(data_ptr: Int, len: Int, seed: Int) -> Int as frgn__xxh64 from "link/xxhash/xxhash.c" fallback 0;
// ^^^^^                                              ^^^^^^^^^^^^^   ^^^^^^^^^^^^^^^^^^^^^^^^^    ^^^^^^^^^^
// foreign/C symbol (what the library exports)        Briv name       source (provenance)          fallback
```

This reads naturally as: "import function `XXH64` from `xxhash.c`, call it `frgn__xxh64` in Briv, and if unavailable return 0."

### The `frgn__` convention

All raw FFI functions exposed to Briv code use a `frgn__` prefix. This makes every call site visually unambiguous:

```briv
defn to_upper(s: String) -> Result<String, StringError> {
    term frgn__to_upper(s);  // the __ marks "this is a raw FFI call"
};
```

Without the prefix, readers can't distinguish "this is a Briv function" from "this crosses the FFI boundary."

When the C symbol already follows the convention (e.g., `frgn__to_upper`), no `as` clause is needed:

```briv
frgn frgn__to_upper(s: String) -> Result<String, StringError> from "link/briv_rt.c";
```

When the C symbol differs, `as` provides the Briv name:

```briv
frgn __print_int(n: Int) -> Int as frgn__print_int from "link/briv_rt.c" fallback ;
frgn XXH64(data_ptr: Int, len: Int, seed: Int) -> Int as frgn__xxh64 from "link/xxhash/xxhash.c" fallback 0;
```

### Required `from` clause

Every frgn MUST have a `from` clause. Without it, the compiler cannot determine which foreign module provides the symbol. The `from` value points to foreign code (`.c`, `.so`, `link/...`, `.h`, etc.) — never to a `.bv` file. The compiler uses this to resolve the symbol at link time, compile the source if needed, or verify the library exists.

### `as` before `from`, requires `from`

The `as` clause comes BEFORE `from` in the declaration to reduce ambiguity when reading:

```briv
frgn func(...) -> Ret as briv_name from "source" fallback ...;
```

And `as` is only valid when `from` is also present (you can't rename an import without specifying what you're importing from).

---

## Part A: ForeignBinding Struct Rename

### Why

The current field names `name` and `as_name` encode the pre-import-model semantics. Renaming them prevents confusion and makes the code self-documenting.

### Changes

**`src/ast/top.rs`:**

```rust
pub struct ForeignBinding {
    pub foreign_name: String,           // C/foreign symbol name (was `name`)
    pub briv_name: Option<String>,     // Briv name (was `as_name`), None = same as foreign_name
    pub from: FromSpec,                 // Required frgn -- provenance source
    pub target: ForeignTarget,
    pub inputs: Vec<(String, Type)>,
    pub success_output: Vec<(String, Type)>,
    pub error_type: String,
    pub error_fields: Vec<(String, Type)>,
    pub input_layout: Option<()>,
    pub output_layout: Option<()>,
    pub precondition: Option<String>,
    pub postcondition: Option<String>,
    pub buffer_mode: Option<String>,
    pub default_watchdog: Option<(u64, u64, u64, Box<Expr>)>,
    pub wasm_impl: Option<String>,
    pub wasm_setup: Option<String>,
    pub fallback: Fallback,
    pub span: Option<Span>,
}

impl ForeignBinding {
    /// Returns the name that Briv code uses to call this frgn.
    /// When `briv_name` is None, falls back to `foreign_name`.
    pub fn effective_briv_name(&self) -> &str {
        self.briv_name.as_deref().unwrap_or(&self.foreign_name)
    }
}
```

**Constructor** — update `ForeignBinding::new(name, as_name, ...)` to `ForeignBinding::new(foreign_name, briv_name, ...)`.

### All 16+ call sites

| File | Current usage | New usage |
|------|---------------|-----------|
| `src/parser/definitions.rs` | `name: name, as_name: as_name` | `foreign_name: foreign_name, briv_name: briv_name` |
| `src/analysis/frgn_dispatch.rs:101` | `fb.name` (error msg) | `fb.effective_briv_name()` |
| `src/analysis/frgn_dispatch.rs:113` | `fb.as_name.unwrap_or(fb.name)` (symbol) | `fb.foreign_name` (simpler) |
| `src/analysis/frgn_dispatch.rs:135` | `fb.as_name.unwrap_or(fb.name)` (symbol) | `fb.foreign_name` (simpler) |
| `src/analysis/frgn_dispatch.rs:145` | `fb.name` (error msg) | `fb.effective_briv_name()` |
| `src/compile.rs:232` | `fb.name` (dispatch key) | `fb.effective_briv_name()` |
| `src/backend/llvm/mod.rs:1770` | `fb.name` | `fb.effective_briv_name()` |
| `src/backend/llvm/mod.rs:1778` | `fb.name` | `fb.effective_briv_name()` |
| `src/backend/webstack.rs:350` | `fb.name` | `fb.effective_briv_name()` |
| `src/backend/webstack.rs:352` | `fb.name` | `fb.effective_briv_name()` |
| `src/import_resolver.rs:633` | `fb.name` | `fb.effective_briv_name()` |
| `src/import_resolver.rs:763` | `&fb.name` | `fb.effective_briv_name()` |
| `src/lsp.rs:797` | `fb.name` | `fb.effective_briv_name()` |
| `src/analysis/layout_optimizer.rs:68` | `fb.name` | `fb.effective_briv_name()` |
| `src/glue/export.rs:260` | `e.name` (export name) | `e.name` (Export struct unchanged) |
| `src/library.rs:124` | `defn.name` (Definition, not FB) | Unchanged |
| All test files | `fb.name`, `fb.as_name` | `fb.foreign_name`, `fb.briv_name` |

---

## Part B: Parser Restructure

### Why

The current parser places `as` immediately after `frgn name` (before params). The new grammar places `as` after the return type and before `from`. The parser must also enforce `from` as required.

### New grammar

```
frgn_decl ::= "frgn" frgn_body ";"
frgn_body ::= foreign_name "(" params ")" [ "->" ret ]
              [ "as" briv_name ]
              "from" source_spec
              [ "fallback" fallback_expr ]
```

### Current parser (definitions.rs:75-173)

```rust
fn parse_frgn_decl() {
    let name = self.expect_identifier()?;                    // Briv name
    let as_name = if self.eat_identifier("as") {             // C symbol
        Some(self.expect_identifier()?)
    } else { None };
    self.expect(Token::LParen)?;
    // ... parse params ...
    self.expect(Token::RParen)?;
    // ... parse return type ...
    let from = if self.eat(&Token::From) {                   // Optional from
        self.parse_from_spec()?
    } else { FromSpec::default() };
    // ... parse fallback ...
    // ... construct ForeignBinding { name, as_name, ... }
}
```

### New parser

```rust
fn parse_frgn_decl() {
    let foreign_name = self.expect_identifier()?;            // C/foreign symbol
    self.expect(Token::LParen)?;
    // ... parse params ...
    self.expect(Token::RParen)?;
    // ... parse return type ...
    let briv_name = if self.eat_identifier("as") {          // Briv name (optional)
        Some(self.expect_identifier()?)
    } else { None };
    if !self.eat(&Token::From) {                             // Required from
        return self.error("frgn requires `from <source>` — add provenance");
    }
    let from = self.parse_from_spec()?;
    // ... parse fallback ...
    // ... construct ForeignBinding { foreign_name, briv_name, ... }
}
```

### FromSpec parsing

`parse_from_spec()` already handles `"string"` (literal path) and `<name>` (compiler registry). No changes needed.

### Error messages

- Missing `from`: `"frgn 'xxh64' requires `from <source>` — specify which foreign module provides this symbol"`
- `as` without `from`: handled by grammar (as is parsed before from, and from is required, so as requires from implicitly)

---

## Part C: Remove inop from Stdlib

### Why

The `inop` keyword was removed from the language spec (see AGENTS.md, Golden Rule 6). Every stdlib `inop` has a replacement using available intrinsics.

### C1. `lib/std/atomic.bv` — 8 inop! declarations

| Current inop! | Replacement |
|---------------|-------------|
| `atomic_load(ptr: Ptr<Int>) -> Int` | `defn { term AtomicLoad#(ptr); }` |
| `atomic_store(ptr: Ptr<Int>, val: Int) -> Int` | `defn { term AtomicStore#(ptr, val); }` |
| `atomic_cas(ptr, expected, desired) -> Int` | `defn { term AtomicCas#(ptr, expected, desired); }` |
| `atomic_fetch_add(ptr, val) -> Int` | `defn { let old = AtomicLoad#(ptr); ... AtomicCas# loop; term old; }` — use `AtomicAdd#` if available, otherwise CAS-loop |
| `atomic_fetch_sub(ptr, val) -> Int` | CAS-loop with `AtomicCas#(ptr, old, old - val)` |
| `atomic_fetch_and(ptr, val) -> Int` | CAS-loop with `AtomicCas#(ptr, old, old & val)` |
| `atomic_fetch_or(ptr, val) -> Int` | CAS-loop with `AtomicCas#(ptr, old, old \| val)` |
| `atomic_fetch_xor(ptr, val) -> Int` | CAS-loop with `AtomicCas#(ptr, old, old ^ val)` |

Available atomic intrinsics: `AtomicLoad#`, `AtomicStore#`, `AtomicCas#`, `AtomicXchg#`, `AtomicAdd#`, `Fence#`.

`AtomicAdd#` can replace `atomic_fetch_add` directly (it atomically adds and returns the old value on x86). For `fetch_sub`/`and`/`or`/`xor`, the standard CAS-retry loop is used.

### C2. `lib/std/syscall.bv` — 6 inop! declarations

```briv
// Before:
inop! syscall6(nr: Int, a1..a6: Int) -> Int { asm target { x86_64 { ... }; aarch64 { ... }; riscv64 { ... }; default { ... }; } } fallback -1;

// After:
defn syscall6(nr: Int, a1: Int, a2: Int, a3: Int, a4: Int, a5: Int, a6: Int) -> Int {
    term SysCall#(nr, a1, a2, a3, a4, a5, a6);
};
```

`SysCall#` intrinsic already exists and is observable (`observable: true`). It handles architecture dispatch internally. Repeat for syscall5 through syscall1.

### C3. `lib/std/skiplist.bv` — 2 inop declarations

`sl_insert` and `sl_remove` currently use BILD LLVM IR with `malloc`/`memcpy`/`free`. Replace with `defn` wrappers using `Malloc#`/`Copy#`/`Free#` intrinsics and pointer arithmetic.

If the Briv-level implementation is slower than the C equivalent, create:
- `benchmarks/skiplist.bv` — Briv implementation benchmark
- `benchmarks/skiplist.c` — C reference benchmark

### C4. `lib/std/state.bv` — Delete

Single `inop! accum` with `(%state)` marker for direct State struct access. This pattern is obsolete — accumulation is expressed directly in Briv:

```briv
// Instead of inop! accum(val: Int) -> Int {%state}
// User writes:
let counter: Int;
txn my_txn [counter < N][counter == N] {
    let old = counter;
    counter = counter + val;
    // ... use old ...
};
```

No stdlib utility needed. Delete the file.

### C5. `examples/bild-asm-target.bv` — Delete

Demonstrates `inop!` with `asm target { }`. Since `inop` is removed from the language, this example has no purpose.

---

## Part D: Stdlib frgn Naming Convention

### Why

All raw FFI declarations should use `frgn__` prefix so call sites are visually unambiguous.

### Rules

1. Every `frgn` declaration's effective Briv name (from `as` clause, or the foreign name itself) starts with `frgn__`.
2. The C/foreign symbol name stays unchanged (whatever the library exports).
3. If the C symbol already uses a recognizable prefix, that's fine — the `as` clause maps it to `frgn__`.

### Migration table

| File | C symbol | Briv name (new) | as clause |
|------|----------|-------------------|-----------|
| `ffi/io.bv` | `__print_int` | `frgn__print_int` | `as frgn__print_int` |
| `ffi/io.bv` | `__print_str` | `frgn__print_str` | `as frgn__print_str` |
| `ffi/io.bv` | `__print_float` | `frgn__print_float` | `as frgn__print_float` |
| `ffi/io.bv` | `__print_char` | `frgn__print_char` | `as frgn__print_char` |
| `ffi/env.bv` | `__getenv_briv` | `frgn__getenv_briv` | `as frgn__getenv_briv` |
| `ffi/env.bv` | `__getenv_int` | `frgn__getenv_int` | `as frgn__getenv_int` |
| `string.bv` | `frgn__to_upper` | `frgn__to_upper` | (none — name matches) |
| `string.bv` | `frgn__to_lower` | `frgn__to_lower` | (none) |
| ... (23 string frgns) | `frgn__*` | `frgn__*` | (none) |
| `encoding.bv` | `frgn__base64_encode` | `frgn__base64_encode` | (none) |
| ... (27 encoding frgns) | `frgn__*` | `frgn__*` | (none) |
| `time.bv` | `frgn__now` | `frgn__now` | (none) |
| ... (26 time frgns) | `frgn__*` | `frgn__*` | (none) |
| `xxhash.bv` | `XXH64` | `frgn__xxh64` | `as frgn__xxh64` |
| `xxhash.bv` | `XXH32` | `frgn__xxh32` | `as frgn__xxh32` |
| `shm.bv` | `frgn__shm_list` | `frgn__shm_list` | (none) |
| `shm.bv` | `frgn__shm_exists` | `frgn__shm_exists` | (none) |
| `shm.bv` | `frgn__shm_size` | `frgn__shm_size` | (none) |
| `shm.bv` | `frgn__msync` | `frgn__msync` | (none) |
| `shm.bv` | `frgn__mmap_write` | `frgn__mmap_write` | (none) |
| `shm.bv` | `frgn__mmap_read` | `frgn__mmap_read` | (none) |
| `shm.bv` | `frgn__mmap_read_u32` | `frgn__mmap_read_u32` | (none) |
| `shm.bv` | `frgn__mmap_write_u32` | `frgn__mmap_write_u32` | (none) |
| `http.bv` | `frgn__http_get` | `frgn__http_get` | (none) |
| `http.bv` | `frgn__http_post` | `frgn__http_post` | (none) |
| `metro_bridge.bv` | `frgn__metro_create_channel` | `frgn__metro_create_channel` | (none) |
| ... (24 metro frgns) | `frgn__*` | `frgn__*` | (none) |

---

## Part E: Add from Clauses

### Why

Every frgn must declare its provenance. The `from` clause tells the compiler which foreign module provides the symbol, enabling module-level compilation and dependency resolution.

### Sources table

| File(s) | C source | from value |
|---------|----------|------------|
| `ffi/io.bv`, `ffi/env.bv`, `string.bv`, `encoding.bv`, `time.bv`, `shm.bv` (mmap/msync), `metro_bridge.bv` (partially) | `lib/std/briv_rt.c` (linked via `import "link/briv_rt.c"`) | `from "link/briv_rt.c"` |
| `xxhash.bv` | `lib/link/xxhash/xxhash.c` | `from "link/xxhash/xxhash.c"` |
| `http.bv` | C runtime linked in `briv_rt.c` | `from "link/briv_rt.c"` |
| `metro_bridge.bv` (SHM/mmap parts) | `lib/std/briv_rt.c` | `from "link/briv_rt.c"` |
| `syscall.bv` | (Replaced by `SysCall#` intrinsic, no `frgn` needed) | N/A |
| `atomic.bv` | (Replaced by atomic intrinsics, no `frgn` needed) | N/A |

Note: After the inop→intrinsic migration in Part C, `syscall.bv` and `atomic.bv` will contain only `defn` wrappers, no `frgn` declarations. They need `from` only if they import from other modules.

---

## Part F: Add fallback Clauses

### Why

Every FFI call must handle the case where the foreign library is unavailable. The `fallback` clause defines what happens.

### Fallback table

| Return type | Fallback | Rationale |
|-------------|----------|-----------|
| `Int` (infallible, e.g., length, hash) | `fallback 0;` | Zero is a safe default for numeric types |
| `Int` (I/O, e.g., print_int) | `fallback ;` (Implicit) | Skip the I/O call silently |
| `Void` | `fallback ;` (Implicit) | Nothing to return |
| `Result<T, E>` | `fallback Err(E { message: "frgn unavailable" });` | Propagate error to caller |
| `String` | `fallback "";` | Empty string is a safe default |
| `Bool` | `fallback false;` | False is a safe default |
| `Ptr<...>` | `fallback 0;` | Null pointer |

---

## Part G: Briv Wrapper defns

### Why

Raw `frgn` calls should have a Briv wrapper that provides a clean, idiomatic API. This separates the FFI boundary from the public interface.

### Wrapper strategy

**I/O functions** (`print_int`, `print_str`, `print_float`, `putchar`): Retry loop (3 attempts).

```briv
defn print_int(n: Int, retries: Int = 3) -> Bool {
    let attempt: Int = 0;
    txn retry [attempt < retries][attempt == retries] -> Bool {
        [frgn__print_int(n) == 0] {
            attempt = attempt + 1;
        };
        term;
    };
    term true;
};
```

**Compute functions** (string, encoding, time, env, xxhash): Passthrough with error normalization.

```briv
defn to_upper(s: String) -> Result<String, StringError> {
    term frgn__to_upper(s);
};
```

**Network/IO** (HTTP): Retry loop.

```briv
defn http_get(url: String, retries: Int = 3) -> Result<String, StringError> {
    let attempt: Int = 0;
    txn retry [attempt < retries][attempt == retries] -> Result<String, StringError> {
        let result = frgn__http_get(url);
        [result.is_ok()] { term result; };
        attempt = attempt + 1;
    };
    term Err("http_get failed after retries");
};
```

**SHM/metro**: Passthrough.

### Where wrappers live

Wrappers go in the canonical `lib/std/X.bv` files. Raw frgn declarations go in `lib/std/ffi/X.bv`.

---

## Part H: Deduplicate lib/std/ffi/

### Why

Two copies of each file (`lib/std/X.bv` and `lib/std/ffi/X.bv`) are identical and will drift. The `ffi/` subdirectory should be the canonical home for raw frgn declarations.

### New structure

| Canonical location | Content | Public entry point | Content |
|-------------------|---------|-------------------|---------|
| `lib/std/ffi/string.bv` | Raw frgns only (with `frgn__`, `from`, `fallback`) | `lib/std/string.bv` | `import "std/ffi/string.bv"` + wrapper defns |
| `lib/std/ffi/encoding.bv` | Raw frgns only | `lib/std/encoding.bv` | `import "std/ffi/encoding.bv"` + wrapper defns |
| `lib/std/ffi/time.bv` | Raw frgns only | `lib/std/time.bv` | `import "std/ffi/time.bv"` + wrapper defns |
| `lib/std/ffi/shm.bv` | Raw frgns only | `lib/std/shm.bv` | `import "std/ffi/shm.bv"` + wrapper defns |
| `lib/std/ffi/http.bv` | Raw frgns only | `lib/std/http.bv` | `import "std/ffi/http.bv"` + wrapper defns |
| `lib/std/ffi/xxhash.bv` | Raw frgns only | `lib/std/xxhash.bv` | `import "std/ffi/xxhash.bv"` + wrapper defns |
| `lib/std/ffi/metro_bridge.bv` | Raw frgns only | `lib/std/metro_bridge.bv` | `import "std/ffi/metro_bridge.bv"` + wrapper defns |
| `lib/std/ffi/io.bv` | Raw frgns only | (used by `lib/std/ffi/out.bv` and imports) | Stays as-is |
| `lib/std/ffi/env.bv` | Raw frgns only | (used by imports) | Stays as-is |
| `lib/std/core/` | — | DELETE | Duplicates of lib/std/ |

---

## Part I: Remove #out

### Why

The `sig #out` modifier predates the `observable <~ true` metadata system. The metadata system is more general: any intrinsic or frgn can declare itself observable, and DCE respects the flag. The `#out` modifier is obsolete.

### Changes

**Delete** `lib/std/out.bv` — contains 5 `sig #out` declarations importing from raw frgns.

**Keep** `lib/std/ffi/out.bv` — contains `defn` wrappers around `Print#` intrinsic (which has `observable: true`). This is the correct modern pattern.

Check for any files importing from `std/out.bv` — if found, redirect to the appropriate wrapper.

---

## Part J: Fix Bugs

### J1. Missing semicolons

`lib/std/encoding.bv` has 2 frgn declarations missing terminating `;`:

```briv
frgn __hex_decode_bytes(s: String) -> Result<List<Int>, EncodingError>
frgn __UTF8_encode(s: String) -> Result<List<Int>, EncodingError>
```

Add `;` after each. Same fix in duplicate `lib/std/ffi/encoding.bv`.

### J2. #export modifier in examples

`examples/glue-python-bridge/bridge.bv` uses `#export defn` — change to `export defn` (straight keyword, no modifier).

---

## Part K: Update Documentation

| File | Update |
|------|--------|
| `spec/SPEC.md` | Update `frgn` grammar: required `from`, optional `as` before `from`, `inop` keyword removed, `#out` modifier removed |
| `docs/architecture/frgn-export-glue-architecture.md` | New `frgn` semantics (import model), naming convention, wrapper pattern |
| `AGENTS.md` | Update plan directives and anti-patterns to reflect current syntax |
| `docs/features/frgn.md` or similar | Document the `frgn__` convention, fallback patterns, retry wrappers |

---

## Part L: Verification

1. `cargo test --lib` — all 1010+ tests pass
2. `cargo test --test glue_test --test glue_bridge_tests --test fallback_tests` — 44 tests pass
3. `cargo build --release` — no warnings
4. grep for `inop` in `lib/std/` → 0 hits
5. grep for `sig #out` in `lib/std/` → 0 hits
6. grep for `\bfrgn\b.*\b__` in `lib/std/` → only in `as frgn__*` clauses (correct)
7. grep for `#export` in `lib/std/` + `examples/` → 0 hits (historical comments excluded)
8. Compile a `.bv` file that imports from stdlib → succeeds without parser errors

---

## Execution Order

```
1.  src/ast/top.rs          — rename ForeignBinding fields, add effective_briv_name()
2.  src/parser/definitions.rs — restructure parse_frgn_decl, enforce required from
3.  All 14 call sites       — update to new field names and effective_briv_name()
4.  cargo test --lib        — verify no regressions
5.  lib/std/encoding.bv     — fix missing ;
6.  lib/std/ffi/encoding.bv — same fix
7.  lib/std/atomic.bv       — replace 8 inop! with defn + intrinsics
8.  lib/std/syscall.bv      — replace 6 inop! with defn + SysCall#
9.  lib/std/skiplist.bv     — replace 2 inop! with defn + Malloc#/Copy#/Free#
10. lib/std/state.bv        — DELETE
11. lib/std/core/           — DELETE directory
12. examples/bild-asm-target.bv — DELETE
13. lib/std/out.bv          — DELETE (#out obsoleted by metadata)
14. All stdlib BV files     — rename __ → frgn__ (all frgn declarations)
15. All stdlib BV files     — add from clauses to all frgns
16. All stdlib BV files     — add fallback clauses to all frgns
17. lib/std/ffi/string.bv   — split: raw frgns only
18. lib/std/string.bv       — import + wrapper defns only
19. Repeat 17-18 for encoding, time, shm, http, xxhash, metro_bridge
20. All stdlib BV files     — add wrapper defns with retry/passthrough
21. examples/glue-python-bridge/bridge.bv — #export → export defn
22. Documentation update    — SPEC.md, architecture docs, AGENTS.md
23. Verify                  — all tests, grep checks, compile check
24. Commit                  — single commit with comprehensive message
```
