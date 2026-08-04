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

### P2 — The NUL allocator invariant
- The Brief arena allocates String buffers as `len+1` and writes `bytes[len] =
  '\0'`. The type stays `[len][bytes]`; the trailing NUL is an implementation
  invariant (one byte, zero semantic change). SSO literals already
  nul-terminate.
- `str_to_c`/`cstr_to_brief` return in-place pointers (zero-copy) where the
  invariant holds; dwarfdump verifies the emitted layout.
- The whole test suite must stay green (no Brief semantics change).

### P3 — Meld-driven interchangeability
- Register melds on the casting graph; the typechecker accepts melded types
  interchangeably (a `CStr` value usable as a `String` with no explicit `as`).
- `lib/glue/c.bv` declares the boundary melds (`meld C_String <-> String`, …).
- The FFI (header, wrappers, shims) derives which representation to present from
  the meld; with the composite + NUL invariant, the marshalling is zero-copy.

### P4 — Verification + docs
- Python-native round-trip test (import the extension, call exports, assert
  results); existing C-driver tests.
- `docs/architecture/casting-protocol.md` (composite ABI contract +
  meld-composite layouts), `docs/guides/ffi-and-export.md` §9 (native
  extension), BUGS.md, plan results table.

## Cross-Cutting

- Additive-only; `cargo test --lib` before each commit; Praetor on changed dirs.
- The compiler carries only the composite ABI contract + meld declarations;
  every language's vocabulary lives in `config/glue.dbvl`.
- Baseline (rule 11): ctypes 2033 ns/call → target 300–500 ns/call.
