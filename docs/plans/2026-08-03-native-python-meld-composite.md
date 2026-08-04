# Native Python (no ctypes) + Meld-Composite Boundaries

**Date:** 2026-08-03
**Status:** Active plan
**Branch:** `glue-host-callable`
**Related:** `docs/plans/2026-08-03-float-protocol-only-rust-speed.md`, `docs/guides/ffi-and-export.md`

---

## Motivation

Three converging threads:

1. **Native Python speed without ctypes.** Python→Brief via ctypes is 2033 ns/call
   (~1900 ns is ctypes marshalling). A CPython C-extension module gets to
   ~300–500 ns/call — a 5–8× win — independent of String layout.
2. **Meld-composite boundaries.** The author's insight: a meld
   (`meld C_String <-> String`) declares a *composite layout* natively serving
   every melded representation — the "frankenstein natively serving both."
   One buffer: Brief reads `[len][bytes]`, C reads a nul-terminated `char*`,
   Python reads a zero-copy `memoryview` over the same bytes.
3. **Preempt other languages.** Lisp, COBOL, or anything else must be additive —
   a config section, never a compiler change.

## The Composite ABI Contract (the one rule everything honors)

A String/Data composite crosses the boundary as an **i64 handle**; every shim
dereferences it to a **state-owned, stable memory region** `(ptr, len)` with a
NUL invariant. The contracts:

1. **Lifetime = the state's life.** A composite view is valid from creation
   until `__glue_release`. The arena owns the memory; the state is the arena's
   handle; while it lives, String/Data memory is stable (Brief values are
   immutable, the arena doesn't move).
2. **Ownership:** Brief owns; hosts borrow read-only for the state's life; each
   shim *pins* the state so the borrow is safe (Python `memoryview` holds the
   state ref, Rust borrows the state, Node Buffer holds the state ref, C is a
   documented don't-release contract, Lisp's CFFI foreign pointer is stable for
   the state's life).
3. **Mutability is declared by the meld, not per language.** Default read-only
   (Brief Strings are immutable — every language must see them read-only). A
   meld may declare a *mutable* composite region; shims honor the declaration.
4. **No language knowledge in the compiler.** The shim is a *recipe*: per-language
   templates + protocol mappings in `config/glue.dbvl`; the compiler's generic
   renderer fills them from export metadata + meld declarations. Adding Lisp or
   COBOL = a `[lisp]`/`[cobol]` section, nothing else.
5. **The NUL invariant is enforced in one place** (the arena alloc writes the
   trailing `\0`) so every consumer is safe without per-language guarantees.

## Phases

### P1 — Native Python extension (the win, first)
- `brief extension <bridge> python` — config-driven:
  - python target gains `native.*` templates in `config/glue.dbvl`: module
    boilerplate (`PyModuleDef`, `PyInit_<bridge>`), per-export method template,
    per-category parse/build snippets (`#Int` → `l`/`PyLong_FromLongLong`,
    `#Float`/`#C_Double` → `d`/`PyFloat_FromDouble`, String/Data → handle as
    `long`/`PyLong_FromVoidPtr`). Built against `Py_LIMITED_API` for portability.
  - The renderer (existing mustache engine) fills the method bodies from the
    export metadata (param/return ABI categories).
  - Compiles + links against the bridge library + Python → the importable
    `.cpython-312-x86_64-linux-gnu.so`.
- Benchmark: native extension vs ctypes vs C. **Baseline:** Python→Brief ctypes
  2033 ns/call (Python→C 1927). **Target:** ~300–500 ns/call.

  **P1 result (2026-08-03, `feature_hash` count=1000, 200k calls):**
  | path | ns/call | vs |
  |------|---------|----|
  | Python → Brief (ctypes) | 3057 | — |
  | Python → Brief (native ext) | 1297 | **2.4×** |
  | Python → Brief add (ctypes) | 1058 | — |
  | Python → Brief add (native ext) | **179** | **6×** |

  The pure-call overhead is ~179 ns — native CPython-extension speed. The
  compute-heavy case sits at the compute floor (~1080 ns is the FNV-1a work
  itself; the Python method adds only ~217 ns).

### P2 — The NUL allocator invariant
- **Audit result (2026-08-03): the invariant already holds everywhere.**
  - `emit_inline_concat` — `compute_alloc_size` allocates `8+len+1`,
    `emit_null_terminate` writes the NUL.
  - Runtime `brief_cstr_to_brief` / `__int_to_str__` — `malloc(len+9)`,
    `buf[8+len]='\0'`.
  - String literals — global `@str.N = ... c"bytes\00"` (initializer carries
    the NUL).
- The composite layout `[len][bytes][\0]` was already the de-facto format.
- **Change:** `brief_str_to_c` now returns the in-place data pointer
  `(handle + 8)` for heap Strings (zero-copy, the composite) instead of
  malloc'ing a copy. Contract: caller must NOT free; valid for the state's
  life. Fixes the leak in C drivers that never freed the copies.
- Full suite green (1459) + `c_driver_boundary` round-trip green.

### P3 — Meld-driven interchangeability
- **Done (2026-08-03).** `lib/glue/c.bv` declares `meld CStr -> String` — the
  composite declaration. The typechecker admits melded pairs at assignment,
  let-init, call args, constructor slots, and term/return without `as`; the
  boundary marshalling inserts the delta (`cstr_to_brief`/`str_to_c`) at those
  implicit sites (verified in the emitted IR). `TopLevel::Meld` survives
  imports so a boundary module's melds apply to the importing bridge.
- The CStr <-> String composite is asymmetric: String → CStr is zero-copy
  (P2's in-place `str_to_c`); CStr → String wraps (a bare C string has no
  length prefix — inherent).
- `examples/glue-host/boundary.bv` dropped its `as` casts — the meld carries
  the interchangeability.

### P4 — Verification + docs
- Python-native round-trip test (`tests/c_driver_python.rs`, toolchain-
  guarded: imports the extension, calls exports, asserts results — including
  Python str args through the CStr <-> String meld path); existing C-driver
  tests all green (1459 lib + 4 glue).
- `docs/architecture/casting-protocol.md` (composite ABI contract +
  meld-composite layouts), `docs/guides/ffi-and-export.md` §9 (native
  extension), BUGS.md, plan results table.

## Cross-Cutting

- Additive-only; `cargo test --lib` before each commit; Praetor on changed dirs.
- The compiler carries only the composite ABI contract + meld declarations;
  every language's vocabulary lives in `config/glue.dbvl`.
- Baseline (rule 11): ctypes 2033 ns/call → target 300–500 ns/call.
