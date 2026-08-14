# Plan: The Iterable Protocol — Schrödinger iteration (op-as-member, two-tier, structural satisfaction)

> **2026-08-14 addendum (docs-reconciliation).** The §11.1 citation at :97 →
> §11.4.1 (the iteration/operator contract section). The Tier-1 row at :103-104
> is superseded by the 4-op cursor contract (`op Iter`+`op Step`+`op IsEnd`+
> `op Current`), which the arch doc now specifies. String unification (§9) is
> real: `#String` is `Iterable<Char>` via a protocol-keyed char decode lane
> (see `docs/plans/2026-08-14-string-unification-and-boundary.md`).

> **2026-08-14 addendum (slice-6 execution).** Slice 5's unconstrained-literal
> diagnostic landed. §10 deletions 1 (ringbuf_inline), 2 (emit_heap_seq/SVO),
> and 3 (hardcoded List foreach arm) are BLOCKED on live paths — documented in
> `BUGS.md` ("Iterable-protocol slice-6 deletions blocked"). The hardcoded
> `List` foreach arm is the live path (tier2 doesn't fire for `List`); `emit_heap_seq`
> serves `Expr::Tuple` + expression-position literals. §10.4/6/7 were already
> done by prior slices.

**2026-08-12.** Implements the normative SPEC §2.1 (types have no canonical
layout), §11.4 (iteration), §15.2/§15.3 (operators/arrows), §16.3
(type-directed literals), §17.1/§17.2 (reflection), and §21.4 (`b-each`) for a
single mechanism: **collections and `String` are ordinary Briv objects whose
iteration resolves structurally through disclosed operator members. The
compiler hardcodes no collection names, no collection layouts, no magic member
names, and no collection intrinsics.**

Design decisions locked 2026-08-12: both tiers in one plan; `foreach` binds the
reference (`&T`); `op At` is new and `op CopyFrom` stays for the `<-` copy-out;
`CharCount#` is a user-callable intrinsic; layer 1 is "Fundamental Intrinsics";
the Tier-2 length op is `op Count`; the stored-length reflection target is
`.^Length` (unabbreviated); **reflection observes and never computes.**

## 1. Motivation

Iteration today is hardcoded and type-name-based, violating SPEC §2.1:

- `foreach` on a `List` reads a baked-in `[len][elem…]` i64 heap layout
  (`emit_stmt.rs:186`, `:1136`), forces the item to `Type::int()`
  (`:1166`), and `panic!`s on any other collection (`:197`).
- `coll[i]` on a `List` reads the same hardcoded layout (`emit_expr.rs:1017`);
  other collections fall through.
- `.^Length` on a collection `panic!`s (`emit_expr.rs:2477`); the runtime
  `.^Size` len-slot path keyed on a magic `name == "len"` slot (`:2389`).
- Web `b-each` renders only inline numeric vectors from layout bytes
  (`web_generator.rs:709`) and skip-warns on everything else
  (`compile.rs:452`).
- `ringbuf_inline` (`context.rs:75`), `emit_heap_seq`/`emit_svo_list`/
  `emit_svo_index` (`emit_expr.rs:1573/1627/1686`), and ~20 `"List"` string
  matches across `src/` are the same disease.

Every one of these is the compiler knowing what a collection *is* and how it
is *laid out*. Schrödinger typing (§2.1) forbids it: layout is selected from
operations, targets, metadata, and observed access — never baked in.

## 2. The layer model

```
┌─────────────────────────────────────────────────────────────────────────┐
│ 1. Fundamental Intrinsics (hardware axioms + value-category semantic    │
│    operations)                                                          │
│    Malloc#, Free#, Copy#, Index#, Ptr#, Deref#, Load#, Store#,          │
│    Sqrt#, CharCount# (new), ...                                         │
├─────────────────────────────────────────────────────────────────────────┤
│ 2. Syntactic Operation Identities (disclosed, op-as-member)            │
│    op Count, op At, op Iter, op Step, op InsertAt,                      │
│    op ExtractFrom, op CopyFrom, op Init                                 │
├─────────────────────────────────────────────────────────────────────────┤
│ 3. Stdlib Definitions (pure Briv in lib/std/collections.bv, string.bv)  │
│    obj List<T>, obj Stack<T>, obj RingBuffer<T>, obj HashMap<K,V>,      │
│    type String: #String                                                 │
└─────────────────────────────────────────────────────────────────────────┘
```

- The compiler knows layer 1 (intrinsics) and layer 2 (operator names,
  disclosed by the `op` keyword + PascalCase). It knows NOTHING in layer 3: no
  `List`, no `String`, no collection layout, no magic member names.
- Layer 3 is where collections live. When codegen sees `List<T>`, its layout
  materializes **on observation** from the struct field definitions; when it
  sees `foreach x in c`, iteration materializes **on observation** from the
  type's operator surface.
- **No collection intrinsics** (`ElemCount#`, `At#`, `RingPush#`) — a
  collection's count/element access is its own declared behavior; the engine
  trap stays dead.

## 3. Op-as-member (the migration)

The current binding form `op CopyFrom: get(#Rh)` resolves the RHS by matching
a bare member name (`mod.rs:447-452`), which is the same rule-14/18 disease as
`n == "List"`. **The operator IS the member** — no RHS, no second name:

```briv
obj List<T> {
    inner: ListBuffer<T>;
    len: Int;                                      // ordinary slot — inert
    op Count() -> Int { term len; };
    op At(i: Int) -> &T { term inner.data[i]; };
    op InsertAt(v: T) { ... };
    op ExtractFrom() -> T { ... };
    op Init(v: T) { ... };
};
```

- The compiler's vocabulary is ONLY the disclosed operator names — the same
  class as `+`, `[]`, and the existing `op Init` family.
- `#Lh`/`#Rh` operand-category annotations move into the operator's own
  signature (self + params).
- `<-`/arrow dispatch, the typechecker's op-binding resolution, the parser,
  and every stdlib `op X: member(#Y)` declaration migrate to op-as-member.
- Migration is Slice 1, isolated (the biggest blast radius lands first).

## 4. The two-tier iteration contract (structural, no hashword)

**Satisfaction is structural** (SPEC §11.1 of the normative plan): a type with
the iteration ops IS iterable. No `#Iterable`, no conformance clause, no
compiler category — `#Iterable` would reintroduce special-casing.

| Tier | Requires | Loop | Covers |
|---|---|---|---|
| **2 — Random Access** | `op Count() -> Int` + `op At(i: Int) -> &T` | counted `0..Count` loop (vectorizable) | `List`, `Stack`, fixed-width `String`, inline vectors |
| **1 — General** | `op Iter() -> Cursor` + `op Step(cur) -> Option<&T>` | external stack cursor | `HashMap`, `LinkedList`, streams, variable-width `String` |

- `foreach` / `b-each` pick the best available tier (Tier 2 when both are
  present, else Tier 1).
- `coll[i]` requires `op At` (Tier 2). Absent → **compile error**, never a
  panic, never a skipped render.
- **Tier-1 cursor**: `op Iter()` returns a fresh stack value (the walk state);
  `op Step(cur)` advances it. Re-iterable, reentrant, zero heap. A `LinkedList`
  cursor is a `Ptr<Node>`; a `HashMap` cursor is a bucket index — declared in
  stdlib, never in the compiler.

## 5. Borrow semantics

Iteration ops yield `&T` (`Ptr<T>`), not copies. `foreach(item in c)` binds
**the reference**; the body reads through it via the existing Ptr field-access
(`emit_field_access`); `let v = item` copies explicitly. Zero-cost for large or
inline elements; trivial for scalars.

The **web** snapshot materializer still copies (the JS boundary requires it) —
a deliberate per-flush boundary copy via the same `At`, never a per-step
iteration copy.

Borrows into reactive `%State` during iteration must be handled by the
flush/ownership analysis (see §12 Risks).

## 6. Reflection vs intrinsic governance

> **Reflection observes; it never computes.** A property that must be derived
> is an intrinsic (`PascalCase#`), called explicitly. Reflection reads
> stored/descriptor properties only, and only where no declared member reaches
> them. (SPEC §17.3 already governs this: `Absolute`, `BitReverse`,
> `Popcount`, `LeadingZeros`, `TrailingZeros` are intrinsics, not projections.)

- **`.^Length`** is stored-length reflection: the `Data` byte header, the
  `String` byte header, the `Vector` descriptor count. On any other type
  (a `List`, a `HashMap`) it is a **compile error** — the count is
  member-managed (`list.len`) or computed, not intrinsic. It **never routes to
  an intrinsic.**
- **`CharCount#`** is the char-count *operation* — disclosed, user-callable,
  protocol-dispatched (`#String`); it replaces the hidden `briv_char_len`.
  A user who needs char counts calls it explicitly.
- **`Bytes#` is not added** — byte length is stored, so `.^Length` reads it.
- The current `.^Absolute` reflection likewise gives way to `Abs#` (§17.3).

## 7. Element type

- Derived from the generic args: `List<String>` → `String`; `Stack<T,N>` → `T`
  (width args skipped); `HashMap<K,V>` → `V`; `String` → `Char`.
- Exposed as compile-time descriptor reflection `.^^Element` (a read, not a
  computation), and **cross-checked** against the read op's return type (a
  mismatch is a compile error).
- Drives the `foreach` item binding and the web shim's element decode.

## 8. Established syntax → resolution

| Syntax | Resolves via |
|---|---|
| `foreach(item in c)` | tier pick → `op Count`+`op At` counted loop or `op Iter`+`op Step` cursor loop (ops internally, never `.^Length`) |
| `b-each:item="c"` | web snapshot materializer driving the same ops |
| `c[i]` | `op At` (indexed borrow) |
| `c.^Length` | stored-length reflection (Data/String-byte/Vector); compile error elsewhere |
| `c <- x` / `x <- c` | `op InsertAt` / `op ExtractFrom` / `op CopyFrom` (unchanged semantics) |
| `let x: List<Int> = [1, 2, 3]` | type-directed literal → `op Init` + `op InsertAt`; unconstrained `= []` → "type annotation required" |
| view `items.^Length` | the materialized array's stored `.length` (target capability — the web materializes iterables into arrays) |

Read-vs-extract is kept distinct: bracket **read** → `op At` (borrow);
arrow **extract** → `op ExtractFrom`/`op CopyFrom` (value out). The current
`"[]" => "ExtractFrom"` rune conflation (`operators.rs:45`) is resolved by
this split.

## 9. String unification

`type String: #String { };` is a bare protocol member with no fields — the
value is a `[len][bytes]` pointer, everything else derived by the casting
graph (`lib/std/types/bootstrap.bv:78-84`). **String is the template**: it is
`Iterable<Char>` with `#String<UTF8>/<UTF16>/<ASCII>` encoding variants, and
satisfies the iteration contract structurally via encoding-selective tiers:

- `String<ASCII>` (fixed-width): Tier 2 — `op Count` = char count, `op At(i)` =
  O(1) byte load.
- `String<UTF8>` (variable-width): Tier 1 for chars (`op Step` = the 1–4-byte
  decoder → `Char`); Tier 2 for **bytes** via a `.Bytes` view
  (`Slice<U8>`/`Data`, the existing `Slice<T>` fat pointer). `str[i]` by char
  on UTF8 is a compile error — honest, never a silent O(N) surprise.
- `.^Length` = the **stored byte count** (header read). Char count is
  `CharCount#`. The two are never conflated.

String's ad-hoc `is_string_operand` checks and the special string loop
renderers are gradually subsumed by the Iterable dispatch (Slice 6).

## 10. What gets deleted (leak cleanup, Slice 6)

Deletions strictly follow their replacements (Rule 5, additive only):

1. `ringbuf_inline` (`context.rs:75`, `mod.rs:4601/4867`, `emit_stmt.rs:1402`)
   — the "any `op InsertAt` type is a 4-slot ring buffer" heuristic.
2. `emit_heap_seq` / `emit_svo_list` / `emit_svo_index`
   (`emit_expr.rs:1573/1627/1686`) — hardcoded list-literal / SVO layouts.
3. The hardcoded `List` layout paths in `emit_stmt.rs:186` and
   `emit_expr.rs:1017`.
4. The `.^Length` collection panic (`emit_expr.rs:2477`) and the runtime
   `.^Size` `name == "len"` slot heuristic (`:2383-2393`).
5. ~20 `"List"` string matches across `src/typechecker/`, `src/backend/llvm/`,
   `src/ssr.rs`, `src/lexer.rs`.
6. `is_string_operand` special-casing as String's Iterable surface lands.
7. **The `.^Len` → `.^Length` rename** — `resolve_reflect`
   (`typechecker/mod.rs:3125`), `emit_reflection` (`emit_expr.rs:2441`), and
   the web shim's view-length mapping.

## 11. The intrinsic-audit roadmap (follow-up plan, not this one)

Briv already covers most of the platform-agnostic intrinsic surface (atomics,
memory, float math, GPU workgroup, pointer/index). The audit adds, with
stdlib wrappers and software fallbacks per target:

- **Bit manipulation**: `Clz#`, `Ctz#`, `PopCount#`, `Bswap#`, `Rotl#`,
  `Rotr#`, `BitReverse#`.
- **Overflow-checked / wide arithmetic**: `AddOverflow#`, `SubOverflow#`,
  `MulOverflow#`, `MulWide#`, `CarryingAdd#`.
- **Float extras**: `Fma#`, `Copysign#`, `MinNum#`, `MaxNum#`, `Trunc#`,
  `Round#`.
- **Portable SIMD primitives**: splat/extract/insert, vadd/vsub/vmul/vdiv,
  vand/vor/vxor/vandnot, vcmpeq/vcmplt/vcmpgt, shuffle/blend, masked
  load/store.
- **Crypto** (optional): `CRC32#`, AES round.
- **Governance**: intrinsic → stdlib wrapper → software fallback → target-
  capability rejection. Also audit how the existing `Length#`/`Get#`/`Insert#`
  relate to the op-as-member surface.

## 12. Slices (green + committed after each)

1. **Op-as-member migration** — parser/typechecker/arrow-dispatch off the
   `op X: member(#Y)` binding form; stdlib migrates; `op Count`/`op At`/
   `op Iter`/`op Step` land as members. Existing behavior preserved.
2. **Tier 2 native** — `foreach`/`[]` resolve `op Count`+`op At` structurally
   (borrows); the `.^Len` → `.^Length` rename lands; the hardcoded `List`
   paths become dead.
3. **Tier 1 native** — the external cursor contract; `HashMap`/`LinkedList`
   iteration.
4. **Web `b-each`** — analysis produces an `IterablePlan` (tier, element
   type, ops); backend emits a snapshot materializer calling the ops →
   `[len][word…]`; shim decodes by element tag; `.^Length` in views →
   `.length`; `Applied`/`Vector` fields classify structurally (mirror
   `signal_type_for`).
5. **Type-directed literals** — `[1,2,3]` → `op Init`+`op InsertAt`;
   unconstrained-literal diagnostic. `emit_heap_seq`/`emit_svo_list` dead.
6. **Leak cleanup** — delete the Slice-10 list; String's Iterable surface
   subsumes `is_string_operand`.
7. **Tests + SPEC + arch docs + BUGS.md** — the SPEC sections in §13 below,
   the arch doc in §14, regression tests per slice.

## 13. Risks / verifications

- **Borrows into reactive `%State`** during iteration — the flush/ownership
  analysis must handle references into state (verify in Slice 2).
- **String's encoding-dependent char access** is the trickiest stdlib piece
  (verify in Slice 2/3).
- **Op-as-member blast radius** touches the established `<-` syntax — Slice 1
  isolates it.
- **Deletions** must not land before their generic replacements (Rule 5).
- **Backend parity**: `cargo test --lib` green after every slice; the existing
  collection tests (instances, stack/queue, foreach) migrate rather than
  weaken.
- **`Length#` audit**: the existing `Length#` intrinsic must be reconciled with
  the new `.^Length` reflection target (implementation detail, Slice 2).

## 14. SPEC updates (Slice 7, same commit family)

- **§2.1** — extend the no-canonical-layout principle: iteration resolves from
  the operation surface; the compiler holds no collection layout.
- **§11.4 Iteration** — the two-tier contract; structural satisfaction; the
  loop variable binds `&T`; non-iterable → compile error; `foreach` lowering
  uses the ops internally, never `.^Length`.
- **§15.2 Operator classes** — add `op Count`, `op At`, `op Iter`, `op Step`;
  the op-as-member form; `[]` read resolves `op At`.
- **§15.3 Transfer arrows** — resolution wording for op-as-member.
- **§16.3 Literals** — explicit `op Init`+`op InsertAt` lowering;
  unconstrained-literal error.
- **§17.1/17.2 Reflection** — `.^Length` = stored-length reflection only
  (Data/String-byte/Vector); error elsewhere; never routes to an intrinsic;
  `.^^Element` = element type from generic args (descriptor read).
- **§21.4 `b-each`** — any structurally-iterable type; snapshot
  materialization; view `.^Length` reads the materialized array's stored
  `.length`.
- **String note** — `String` is `Iterable<Char>` with encoding variants;
  encoding-selective tiers; `.^Length` = byte count, `CharCount#` = char count.

## 15. Architecture doc (Slice 7, same commit family)

`docs/architecture/iterable-protocol.md` — the design reference for the
webstack work: the layer model, op-as-member, the two-tier contract, borrow
semantics, reflection/intrinsic governance, String unification, the deletion
map (old site → new mechanism), the established-syntax resolution table, and
the intrinsic-audit roadmap. Linked from the reference index in `AGENTS.md`.

## 16. Non-goals (this plan)

- No collection-engine intrinsics (`HeapSeqAppend#`, `RingPush#`, `ElemCount#`,
  `At#`).
- No `#Iterable` category/hashword.
- No re-implementation of SPIR-V/CIRCT backends (they inherit the mechanism
  downstream).
- No Kani/proof-engine iteration-model rewrite (downstream).
- No `b-each` queue-drain semantics (a front-only collection is a compile
  error with guidance; drain iteration is a documented follow-up).
- The intrinsic audit (§11) is a separate follow-up plan.
