# Plan: `coll` — declaration-level Length semantics (compiler-owned length)

**Date:** 2026-08-15. **Head commit:** `87d15860` (the working tree also
carries uncommitted stdlib-cleanup work: closure ABI, fn-param marshaling,
literal-arg construction — the §1 background this plan builds on).
**Supersedes/continues:**
- `docs/plans/2026-08-12-iterable-protocol.md` — the iterable contract (two
  tiers, op-as-member, type-directed literals §16.3, `.^Length` stored-length
  reflection §17.1). This plan is the **declaration-level capstone**: the
  compiler scaffolds the whole surface from a `coll` marker instead of
  requiring hand-written `op Count`/`op At`/`op Init`/`op InsertAt` members.
- `docs/plans/2026-08-14-iterable-slice6-cleanup.md` — the two remaining
  deletions (`emit_heap_seq`, the production-dead `IterKind::List` arm). This
  plan resolves both by construction.
- `docs/plans/2026-08-14-stdlib-cleanup.md` — the adapter typecheck+run work
  (closure ABI, fn-param marshaling, literal-arg construction). This plan
  absorbs its literal-layout fix and generalizes it.
- `docs/plans/2026-08-14-string-unification-and-boundary.md` — the String
  model that defines the two length notions (`.^Length` stored bytes vs
  `CharCount#` chars) that `coll` generalizes to collections.
- `docs/plans/2026-08-13-layout-keywords.md` — the keyword precedent
  (`pack`/`union`/`atomic`): user-facing directive keywords, disclosed, never
  a speed win. `coll` follows the same discipline.

A new agent can execute this with this document alone plus the referenced
files. Read `docs/architecture/iterable-protocol.md` first for the iterable
contract, and the two OPEN BUGS.md entries this plan resolves.

---

## 1. Background: the catch this plan resolves

The stdlib-cleanup and slice-6 plans exposed a design tension, articulated
during design review as **"a list is not a primitive."**

Bootstrap primitives (`Int`, `Float`, `Bool`, `Ptr`, `Void`) are the sole
types the compiler may know. `List` is not one of them, yet the compiler
hardcodes it in 14+ places:

- the typechecker synthesizes `Type::Applied("List", …)` for any `[a, b]`
  literal and adopts empty-list element types (`typechecker/mod.rs:742`);
- the backend matched `n == "List"` for iteration, indexing, mask gather,
  and the stale `[len][elems]` heap layout (`emit_expr.rs`, `emit_stmt.rs`);
- two competing layouts circulated: the stale `emit_heap_seq` `[len][elems]`
  (a bare product) and the canonical `obj List<T>` layout
  `[inner.data@0, inner.cap@1, len@2]` (`lib/std/collections.bv:77-99`).
  The mismatch segfaulted `iter_map_loop` (length read from slot 0 = data
  ptr).

The 2026-08-14 stdlib-cleanup work routed list-literal arguments through the
collection's own ops (`construct_local_collection`, `emit_toplevel.rs:1189`)
so the canonical layout is always produced. But that is a *runtime fix*; the
design tension remains: **the compiler must not hardcode `List`, yet lists
(and every other iterable) need a compiler-scaffolded Length surface.**

### The resolution

Introduce a user-facing directive keyword, **`coll`**, on `obj` and `struct`
declarations. The keyword is the *disclosure* (Golden Rule 2): it declares
"this type has compiler-owned Length semantics; scaffold the surface." The
compiler knows the **capability**, never a type name — `coll obj List<T>` and
`coll obj MyQueue<T>` get identical treatment. This is the same pattern as
`pack`/`union`/`atomic` (layout keywords): a disclosed, non-acceleration
keyword.

The compiler owns the **length** as a hidden property:

- it is **not** a declared field (`no `len: Int` slot to write);
- it is **not** accessible to user-written mutation members;
- it is exposed **only** through the two length accessors (below).

### Why the name `coll` (and not `iter`)

`iter` was the candidate, but it already collides in the language:

- `defn iter<K,V>(map: HashMap<K,V>) -> List<(K,V)>` — `lib/std/hashmap.bv:49`;
- `defn iter<T>(set: HashSet<T>) -> List<T>` — `lib/std/hashset.bv:25`;
- the Tier-1 cursor operation `op Iter()` — `lib/std/collections.bv:167`.

Making `iter` a keyword would break all three (forced renames of stdlib +
any user code). `coll` has zero collisions, matches the short-keyword style
(`obj`, `txn`, `seq`, `pack`, `vol`, `union`, `atomic`), and names the full
surface (length + iteration + construction), not just the foreach half.

## 2. Design decisions (locked)

These were settled in design review with the requester:

1. **Keyword: `coll`**, a prefix on `obj` and `struct` declarations
   (`coll obj List<T> { … }`, `coll struct Fixed<T, N> { … }`). It is the
   **native strategy keyword for declaring collections** — convenient (write
   the storage shape, the compiler owns the rest) and as fast as the
   compiler can make it (the scaffolded ops fold to hand-written-equivalent
   code). A user-facing directive keyword — no `#`, no `!`, ordinary
   disclosure. **Never makes code faster** (Golden Rule 2): the default path
   stays the efficient path.
2. **Length is a compiler property.** No `len` field is declared or
   writable. Only the two accessors below can read it. The compiler
   scaffolds the ops that *maintain* it.
3. **Two length notions** (the String model, spec:1043-1045):
   - `.^Length` (reflection) = **stored/absolute length** — the hidden
     slot value, O(1) header read (`coll obj`); the fixed constant N
     (`coll struct`, folded — see §4.3).
   - `Count#` (intrinsic, `op Count`) = **number of elements**.
   They coincide when the stored unit is the element (the common case) and
   diverge exactly like String's stored-bytes vs char-count.
4. **Storage** (most efficient, per requester):
   - `coll obj` — the compiler **appends two hidden trailing `i64` slots**
     (`cap`, `len`) to the layout. For `List` this reproduces the current
     canonical layout `[inner.data, inner.cap, len]` byte-for-byte — zero
     migration, O(1) length + capacity, existing `op Count`/`op At` readers
     keep working.
   - `coll struct` — **fixed `T[N]` only in this slice** (ambiguity #1):
     length == capacity == N, a compile-time constant, no appended slot, C
     ABI preserved. A `Ptr<T>`-backed `coll struct` (needs a length-prefix
     buffer convention) is a documented follow-up, out of scope.
5. **Scaffold scope: Length + iteration surface.** The compiler generates:
   `op Count`, `op At(i)`, `.^Length`, `Count#`, literal construction, and
   `foreach` lowering — from the type's **one sequence member** (the single
   `Ptr<T>`/`T[N]` member), never from a name.
6. **Mutation is scaffolded too** (consequence of #2). Since user members
   cannot write the hidden length, the compiler owns the mutation ops that
   maintain it: `op InitEmpty`, `op Init`, `op InsertAt` (push), `op
   ExtractFrom` (pop). Growth policy is compiler-owned but overridable
   (§3.6).
7. **Capacity is compiler-owned, like length.** Both `len` and `cap` are
   hidden trailing slots (`coll obj`). Neither is a declared field; neither
   is writable by user members. The only interface is the capacity
   intrinsics (§3.6).
8. **`op Grow` / `op Shrink` are disclosed strategy bindings** — the same
   mechanism where `HashMap` overrides `op InsertAt: insert(#Lh, #Rh)`
   (`collections.bv:157`). A `coll` type may bind its own Grow/Shrink
   (`op Grow: grow(#Lh)` — handle only, ambiguity #2; the body computes the
   new cap from `Capacity#` and applies it via `Resize#`); resolution rides
   the existing `operator_defs` → `find_*_strategy` → `emit_strategy_fn_call`
   path (`emit_stmt.rs:1793`), no new dispatch machinery. This is the
   extension hook for the "neat tricks": a hash map's custom Grow
   rehashes-and-expands, a custom Shrink rehashes-down — the compiler knows
   nothing about hashing.
9. **Four capacity intrinsics** (all map to compiler strategies, mirroring
   `Malloc#`/`Copy#`): `Capacity#(h)`, `Resize#(h, cap)`, `EnsureCap#(h, n)`,
   `TrimCap#(h)`. These are how a custom Grow/Shrink body reads and sets
   capacity "without setting a property" (§3.6).
10. **No `.^Capacity` reflection.** Capacity has ONE notion (no
    stored-vs-elements split, unlike length), and it is operational (the
    control knob for Grow/Shrink). Reflection is "observes and never
    computes" for non-operational metadata (§4); capacity is not that. One
    surface: `Capacity#` intrinsic. (General reflection rule, §4.)
 11. **Auto-trigger** — the scaffolded `op InsertAt` calls Grow when
     `len == cap` (before the write); the scaffolded `op ExtractFrom` calls
     Shrink when `len < cap / 2` (after the read). Same triggers as today's
     `push`/`pop` preconditions. **`coll obj` only** — a `coll struct`
     (fixed `T[N]`) has no Grow/Shrink; InsertAt past N is a precondition
     error (ambiguity #3).

> **2026-08-15 addendum (storage is the compiler's choice; `seq coll`
> guarantees contiguity).** "Data should be handled in the most effective
> way possible" — a `coll` declaration is a promise the compiler optimizes,
> not a fixed storage strategy. The compiler picks the most effective
> representation for each coll from its shape and use; **`seq coll` adds one
> hard constraint: the elements sit in a single contiguous memory block.**
> Decisions locked in review:
> 1. **`seq coll`** = the collection's ELEMENTS live in one contiguous block
>    (the element store is one allocation; the `[data, cap, len]` header may
>    be separate). For a `Ptr<T>` coll the data buffer IS one block, so `seq`
>    is a hard guarantee of what the shape already gives; for a fixed `T[N]`
>    coll `seq` forbids the columnar/pooled layout (inline array only).
> 2. **Plain `coll`** = full compiler choice: heap block, pooled columns, or
>    inline array, per shape and use (the "most effective way").
> 3. **Pooling rule** — a coll may use the instance-pool representation
>    (unpacked `base.member` columns) ONLY when it is fixed-size (`T[N]`) AND
>    zero/literal-initialized as a named instance. Growable `Ptr<T>` colls
>    always heap (mirrors the existing List-vs-Stack split, mod.rs:4852-4882;
>    the value must outlive the creating scope — emit_toplevel.rs:1215).
>
> **Storage matrix (what the compiler chooses):**
>
> | coll form | Element store | Default best | `seq` adds |
> |---|---|---|---|
> | `coll obj` + `Ptr<T>` | one malloc'd block | heap `[data,cap,len]` | hard guarantee (already contiguous) |
> | `coll obj` + `T[N]` | inline array | inline OR pooled columns (fixed) | force inline, forbid columns |
> | `coll struct` + `T[N]` | inline array | inline (C ABI) | already satisfied — no-op guarantee |
>
> **Implementation consequence:** the blunt `coll_types: HashSet<String>`
> exclusion (all colls never pool) is replaced by a shape-aware
> `coll_storage: HashMap<String, CollStorage>` with
> `CollStorage ∈ { HeapGrowable, InlineFixed, Poolable }`, derived from the
> sequence member at registration. `instance_prefix_for` and
> `build_field_index` consult it: `T[N]` colls may unpack (Stack shape);
> `Ptr<T>` colls never do. `seq` on a fixed coll forces inline (skips pool
> columns). `seq coll obj` parses (TypeDef gains a `seq` flag).

## 3. Work items (implementation order)

### 3.1 Lexer + parser + AST: the `coll` keyword

1. **Lexer** — add `Token::Coll` (`#[token("coll")]`) beside `Pack`/`Union`
   (`src/lexer.rs:77-104`); add to the display match and the reserved-keyword
   list (`:686-689`).
2. **Parser** — in `parse_top_level` (`src/parser/definitions.rs:16-43`),
    accept `Some(Token::Coll)` followed by `obj`/`struct`:
    - `coll obj` → `parse_obj_like` with a `coll` flag;
    - `coll struct` → `parse_struct_def` with a `coll` flag;
    - `seq coll obj` / `coll seq obj` → `parse_obj_like` with `coll` + `seq`
      flags (2026-08-15 addendum: `seq coll` forces the contiguous element
      block).
    Mirror the `pack`/`seq` prefix loop already used for structs
    (`:2242-2256`); `coll` is order-independent with `pack`/`seq`
    (`coll pack struct`, `pack coll struct` both valid).
3. **AST** —
    - `TypeDef` (`src/ast/top.rs:975`) gains `pub coll: bool` and
      `pub seq: bool` (2026-08-15 addendum);
    - `StructDef` (`src/ast/top.rs:927-935`) gains `pub coll: bool` beside
      `pack`/`union`;
    - `TypeDefBody` unchanged (the scaffolded ops are synthesized at
      registration, not parsed).
4. **Syntax highlighter / grammar** — `vocab.rs` keywords list
   (`src/vocab.rs:686-689` region) and the SPEC grammar (§2/§8).

### 3.2 Frontend: recognize `coll`, derive the sequence member

The compiler derives the sequence member **structurally** from the type's
declared slots, never from a name:

1. **Typechecker** — a `coll` type is iterable by declaration. Wherever
   iterability is currently probed via `tier2_op_collection`
   (`emit_toplevel.rs:165`) or `tier2_collection_type` (`:215`), a `coll`
   type also answers "yes" (its `op Count`/`op At` are scaffolded — see §3.4,
   so the probe needs no special case; the scaffolded ops make the existing
   structural probe fire).
2. **Element type** — from the sequence member: `Ptr<T>` → `T` (coll obj),
   `T[N]` → `T` (coll struct), nested `inner: ListBuffer<T>` → `T`. This
   replaces the hardcoded "`At` member returns `T`" derivation with a
   structural one (but see §3.4.3 — keep the `op At` return as the single
   source of truth).
3. **Validation errors** — a `coll` type must declare **exactly one**
   sequence member (`Ptr<T>` or `T[N]`, possibly nested one level like
   `inner: ListBuffer<T>`). Zero or two is a helpful compile error, never a
   silent guess. A `coll struct`'s sequence member must be a **`T[N]`
   array** — a `Ptr<T>` member errors this slice (ambiguity #1: fixed
   `T[N]`-only `coll struct`).
4. **Reject user length access** — because the length slot has no name, any
   `x.len` / `.^Length`-on-a-declared-field attempt is naturally a missing
   field error. Add an explicit diagnostic if a `coll` type declares a slot
   named `len` ("the length of a `coll` type is compiler-owned; it is not a
   field"), and likewise a slot named `cap` ("capacity is compiler-owned;
   use the capacity intrinsics").
5. **Grow/Shrink bindings are part of the surface** — a `coll` type may
   declare `op Grow: fn(#Lh)` / `op Shrink: fn(#Lh)` bindings (handle only,
   ambiguity #2; parsed by the existing `parse_op_definition`,
   `definitions.rs:1843`); they are validated like any op binding and merged
   over the defaults (§3.6). A two-arg form `grow(#Lh, #Rh)` is a binding
   error.

### 3.3 Backend layout: append/derive the hidden length + capacity

1. **`coll obj`** — when registering the struct key (`ensure_mono`,
   `emit_toplevel.rs:1511`; `build_field_index`, `mod.rs` obj/struct
   registration), append **two synthetic trailing slots** to
   `struct_types`: `cap` then `len` (plain names — the synthesized member
   bodies `term len` / `data[len]` resolve them through the boxed-self GEP;
   §3.2 rejects user-declared `cap`/`len` fields). For `List` this
   reproduces the current canonical layout
   `[inner.data@0, inner.cap@1, len@2]` byte-for-byte — zero migration,
   O(1) length + capacity. `struct_type_size` (`emit_expr.rs:3106`) and the
   universe registration pick it up automatically.
   **Storage mode** (2026-08-15 addendum): a `Ptr<T>`-backed `coll obj` is
   `HeapGrowable` (never pooled; the value must outlive the creating scope);
   a `T[N]`-backed `coll obj` is `InlineFixed`/`Poolable` by initializer.
   `seq coll obj` forces the contiguous element block (which `Ptr<T>` already
   gives; for `T[N]` it forbids the columnar layout).
2. **`coll struct`** — no append; **fixed `T[N]` only in this slice.** The
   declared data member is a `T[N]` array; length == capacity == N, a
   compile-time constant. **A `Ptr<T>`-backed `coll struct` is out of scope**
   (ambiguity #1): a bare `Ptr<T>` carries no length/capacity metadata, and
   defining a length-prefixed buffer convention (`[len][elems]` or
   `[cap][len][elems]`) is a separate follow-up. C ABI preserved: a
   `coll struct` is exactly its declared `T[N]` field, nothing appended.
   `seq coll struct` is a no-op guarantee (the inline `T[N]` field is already
   contiguous).
3. **Hidden slots in state/init** — the appended slots are zero-initialized
   like any other; the scaffolded `op InitEmpty`/`op Init` write them.
4. **`@ll_empty_list` is DELETED, not resized** (ambiguity #4). The 2-slot
   `{i64,i64}` sentinel (mod.rs:3098) cannot become a shared 3-slot constant
   — a shared sentinel with `cap=0, len=0, data=null` aliases across every
   `[]` user: the first `<-` push on one list would mutate the shared block
   (the same aliasing hazard the repromap fix removed). Every `[]` literal
   constructs via the scaffolded `op InitEmpty`, which **pre-allocates the
   default cap from config** (e.g. `Malloc#(128)`, cap=16, matching today's
   `init_empty`), so the first InsertAt never hits the `data=null` grow path.

### 3.4 Backend scaffold: synthesize the op members

At registration, synthesize the same op members the stdlib currently writes
by hand (`collections.bv:83-93`), so every existing consumer
(`tier2_op_collection`, `construct_local_collection`, `emit_method_call`,
`Count#`/`At#`/`Slice#` generative dispatch, `foreach`) lights up with zero
changes:

1. **`op Count() -> Int`** — `coll obj`: read the hidden trailing len slot.
   `coll struct`: the compile-time constant N.
2. **`op At(i: Int) -> T`** — `term seq[i]` where `seq` is the sequence
   member (deref the Ptr or index the array). The element type is the
   declared element type.
3. **`op InitEmpty` / `op Init` / `op InsertAt` / `op ExtractFrom`** — the
   mutation surface (§3.6). These make type-directed literals
   (`[1,2,3]` → `op Init` + `op InsertAt`, spec §16.3) and `<-` push/pop
   work without `emit_heap_seq`.
4. **`op Grow` / `op Shrink` are STRATEGY entries, not member bodies**
   (ambiguity #5). They are synthesized into `operator_defs` (the same map
   HashMap's `op InsertAt: insert(#Lh, #Rh)` binding populates), resolved
   through `find_*_strategy` → `emit_strategy_fn_call` (`emit_stmt.rs:1793`)
   — NOT through `emit_member_body`. **`coll obj` only** — a `coll struct`
   has no Grow/Shrink (fixed `T[N]`, ambiguity #3). Two forms:
   - **default**: a synthesized strategy when the type declares no binding;
   - **override**: the type's own `op Grow: grow(#Lh)` binding wins
     (the `find_*_strategy` lookup already prefers the declared op).
   **Binding args** (ambiguity #2): Grow/Shrink receive the collection
   handle only — `op Grow: grow(#Lh)`. They do NOT take a target capacity
   (there is no `#Rh`); the body computes the new cap from `Capacity#` and
   applies it via `Resize#`. The `grow(#Lh, #Rh)` example is wrong and is
   not used.
5. **Where to synthesize** — at obj/struct registration time, inject the
   synthesized members into `obj_members[key]` and `operator_defs[base]`
   (the same maps the hand-written members populate). The scaffolded Count/
   At/Init/InsertAt/ExtractFrom bodies are stored as ordinary member bodies
   so `emit_member_body` (`emit_expr.rs:2516`) and
   `emit_init_op_construction` (`emit_toplevel.rs:1019`) run them unchanged —
   **no new codegen path**. Grow/Shrink defaults go into `operator_defs` and
   ride the strategy-call path (above).
6. **`.^Length` reflection** — the runtime `Length` arm
   (`emit_expr.rs:2801`) gains a `coll`-type case: `coll obj` reads the
   hidden slot (O(1)); `coll struct` derives. The typechecker's `Length`
   gate (`typechecker/mod.rs:3370-3401`) admits `coll` types instead of
   erroring on `Type::Applied(..)`.
7. **`Count#`** — already dispatches to `op Count` for any type that
   declares it (`intrinsics.rs:157-170`); with the scaffolded `op Count` it
   works with no change.
8. **Capacity intrinsics** — `Capacity#(h)`, `Resize#(h, cap)`,
   `EnsureCap#(h, n)`, `TrimCap#(h)` added to `vocab.rs` op identities +
   `intrinsics.rs`. They map to compiler strategies over the hidden cap slot;
   `Resize#` is the primitive the default Grow/Shrink bodies and the custom
   binding bodies both call. **No `.^Capacity` reflection** (decision #10).

### 3.5 Delete the stale list paths (unblocks slice-6)

Once `coll` scaffolding produces the canonical layout from every literal:

1. **`emit_heap_seq` (`emit_expr.rs:1763`)** — the stale `[len][elems]`
   layout. Scope after this plan:
   - `Expr::Tuple` (`emit_expr.rs:746`) — a tuple is a **heterogeneous
     product**, NOT a `coll` collection; it still needs a heap-seq path.
     Split the tuple path into `emit_tuple` (keeps the `[len][elems]`
     layout) and remove `Expr::List`'s use of `emit_heap_seq` entirely.
     (This is slice-6 §2.1's required "tuple replacement first".)
   - `Expr::List` (`emit_expr.rs:749`) — expression-position literals route
     through type-directed construction (`op Init`/`op InsertAt`) with the
     expected type from context (the working-tree literal-arg fix already
     does this for defn args; extend it to method args, `term [1,2]`, and
     arrow targets).
2. **`IterKind::List` (`emit_stmt.rs:250,1374,1479`)** — the hardcoded
   `[len][elems]` foreach read. Production-dead already (BUGS.md §2); after
   this plan `tier2_op_collection` fires for every `coll` type, so the arm
   is truly dead. Keep the unit-test path (stdlib-free tests) or migrate the
   tests to load stdlib, then delete the arm.
3. **`emit_svo_list`/`emit_svo_index`** — SVO inline lists are the same
   stale layout family; delete with the same reasoning (slice-6 §10.2).

### 3.6 The mutation model: overridable `op Grow` / `op Shrink` bindings

The requester's ruling — *"Length is a compiler property. Not to be accessed
outside of exposed reflection and intrinsic values"* — forces the compiler to
scaffold mutation. The growth policy is **compiler-owned with a disclosed
override hook**, resolved via the SAME strategy-binding machinery that already
lets `HashMap` override `op InsertAt: insert(#Lh, #Rh)`:

- **The default is scaffolded.** A `coll obj` with a `Ptr<T>` sequence member
  and no `op Grow`/`op Shrink` binding gets the compiler-owned doubling policy:
  `op InitEmpty` allocates a default cap (config, `config/ir-lowering.dbvl`),
  `op InsertAt` doubles when full, `op ExtractFrom` halves when sparse. This
  is what `collections.bv` already does by hand (`init`/`grow`/`push`, :101-140)
  — it becomes a compiler-owned, disclosed generic policy for any `coll` type
  with that shape. Efficient default ⇒ Golden Rule 2 holds (a modifier-beaten
  default is a compiler bug). **The default is a `coll obj` policy only** — a
  `coll struct` (fixed `T[N]`) never grows.
- **A type overrides with a binding** (extension, not a new mechanism):
  ```briev
  coll obj List<T> {
      inner: ListBuffer<T>;
      op Grow: grow(#Lh);     // override default doubling
      op Shrink: shrink(#Lh); // override default shrink
  };
  ```
  The binding takes the collection handle only (`#Lh`, ambiguity #2); the
  body computes the new cap from `Capacity#` and applies it via `Resize#`.
  Resolution: the scaffolded `op InsertAt`/`op ExtractFrom` call the bound
  strategy when present (`operator_defs` lookup → `emit_strategy_fn_call`,
  `emit_stmt.rs:1793`), else the default. **Binding wins.**
- **The compiler knows nothing about hashing/rehashing** — a HashMap's custom
  Grow/Shrink may rehash-and-expand / rehash-down freely; the compiler only
  calls the bound function and maintains the hidden cap+len slots.
- **The four capacity intrinsics** back any custom body (read+write without a
  property, per the ruling):
  - `Capacity#(h)` → current cap (hidden-slot read);
  - `Resize#(h, cap)` → set capacity (realloc-or-copy; the default
    grow/shrink body);
  - `EnsureCap#(h, n)` → grow to at least n (convenience on Resize#);
  - `TrimCap#(h)` → shrink to len (shrink-to-fit convenience).
  These map to compiler strategies (`Malloc#`/`Copy#`-style), NOT to a
  declared `cap` field — capacity stays compiler-owned.
  **Write-intrinsics on a `coll struct` are a compile error** (ambiguity #7):
  a fixed `T[N]` capacity is a compile-time constant, not writable —
  `Resize#`/`EnsureCap#`/`TrimCap#` on it never compile; `Capacity#` reads N.
- **Auto-trigger** (decision #11, `coll obj` only): `op InsertAt` calls Grow
  when `len == cap` before the write; `op ExtractFrom` calls Shrink when
  `len < cap / 2` after the read — the same thresholds as today's
  `push`/`pop` preconditions. **A `coll struct` `op InsertAt` past N is a
  precondition error** (`len < N`, matching today's `push` precondition);
  no Grow exists for it (ambiguity #3).
- **Interpreter arms** (ambiguity #6, rule #4 — the interpreter is the
  reference): the interpreter's collection value is a `Value::Product` (a
  Vec) with no capacity concept. Define parity:
  - `Capacity#(product)` = its field count (a Vec is exact-fit);
  - `Resize#`/`EnsureCap#`/`TrimCap#` on a product are **no-ops** (a Vec
    grows freely; capacity is not observable) — they evaluate args and
    return the handle, matching "capacity is not a declared property".

**Consequences for §3.3/§3.4:** the hidden-slot work appends TWO slots
(`cap` + `len`) for `coll obj`; the scaffold synthesizes `op Count`/`op At`/
`op InitEmpty`/`op Init`/`op InsertAt`/`op ExtractFrom` as member bodies AND
the default `op Grow`/`op Shrink` as `operator_defs` strategy entries;
`operator_defs` merges an override binding over the default (binding wins).
The `init`/`grow`/`push`/`pop` members in `collections.bv` are deleted —
replaced by the scaffold + intrinsic surface. The four capacity intrinsics
get interpreter arms (§3.6, ambiguity #6).

### 3.7 Stdlib migration

1. **`collections.bv` List** becomes:
   ```briev
   struct ListBuffer<T> {
       data: Ptr<T>;            // cap REMOVED — capacity is compiler-owned
   };

   coll obj List<T> {
       inner: ListBuffer<T>;
   };
   ```
   **`ListBuffer` must drop its `cap: Int` field.** The current `List` is
   `[inner.data@0, inner.cap@1, len@2]` (3 slots) because `inner` carries
   `cap`. If `ListBuffer` kept `cap`, the compiler appending hidden cap+len
   would produce 4 slots `[data, cap, hidden_cap, hidden_len]` — breaking
   the byte-identical guarantee. With `cap` removed, the declared part is
   `[data@0]` and the compiler appends `cap@1`, `len@2` → 3 slots,
   byte-identical to today. Every existing `inner.cap` reader/writer in the
   stdlib migrates to the capacity intrinsics.
   The hand-written `len: Int`, `op Count`, `op At`, `op InitEmpty`,
   `op Init`, `op InsertAt`, `op ExtractFrom`, `init`, `grow`, `push`,
   `pop` members are deleted — the compiler scaffolds them. `get(i)`/`size()`
   convenience defns may stay if they only read through `op At`/`op Count`.
2. **`iterator.bv`** — no change needed; it consumes `op Count`/`op At`/
   `op InsertAt`, which the scaffold provides. Re-run the adapter runtime
   checks (the working-tree `iter_map` acceptance).
3. **`Stack<T,N>`/`RingBuffer<T>`** — candidate `coll struct`/`coll obj`
   migrations in a follow-up; **not** in this slice (they have no
   `op Count` today — their count is a `size()` defn; migrating them changes
   their surface, out of scope here).

## 4. Why this composes (not a new mechanism)

Every consumer already probes the op surface:

- `tier2_op_collection` / `tier2_collection_type` — foreach + literal-arg
  routing gate on `has Count && has At` (`emit_toplevel.rs:180-186`);
- `construct_local_collection` — gates on `has("Count") && has("At")`
  (`:1206`);
- `Count#`/`At#`/`Slice#`/`InsertAt#` — generative dispatch to op members
  (`intrinsics.rs:149-171`);
- `.^Length` — one new reflection arm.

Synthesizing `op Count`/`op At`/`op Init`/`op InsertAt` into `obj_members`
makes all of them work **without new dispatch code**. The `coll` keyword is
the only new syntax; everything downstream is existing op-surface machinery.
This is the general, name-free treatment — `coll obj MyQueue<T>` works
identically (the "not just the test case" contract).

### 4.1 Reflection vs intrinsic: the boundary rule

**Reflection (`.^`) = stored/frozen facts that "observe and never compute"**
(plan:41, spec:1019); **intrinsics (`X#`) = operations.** The test is:
*does the value differ from what an intrinsic would give, and is it
non-operational?* If it is a stored fact with a semantic distinction from an
operation (String's stored bytes vs char count), it is reflection. If it is
an operation or an implementation detail of one, it is an intrinsic.

### 4.2 The `coll` property inventory (nothing else needed)

| Property | Surface | Why |
|---|---|---|
| Stored/absolute length | `.^Length` (reflection) | stored header fact; differs from element count for multi-slot elements |
| Element count | `Count#` (intrinsic/op) | an operation (dispatches to `op Count`) |
| Capacity | `Capacity#` (intrinsic) | ONE notion (no stored-vs-elements split); operational — the Grow/Shrink control knob |
| Set capacity | `Resize#`/`EnsureCap#`/`TrimCap#` (intrinsic) | the write side of the same control surface |
| Element type | `.^^Element` (compile-time) | frozen descriptor |
| Category | `.^^Type` (compile-time) | frozen descriptor |
| Shape | `.^^Size`/`.^^Bytes` (compile-time) | frozen descriptor |

**Explicitly not exposed:** is-empty (`Count#() == 0`), load factor
(HashMap-specific defn, not a generic compiler property), growth policy
(compiler/config-owned; the override IS the binding, not a query).

**No `.^Capacity`.** Capacity has one notion and is operational — two
surfaces for the same value is redundant (accidental complexity, Golden
Rule 2). Splitting read→reflection, write→intrinsic fragments the control
surface; `Capacity#` sits with its control siblings.

### 4.3 Open sub-question (one): `.^Length` on `coll struct`

For `coll struct` (fixed `T[N]`), `.^Length` = N, a **compile-time
constant** — arguably `.^^` (compile-time descriptor) domain, not `.^`
(runtime). But mixing surfaces per struct/obj fragments the target.
**Recommendation: `.^Length` for both; the compiler folds the constant case**
(like it folds `.^^Type` today). One target, simpler.

## 5. Tests

### 5.1 Parser/lexer
- `coll obj List<T> { inner: ListBuffer<T>; };` parses; `coll` flag set.
- `coll struct Fixed<T, N> { data: T[N]; };` parses; flag set.
- `coll pack struct`, `pack coll struct` both parse (order-independent).
- `coll` in an expression position is a syntax error (reserved keyword).
- `defn iter` / `op Iter` still parse (collision avoidance verified).

### 5.2 Frontend
- A `coll` type with zero sequence members errors helpfully.
- A `coll` type with two sequence members errors helpfully.
- `coll obj` declaring a `len` slot errors ("compiler-owned").
- `coll obj` declaring a `cap` slot errors ("capacity is compiler-owned").
- `coll struct` with a `Ptr<T>` member errors this slice ("fixed T[N] only"
  — ambiguity #1 resolution).
- `let xs: List<Int> = [1,2,3]` typechecks (element adopted).
- `.^Length` on a `coll` type returns Int; `Count#` on a `coll` type
  returns Int.
- `Resize#`/`EnsureCap#`/`TrimCap#` on a `coll struct` are a compile error
  (fixed capacity not writable — ambiguity #7).
- `op Grow: grow(#Lh, #Rh)` is a binding error — Grow takes the handle only
  (ambiguity #2).

### 5.3 Backend layout
- `coll obj List<Int>` layout = `[inner.data, cap(hidden), len(hidden)]`
  (24 bytes), byte-identical to today.
- `coll struct Fixed<Int, 4> { data: Int[4]; }` — no appended slot;
  `.^Length` = 4 (constant), `Count#` = 4.
- `@ll_empty_list` DELETED; every `[]` constructs via `op InitEmpty`
  (pre-allocated default cap); an empty `coll obj` never dereferences
  slot 0 and never shares a mutable sentinel (ambiguity #4).

### 5.4 End-to-end runtime (the reference checks)
- `iter_map([1,2,3], x -> x * 2)` prints `6` (regression — the working-tree
  acceptance).
- `iter_filter`, `iter_chain`, `iter_fold`, `iter_flatmap` run correctly
  (adapter chain, working-tree suite).
- `foreach x in [1,2,3]` iterates the canonical layout.
- `xs[2]`, `xs.Count#()`, `xs.^Length` all read the hidden slot correctly.
- `&xs <- 4; xs.pop()` maintain the hidden length.
- A **user** `coll obj MyQueue<T>` (non-stdlib) with the same shape runs the
  same adapter chain — proves capability-generic, not name-based.
- Interpreter parity: `Count#`/`At#`/`.^Length` on a product value match
  (`intrinsics.rs:66-90`, `eval.rs:686-694`).

### 5.5 Capacity + Grow/Shrink (the override machinery)
- Default grow: pushing past `cap` doubles; `Capacity#` reflects the new cap.
- Default shrink: popping below `cap / 2` halves; `TrimCap#` shrinks to `len`.
- `Capacity#`/`Resize#`/`EnsureCap#`/`TrimCap#` round-trip (set, read, verify).
- `coll obj MyQueue<T>` with a custom `op Grow: geometric(#Lh)` binding
  (e.g. triple, not double) — the override runs, not the default.
- HashMap-style custom Grow that rehashes on expansion — the compiler calls
  the bound strategy and maintains cap+len, knowing nothing about hashing.
- Auto-trigger at exactly `len == cap` (InsertAt) and `len < cap / 2`
  (ExtractFrom) — boundary tests.
- `coll struct Fixed<Int, 4>` — `.^Length` and `Capacity#` both = 4
  (constant); no hidden slots appended; C ABI verified.
- `coll struct` InsertAt past N is a precondition error (no Grow exists —
  ambiguity #3).
- Interpreter: `Capacity#(product)` = field count; `Resize#`/`EnsureCap#`/
  `TrimCap#` on a product are no-ops (ambiguity #6 parity).

### 5.6 Suite / benchmarks
- `cargo test --lib` green (prior baseline 1856+; no regressions).
- Per-commit checklist: Praetor on changed dirs; Kani where safety-critical.
- Benchmarks (rule 11): baseline table BEFORE changes from a clean
  `cargo build --release` + `bash benchmarks/build_and_bench.sh --runtime`,
  then the same after. Watch `queue_drain`/`stack_push_pop`/`hash_ops`
  (collection benchmarks) — the scaffolded ops must match or beat the
  hand-written ones (LLVM folds the simple member bodies).

## 6. Acceptance criteria

1. `coll obj` / `coll struct` parse, typecheck, and emit.
2. Length AND capacity are compiler-owned: hidden slots (obj) / fixed
   constant (struct), exposed only via `.^Length`, `Count#`, and the
   capacity intrinsics (`Capacity#`/`Resize#`/`EnsureCap#`/`TrimCap#`). No
   `.^Capacity` reflection.
3. `coll struct` is fixed `T[N]`-only; a `Ptr<T>`-backed one errors this
   slice; InsertAt past N is a precondition error; the capacity write
   intrinsics on a `coll struct` are compile errors (ambiguities #1/#3/#7).
4. The stdlib `List` is a `coll obj` with NO hand-written length/count/
   at/insert/grow/shrink members, and every existing consumer (foreach,
   `[]`, `Count#`, `.^Length`, literals, `<-`, adapters) works against the
   scaffolded ops.
5. A non-stdlib `coll obj MyQueue<T>` gets identical treatment (no "List"
   match anywhere on the scaffold path).
6. `op Grow` / `op Shrink` bindings (handle-only, `op Grow: grow(#Lh)`)
   override the scaffolded default (the binding-wins merge), and a custom
   body can call the capacity intrinsics (e.g. a rehashing HashMap grow).
7. The four capacity intrinsics have interpreter arms matching the codegen
   semantics (ambiguity #6 parity).
8. `emit_heap_seq` no longer serves `Expr::List`; `IterKind::List` and
   `emit_svo_list`/`emit_svo_index` are deleted.
9. Full suite green; collection benchmarks MATCH (rule 11/11b).
10. SPEC + arch docs updated in the same commit family as the code.

## 7. Out of scope (future plans)

- **`Stack<T,N>`/`RingBuffer<T>` migration** to `coll` — their count is a
  `size()` defn today, not `op Count`; migrating changes their surface.
- **Tier-1 cursor scaffolding** (`op Iter`/`op Step`/`op IsEnd`/
  `op Current` for `HashMap`) — the `coll` scaffold covers Tier-2
  (Count/At) + mutation; Tier-1 stays hand-written (its iteration needs
  occupancy flags, not a bare sequence).
- **Hashing/load-factor as a generic `coll` property** — a HashMap's load
  factor, occupancy, and rehashing policy are type-specific; the `coll`
  surface provides the Grow/Shrink hook + capacity intrinsics, never a
  hash-aware default. HashMap's own logic stays in stdlib.
- **`Ptr<T>`-backed `coll struct`** — a fixed `coll struct` is `T[N]`-only
  this slice (ambiguity #1); a Ptr-backed one needs a length-prefixed buffer
  convention (`[len][elems]` or `[cap][len][elems]`), a documented follow-up.
- **Protocol-constrained generics** (`<T: #Int>`) — the stdlib-cleanup plan's
  §7; unrelated to `coll`.
- **Value specialization / width-specific codegen** — the erased model stands.
- **`iter` as a keyword** — rejected (collisions, §1); `iter` stays a
  function name and a Tier-1 op name.

## 8. Known file map

- `src/lexer.rs` — add `Token::Coll` (`:77-104` region), display + reserved
  list (`:686-689`).
- `src/parser/definitions.rs` — `parse_top_level` (`:16-43`), `parse_obj_like`
  (`:2143`), `parse_struct_def` (`:2237`).
- `src/ast/top.rs` — `TypeDef` (`:975`), `StructDef` (`:927-935`).
- `src/typechecker/mod.rs` — `.^Length` gate (`:3370-3401`), `Count#` path
  (`:1293`), iterable probes.
- `src/backend/llvm/emit_toplevel.rs` — `tier2_op_collection` (`:165`),
  `tier2_collection_type` (`:215`), `construct_local_collection` (`:1189`),
  `ensure_mono` (`:1511`), `emit_init_op_construction` (`:1019`),
  `emit_init_state` (`:1761`), obj/struct registration (`mod.rs:4790-4870`).
- `src/backend/llvm/emit_expr.rs` — `Expr::List` (`:749`), `Expr::Tuple`
  (`:746`), `emit_heap_seq` (`:1763`), `.^Length` reflection arm (`:2801`),
  `emit_member_body` (`:2516`), `struct_type_size` (`:3106`).
- `src/backend/llvm/mod.rs` — `@ll_empty_list` (`:3098`), `emit_one_closure`
  (`:4355`).
- `src/backend/llvm/intrinsics.rs` — `Count#`/`At#`/`Slice#` generative
  dispatch (`:149-171`); the four capacity intrinsics
  (`Capacity#`/`Resize#`/`EnsureCap#`/`TrimCap#`).
- `src/interpreter/intrinsics.rs` — interpreter arms for the four capacity
  intrinsics (`Capacity#(product)` = field count; the write forms no-op) —
  ambiguity #6 parity.
- `src/vocab.rs` — keyword/op-identity tables (`:255-281`); add
  `Grow`, `Shrink`, `Capacity`, `Resize`, `EnsureCap`, `TrimCap` to
  `operation_identities`.
- `src/backend/llvm/emit_stmt.rs` — strategy resolution
  (`emit_strategy_fn_call` `:1793`, `emit_strategy_member_call` `:1719`);
  `try_emit_tier_iteration` (`:201`), `foreach_collection_kind` (`:239`),
  `IterKind::List` (`:250,1374,1479`).
- `lib/std/collections.bv` — List migration (`:70-142`); `ListBuffer` drops
  `cap`.
- `config/ir-lowering.dbvl` — the default-cap / growth-policy tunables
  (the scaffolded default, §3.6).

## 9. BUGS.md

- Resolve the OPEN entry "stdlib iterator.bv / hashmap.bv written
  aspirationally" (the adapter runtime checks that motivated the literal-layout
  fix) and "Iterable-protocol slice-6 deletions blocked on two live paths"
  (`emit_heap_seq` + hardcoded List foreach arm) — both are resolved by §3.5
  + §3.7.

## 10. Documentation

- `spec/SPEC.md` — §2.1 (no canonical layout → extend: `coll` is the
  disclosure, the compiler scaffolds from the sequence member), §8 (the
  `coll` keyword), §11.4 (iteration resolves through scaffolded ops),
  §15.2 (the `op Grow`/`op Shrink` op classes, handle-only binding),
  §16.3 (type-directed literals — `coll` provides the ops), §17.1
  (`.^Length` on a `coll` type), §17 (the reflection-vs-intrinsic boundary
  rule §4.1; the capacity intrinsics `Capacity#`/`Resize#`/`EnsureCap#`/
  `TrimCap#` are operations, not reflection).
- `docs/architecture/iterable-protocol.md` — the `coll` declaration layer.
- `docs/architecture/layout-keywords.md` (if present) — add `coll`.
- `learn-briev/` tutorial — the `coll` keyword with a `MyQueue` example
  (including a custom `op Grow` override).
- The syntax highlighter (`vocab.rs` keywords + any editor grammar files).
- All rationale comments carry the provenance rules (rule 15): when, why,
  what pattern, how to undo.

---

## 11. Implementation status (2026-08-15, later)

Commits landed on `main` (working tree clean):

- `b97c6739` §3.1 — `coll` keyword: lexer `Token::Coll`, parser
  `coll obj`/`coll struct`/`coll pack struct`/`pack coll struct`, AST
  `TypeDef.coll`/`StructDef.coll`, vocab entry.
- `89177a97` §3.2 — frontend validation: exactly one sequence member
  (`Ptr<T>`/`T[N]`/nested buffer), `coll struct` fixed-`T[N]`, no declared
  `len`/`cap`.
- `da80e51b` §3.3 — backend layout: hidden `cap`+`len` slots appended for
  `coll obj` (canonical `[data, cap, len]`), `coll struct` C ABI preserved.
- `a6589314` §3.4 + storage addendum — `CollStorage {HeapGrowable,
  InlineFixed}` shape-aware classification; `coll obj` fully functional
  (literals, `<-` push, `Count#`, index, `foreach`); synthesized member
  bodies allocate the data buffer. Docs updated in the same family
  (SPEC §8.10, learn-briev, iterable-protocol).
- `37670ad4` + `d895131f` — `seq coll obj` / `coll seq obj`: `TypeDef.seq`.
- `f23fd645` §3.6 — capacity intrinsics `Capacity#`/`Resize#`/
  `EnsureCap#`/`TrimCap#` (+ `__briev_coll_resize` runtime helper),
  typechecker signatures, backend whitelist, interpreter parity arms
  (`Capacity#(product)` = field count; write forms no-op Void).
- `0e1b720b` §3.7a — nested-buffer sequence member (`inner: ListBuffer<T>`,
  the List shape): `seq_access` field-path emission, typechecker slot-map
  derivation, `is_sequence_member_ty` nested recognition.
- `7da7a581` §3.7b — stdlib `List` migrated to `coll obj`; `ListBuffer`
  drops `cap`. Bool-closure zext fix (`result.ty == bool_()`). Adapter chain
  (iter_map/filter/fold/chain) verified.
- `36f98339` §3.5 — SVO deleted (`feature_svo`, `emit_svo_list`,
  `emit_svo_index`, `pack_svo_header`, `svo_max_elements` config,
  `is_vector_like`/`svo_capacity`). `emit_heap_seq` kept for tuples;
  `IterKind::List` kept for stdlib-free unit tests.

**Verified:** 1863 lib tests green; all 37 benchmarks MATCH (List migration
and SVO deletion are performance-neutral). The stdlib-cleanup adapter
acceptance (`iter_map([1,2,3], x->x*2)` etc.) works against the migrated
coll List.

**Deferred (documented in §3.5/§3.7):** `IterKind::List` and the
`Expr::List → emit_heap_seq` fallback remain for stdlib-free unit tests and
untyped expression-position literals; a follow-up can route those through
the coll ops or migrate the tests. `grow`-on-full auto-trigger (plan §3.6)
is a future slice — the scaffold's `push` allocates the default cap (16) but
does not yet auto-grow past it.
