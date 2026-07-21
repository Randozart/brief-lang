# Zero-Copy GLUE Bridge — Implementation Plan

## Summary

The GLUE protocol already supports zero-copy for scalar types (`Int` → `i64` in register).
This plan extends zero-copy to composite types (structs, String, heap objects) through
four phases. Every phase is driven entirely by Brief declarations — the user never
touches C, Rust, or Python bridge code.

## Testing Mandate

Every new code path, match arm, and feature in ALL phases MUST have corresponding
unit tests. No exceptions. The existing 1448-test baseline must never regress.

Each phase below lists the specific tests to add. As a general rule:
- **Parser changes**: test both the new syntax AND that old syntax still parses
- **Codegen changes**: compile the new pattern to LLVM IR and verify the IR text
- **Typechecker changes**: test both valid programs (no diagnostics) and invalid
  programs (expected warnings/errors)
- **Regression tests**: for every bug found during implementation, add a test
  that exercises the precise failing pattern before fixing it

Run `cargo test --lib` before every commit. Run `bash benchmarks/build_and_bench.sh
--correctness` before every Phase 1 and Phase 2 commit.

## Constraint: Flat Control Flow

ALL code written or modified in these phases MUST follow the 2-level nesting depth
limit. Arrowhead code (3+ levels of `if let`, `match`, or `?` chains) is forbidden.
Use guard clauses, `?`, early returns, and extracted helpers:

```rust
// Correct:
fn process(x: Option<Value>) -> Option<i64> {
    let val = x?;
    let result = val.as_i64()?;
    if result <= 0 { return None; }
    Some(result)
}

// Forbidden — deeper than 2 levels:
fn process(x: Option<Value>) -> Option<i64> {
    if let Some(val) = x {
        if let Some(result) = val.as_i64() {
            if result > 0 { return Some(result); }
        }
    }
    None
}
```

If a function requires deeper nesting, extract the inner logic into a named helper
function. Every modified file must be reviewed for nesting depth before commit.

---

## Phase 1 — LLVM Struct Signatures (Rust LTO)

### Goal

Emit proper LLVM struct types at the FFI boundary instead of boxing everything to `i64`.
A `struct CBuffer { ptr: Int, len: Int }` parameter becomes `%CBuffer` in the LLVM
function signature, passed by value in registers (or by pointer for large structs).

### What the user writes

```brief
struct CBuffer { ptr: Int, len: Int };
struct RSBuffer { data: Int, size: Int };
meld CBuffer <:> RSBuffer;  // proves compatibility — no codegen needed

#export
defn process(buf: CBuffer) -> Int {
    term buf.ptr + buf.len;
};
```

### What the compiler generates

```llvm
%CBuffer = type { i64, i64 }                    ; ← Phase 1

define i64 @process(ptr %state, %CBuffer %buf)  ; ← NOT boxed to i64
  %r = extractvalue %CBuffer %buf, 0            ; ← zero-cost field access
  %s = extractvalue %CBuffer %buf, 1
  %t = add i64 %r, %s
  ret i64 %t
```

### String falls out naturally

`String` is already a struct conceptually `{ ptr: Ptr<Byte>, len: Int }`. Making it
a proper struct type in Brief's type universe means `llvm_type("String")` returns
`%String = type { i8*, i64 }` automatically.

### Files to modify

| File | Change | Nesting check |
|------|--------|---------------|
| `src/backend/llvm/emit_toplevel.rs` | `fn llvm_type()` — return `%Name` for registered struct types | Function body must flatten type dispatch into a helper with early returns |
| `src/backend/llvm/emit_toplevel.rs` | New `fn declare_struct_types()` — emit `%Foo = type { i64, i8, ... }` in LLVM header, called from `generate()` | Single loop + format, max 2 levels |
| `src/backend/llvm/emit_toplevel.rs` | `fn emit_definition()` — skip `ptrtoint`/boxing for struct-typed params | Extract boxing into `fn box_param()`; struct arm returns early |
| `src/backend/llvm/emit_toplevel.rs` | Export wrapper generation — use same struct types in `define` line | Straight-line format |
| `src/backend/llvm/helpers.rs` | New `fn struct_field_types()` — return field LLVM types for a struct name | Guard clause + match, max 2 levels |
| `src/backend/llvm/context.rs` | `fn llvm_type()` on backend context — same change | Delegate to shared helper in emit_toplevel |
| `src/backend/llvm/expr/projection.rs` | Handle `extractvalue` for by-value struct params (Phase 1b) | Early return if not struct; single match |
| `src/type_universe.rs` | Register String as a struct type if not already | ~5 lines, no nesting concern |

### Unit tests

| Test file | Test name | What it covers |
|-----------|-----------|----------------|
| `src/backend/llvm/tests.rs` | `test_export_struct_param_llvm_type` | Struct param emits `%StructName` in LLVM signature |
| `src/backend/llvm/tests.rs` | `test_export_struct_param_no_box` | Struct param is NOT `ptrtoint`-boxed at entry |
| `src/backend/llvm/tests.rs` | `test_export_struct_field_extractvalue` | Field access on struct param uses `extractvalue` |
| `src/backend/llvm/tests.rs` | `test_export_empty_struct` | `struct Empty {}` param emits `%Empty = type {}` |
| `src/backend/llvm/tests.rs` | `test_export_large_struct_by_ptr` | Struct over 128 bits passed by pointer |
| `src/backend/llvm/tests.rs` | `test_string_struct_llvm_emission` | String → `%String = type { i8*, i64 }` |
| `src/backend/llvm/tests.rs` | `test_intern_struct_no_export_unchanged` | Internal-only structs (not exported) stay boxed |
| `src/type_universe.rs` | `test_string_registered_as_struct` | String appears in struct_types with ptr + len fields |

All tests must verify the emitted LLVM IR text directly (grep for `%StructName`,
`extractvalue`, etc.) — not just that the compiler doesn't crash.

### Benchmark verification

Before Phase 1 work begins:
```
cargo test --lib
bash benchmarks/build_and_bench.sh --correctness
bash benchmarks/build_and_bench.sh --runtime
```

Record results in a table. After Phase 1, re-run all three and verify:
- No correctness regressions
- Runtime benchmarks must NOT regress beyond noise threshold
- Record in the commit message

### Edge cases

- **Empty structs**: `struct Empty {}` → emit `%Empty = type {}`, pass as void
- **Structs larger than 2 registers**: Pass by pointer (`%Struct*`), add `nocapture` to LLVM IR
- **Nested structs**: `struct Outer { inner: Inner, flag: Bool }` → LLVM nested `%Outer = type { %Inner, i8 }`; `extractvalue` handles nesting naturally
- **Phase 1 only applies to `#export` defn**: internal-only structs (never cross FFI) stay boxed — avoids changing non-export code paths

---

## Phase 2 — Meld-Routed GEP Access (Python Heap)

### Goal

Accept an opaque pointer to a foreign GC object (e.g., `PyLongObject*`) at the FFI
boundary. Route field access through meld-specified offsets. Single instruction
per field read — no copies.

### What the user writes

```brief
import { PyLongObject } from "glue/python/layouts.bv";

meld Int <:> PyLongObject {
    Ptr -> ob_digit_0;     // offset 24 in PyLongObject
    Size -> ob_size;       // offset 16
    Bytes -> 4;            // digit width
};

frgn get_raw_pylong(src: PyLongObject) -> PyLongObject ;
// Accepts a ptr to PyLongObject, returns same ptr.
// Bridges don't own the Python heap — they just project onto it.

#export
defn extract_value(p: PyLongObject) -> Int {
    let digits: Int = p.ob_size;        // gep at offset 16
    let first_digit: Int = p.ob_digit_0; // gep at offset 24
    term if digits > 0 { first_digit } else { -first_digit };
};
```

`glue/python/layouts.bv` already declares the struct layouts. The meld route
expressions tell codegen: `Ptr = offset 24`, `Size = offset 16`. Codegen emits:

```llvm
define i64 @extract_value(ptr %state, ptr %p) {
  ; p points to a PyLongObject on the Python heap
  %digits_ptr = getelementptr i8, ptr %p, i64 16   ; meld route: Size -> ob_size @ 16
  %digits = load i64, ptr %digits_ptr
  %digit_ptr = getelementptr i8, ptr %p, i64 24     ; meld route: Ptr -> ob_digit_0 @ 24
  %first_digit_ptr = bitcast ptr %digit_ptr to ptr
  %first_digit = load i32, ptr %first_digit_ptr     ; 4 bytes per digit
  %first_digit_i64 = zext i32 %first_digit to i64
  ; ... conditional negate
  ret i64 %result
}
```

### Files to modify (in order)

| File | Change | Nesting check |
|------|--------|---------------|
| `glue/python/layouts.bv` | Already exists — verify meld routes have explicit offset annotations | N/A (Brief source) |
| `src/backend/llvm/helpers.rs` | `fn try_meld_projection()` — when the source register is an export `ptr` param, compute GEP offset from route expression | Extract offset computation into `fn meld_route_offset()`; guard clause for no-meld |
| `src/backend/llvm/helpers.rs` | `fn emit_route_expression()` — new path: if expr is a struct field, emit `gep base_ptr, 0, field_index` | Flatten: determine route type → dispatch to GEP or identity or intrinsic |
| `src/backend/llvm/helpers.rs` | `fn emit_decay()` — when chimera backing type matches target layout, emit noop | Early return for matching types; only allocate on mismatch |
| `src/backend/llvm/emit_toplevel.rs` | Export param emission for `ptr` types — mark register as chimera with meld backing | ~5 lines after the existing boxing code |
| `src/backend/llvm/emit_toplevel.rs` | `fn emit_library_shim()` — no change needed (already emits ptr-typed exports) | — |
| `src/backend/llvm/mod.rs` | `fn mark_chimera()` — ensure it works for function params (already does) | — |

### Chimera tracking for export params

When an export function takes a `PyLongObject` parameter:
1. The LLVM signature gets `ptr %arg0` (because `PyLongObject` is declared with `type`, not a struct)
2. After entry, the register `%arg0` is marked chimera with `backing_type = "PyLongObject"`
3. Any field access `arg.ob_digit_0` hits `try_meld_projection()` which finds the meld route `Ptr -> ob_digit_0`
4. The route expression `ob_digit_0` is evaluated as a struct field access: GEP at offset 24
5. The load reads directly from the Python heap — zero copy

### Unit tests

| Test file | Test name | What it covers |
|-----------|-----------|----------------|
| `src/backend/llvm/tests.rs` | `test_meld_gep_export_param` | Export `ptr` param marked chimera; field access emits GEP at meld offset |
| `src/backend/llvm/tests.rs` | `test_meld_gep_offset_computation` | Meld route `Ptr -> ob_digit_0` computes correct byte offset |
| `src/backend/llvm/tests.rs` | `test_meld_identity_route_noop` | `Ptr -> Ptr` route emits no instructions |
| `src/backend/llvm/tests.rs` | `test_chimera_noop_decay` | `emit_decay` returns register unchanged when backing matches target |
| `src/backend/llvm/tests.rs` | `test_meld_gep_mutability` | Writing to chimera field emits `store` at meld offset |
| `src/backend/llvm/tests.rs` | `test_export_chimera_param_marking` | After entry, export `PyLongObject` param is in chimera_map |
| `src/backend/llvm/tests.rs` | `test_meld_gep_unknown_route_error` | Field not in meld route produces compiler error |
| `src/backend/llvm/helpers.rs` (test module) | `test_meld_route_offset_intrinsic` | Route with `strlen#` intrinsic delegates to intrinsic path |

LLVM IR verification: grep for `getelementptr`, verify the offset matches the
meld route. Also verify zero `malloc`/`alloca` calls in the export body (no
materialization).

### Edge cases

- **Mutability**: If a chimera value is written to (e.g., `p.ob_digit_0 = 42`), emit a `store` at the meld offset. Brief's mutability rules already track this.
- **GC safety**: Python GC can move objects. The `ptr` must be valid for the duration of the call. The `frgn` declaration attests to this — no copying is safer than pinning.
- **Route with no GEP**: Some meld routes are identity projections (`Ptr -> Ptr`). These emit no instructions.
- **Route with intrinsic**: `Size -> strlen#(Ptr)` evaluates the intrinsic at the projected value — not zero-copy per se, but the correct computation.

---

## Phase 3 — `frgn` Error Enforcement

### Goal

Every `frgn` call site must handle the failure case explicitly. Provide two mechanisms:
1. **Implicit `Result<T, FrgnError>`** — structured, compiler-enforced
2. **`[fail: sentinel]` contract** — for C-ABI functions with sentinel error values

### Syntax

```brief
// Path A — full safety: returns Result<T, FrgnError>
frgn open_file(path: String) -> Int;
// Desugars to: external fn open_file(path: String) -> Result<Int, FrgnError>;

// Path B — C-idiomatic: sentinel value, compiler warns on unchecked use
frgn open_file(path: String) -> Int [fail: -1];
// Compiler tracks that open_file may return -1 on error.
// Warning: "value from fail-tagged frgn 'open_file' used without sentinel check"

// Both can be handled:
let fd = open_file("/tmp/data")?;         // Path A: ? propagates FrgnError
match open_file("/tmp/data") {            // Path A: explicit handling
    Ok(fd) => { ... }
    Err(e) => { eprintln!("{}", e.code); }
};

let fd = open_file("/tmp/data");  // Path B: warning — unchecked
if fd != -1 {                      // Path B: OK — sentinel checked
    // use fd
};
```

### `FrgnError` type

Defined in `glue/bridge-prelude.bv` (or a new `glue/frgn.bv`):

```brief
struct FrgnError {
    code: Int;           // platform errno or custom error code
    source: String;      // the frgn symbol that produced the error
    message: String;     // human-readable diagnostic (if available)
};
```

The compiler never hardcodes `FrgnError` — it's just a struct like any other.
The Result wrapper is constructed via an intrinsic `__wrap_frgn_result(val, errcode, "symbol")`
that the codegen pass for `frgn` calls emits.

### Files to modify

| File | Change | Nesting check |
|------|--------|---------------|
| `src/parser.rs` | Parse `[fail: expr]` annotation on `frgn` declarations — store sentinel expression on `ForeignBinding` | `parse_frgn_binding()`: one extra match arm after contract; extract to `fn parse_frgn_annotations()` |
| `src/ast.rs` | Add `sentinel: Option<Expr>` field to `ForeignBinding` | Straight struct field |
| `src/typechecker.rs` | When a `frgn` has `[fail: ...]`, track that the return value is "may-fail"; warn if used without comparison to sentinel | Extract sentinel tracking into `fn check_frgn_call()` helper |
| `src/typechecker.rs` | When a `frgn` has no `[fail: ...]`, wrap return type in `Result<T, FrgnError>` | Simple type substitution in the resolver |
| `src/backend/llvm/expr/call.rs` | `emit_frgn_call()` — for Path A, emit `__wrap_frgn_result` intrinsic; for Path B, emit sentinel comparison | Two branches with early return |
| `src/backend/llvm/intrinsics.rs` | Add `__wrap_frgn_result` intrinsic — constructs `FrgnError` struct on failure, returns i64 with discriminant | Single function, max 2 levels |
| `glue/bridge-prelude.bv` | Add `FrgnError` struct definition | N/A |
| `lib/std/os/` | Update all OS module `frgn` declarations with appropriate `[fail: sentinel]` annotations | Bulk update, file-by-file |

### Compiler warnings for Path B

The typechecker maintains a set of "may-fail" registers (those returned by a fail-tagged
`frgn`). When a may-fail value is:
- **Used in a comparison**: `if fd == -1` → clears the may-fail flag
- **Assigned to a field or passed to another fn**: propagates the may-fail flag
- **Terminated without checking**: `term fd;` → warning emitted

This is a lightweight taint-tracking pass. Not full dataflow — just one hop. A value
is "checked" if the immediate next use is a comparison against its sentinel.

### Unit tests

| Test file | Test name | What it covers |
|-----------|-----------|----------------|
| `src/parser.rs` | `test_parse_frgn_with_sentinel` | `[fail: -1]` parses and stores sentinel expression |
| `src/parser.rs` | `test_parse_frgn_no_sentinel` | `frgn` without `[fail: ...]` has no sentinel (triggers implicit Result) |
| `src/ast.rs` | `test_frgn_sentinel_field` | `ForeignBinding.sentinel` is `Some` for fail-tagged frgn |
| `src/typechecker.rs` | `test_frgn_sentinel_checked_ok` | Sentinel compared to value — no warning |
| `src/typechecker.rs` | `test_frgn_sentinel_unchecked_warning` | Sentinel value used without check — warning emitted |
| `src/typechecker.rs` | `test_frgn_result_handled` | `match frgn_call() { Ok(v) => ..., Err(e) => ... }` — no warning |
| `src/typechecker.rs` | `test_frgn_result_unhandled_warning` | `let x = frgn_call()` without match — warning emitted |
| `src/typechecker.rs` | `test_frgn_result_propagated` | `frgn_call()?` — propagates, no warning |
| `src/backend/llvm/tests.rs` | `test_frgn_emit_sentinel_check` | LLVM IR for `[fail: -1]` includes comparison + branch |
| `src/backend/llvm/tests.rs` | `test_frgn_emit_result_wrap` | LLVM IR for implicit Result includes `__wrap_frgn_result` |
| `src/backend/llvm/intrinsics.rs` | `test_wrap_frgn_result_ok_path` | `__wrap_frgn_result` with no error returns success discriminant |
| `src/backend/llvm/intrinsics.rs` | `test_wrap_frgn_result_err_path` | `__wrap_frgn_result` with error returns error struct |

Additionally: update ALL existing `frgn` declarations in `lib/std/os/*.bv` with
appropriate `[fail: sentinel]` annotations. These are exercised by the benchmark
suite — if a benchmark breaks, the annotations were applied incorrectly.

---

## Phase 4 — `export` Keyword

### Goal

Promote `#export` from a modifier annotation to a first-class keyword `export`,
mirroring `frgn` syntactically.

### Syntax

`export` is a first-class keyword that can precede `defn` or `txn`. It mirrors
`frgn` syntactically — `frgn` is an incoming external call, `export` is an
outgoing one. Both are part of the function/txn signature, not annotations:

```brief
// ── export defn (pure function) ─────────────────────────────────

// Basic: same symbol name
export defn add(a: Int, b: Int) -> Int { term a + b; };
// LLVM: define i64 @add(ptr %state, i64 %a, i64 %b)

// With explicit foreign symbol name:
export("my_add_api") defn add(a: Int, b: Int) -> Int { term a + b; };
// LLVM: define i64 @my_add_api(ptr %state, i64 %a, i64 %b)

// ── export txn (callable convergent loop — with params) ─────────

// Callable txn that converges: iterates until postcondition
export txn fact_loop(n: Int, acc: Int, i: Int) [i <= n][i > n] -> Int {
    let next: Int = acc * i;
    i = i + 1;
    term next;
};
// LLVM: define i64 @fact_loop(ptr %state, i64 %n, i64 %acc, i64 %i)
// Body emits the convergence loop; returns when postcondition met.
// Foreign caller sees a single call — the loop runs inside.

// With override symbol:
export("compute_factorial") txn fact_loop(n: Int, acc: Int, i: Int) [i <= n][i > n] -> Int {
    let next: Int = acc * i;
    i = i + 1;
    term next;
};

// ── NOT valid: export node (reactive txns are state-internal) ──
//   node tick [x < 100][x == 100] { ... };
// Reactive txns have no well-defined single-entry, single-exit calling
// convention. They react to state changes and may never terminate.
// The compiler MUST reject `export node` with a clear error.

// ── NOT valid: export on non-defn/txn items ────────────────
//   export struct Foo { ... };
//   export const N: Int = 42;
// Only defn and txn have a calling convention at the FFI boundary.
// The compiler MUST reject `export struct` etc. with a clear error.

// ── Symmetry with frgn ─────────────────────────────────────

// Incoming:
frgn  open_file(path: String) -> Int [fail: -1];

// Outgoing:
export defn process(buf: CBuffer) -> Int;

// Both cross the Brief↔foreign boundary. Both participate in the same
// meld-verified ABI dispatch. The compiler handles them identically for
// codegen — the only difference is which side owns the implementation.
```

### Backward compatibility

- `#export` annotation still works (Phase 4 is additive)
- `export defn` and `#export defn` behave identically
- `export txn` and `#export txn` behave identically (if `#export txn` was already supported)
- Migration warning: "`#export` annotation is deprecated; use `export` keyword"
- Remove `#export` annotation support in a later release (after at least one full cycle)

### Unit tests

| Test file | Test name | What it covers |
|-----------|-----------|----------------|
| `src/parser.rs` | `test_parse_export_defn_basic` | `export defn add(...)` parses as defn with `export_name = Some("add")` |
| `src/parser.rs` | `test_parse_export_defn_override` | `export("my_name") defn add(...)` — `export_name = Some("my_name")` |
| `src/parser.rs` | `test_parse_export_txn` | `export txn accum(...) [pre][post] -> Ret { ... }` parses as txn with export flag |
| `src/parser.rs` | `test_parse_export_txn_override` | `export("loop") txn iter(...) [pre][post] -> Ret { ... }` |
| `src/parser.rs` | `test_parse_export_rct_txn_rejected` | `export node tick ...` produces clear error |
| `src/parser.rs` | `test_parse_export_struct_rejected` | `export struct Foo ...` produces clear error |
| `src/parser.rs` | `test_parse_export_const_rejected` | `export const N ...` produces clear error |
| `src/parser.rs` | `test_parse_hash_export_still_works` | `#export defn add(...)` still parses (backward compat) |
| `src/ast.rs` | `test_export_name_field` | `defn.export_name` populated for `export defn`, `None` for regular defn |
| `src/ast.rs` | `test_export_txn_name_field` | `txn.export_name` populated for `export txn`, `None` for regular txn |
| `src/backend/llvm/tests.rs` | `test_emit_export_defn_library` | `export defn add` in library mode emits correct LLVM symbol |
| `src/backend/llvm/tests.rs` | `test_emit_export_defn_override` | `export("my_add") defn add` in library mode emits `@my_add` |
| `src/backend/llvm/tests.rs` | `test_emit_export_txn_library` | `export txn` in library mode emits correct convergence loop in LLVM |
| `src/glue/export.rs` | `test_export_extracts_export_name` | `export defn` appears in bridge-exports.dbvl |
| `src/glue/export.rs` | `test_hash_export_still_extracted` | `#export defn` still appears in bridge-exports.dbvl (backward compat) |

Additionally: verify that ALL existing test programs using `#export` still produce
identical LLVM IR after the change. Use `git diff --word-diff` on the `.ll` outputs
of the existing test suite before vs after the refactor.

### Files to modify

| File | Change | Nesting check |
|------|--------|---------------|
| `src/lexer.rs` (or token definitions) | Add `Token::Export` | Single token variant |
| `src/parser.rs` | In `parse_top_level()`: match `Token::Export` → parse optional `("symbol")` → expect `defn` or `txn` → set export name on the parsed item | Extract into `fn parse_export_defn()` — guard clause for `(` vs `defn` |
| `src/parser.rs` | In `parse_definition()`: accept modifier annotations AND the new export keyword form | `parse_definition()` stays unchanged; the keyword wrapping happens in `parse_top_level` |
| `src/ast.rs` | Remove `SigModifier::Export` — move export_name to `Definition.export_name: Option<String>` and add `export_name: Option<String>` to `Transaction` | Struct field, clean rename; verify all existing match arms removed |
| `src/backend/llvm/emit_toplevel.rs` | Key off `defn.export_name` instead of scanning modifiers for `#export` | Replace `get_export_name()` call with direct field access |
| `src/glue/export.rs` | Same — use `defn.export_name` | Straightforward field access |
| `src/backend/llvm/emit_toplevel.rs` | Handle `txn.export_name` in `emit_callable_txn()` — emit wrapper when present | Extract export wrapper emission into `fn emit_export_wrapper()` — shared by defn and txn |

---

## Verification Per Phase

### Before every commit in any phase

```
cargo test --lib
```

Must pass. No exceptions.

### Phase 1 (LLVM struct signatures)

```
bash benchmarks/build_and_bench.sh --correctness
bash benchmarks/build_and_bench.sh --runtime

# Manual verification:
cd examples/glue-rust-bridge
brief build --no-stdlib --library bridge.bv --out .
grep '%CBuffer' bridge.ll    # Phase 1: struct type declared
grep 'extractvalue' bridge.ll  # Phase 1: field access via extractvalue
```

### Phase 2 (Meld GEP)

```
# Manual verification:
cd examples/glue-python-bridge
brief build --no-stdlib --library bridge.bv --out .
grep 'getelementptr' bridge.ll  # Phase 2: GEP with meld offset
python3 gluerun.py               # End-to-end

# Benchmarks still pass:
bash benchmarks/build_and_bench.sh --correctness
```

### Phase 3 (frgn enforcement)

```
# New tests in src/typechecker.rs:
# - test_frgn_sentinel_checked_ok
# - test_frgn_sentinel_unchecked_warning
# - test_frgn_implicit_result_wrap
# - test_frgn_result_handled

# Verify existing benchmarks compile (they use frgn for IO):
bash benchmarks/build_and_bench.sh --correctness
```

### Phase 4 (export keyword)

```
# Both forms compile identically:
cat > /tmp/test_export.bv << 'EOF'
export defn add(a: Int, b: Int) -> Int { term a + b; };
#export defn add2(a: Int, b: Int) -> Int { term a + b; };
EOF
brief build --no-stdlib --library /tmp/test_export.bv --out /tmp/test_export
grep 'define.*@add' /tmp/test_export/test_export.ll  # Phase 4
grep 'define.*@add2' /tmp/test_export/test_export.ll # Phase 4

# All existing tests still pass:
cargo test --lib
```

---

## Documentation Updates

Every phase must update these docs:

| Doc | Phase | What to add |
|-----|-------|-------------|
| `docs/architecture/features/backend-dispatch.md` | 1 | LLVM struct type emission strategy |
| `docs/architecture/features/meld.md` | 2 | Meld route evaluation → GEP offset documentation |
| `docs/architecture/features/ffi.md` | 3 | `frgn` error enforcement: sentinel vs Result |
| `docs/architecture/features/export.md` | 4 | `export` keyword syntax and semantics |

## Rationale Comments

Every modified code site must have a rationale comment:

```rust
// 2026-07-10: Phase 1 — emit LLVM struct type for struct params
// Pattern: struct CBuffer → %CBuffer = type { i64, i64 }
// Without this, all struct params are opaque i64 and must be copied
// to state fields for access. extractvalue eliminates the copy.
```

Format: `// YYYY-MM-DD: Phase N — <short description of change>`
