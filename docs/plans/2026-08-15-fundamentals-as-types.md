# Plan: Fundamentals as Types — `Data` root, `Bit<N>`, `Blob`, category-`#` removal

**Date:** 2026-08-15. **Head commit:** `87d15860` (working tree carries
uncommitted stdlib-cleanup + coll-plan work; this plan is design-first,
docs land before code).

**Design decisions locked in review with the requester (2026-08-15):**
- Fundamentals are compiler-native **primordials** — no stdlib
  redeclaration, no overloadable ops.
- `Data` is the **universal reflective floor** — every value can be observed
  as its raw storage (the treat-as-bits view); it is **NOT a supertype** and
  adds no universal inheritance edge to the casting graph.
- `Bit<N>` is the unified bit type at any declared width (`Bit` bare =
  flexible width; `Bit<N>` = exact N). No separate `Bits` concept.
- The byte-buffer type (today `Data`) is renamed **`Blob`**.
- Category hashwords `#Int`/`#Float`/`#String`/`#Bool`/`#Char`/`#Bit`/
  `#Data`/`#Bits` are removed from fundamentals; non-category `#` roles
  (`#Lh`/`#Rh`/`#T`, `Intrinsic#` suffix, `#Link`, `#System`,
  `#String<UTF8>` variants) are preserved.
- `Double` is `type Double: Float` (parent refinement).
- `Blob ⇄ String` is an unchecked lens; `Blob ⇄ scalar` is explicit stdlib
  fns; `Blob ⇄ Bit<N>` is the raw-bytes view.

**Cross-references:**
- `docs/plans/2026-08-15-coll-length-semantics.md` — the `coll` keyword.
  Fundamentals get ops via intrinsic, coll types via scaffold; both expose
  the op surface (§11.4). The `Data` root gives coll types raw-storage
  floors.
- `docs/plans/2026-08-14-string-unification-and-boundary.md` — `#String`→
  `String`, `declared_protocol_of` derivation, `IterKind::String`.
- `docs/plans/2026-08-13-layout-keywords.md` — `pack`/`seq`/`union`/
  `atomic`/`spec` keywords; `Bits<N>` spelling changes to `Bit<N>` here.
- `docs/architecture/bits-thesis.md` — the foundational bit-type document;
  updated to the `Data`/`Bit<N>` hierarchy in the same family.

---

## 1. The hierarchy (the model this plan installs)

```
Data  — the universal reflective floor. Every value is observable as its
        raw storage (the treat-as-bits view); NOT a supertype — "parent" is
        a reflective category, never an inheritance edge.
  │
  ├─ Bit<N>  — the bit representation at any width. Bit bare = flexible
  │            (resolved later); Bit<N> = exact N bits. Every type is
  │            composed of bits (lowercase "bits" = the universal material);
  │            Bit<N> is how you name a run of bits directly.
  ├─ Blob    — the [len][bytes] byte buffer. A length-carrying byte sequence
  │            with no encoding interpretation. Safe, never null.
  ├─ String  — [len][bytes] interpreted as UTF-8. Iterable<Char>.
  ├─ Int / UInt / Float / Bool / Char / Ptr / Void  — the numeric/scalar
  │            fundamentals.
  └─ <user types> — everything else is observable as raw storage through the
                    reflective floor; no implicit `Data` edge.
```

**Reflective floor, not a supertype (2026-08-15 decision).** The tree above
is a *conceptual* floor: every value can be observed/reflected as its raw
storage (the universal treat-as-bits material view). It is **not** an
inheritance hierarchy — no universal `Data` edge is added to the casting
graph, and "parent" is a reflective category, never a supertype relationship.

**Bit ≠ Bits clarified.** "Everything is bits" (lowercase) = every type can
be treated as its constituent bits (the universal `Cast.Bit` membership).
`Bit<N>` is the *most direct representation* of N bits. Multiple bits is just
`Bit<N>` — there is no separate `Bits` type. `Bit` bare is `Bit<0>` (flexible
width, resolved later) — this plan keeps the current `Bits(0)` semantics
under the name `Bit` (decision: option a).

**Data ≠ Blob.** `Data` is the abstract universal reflective floor (raw
storage, width-free) — a reflective category, not a supertype. `Blob` is a
*concrete* `[len][bytes]` buffer, a `Data` member like String. A Blob is
always length-carrying and safe; it is never absence (absence is
`Option::None`, spec §16.3).

**Blob is the safe universal byte-carrier, NOT a null pointer.** Its
defining property is the always-present length; an empty Blob is `len == 0`,
a valid empty value, never a null dereference hazard. The `[len][bytes]`
composite (briev_rt.c:53-115) already eliminated the raw-pointer/null-string
heuristics; Blob is the typed, safe replacement, not a reintroduction of
null semantics.

## 2. Verified current state (facts, not assumptions)

1. **Fundamentals already ARE primordials** — `Int`/`UInt`/`Float`/`String`/
   `Bool`/`Char`/`Double`/`Data`/`Void` pre-seeded in
   `type_universe/mod.rs:120-155` with `Cast.#Int`/`Cast.#Float`/
   `Cast.#String` props. stdlib `type Int: #Int {}` *replaces* them
   (`:115-117`) — pure redeclaration.
2. **`op Add: add(#Lh, #Rh)` on fundamentals is documentation-only** — test
   `colon_form_doc_binding_is_not_elaborated` (typechecker/mod.rs:4561-4575):
   "never rewritten to the undefined `add` symbol." `Int + Int` lowers via
   `get_operator_intrinsic` (protocol-driven).
3. **Fundamentals aren't overloadable** — `get_operator_intrinsic` returns
   None for primordials (typechecker/mod.rs:314); overloading is for user
   types.
4. **Name-free dispatch already holds** — `operand_implements_protocol`
   (typechecker/mod.rs:385-402) checks `Cast.#Int` on the universe-key, NOT
   `t == "Int"`. Rules 14/18 already satisfied.
5. **`Double` is inconsistently declared** — `type Double: #Float`
   (bootstrap.bv:58) is protocol-membership; should be `type Double: Float`
   (parent refinement).
6. **`#Bit` is universal** — every type is-a Bit via `Cast.#Bit`
   (`param_covers` :358-361). `Bit` is the sole hardcoded constant
   (`:95-97`).
7. **`Bits` is a hashword category but NOT a type** — `Bits` in vocab.rs:256
   and `Type::Bits(N)` → category `"Bit"` (operators.rs:128); no `type Bits`
   exists. The physical primitive is `Type::Bits(N)` (ast/types.rs:13-16,
   "The sole physical primitive").
8. **`Data` today is a byte-buffer** — `type Data: Int { spec Bits: 64; }`
   (bootstrap.bv:91), `[len][bytes]`, `#Data` protocol, `is_data_operand`,
   `#b"..."` literals (emit_expr.rs:2047), 124 backend/typechecker/graph
   references, FFI uses (`metropolitan_send`).

## 3. Work items

### 3.1 `Data` becomes the universal reflective floor (NOT a graph root)

2026-08-15 (decision): `Data` is a **reflective floor**, not a supertype.
Every value can be observed/reflected as its raw storage, but **no universal
inheritance or cast edge is added to the casting graph**. "Parent" is a
reflective category, never an inheritance relationship.

1. **No universal `Data` edge.** The casting graph gets no implicit
   every-type→`Data` lane. The universal raw-storage view is the
   **treat-as-bits material view** (`Cast.Bit` stays the universal material
   membership, typechecker/mod.rs:358-361); it is a material property, not a
   supertype edge.
2. **Universe `base`** — primordials keep their own base relationships; there
   is **no** blanket `base: "Data"` rewrite for every type. Only genuinely
   storage-rooted types declare explicit edges (`type Blob: Data`,
   `type String: Data`).
3. **`Bit` becomes the flexible-width `Bit<N>` family** — the "axiomatic
   anchor, no width" (mod.rs:94-97) folds into `Bit<0>`/`Bit<N>`. It keeps
   its constant, non-overloadable status.
4. **`Cast.Bit` stays the universal treat-as-bits view** — the material
   floor. Reflective raw-storage observation rides this view; it is not an
   inheritance edge.

### 3.2 Delete the fundamental redeclarations (bootstrap.bv)

- Remove `type Int: #Int { op Add: ... }`, `type UInt: Int`,
  `type Float: #Float {...}`, `type String: #String { }`,
  `type Bool: #Bool {...}`, `type Char: #Char {...}`.
- The primordials stand alone; op lists move to SPEC/hash-words.md
  documentation (confirmed: rely on SPEC, no comment replacement).
- Verify `--no-stdlib` still type-checks `let x: Int = 5`.

### 3.3 Fix `Double` (and align `Half`/`BFloat`/`Float32`/`Float64`)

- `type Double: Float { spec Bits: 64; }` — parent refinement, protocol via
  parent walk (`declared_protocol_of` :197-208).
- Check siblings in bootstrap.bv/float.bv for the same inconsistency; align
  to the parent form.

### 3.4 Rename byte-buffer `Data` → `Blob`

Frees `Data` for the universal reflective floor. Sweeps:
- `#Data` protocol → `Blob` protocol; `is_data_operand` → `is_blob_operand`
  (helpers.rs:1307-1312); `Cast.#Data` → `Cast.Blob`.
- `#b"..."` literal type (emit_expr.rs:2047-2060, mod.rs:200/549/3073).
- FFI uses: `metropolitan_send`/`read_from_shared`/`Data::from_size`
  (metropolitan_ffi.bv), `write_to_shared`.
- `type Data: Int { spec Bits: 64; }` → `type Blob: Data { spec Bits: 64; }`
  or a `Data`-rooted primordial.
- 124 backend/typechecker/graph references — one atomic commit.

### 3.5 Blob casting rules

| Cast | Rule | IR |
|---|---|---|
| `Blob ⇄ String` | Unchecked zero-copy lens — both `ptr` to `[len][bytes]`; invalid UTF-8 decodes as replacement chars at iteration. | `bitcast ptr to ptr` |
| `Blob ⇄ Bit<N>` | Byte↔bit view (the raw-bytes universal fallback, casting-protocol.md:183). A Blob of length L IS `Bit<8L>`. | construct/view buffer |
| `Blob → Data` | View — Blob is an explicitly Data-rooted type (declared edge; Data is the reflective floor, not a universal parent). | no-op |
| `Data → Blob` | Checked downcast — the Data value must be buffer-shaped. | shape check |
| `Blob ⇄ Bit` (bare) | The flexible-width form; treated as `Bit<8L>` view or explicit-width checked cast. | view/check |
| `Blob ⇄ Int/Float/Bool/Char` | **NO implicit cast.** Explicit stdlib fns: `blob_to_int`, `int_to_blob`, `blob_to_float`, etc. (little-endian, length-checked). | explicit fn call |

**Removes the stale `Data` lanes:** `ExtractData`/`Bitcast` (old `{i64,i64}`
fat-pointer) and all `IntToPtr`/`PtrToInt` pointer-as-int lanes
(graph.rs:185-186, 204-205, 220-221, 255-262, 269-276, 279-285) — replaced
by the table above. The scalar↔pointer reinterpretation hacks die with the
old `type Data: Int` model.

### 3.6 Remove `#` from category hashwords in fundamentals

- **Scope (confirmed):** category hashwords on fundamentals — `#Int`→`Int`,
  `#Float`→`Float`, `#String`→`String`, `#Bool`→`Bool`, `#Char`→`Char`,
  `#Bit`→`Bit`, `#Data`→`Data`, `#Bits`→`Bit<N>`.
- **Preserve (NOT touched):** `#Lh`/`#Rh`/`#T` (strategy markers),
  `Intrinsic#` suffix (`Malloc#`), `#Link<name>`, `#System`, protocol
  variants `#String<UTF8>`.
- `op Add(#Int)` → `op Add(Int)` in op signatures. `param_covers`/
  `variant_covers` keep working: the concrete `Int` form resolves via
  `operand_implements_protocol(operand, "Int")` → universe-key `Cast.Int`.
- **The key change:** the `Cast.#X` property becomes `Cast.X` (or the code
  strips `#`). The universe-key lookup is unchanged; only the property-name
  spelling changes. Grep all `Cast.#X` property references — one atomic
  commit (it's the name-free key the whole dispatch rides on).

### 3.7 Spelling: `Bits<N>` → `Bit<N>`

- `Type::Bits(N)` AST stays, doc updated to "`Bit<N>`".
- Source spelling: `Bits<48>` → `Bit<48>`, `Bits<12>` → `Bit<12>`
  (benchmarks/pack_*.bv, spec §8.2, pack struct examples, learn-briev).
- `spec Bits: 32` stays (it's the metadata key, not the type).
- `Bit` bare = flexible (resolved later); `Bit<N>` = exact.

## 4. The "when to use" guide (for docs)

| You want to… | Use |
|---|---|
| touch individual bits / exact width | `Bit<N>` |
| hold raw bytes, interpret later | `Blob` |
| accept *any* value | `<T: Data>` (the root) |
| passive fixed record | `struct` |
| state + behavior + lifecycle | `obj` |

The overlaps are the design: `Blob` and `Bit<N>` both are bytes, differ in
intent (buffer vs pattern); `struct` and `obj` both carry fields, differ in
behavior (passive vs active); `Data` underlies all four; the casting graph
moves between them with zero ceremony.

## 5. Tests

1. `--no-stdlib` fundamental check (Int/Float/String/Bool/Char all
   type-check bare).
2. `op Add(Int)` on a user type still lowers `MyNum + Int` (cross-type
   overload test at typechecker/mod.rs:4523).
3. `Int + Int` still lowers via intrinsic, not the (now-deleted) binding.
4. `Double + Double` still works via `Float` parent protocol.
5. `variant_covers` with concrete `Int` vs hashword — both cover.
6. `Blob as String` lens (unchecked); `Blob as Bit<64>` view;
   `blob_to_int`/`int_to_blob` round-trip (explicit stdlib fns).
7. `Data` as a usable universal bound (`<T: Data>`); `Blob` is a `Data`
   member.
8. `Bit<N>` spelling parse (`Bit<48>`, `Bit<4>`); `Bit` bare flexible.
9. Full suite green (baseline 1856+).

## 6. Documentation (in the same commit family as the code)

- `spec/SPEC.md` — §2.1 (Data root, Bit<N>, Blob), §8.2 (`Bits<N>`→`Bit<N>`,
  pack struct), §8.5 (`type` — fundamentals need no declaration), §8.10
  (`coll` — Data root), §15.2 (op classes — `op Add(Int)`), §16.3 (no
  null/nil; Blob), §17 (Bit vs Bits vs Blob vs Data).
- `docs/architecture/bits-thesis.md` — the `Data`/`Bit<N>`/`Blob` hierarchy.
- `docs/architecture/hash-words.md` — category-hashword section rewritten
  (`#Int`→`Int` etc.); non-category `#` roles documented as preserved.
- `docs/architecture/agent-reference.md` — fundamentals, `#Float` width
  table (`#Float<Double>`→`Float<Double>`), `#String<UTF8>`→`String<UTF8>`.
- `docs/architecture/iterable-protocol.md` — `coll` + fundamentals both
  expose the op surface; `#String`→`String`.
- `learn-briev/` — 05-data-types.md (`Bits<N>`→`Bit<N>`, the guide),
  06-string.md (`Data`→`Blob`), 15-custom-types.md (`#Int`→`Int`,
  `#Bits`→`Bit<N>`).
- All rationale comments carry provenance (rule 15).

## 7. Acceptance criteria

1. `let x: Int = 5` compiles with `--no-stdlib`.
2. No `type Int: #Int` / `type Float: #Float` / `type String: #String` /
   `type Bool: #Bool` / `type Char: #Char` remains in stdlib.
3. `Double` is `type Double: Float` (parent).
4. `Data` is the universal reflective floor — every value observable as raw
   storage via the treat-as-bits view; **no universal inheritance/cast edge**
   (`base: "Data"` only on explicitly-rooted types); `Bit<N>` is the unified
   bit type; no separate `Bits` type.
5. `Blob` is the renamed byte-buffer with the casting table in §3.5; no
   stale `Data` byte-buffer lanes remain.
6. `op Add(Int)` user overloads still work; `Int + Int` still intrinsic.
7. No `#`-category in front of a fundamental type name; all non-category
   `#` roles intact.
8. Full suite green; benchmarks unchanged.

## 8. Sequencing

- **Independent of coll** — can run before/after.
- **Two atomic global sweeps** (each one commit): the `Cast.#X` →
  `Cast.X` property rename, and the `Data`→`Blob` rename. They are the
  name-free keys the whole dispatch rides on.
- **Docs land first** (this plan + SPEC + bits-thesis + learn-briev +
  arch docs), then code, per the working-tree discipline.
