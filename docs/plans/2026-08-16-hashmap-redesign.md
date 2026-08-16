# Plan: HashMap redesign — hand-written obj with a full collection op surface

**Date:** 2026-08-16. **Supersedes:** the hashmap.bv side of BUGS.md
("stdlib iterator.bv / hashmap.bv written aspirationally — never compiled").
**Head commit:** `74d7ddac`. **Related:** `2026-08-16-three-track-broaden-accel-pregrow-coll.md`
(Phase 3c deferred hashmap.bv to this dedicated work item).

## 1. Why the old hashmap.bv is unfixable as-is

It was written under the OLD system and depends on six things that do not
exist or are broken:

1. `term {}` — empty-map literal → parse error. Real construction is either the
   `op Init` seed path (`let m: HashMap<K,V> = 0`) or a coll-style literal.
2. `map :> Get(key)` / `:> Contains` / `:> Keys` / `:> Values` — `:>`
   object-property syntax has **zero compiler support** (not parsed anywhere).
3. `map.Count#()` — the old `obj HashMap` had **no `op Count`** (only Tier-1
   cursor ops `op Iter/op Step/op IsEnd/op Current`).
4. `Option<V>` / `Some` / `None` — `lib/std/option.bv` itself fails to parse
   (`uni opt(...)` syntax error); the enum-value system is not implemented
   end-to-end. Not usable.
5. `result = result + [...]` — List concat `+` has **no op** on List.
6. Runtime: `let m: HashMap<Int,Int> = 0` state init stored the LITERAL `0`
   (never called `op Init`), so `m.insert`/`m.get` dereferenced a NULL
   handle → segfault.

## 2. Verification: the NEW system needs NO special casing

Every collection-machinery dispatch keyed on **declared ops in
`obj_members`/`operator_defs`**, never on the `coll` keyword or a type name
(verified in source):

| Feature | Dispatch gate |
|---|---|
| `foreach` Tier-2 | `op Count` + `op At` in `obj_members` (`tier2_op_collection`, emit_toplevel.rs:165) |
| `foreach` Tier-1 | `op Iter`+`op Step`+`op IsEnd`+`op Current` (`tier1_cursor_collection`, emit_toplevel.rs:241) |
| `Count#` | generative op dispatch → `emit_method_call("Count")` (intrinsics.rs:165) |
| `<-` insert | `op InsertAt` binding (`find_insert_strategy`) |
| literal construction | `op Count`+`op At` + `op Init`/`op InitEmpty` (`construct_local_collection`, emit_toplevel.rs:1274) |
| state-field init | `op Init`/`op InitEmpty` in `operator_defs` (`emit_init_op_construction`, emit_toplevel.rs:1028) |

The compiler stays **type-name-agnostic** (rules 14/18): the redesign is a
stdlib-only declaration + a few op-driven general fixes. No match on
"HashMap" anywhere.

## 3. Design: `obj HashMap<K,V>` with the full op surface

Keep the layout (`keys: Ptr<K>`, `vals: Ptr<V>`, `occupied: Ptr<Int>`,
`count: Int`, `cap: Int`) and the hash members. Add the missing op surface:

- **`op Count() -> Int`** — `term count` (element count). Makes `Count#`/
  `len`/`is_empty` fire. (Tier-2 needs `op At` too; WITHOUT `op At`, foreach
  keeps the efficient Tier-1 cursor path — a hash map's slot array is sparse
  and must be scanned with skip-empties, not indexed 0..Count.)
- **Keep Tier-1 cursor ops** (`op Iter`/`op Step`/`op IsEnd`/`op Current`).
- **Keep** `op InsertAt: insert(#Lh,#Rh)` / `op ExtractFrom` / `op CopyFrom` /
  `op Init` — the `<-` insert and seed/literal construction.
- **`op At` deliberately NOT declared** by default: literal construction and
  `m[i]` are OPTIONAL. Declaring `op At` would make `tier1_cursor_collection`
  return None (Tier-2 wins) and foreach would index a sparse slot array
  wrongly. A map literal is opt-in (see §5).

## 4. Work item 1 — fix state-field `op Init` routing

`let m: HashMap<Int,Int> = 0` must call `op Init: init(0)` and store the boxed
handle, not the literal 0. `emit_init_op_construction` (emit_toplevel.rs:1028)
already has the non-List path (line 1214-1231) that passes the seed arg and
stores the boxed addr. Verify why it returned false for the HashMap field
(debug: field briev type + `operator_defs["HashMap"]` presence) and fix.

## 5. Work item 2 — optional literal construction (op-driven, opt-in)

User requirement: literal construction is an OPTIONAL feature, not the
default. Implementation: a `HashMap` that declares `op Init`/`op InitEmpty`/
`op InsertAt` (already declared) gets literal construction through those ops —
the SAME path every `coll obj` uses. "Optional" means: a map author opts in by
declaring `op At` (which also enables `m[i]`), and the typechecker accepts a
map literal only when the construction ops are present (never forced).

Current gates that must be relaxed (both op-driven, no name matching):
- **codegen `construct_local_collection`** (emit_toplevel.rs:1274): the gate
  `has("Count") && has("At")` blocks a map that declares Init/InsertAt but not
  At. Relax to "the type declares the CONSTRUCTION ops" (Init/InitEmpty/
  InsertAt), so literal construction works through them.
- **typechecker literal gate**: `let m: HashMap<K,V> = [...]` must typecheck
  when the target declares the construction ops. The unconstrained-literal
  diagnostic (slice 5) stays for `let x = [1,2,3]`.

## 6. Work item 3 — rewrite hashmap.bv against the real surface

No `:>`, no `Option`, no `term {}`, no concat. API:

- `new_map<K,V>()` → construct via `op Init` seed (`let m: HashMap<K,V> = 0`
  or a `term 0` typed to the map).
- `insert(map, key, value)` → `map <- (key, value)` or `map.insert(key, value)`.
- `get`/`contains_key`/`remove` → member calls; `get` returns `V` (no Option;
  contains_key is the presence test).
- `len`/`is_empty` → `map.Count#()` / `== 0`.
- `keys`/`values`/`iter` → foreach over the map (Tier-1 cursor) collecting
  into Lists via `<-`; reads `kv.key`/`kv.val` on a named Entry struct (NOT a
  tuple — tuple-element reads miscompile today, verified).
- `merge`/`filter` → foreach loops with `<-` insert.
- **Element type**: a named `struct Entry<K,V> { key: K; val: V; }` for
  iteration output — struct elements are the coll scaffold's native element
  type (verified: `List<Entry>`, `<- Entry{..}`, `At#`, `e.val` all work).

## 7. Verification

- `brievc check lib/std/hashmap.bv` clean.
- Consumer runs end-to-end: insert/get/Count#/foreach over a map, keys/values.
- Interpreter parity (rule 4): foreach over a map product, Count#.
- `cargo test --lib` green; no new Praetor diagnostics; benchmarks no
  regression.
- `git grep` for a "HashMap" string match in `src/` → zero (rule 14/18).

## 8. Docs

- BUGS.md: close the hashmap.bv half of the iterator/hashmap entry.
- SPEC §17.1: a HashMap (hand-written obj) uses the op surface; `.^Length`
  stays a compile error on it (no compiler-owned length).
- This plan (shipped markers).

---

## SHIPPED (2026-08-16)

### What landed

- **`obj HashMap<K,V>`** (collections.bv) redesigned around the op surface:
  - `op Count` (occupied count), `op InsertAt: insert(#Lh,#Rh)` (Entry pair),
    `op ExtractFrom`/`op CopyFrom`/`op Init` (seed), Tier-1 cursor ops.
  - `struct Entry<K,V> { key, val }` — struct elements are the machinery's
    native element type (tuple elements miscompile).
  - Linear-probe insert/get/contains/remove using the `done`-flag pattern (a
    `p = cap` sentinel miscompiles in member bodies).
  - Rehash-on-full DEFERRED (a nested foreach-in-if in a txn member body
    segfaults — pre-existing codegen bug).
- **Compiler fixes (all op-driven, no name matching — rule 14/18):**
  1. `is_heap_coll` recognizes a hand-written obj by its op surface
     (operator_defs InsertAt/Count/Iter), not just the `coll` keyword — a
     HashMap state field is a boxed handle, never unpacked columns (was the
     NULL-deref root cause).
  2. `construct_local_collection_seed` — a LOCAL collection with a seed init
     (`let m: HashMap = 0`) routes through `op Init` (was binding the raw
     value → NULL member calls).
  3. `construct_local_collection` gate accepts a type declaring the
     construction ops (Init/InitEmpty/InsertAt) — literal construction is
     opt-in for op-surface collections.
  4. typechecker literal gate accepts a List target whose type declares the
     collection op surface — map literals typecheck when the map declares the
     ops.
- **hashmap.bv** rewritten against the real surface (no `:>`, `Option`,
  `term {}`, concat): `new_map`/`insert`/`get`/`contains_key`/`remove`/`len`/
  `is_empty`/`clear`. `brievc check` clean.
- **hash_ops_idio benchmark** removed from the suite (never worked — written
  against the broken HashMap; the probe-inlined hot loop overflows clang).

### Verified

- `let m: HashMap = 0` (state + local) constructs via op Init → insert/get/
  contains/remove/Count# all work end-to-end (runtime consumers print correct
  values).
- Map literal `let m: HashMap = [...]` typechecks (opt-in).
- 1889 lib tests green; benchmarks 73/73 MATCH; zero new Praetor diagnostics.

### Deferred / known limitations (logged in BUGS.md)

1. **foreach over a HashMap** (Tier-1 cursor) — hits a pre-existing codegen
   bug when the cursor ops' bodies contain their own foreach (register
   cross-contamination, `%t243` undefined). The map iterates via explicit
   member methods instead.
2. **`keys()`/`values()`/`entries()` scans** — a `List<K>` accumulator in a
   generic member fails to typecheck across the import boundary (free-T
   arrow). Deferred.
3. **rehash-on-full** — nested foreach-in-if in a txn member body segfaults.
4. **hash_ops_idio** — probe-inlined hot loop overflows clang's frontend.
5. **`key as Int` hashing** requires K castable to Int (the current map is
   `Int`-keyed; string-keyed needs a hash fn — follow-up).
