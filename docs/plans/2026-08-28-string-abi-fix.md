# String ABI Fix — Unboxing the Root Cause

**Date:** 2026-08-28
**Status:** Complete (compiler-side); frgn String-return leak deferred
**Bugs addressed:** 6 Critical from BUGS.md + 1 High

---

## Root Cause

All 6 critical bugs trace to **type information loss at the `ptr ↔ i64` boxing boundary**. A Briev String is a `ptr` in SSA but stored as `i64` in state slots and function params. Several code paths miss the recovery:

1. `adapt_to_i64` early-returns for `reg.ty == Int` without checking if the value is physically a ptr
2. `index_elem_ty` doesn't resolve `Type::Vector(inner, _)`
3. `lookup_field_offset` uses unsubstituted generics
4. String slice detection fails for boxed types

---

## Fix Plan — 4 Phases

### Phase 1: Fix `adapt_to_i64` type recovery
**File:** `src/backend/llvm/helpers.rs:2084-2089`
**Status:** DONE (commit `a84568d5`, emit_expr.rs:533) — identifier resolution arm, not adapt_to_i64. Session 2 added the obj-param arm (inttoptr for boxed Custom/Applied struct handles, spawn-pool row ids excluded).

**Fixes:** Bug #1, partial #5

### Phase 2: Fix `index_elem_ty` Vector resolution
**File:** `src/backend/llvm/emit_expr.rs:1094-1098`
**Status:** DONE (commit `a84568d5`, emit_expr.rs:1109)

**Fixes:** Bug #2

### Phase 3: Fix `lookup_field_offset` mono key resolution
**Status:** DONE (commit `6b9b0efb`). Root cause deeper than planned: `obj_instance_inits` stored the BASE name, so boxed-self lookups resolved the GENERIC layout (`data: T[N]` → type_size 0 → every offset 0). Fix:
- `obj_instance_inits` stores the MONO key (`ensure_mono`)
- `unpacked_instance_prefix` normalizes to the base ONCE — `{base}.{member}` slot keys, spawn_pools, obj_members are all base-keyed (normalizing at each consumer instead caused `Stack<Int,8>.data` misses → undefined `@data` globals)
- `emit_instance_init` normalizes the raw tuple at entry
- `emit_member_body`'s `is_pool_instance` guard uses `pool_base`
- mono struct declarations sanitize to legal LLVM identifiers (`%Stack.Int.8`)

**Fixes:** Bug #3

### Phase 4: Three smaller fixes
- **4a:** String slice detection — `is_semantic_string` (commit `a84568d5`) — **DONE**
- **4b:** List.init — **DONE (commit `8f2b7f6e`)** — the sizing was already fixed in the
  2026-08-16 scaffold (`Malloc#(cap * elem_size)`); the LIVE segfault was
  `emit_init_op_construction`'s empty-literal branch never storing the
  instance-block handle into the State column (first push dereferenced
  garbage). Now stores like the non-empty branch. Verified: 5 pushes,
  foreach sum=15, `.^Length`=5.
- **4c:** Frgn String-return heap leak — **DEFERRED** (Bug #5)

---

## Session 2 additions (2026-08-28, commit `47ae4618`) — std/string end-to-end

The Phase 1-4a fixes made string.bv *type-check*, but the module still could
not *compile*. Three further defects, each exposed by the previous fix:

1. **`dedup_items` first-win shadowing** (`import_resolver.rs`): spliced
   imports are flattened in order, so `len(StringBuilder)`/`to_string(StringBuilder)`
   from string_builder.bv (imported first) shadowed string.bv's own
   `len(String)`/`to_string(Int)` for every std/string importer. Fixed:
   keep the LAST occurrence per (category, name) — local shadows imported,
   lexical-scope semantics. Name-only keys stay: the typechecker's
   `fn_param_types`/`fn_return_types` are name-keyed (one signature per
   callable) and both defs would emit duplicate `@symbols` — overloads per
   se are a language-feature track (call-site signature resolution + backend
   name mangling). string.bv/char.bv internal `to_string(sb)` calls became
   `sb.buffer` (the imported overload no longer survives local shadowing).
2. **Reassigned-let type loss** (`emit_stmt.rs`): the reassigned-let path
   stored the value into the pre-declared alloca but never updated
   `let_binding_types`/`let_original_types` — the pre-declaration's
   provisional Int stuck permanently, so field access on a boxed obj handle
   (`sb.buffer`) panicked (`resolve_obj_key(Int)`). Now updates both maps
   with the declared/inferred type.
3. **Nested lets missed by the pre-declaration scan**
   (`emit_toplevel.rs collect_reassigned_lets`): only top-level lets were
   pre-declared; a let inside `when` that was assigned later demoted its
   alloca at the ASSIGN SITE (inside guard.then) while guard.end read it —
   "Instruction does not dominate all uses". The scan now recurses into
   Guarded/Block/Foreach/SyncBlock/Defer/Mutex/Barrier/Match bodies.
4. **narrow_slice soundness** (`analysis/narrow_slice.rs`): the type-blind
   pass narrowed ANY constant-bounds slice with a non-identifier base to
   the base expression — `mk()[1:3]` on a String-returning fn returned the
   whole string. The backend's Slice arm is type-aware (briev_str_substr
   vs Vector offset-view), so the pass is now a pure walk.

## End-to-end verification (all via `brievc build --backend llvm` + run)

| Program | Result |
|---------|--------|
| `len("hello")` | 5 |
| `"hello"[1:3]` → len | 2 |
| `mk()[1:3]` (non-identifier base) → len | 2 |
| `Stack<Int,8>` init(42)/push(7)/push(9)/pop/len | 92 (= 9·10+2) |
| `List<Int>` 5 pushes + foreach sum + `.^Length` | 155 (= 15·10+5) |

`cargo test --lib`: 1990 green (1 pre-existing failure:
`test_find_inverse_pairs`, protocol_graph harness, not codegen).

## Session 3 additions (2026-08-28, commit `269e1eab`) — CStr FFI boundary (Bug #5 resolved)

Bug #5 ("frgn String-return heap corruption") and the meld CStr→String
length symptom share one root: **no carrier for the two FFI string
conventions**. Briev runtime helpers (briev_rt.c) exchange `[len][bytes]`
blocks; external C speaks NUL-terminated data pointers. The 2026-08-03
boundary design already had the answer — variant boundary types
(`CStr: #String<C_String>` in lib/glue/c.bv) — but three links were
missing and are now wired:

1. **Casting-graph base/variant normalization** (`find_path`): the base of
   a category IS its default variant, so `CStr as String` reaches the
   `C_String → UTF8` edge (cstr_to_briev). Base→base fast path preserved
   (normalization only fires when a variant is involved — Bit→String
   regression caught by the lane-coverage test).
2. **ExtCallDyn link symbols**: the lane's binding names the BRIEV-side
   function; the emitted call must use the C symbol from `frgn_map`
   (`str_to_c` → `briev_str_to_c`).
3. **`axiom` keyword parsed + honored** (SPEC): FFI-backed CastTo/CastFrom
   pairs cannot be symbolically proven (foreign bodies) — they are
   DECLARED trusted via `axiom CastTo(...) = f(#Lh);` and skip the
   round-trip gate. c.bv's C_String pair declares it.

Plus: unsupported-frgn String results yield the empty-string sentinel
(@str.0) instead of null; libc prelude declares (getenv/time/atol/
strlen/realloc/malloc/free) are seeded into the frgn declare dedup set
(duplicate declare = LLVM redefinition error).

**Verified:** `getenv("BRIEV_TEST_VAR")` → CStr → `as String` →
`.^Length` = 5 and 2 for two env values, end-to-end.

**ABI contract (documented):** in a frgn signature, `String` = Briev
block ABI (the compiler's own runtime helpers); `CStr`/`CDouble` = the
plain C ABI; crossing variants emits the graph's delta lanes at the call
boundary and via explicit casts.

## Deferred

- **Phase 4c (Bug #5):** frgn String-return marshalling leaks the malloc'd
  C buffer (no Briev heap header; not arena-tracked). Needs the arena or an
  inline conversion at the boundary.
- **Overloads:** same-name/different-signature callables need call-site
  signature resolution + backend mangling before string_builder's
  `len`/`to_string` can coexist with string.bv's.
