# Plan: Principled `.^Length` semantics + `CharCount#` + the stdlib migration

**2026-08-12.** Resolves the documented-vs-implemented mismatch found in the
post-iteration audit. The user chose the principled path (Option A).

## 1. Problem

`.^Length` on a `String` returns the **UTF8 char count** (`briev_char_len`
scan — `emit_expr.rs:2548`, `eval.rs:683`) — a COMPUTED property. This
violates the reflection governance ("reflection observes, never computes";
SPEC §17.3) and contradicts SPEC §17.1 + the arch doc, which say `.^Length` on
a String is the **stored byte count** (header) and char count belongs to a
`CharCount#` intrinsic. Separately, `.^Length` on a **collection** (List,
Stack, HashMap…) is accepted by the typechecker (`mod.rs:3168`) but PANICS at
codegen (`emit_expr.rs:2573`) — the stdlib's collection-count sites
(`stack.^Length`, `map.^Length`, `list.^Length` in iterator/stack/queue/
skiplist/hashmap/hashset) are dead/aspirational.

## 2. The target semantics

| Receiver | `.^Length` | `CharCount#` |
|---|---|---|
| `String` | **stored byte count** (O(1) header read) | UTF8 char count (the scan) |
| `Data` | byte count (unchanged) | — |
| `Vector` | element count (unchanged, folds) | — |
| collection (List/Stack/HashMap…) | **compile error** (SPEC §17.1 — the count is member-managed, not intrinsic) | — |

## 3. Implementation slices (green + committed after each)

### Slice A — `CharCount#` intrinsic

1. **Registry** (`src/intrinsic_signatures.rs`): `"CharCount#" => Signature {
   name: "CharCount#", parameters: [("s", Type::string())],
   return_kind: Native("Int"), observable: false, variadic: false }` + the
   allowed-intrinsic list.
2. **Backend** (`src/backend/llvm/intrinsics.rs`): `"CharCount#" =>
   emit_char_count` → `call i64 @briev_char_len(ptr <arg>)`, reusing the
   existing `string_ptr`/handle-unboxing path (`emit_expr.rs:2567`).
3. **Interpreter** (`src/interpreter/eval.rs` `execute_intrinsic`): the
   char-count arm (the existing UTF8 count logic).
4. **Typechecker**: flows through the registry (arity/type validation).

### Slice B — `.^Length` semantics

1. **Backend** (`emit_expr.rs:2548`): String arm → `load i64, ptr <handle>`
   (the stored byte header), removing the `briev_char_len` call. The
   `is_semantic_string` arm (`:2567`) similarly becomes the header read.
2. **Interpreter** (`eval.rs:683`): String arm → the stored byte length.
3. **Typechecker** (`mod.rs:3168`): `Type::Applied(..)` and non-String
   `Custom` → a **compile error** ("no intrinsic length — the count is
   member-managed or computed; use `op Count` / `CharCount#`").
4. **Tests**: `test_string_len_and_bytes_reflect` (assert the header load,
   not `briev_char_len`); `len.bv` ("héllo" → `.^Length` = 6, `CharCount#` = 5).

### Slice C — the stdlib migration (the bulk, semantic)

| Class | Sites | Change |
|---|---|---|
| **1. String char-scans** | ~180: `string.bv` (83), `json.bv` (46), `encoding.bv` (26), `soa_reorder.bv` (11), `reader.bv` (9), `string_builder.bv` (4), `char.bv` (1) + iterator's String uses | `s.^Length` → `CharCount#(s)` (scan bounds, char comparisons, contracts over char counts) |
| **2. Collection counts** | ~57: `stack.bv` (6), `queue.bv` (6), `skiplist.bv` (6), `hashmap.bv` (7), `hashset.bv` (7), `iterator.bv` (25) | `c.^Length` → the collection's `op Count` (add `op Count` + `op At` to Stack, RingBuffer, queue, skiplist, hashset — the indexable ones, mirroring List; HashMap keeps its Tier-1 cursor ops) |
| **3. Data/bytes** | ~15: `xxhash.bv`, `shm.bv`, `process.bv`, `metropolitan_ffi.bv` | **unchanged** (`.^Length` = byte count already) |

Each Class-1/2 site is a semantic decision: a String **char scan** uses
`CharCount#`; a String **byte/buffer size** stays `.^Length`.

### Slice D — docs + tests

1. SPEC §17.1/§17.3: verify byte-count `.^Length` + document `CharCount#`.
2. The stale cursor-ops fixes: SPEC §21.4 + arch doc op list + syntax table
   (add `op IsEnd`/`op Current`).
3. The undocumented features: web state exports (`__briev_state_ptr`/
   `__web_boot`/`render_frame` + the `_txn` state-prepend), the
   `__view_items_<field>()` materializer + `[len][word…]` + String decode,
   local-collection construction.
4. `CharCount#` tests (native + interpreter).

## 4. Risks & mitigation

1. **The migration is the risk** — a misclassified `.^Length` site silently
   changes behavior (char scan gets byte counts, or a byte site gets a char
   count). Mitigation: the suite stays green after every slice; the collection
   defns (iterator/stack/queue/etc.) are exercised by compiling them; the
   `string.bv` `len` defn's semantics shift to byte count — check every
   downstream use.
2. **Collection `.^Length` → compile error** breaks the stdlib if any site is
   missed. Mitigation: Slice C lands in the SAME commit as Slice B (the error
   + the migration), so no intermediate broken state.
3. **Adding `op Count`/`op At` to 5 collections** expands the iteration
   surface — a stdlib behavior addition, tested by the suite.

## 5. Non-goals

- No `Bytes#` intrinsic (byte length is stored — `.^Length` reads it).
- No `.^Len` re-addition (the unabbreviated `.^Length` is canonical).
