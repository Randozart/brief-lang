# The Iterable Protocol — Schrödinger Iteration

**2026-08-12.** The architecture reference for iteration, indexing, length,
and `String` in Briev. Read before working on `foreach`, `b-each`, collection
codegen, reflection, or the web snapshot materializer. Plan:
`docs/plans/2026-08-12-iterable-protocol.md`. Spec: §11.4, §15.2/§15.3,
§16.3, §17.1/§17.2, §21.4.

## 1. The principle

SPEC §2.1: *types have no canonical layout*. Physical representation is
selected from operations, target constraints, metadata, and observed access —
never baked into the compiler.

Iteration is the same statement, applied to behavior instead of layout:

> **The compiler holds no collection layout, no collection names, and no magic
> member names.** A type is iterable because it provides the iteration
> operations, structurally — no `#Iterable`, no conformance, no category.

A collection's representation materializes **on observation** from its own
field definitions; iteration materializes **on observation** from its operator
surface. `String`, `List<T>`, `HashMap<K,V>`, and a user's custom collection
are all ordinary types — none is a compiler special case.

## 2. The layer model

```
┌─────────────────────────────────────────────────────────────────────────┐
│ 1. Fundamental Intrinsics (hardware axioms + value-category semantic    │
│    operations)                                                          │
│    Malloc#, Free#, Copy#, Index#, Ptr#, Deref#, Load#, Store#,          │
│    Sqrt#, CharCount#, ...                                               │
├─────────────────────────────────────────────────────────────────────────┤
│ 2. Syntactic Operation Identities (disclosed, op-as-member)            │
│    op Count, op At, op Iter, op Step, op InsertAt,                      │
│    op ExtractFrom, op CopyFrom, op Init                                 │
├─────────────────────────────────────────────────────────────────────────┤
│ 3. Stdlib Definitions (pure Briev)                                      │
│    obj List<T>, obj Stack<T>, obj RingBuffer<T>, obj HashMap<K,V>,      │
│    type String: #String                                                 │
└─────────────────────────────────────────────────────────────────────────┘
```

The compiler knows layers 1–2. It knows nothing in layer 3. Collections are
stdlib; new collections are user code; neither needs compiler knowledge.

## 3. Op-as-member

The operator IS the member. No `op X: member(#Y)` binding RHS, no `#Lh`/`#Rh`
operand-category annotations, no bare user-facing member name resolved by the
compiler.

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

The compiler resolves the operator and inlines the member body. The migration
off the binding form is Slice 1 of the plan.

## 4. The two-tier iteration contract

Satisfaction is **structural** — presence of the ops is iterable-ness.

| Tier | Requires | Loop | Covers |
|---|---|---|---|
| **2 — Random Access** | `op Count() -> Int` + `op At(i: Int) -> &T` | counted `0..Count` loop (vectorizable) | `List`, `Stack`, fixed-width `String`, inline vectors |
| **1 — General** | `op Iter() -> Cursor` + `op Step(cur) -> Option<&T>` | external stack cursor | `HashMap`, `LinkedList`, streams, variable-width `String` |

- `foreach`/`b-each` pick the best available tier.
- `c[i]` requires `op At`; anything absent → **compile error**, never panic,
  never a skipped render.
- Tier-1 cursor is an external stack value (`op Iter()` yields fresh walk
  state; `op Step(cur)` advances) — re-iterable, reentrant, zero heap. A
  `LinkedList` cursor is a `Ptr<Node>`; a `HashMap` cursor is a bucket index.

## 5. Borrow semantics

Iteration ops yield `&T` (`Ptr<T>`), not copies. `foreach(item in c)` binds
**the reference**; the body reads through it (existing Ptr field-access);
`let v = item` copies explicitly. Zero-cost for large/inline elements. The web
materializer copies only at the boundary (per flush, via `At`).

## 6. Reflection vs intrinsic governance

> **Reflection observes; it never computes.** A property that must be derived
> is an intrinsic (`PascalCase#`), called explicitly. Reflection reads
> stored/descriptor properties only, and only where no declared member reaches
> them. (SPEC §17.3.)

| Surface | Meaning | Example |
|---|---|---|
| `.^Length` | **stored** length: `Data` byte header, `String` byte header, `Vector` descriptor count | `str.^Length` → byte count |
| `CharCount#` | **computed** char count (operation intrinsic, protocol-dispatched) | `CharCount#(str)` |
| `op Count` | **element count** (iteration contract) | `foreach`/`b-each` lowering |
| `.^^Element` | element type from the generic args (descriptor read) | `List<String>` → `String` |
| `.^^Ops` | the type's operator surface (descriptor read) | iteration capability |

`.^Length` never routes to an intrinsic. A `List`/`HashMap`/custom `.^Length`
is a compile error — that length is member-managed or computed, not intrinsic.
The current `.^Absolute` reflection likewise gives way to `Abs#` (§17.3).
`Bytes#` is not added — byte length is stored, `.^Length` reads it.

## 7. Element type

Derived from the generic args: `List<String>` → `String`; `Stack<T,N>` → `T`
(width args skipped); `HashMap<K,V>` → `V`; `String` → `Char`. Cross-checked
against the read op's return type. Drives the `foreach` item binding and the
web element decode.

## 8. String unification

`type String: #String { };` is a bare protocol member; the value is a
`[len][bytes]` pointer, layout/encoding derived by the casting graph. `String`
is `Iterable<Char>` with `#String<UTF8>/<UTF16>/<ASCII>` encoding variants:

- **Fixed-width** (ASCII): Tier 2 — `op Count` = char count = byte count,
  `op At(i)` = O(1) byte load.
- **Variable-width** (UTF8): Tier 1 for chars (`op Step` = the 1–4-byte
  decoder → `Char`); Tier 2 on the byte view (`.Bytes`, a `Slice<U8>`/`Data`).
  Character random access by index on UTF8 is a compile error.

`.^Length` on `String` = stored byte count. `CharCount#` = char count. The
`is_string_operand` special-casing is gradually subsumed by the Iterable
dispatch.

## 9. Established syntax → resolution

| Syntax | Resolves via |
|---|---|
| `foreach(item in c)` | tier pick → `op Count`+`op At` counted loop or `op Iter`+`op Step` cursor loop (ops internally, never `.^Length`) |
| `b-each:item="c"` | web snapshot materializer driving the same ops |
| `c[i]` | `op At` (indexed borrow) |
| `c.^Length` | stored-length reflection (Data/String-byte/Vector); error elsewhere |
| `c <- x` / `x <- c` | `op InsertAt` / `op ExtractFrom` / `op CopyFrom` (unchanged) |
| `let x: List<Int> = [1, 2, 3]` | type-directed literal → `op Init` + `op InsertAt`; unconstrained `= []` → "type annotation required" |
| view `items.^Length` | the materialized array's stored `.length` (target capability) |

Read-vs-extract stays distinct: bracket read → `op At` (borrow); arrow extract
→ `op ExtractFrom`/`op CopyFrom` (value out). The `"[]" => "ExtractFrom"`
rune conflation (`src/type_universe/operators.rs:45`) is resolved by this
split.

## 10. Web materialization (`b-each`)

The analysis produces an `IterablePlan` (tier, element type, ops). The backend
emits a snapshot materializer — generated code that drives the same iteration
ops to fill `[len][word…]` in a scratch buffer. The shim reads it by the
element type tag (numbers raw, strings as pointers, objects as handles). View
`.^Length` reads the materialized array's stored `.length`. `Applied`/`Vector`
fields classify structurally (mirror `signal_type_for`).

## 11. Deletion map (old site → new mechanism)

| Old hardcode | Location | Replaced by |
|---|---|---|
| `foreach` `List` layout `[len][elem…]` i64, item forced `Int`, `panic!` | `emit_stmt.rs:186,1136,1166,197` | structural `op Count`+`op At`/`op Iter`+`op Step` loop, borrow-typed item |
| `[]` `List` layout read | `emit_expr.rs:1017` | `op At` dispatch (inline member call) |
| `.^Len` collection panic; runtime `.^Size` `name == "len"` slot heuristic | `emit_expr.rs:2477,2383-2393` | `.^Length` stored-length reflection; `CharCount#`; error elsewhere |
| `ringbuf_inline` ("any `op InsertAt` type is a 4-slot ring") | `context.rs:75`, `mod.rs:4601/4867`, `emit_stmt.rs:1402` | deleted after generic layout-on-observation |
| `emit_heap_seq` / `emit_svo_list` / `emit_svo_index` | `emit_expr.rs:1573/1627/1686` | type-directed literals via `op Init`+`op InsertAt`; generic indexing |
| `is_string_operand` special-casing | throughout | String's Iterable surface |
| ~20 `"List"` string matches | `src/typechecker/`, `src/backend/llvm/`, `src/ssr.rs`, `src/lexer.rs` | structural resolution |
| `.^Len` → `.^Length` rename | `resolve_reflect` (`typechecker/mod.rs:3125`), `emit_reflection` (`emit_expr.rs:2441`), shim | language-wide |

Deletions strictly follow their replacements (Rule 5, additive only).

## 12. The intrinsic-audit roadmap (follow-up plan)

Briev already covers most of the platform-agnostic intrinsic surface (atomics
`Atomic*#`/`Fence#`, memory `Malloc#`/`Free#`/`Copy#`, float `Sqrt#`/`Pow#`,
GPU workgroup, pointer/index `Ptr#`/`Index#`/`Deref#`). The audit adds, with
stdlib wrappers and software fallbacks per target:

- Bit manipulation: `Clz#`, `Ctz#`, `PopCount#`, `Bswap#`, `Rotl#`, `Rotr#`,
  `BitReverse#`.
- Overflow-checked / wide arithmetic: `AddOverflow#`, `SubOverflow#`,
  `MulOverflow#`, `MulWide#`, `CarryingAdd#`.
- Float extras: `Fma#`, `Copysign#`, `MinNum#`, `MaxNum#`, `Trunc#`, `Round#`.
- Portable SIMD primitives: splat/extract/insert, vadd/vsub/vmul/vdiv,
  vand/vor/vxor/vandnot, vcmpeq/vcmplt/vcmpgt, shuffle/blend, masked load/store.
- Crypto (optional): `CRC32#`, AES round.
- Governance: intrinsic → stdlib wrapper → software fallback → target-
  capability rejection. Audit how the existing `Length#`/`Get#`/`Insert#`
  relate to the op-as-member surface.

## 13. Hard constraints

- **No collection intrinsics** (`ElemCount#`, `At#`, `RingPush#`).
- **No `#Iterable`** category or hashword.
- **No magic member names** — the compiler resolves operators, never `len`/
  `get`/`push`-style bare names.
- **Reflection never computes**; computed properties are intrinsics.
- **Never panic or skip** — a non-iterable is a compile error with guidance.
- **Deletions follow replacements.**
