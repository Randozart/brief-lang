# Plan: Deferred Layout — physical-layout keywords (spec, pack, atomic, trap, union) + Bits-unit restoration

**2026-08-13.** Implements the normative SPEC §2.1 (types have no canonical
layout), §8.1/§8.2 (structs, `seq`), §8.9 (metadata), §19.7 (frozen
descriptors `Bytes`/`Alignment`/`Endian`) as **first-class physical-contract
keywords**: `spec` (per-type physical facts), `pack` (bit-contiguous
struct modifier), `atomic` (lock-free field modifier over the existing
`Atomic*#` intrinsics), `trap` (hardware-abort statement), and `union`
(untagged memory overlay). Restores the founding thesis that **`Bits<N>` is
N bits** (`src/ast/types.rs:3` — "All types are Bits(N) with metadata
overlays"), and **removes the `<...>` Layout DSL** (zero live usages).

Design decisions locked 2026-08-13:

- **Naming.** Formal term: **Deferred Layout** — "a type has no canonical
  representation until a backend *collapses* it (materialization) or
  `pack`/`seq`/`spec` *pins* it." Brand/story term: **Boxed Cat Typing**
  (docs-intro, tutorial, talk-track — same family as Duck Typing). SPEC
  predicate stays "layout-adaptive" (SPEC §8.1). The word "Schrödinger" is
  renamed **only in current guidance** (`docs/architecture/iterable-protocol.md`,
  1 occurrence); the historical plan `docs/plans/2026-08-12-iterable-protocol.md`
  is a timestamped record and is **not retroactively edited** (rule 5).
- **`Bits<N>` = exactly N bits**, any N ≥ 1. `Bits<0>` remains flexible
  width. Storage rounds up to bytes (`div_ceil(N,8)`) at `type_size` only.
  Sub-byte widths are preserved in the AST (today `from_bits` rounds them
  away).
- **`spec` = PascalCase name → lowercase metadata key**, fixed mapping table
  (below). Unknown spec name is a `SyntaxError` listing the known names.
  `spec` parses in exactly three bodies: `type`, `obj`, `struct`. No
  `spec Layout`, no `spec Tbaa`, no `spec Encoding` — `tbaa` stays `!>`
  (it can be any value; genuinely just metadata), `encoding` is dead.
- **The `<...>` Layout DSL is removed** (both brace and angle forms). Its
  value was carried only in `!> layout:` / `!> layout_struct:`, consumed only
  by `register_types.rs`; zero uses in stdlib, examples, tests, or fuzzers.
- **`pack` = bit-contiguous, zero padding.** Bit order is **declared, not
  impl-defined** and coupled to endianness like the real world: default
  `Target` (native — no surprise for existing code), `spec Endian: Big` =
  first field occupies the most-significant bits of byte 0 + big-endian
  multi-byte fields (RFC style), `spec Endian: Little` = LSB-first +
  little-endian bytes.
- **`!>` survives for annotation-only metadata** the compiler does not reason
  about (`tbaa`, doc keys). Semantics-bearing keys migrate: `bits`/`maxbits`/
  `alignment` → `spec`; `ExtractFrom`/`InsertAt` → `op` (they are bound to
  language sigils). `accel` and `IsZero`/`IsOne` migration is flagged below.
- **`vol struct` deferred** (`vol` stays let/field-level). **`atomic { }`
  block form deferred** — atomicity is a property of the storage, not the
  statement; a no-op block would violate rule 2 (a modifier must never be a
  silent win, but also must never be dead syntax). Field-level `atomic`
  modifier ships in this plan.
- **Interpreter is the reference** (rule 4): bit-slice, atomic, trap, and
  union semantics land in the interpreter before codegen.
- **Benchmark gate** (rules 11/11b): before-table captured from a clean
  `cargo build --release` + `bash benchmarks/build_and_bench.sh --runtime`;
  after-table from the same harness; A/B via
  `bash benchmarks/compare_baseline.sh <name>`. All changes are additive to
  existing paths; expected parity.
- **Housekeeping findings** (no code change, logged for provenance): AGENTS.md
  rule 11b says the baseline worktree is `../briev-compiler-baseline`, but on
  disk it is **`../briv-compiler-baseline`** — AGENTS.md reference is stale;
  fix the doc reference in the AGENTS.md rule-2/11b edit of Phase 7. The
  worktree audit (Phase 0 below) found no mergeable unmerged work anywhere.

---

## 1. Motivation — the current state is a corrupted founding thesis

`src/ast/types.rs:3` declares the thesis: *"All types are Bits(N) with
metadata overlays. No built-in primitives."* The implementation betrayed it:
`Type::Bits(u64)` stores **bytes**, not bits.

1. **`from_bits` destroys sub-byte width.** `src/ast/types.rs:99-102` —
   `Type::from_bits(bits)` does `bits.div_ceil(8)` and stores bytes.
   `Bits<4>` and `Bits<8>` both collapse to `Type::Bits(1)`. `bit_width()`
   (`:104-108`) returns `bytes * 8`, so it *re-*mults by 8 and lies about the
   stored value. The width is lost at parse time (`src/parser/types.rs:139`
   calls `from_bits`).
2. **The LLVM emitter treats the value as bytes.** `src/backend/llvm/types.rs:15`
   emits `i{n*8}`; `:85` returns `n` as the size in bytes. The byte→bits
   conversion lives at the consumer, inconsistently.
3. **Downstream callers disagree about the unit.** `src/backend/llvm/builder.rs:573-582`
   matches `Type::bits(1/2/4)` and emits `i8/i16/i32` — i.e. it reads the
   stored value as a **byte count**. `src/derive/smt.rs:28` emits a BitVec of
   width `n * 8` (bytes→bits). `src/backend/llvm/intrinsics.rs:724`
   constructs `Ptr<Type::bits(8)>` intending a byte pointer but produces an
   i64 element type — a latent bug this fix corrects (it becomes a true i8
   pointer).
4. **Physical layout is metadata-only and structs cannot express it.**
   `parse_struct_def` (`src/parser/definitions.rs:2147-2190`) reads a `seq`
   prefix and `#`-annotations only — no `!>`, no `spec`; `StructDef.metadata`
   (`src/ast/top.rs:916`) is populated solely with the `"annotations"` string.
   The StaticStruct LLVM registration (`src/backend/llvm/mod.rs:2402-2424`)
   hardcodes `alignment: 8` and `bytes = Σ type_size`. A systems programmer
   cannot pin alignment, size, or bit-width on a struct today.
5. **The `<...>` Layout DSL is a prototype dead end.** `!> layout:`
   (`src/parser/definitions.rs:1780-1789` + `src/parser/helpers.rs:362`
   `read_layout_body`), the brace form `!> layout: { ... }`
   (`parse_layout_struct_body`, `src/parser/definitions.rs:2056`), the parser
   `src/beast/layout.rs`, the AST `src/ast/layout.rs`, and the consumers
   `compute_layout_total_bits`/`attach_layout_fields` +
   `register_types.rs:85-90,183-210` — **zero live uses** in `lib/`,
   `examples/`, `benchmarks/`, or tests. It tokenizes raw source text with a
   hand-rolled scanner (`src/beast/layout.rs:29-68`) and never became a
   first-class pass.
6. **Semantics-bearing metadata hides behind `!>`.** `lib/std/types/bootstrap.bv`
   (`!> bits:`), `lib/std/types/float.bv` (`!> maxbits:`, `!> alignment:`),
   `lib/glue/*/types.bv` (`!> maxbits:`, `!> alignment:`) — all consumed by
   the compiler (`src/backend/register_types.rs:37,39,133`,
   `src/casting/graph.rs:777-795`), yet written as optional-looking hints.
   Physical facts are primary contracts; they read like hints.

## 2. Language design — the shape

### 2.1 `spec` — per-type physical facts

```briv
type UInt4: Int { spec Bits: 4; };
type Float16: #Float { spec MaxBits: 16; };

pack seq struct IpHeader {
    spec Endian: Big;
    spec Align: 1;
    version: Bits<4>,
    ihl: Bits<4>,
    tos: Bits<8>,
    total_len: Bits<16>,
    frag_offset: Bits<13>,
};

seq struct UartRegisters {
    spec Align: 4;
    spec Bytes: 16;          // exact 16-byte register file; overflow = error
    data_reg: Bits<32>,
    baud_rate: Bits<16>,
    _padding: Bits<16>,
};
```

Canonical spec names (PascalCase) and their metadata-key mapping:

| spec | key | consumer | status |
|---|---|---|---|
| `spec Align: N` | `alignment` | `register_types.rs:133`, LLVM alloca/global align | live |
| `spec Bits: N` | `bits` | `register_types.rs:37`, `casting/graph.rs:777` | live |
| `spec MaxBits: N` | `maxbits` | `register_types.rs:39`, `layout_optimizer.rs:516` | live |
| `spec Bytes: N` | `bytes` | `register_types.rs` (new) — authoritative size | new |
| `spec Endian: Big|Little|Target` | `endian` | stored property; packed bit-order (Phase 2) | new |

Rules:

- The map is a single table `spec_name_to_key()` in the parser. Unknown spec
  name → `SyntaxError`: `unknown spec '<X>'; known specs: Align, Bits,
  MaxBits, Bytes, Endian`.
- Values use the existing metadata grammar
  (`parse_metadata_value_standalone`, `src/parser/statements.rs:375`):
  identifier, integer, bool, string, list. `spec Endian` accepts only the
  identifiers `Big`, `Little`, `Target` (else `SyntaxError`).
- `spec` writes into the **same** metadata map as `!>`
  (`TypeDefBody.metadata`, `StructDef.metadata`) — the consumers need no new
  read paths for existing keys.
- **Size precedence** (first match wins):
  `spec Bytes` > `spec Bits`-derived (`div_ceil(N,8)`) > slot-sum
  (`type_size` of fields) > primordial > 8-byte fallback (existing warning).
  For `spec Bytes`, if the fields' total exceeds N → error
  `spec Bytes: N is smaller than the declared fields (need M)`.
- **Alignment precedence**: `spec Align` > primordial > `min(bytes,8)`
  fallback (existing warning).
- Canonical form (`src/ast/canonical.rs`) reverse-maps the five spec keys to
  `spec PascalCase: value;` output (lossless — after migration those keys are
  produced only by `spec`). All other metadata keys print as `!>`.

### 2.2 `Bits<N>` — N bits

- `Type::Bits(u64)` stores **bit width**. `from_bits(N)` = `Type::Bits(N)`
  (identity, no rounding). `bit_width()` returns the stored value. `type_size`
  storage = `N.div_ceil(8)` (min 1 for N > 0).
- LLVM scalar emission: `i{N}`. Sub-byte scalars in **adaptive** structs are
  the optimizer's choice (Schrödinger — the backend may promote to a whole
  byte); only `pack` pins the exact bit. Standalone sub-byte values remain
  legal `i{N}`.
- **Caller reclassification sweep** (the unit flip): every site that does
  arithmetic on the stored value or constructs a byte-count must be
  classified by intent. Known list:
  - `src/ast/types.rs:94-102` — `bits(bytes)` ctor becomes bit-count (==
    `from_bits`); keep the name, change the meaning.
  - `src/parser/types.rs:139` — `Bits<N>` → `Type::Bits(N)`.
  - `src/backend/llvm/types.rs:15` `i{n*8}`→`i{n}`; `:85` size
    `*n`→`n.div_ceil(8)`.
  - `src/backend/llvm/builder.rs:573,579,581,617,619,621` —
    `bits(1/2/4)`→`bits(8/16/32)` (they mean i8/i16/i32).
  - `src/derive/smt.rs:28` — `n * 8`→`n`.
  - `src/backend/llvm/intrinsics.rs:724` — unchanged, but becomes *correct*
    (byte pointer).
  - `src/derive/verify.rs:44,48,66` — `bits(64)` unchanged (64 bits = i64).
  - `src/derive/verify_smt.rs:279`, `src/backend/register_types.rs:37,39`
    (metadata `bits` is already bit-count — no change), `src/casting/graph.rs:777-795`.
  - `Type::int()` = `Type::Custom("Int")` (`ast/types.rs:70-72`) — **not**
    `Bits`; all ~300 `Type::int()` call sites are unaffected.
- `Bits<0>` = flexible width (parser `types.rs:141`) — unchanged.

### 2.3 `pack` — bit-contiguous struct modifier

```briv
pack seq struct EthernetFrameHeader {
    dst_mac: Bits<48>,
    src_mac: Bits<48>,
    ethertype: Bits<16>,
};
```

- `StructDef` gains `pack: bool` (parallel to `seq`). Parser collects
  `{pack, seq}` in any order before `struct`.
- **Whole-byte pack** (every field width % 8 == 0): `%T = type <{ ... }>`
  (LLVM packed — no inter-element padding). Member access via LLVM-native
  GEP. Struct size = Σ member bytes (no alignment rounding).
- **Sub-byte pack** (any field width % 8 != 0): `%T = type { [N x i8] }`,
  N = `Σ field bits / 8` rounded up. Every field access is a bit-slice:
  byte index = `bitpos / 8`, load the covering bytes, shift + mask (store:
  mask + shift + or). Bit order per §2.5.
- `struct_type_size` (`src/backend/llvm/emit_expr.rs:2693`) and the StaticStruct
  registration size (`src/backend/llvm/mod.rs:2402`) use bit-granular sums.
- **Rule 19 experiment (Phase 2.3)**: build a hand-written `.bv` packed header,
  compile through the full harness, inspect the linked binary
  (`clang -O3 -flto ...`, see AGENTS.md rule 6) to confirm byte-aligned
  slices fold to plain loads; only fall back to the byte-array path when
  sub-byte fields actually exist.

### 2.4 `atomic` — lock-free field modifier

```briv
seq struct ThreadStats {
    vol status: Bits<32>,      // MMIO register
    atomic counter: Bits<64>,  // lock-free
};
```

- Field prefix keyword in `struct`/`obj`/`type` bodies. Stored in the
  existing per-field `annotations` map of `parse_struct_def`
  (`src/parser/definitions.rs:2155`); `TypeDefSlot` gains a parallel
  `atomic: bool` (or a shared field-annotation carrier) for obj/type slots.
- Semantics: reads → `AtomicLoad#`, writes → `AtomicStore#`, RMW
  (`x = x <op> c`) → `AtomicAdd#`/`AtomicXchg#`, `seq_cst` (matches the
  existing intrinsic implementations). Backend intrinsic emission already
  exists: `src/backend/llvm/intrinsics.rs:66-70`; interpreter:
  `src/interpreter/intrinsics.rs:288-320`; signatures:
  `src/intrinsic_signatures.rs:193-222`.
- Backend needs a `ctx.atomic_fields: HashSet<String>` (field index names);
  load/store emission sites check membership (rules: the SAME `load`/`store`
  emitters as `vol`, i.e. additive match arms only — rule 5).
- `std/atomic.bv` continues to provide higher-level operations; the keyword is
  the low-level sugar over the intrinsics.
- **Rule 21 unchanged**: `atomic` is not implicit concurrency; `async`/`sync`
  classification still governs co-firing.

### 2.5 Bit order and endianness (declared, not impl-defined)

- Packed sequences lay out fields bit-contiguously in declaration order.
- Default `Target`: native byte order, native bit convention — identical
  behavior to today's whole-byte structs (no surprise).
- `spec Endian: Big`: field 1 occupies the most-significant bits of byte 0;
  multi-byte fields big-endian (RFC/IP-diagram convention).
- `spec Endian: Little`: LSB-first within each byte; multi-byte fields
  little-endian.
- One knob, two coherent conventions (matches C bitfield reality: GCC/MSVC on
  little-endian pack LSB-first; ARM big-endian packs MSB-first).

### 2.6 `trap` — hardware abort

```briv
when [index >= len] trap;
match status {
    Status::Ok => process(),
    Status::Corrupted => trap,
}
```

- `Token::Trap`. Three positions: statement (`trap;`), `when`-guard body
  (`when [g] trap;`), match-arm value (`=> trap`). Mirrors how `rollback`
  parses (`src/parser/statements.rs:53,197`; match arms resolve arm bodies
  through the statement parser).
- AST `Statement::Trap`. Codegen: `call void @llvm.trap()` + `unreachable`
  (existing pattern `src/backend/llvm/emit_toplevel.rs:726`). Typechecker:
  never-type value (unifies with any arm). Interpreter: abort with a
  testable `Trap` diagnostic.

### 2.7 `union` — untagged overlay

```briv
pack seq union PacketData {
    raw_bytes: Bits<8>[4],
    as_number: Bits<32>,
};
```

- `Token::Union`. `union Name { field: Type, … };` → `StructDef.union: bool`
  (parallel to `seq`/`pack`). Fields share offset 0 (C-union semantics).
- Frontend: size = max(aligned field storage); alignment = max(field
  alignment); one `ResolvedType`. Whole-byte fields only in this slice;
  sub-byte union fields are deferred (bit-slice of an overlay is the rare
  case; reject with a clear diagnostic for now).
- Backend: `%T = type { [N x i8] }`; field access = `GEP 0,0` +
  `bitcast` to the field LLVM type + load/store — an **explicit** pun, not a
  hidden cast-graph bypass.
- GLUE export: emit a C `union` (extend `src/glue/export.rs`; the existing
  `OutputType::Union` is a *sum*, unrelated).
- Interpreter: overlay reference semantics first.

## 3. Implementation phases

Every phase ends with `cargo test --lib` green, `cargo build` warning-free,
Praetor on the changed dirs, and a commit. Phase 0's commit is the plan file
itself (this document). Rule 12: each phase lists its doc/comment updates and
how existing rationale comments are preserved (rewritten, never deleted).

### Phase 0 — worktree, baseline, worktree audit

- **Worktree**: `git worktree add ../briv-compiler-spec -b feat/spec-layout-keywords`
  from `main` (HEAD 58b9808c). Main's uncommitted `src/parser/mod.rs` +
  `src/parser/dbg_test.rs` (leftover iterable-protocol debug test) stay in
  main, isolated.
- **Baseline (rule 11)**: `cargo build --release`; `bash benchmarks/build_and_bench.sh --runtime`;
  paste the result table into this document's §6. Commit.
- **Worktree audit (recorded, no action)**: all feature branches
  (`compiler-in-brief`, `feat/out-observability`, `feat/accel-gpu`) are
  fully merged (0 unmerged commits vs `main`). `brief-compiler-dogfood`,
  `brief-compiler-out`, `briv-lang-baseline` are stale detached copies with
  dead git bindings (their `.git` files point to a deleted
  `brief-compiler/.git/worktrees/...`) — nothing recoverable. `recovery-branch`
  holds 35 superseded July perf experiments. `briv-compiler-baseline`
  (detached) holds only a rebuilt benchmark binary + `bxbase_out/`.

### Phase 1 — `spec` keyword + Bits-unit restoration

1. **Lexer** (`src/lexer.rs`): `#[token("spec")] Token::Spec`; Display impl;
   add `"spec"` to the keyword string list (`:651-652`). Verify logos does
   not break `Bits`/`spec` identifier lexing.
2. **Parser**:
   - Factor the `!>` key-dispatch at `src/parser/definitions.rs:1757-1797`
     into `parse_metadata_clause(&mut self, metadata)` (keeps the
     `ctd`/`alu`/`layout` special cases; the `layout` case is deleted in
     Phase 3). Both `!>` and `spec` call it. `spec` reads a PascalCase key,
     `:`, value, `;`.
   - `spec_name_to_key()` table; unknown-name error per §2.1.
   - Add the `spec` branch to: type body loop (`:1754`), `parse_obj_like`
     (`:2091`), and — new — `parse_struct_def` body (`:2156`, alongside the
     `#`-annotation loop) writing into `StructDef.metadata`.
   - `Bits<N>` parse → `Type::Bits(N)` (drop `from_bits` call).
3. **AST/type fix**:
   - `src/ast/types.rs`: `Type::Bits` stores bits; `from_bits` identity;
     `bit_width()` returns stored; `bits(bytes)` → bit-count ctor.
   - `src/backend/llvm/types.rs`: `i{n}`; size `div_ceil`.
   - `src/backend/llvm/builder.rs:573-621` → `bits(8/16/32)`.
   - `src/derive/smt.rs:28` → `n`.
   - Grep sweep for any remaining `*8`/`/8`/`div_ceil` on `Bits` values; each
     classified by intent; update the stale rationale comments (rule 15:
     the old "Bits(64) = 8 bytes" comments are rewritten to the restored
     thesis with the 2026-08-13 provenance).
4. **Consumption**:
   - `register_types.rs`: add `bytes` read (authoritative size, §2.1
     precedence); store `endian` into `properties` (surfaces via `.^^`).
   - `src/backend/llvm/mod.rs:2402-2424` StaticStruct registration: read
     `spec Align`/`spec Bytes`/`spec Bits` from `StructDef.metadata` instead
     of hardcoded `alignment: 8`/Σ-type_size (additive: fall back to current
     behavior when absent).
   - `src/ast/canonical.rs`: `format_struct_into` prints `spec` lines (and
     `pack`/`union`/`seq` prefixes); TypeDef metadata reverse-maps the five
     spec keys to `spec PascalCase` form.
5. **Docs in-commit**: SPEC §8.9 rewrite (spec supersedes physical `!>` keys;
   `!>` annotation-only), SPEC §2.1 "Deferred Layout" naming.
6. **Tests**: parser (spec in struct/type/obj; unknown-spec error; `spec
   Endian` value check); canonical roundtrip (spec lines, `Bits<N>`);
   register-types size/alignment from spec (TypeDef + StaticStruct); sub-byte
   `Bits<4>` round-trip unit; `Bits<0>` flexible preserved.

### Phase 2 — `pack` + bit-contiguous emission

1. **Lexer**: `Token::Pack`.
2. **Parser**: collect `{pack, seq}` prefix before `struct` (top-level
   dispatch `src/parser/definitions.rs:35` arm extended; `parse_struct_def`
   accepts flags). `StructDef.pack: bool`.
3. **Rule 19 experiment** (§2.3) before committing the emission strategy:
   whole-byte packed vs byte-array; verify against the linked binary.
4. **Emission** (`src/backend/llvm/emit_toplevel.rs:246-290`):
   whole-byte packed → `type <{ ... }>`; sub-byte packed → `[N x i8]` +
   bit-slice access per §2.3/§2.5. `spec Align` → alloca/global alignment.
   `struct_type_size` + registration size → bit-granular.
5. **Interpreter first** (rule 4): packed bit-slice read/write reference
   semantics; endian Big/Little variants.
6. **Tests**: IR for `pack seq struct` (whole-byte `<{ }>`, size, GEP), IR for
   sub-byte (`[N x i8]` + mask/shift), bit-order Big vs Little, spec Align on
   alloca, interpreter equivalence.
7. **Docs in-commit**: SPEC §8.2 (pack + bit order + endian coupling).

### Phase 3 — `<...>` Layout DSL removal

- Delete: `src/beast/layout.rs`, `src/ast/layout.rs` (+ its `mod` declaration
  in `src/ast/mod.rs:16`), `read_layout_body` (`src/parser/helpers.rs:362`),
  `parse_layout_struct_body` (`src/parser/definitions.rs:2056`),
  `compute_layout_total_bits` + `attach_layout_fields`
  (`src/backend/register_types.rs`), the `"layout"`/`"layout_struct"` arms in
  `register_types.rs:85-90,183-210`, the parser `!> layout:` special case
  (`definitions.rs:1780-1789` → deleted; bare `!>` falls to the generic
  path), beast serialize/deserialize layout references if any.
- **Regression guard**: re-run `cargo test --lib`; grep for
  `layout_struct`/`read_layout_body`/`LayoutPattern`/`beast::layout` returning
  zero. Confirm no test/fuzz fixture emits `!> layout`.
- **Rationale preservation** (rule 15): the removal plan comment records *why*
  the DSL was a dead prototype (history: `git log -S read_layout_body` shows
  layout-DSL work predating the 2026-08-01 frontend-driven-dispatch rewrite),
  and points to this plan for the resurrection rule (a consumer must exist).
- **Docs in-commit**: SPEC §8.1/§8.6 layout-DSL references removed.

### Phase 4 — `trap`

1. **Lexer**: `Token::Trap`. **Parser**: statement (`trap;`), `when`-guard
   body, match-arm value. **AST**: `Statement::Trap`.
2. **Codegen**: `call void @llvm.trap(); unreachable` (pattern
   `emit_toplevel.rs:726`). **Typechecker**: never-type. **Interpreter**:
   abort diagnostic.
3. **Tests**: all three positions; IR contains `llvm.trap`; arm-type
   unification; interpreter `Trap`.

### Phase 5 — `atomic` field modifier

1. **Parser**: `atomic` field prefix in struct/obj/type bodies →
   `parse_struct_def`'s `annotations` map (key `"atomic"`) + a
   `TypeDefSlot`/slot-side carrier for obj/type slots.
2. **Backend**: `ctx.atomic_fields: HashSet<String>`; the field load/store
   emitters add a membership check → emit `AtomicLoad#`/`AtomicStore#`
   (existing intrinsics); RMW detection `x = x <op> c` → `AtomicAdd#`.
   Additive arms only (rule 5).
3. **Interpreter**: atomic ops already present (`intrinsics.rs:288-320`) —
   wire the field modifier to them.
4. **Tests**: IR `load atomic`/`store atomic`/`atomicrmw`; interpreter RMW;
   no-speed-path regression (plain fields unchanged).
5. **Docs in-commit**: SPEC §8.2 field modifiers table + new atomic note.

### Phase 6 — `union`

1. **Lexer**: `Token::Union`. **Parser**: `union Name { field: Type, … };`
   → `StructDef.union: bool`; fields share offset 0. Sub-byte union field →
   `SyntaxError` (deferred, explicit).
2. **Frontend**: size = max(aligned field storage), alignment = max field
   alignment; one `ResolvedType`.
3. **Backend**: `%T = type { [N x i8] }` + per-field `GEP 0,0` + `bitcast` +
   load/store. **GLUE export**: C union.
4. **Interpreter**: overlay reference.
5. **Tests**: IR overlay + field punning; size/alignment; C-export shape;
   interpreter equality.
6. **Docs in-commit**: SPEC §8.x new union section.

### Phase 7 — stdlib migration, docs, highlighter, gates

**Stdlib migration** (mechanical, verified by full suite):

| file | today | after |
|---|---|---|
| `lib/std/types/bootstrap.bv:32-91` | `!> bits: N` | `spec Bits: N` |
| `lib/std/types/float.bv:11-24` | `!> maxbits:` `!> alignment:` | `spec MaxBits:` `spec Align:` (keep `!> tbaa`) |
| `lib/glue/{python,rust,node,web}/types.bv` | `!> maxbits:` `!> alignment:` | `spec MaxBits:` `spec Align:` |
| `examples/eor-demo.bv`, `examples/glue-*-bridge/bridge.bv` | `!> maxbits:` `!> alignment:` | spec |
| `lib/std/memory/crossword.bv:24-25` | `!> InsertAt:` `!> ExtractFrom:` | `op InsertAt: crossword_push(#Lh, #Rh);` `op ExtractFrom: crossword_pop(#Lh, #Rh);` — verify actual target signatures first (op colon-form requires parens, `definitions.rs:1930`) |
| `lib/std/skiplist.bv:5` | `!> ExtractFrom: sl_remove;` | `op ExtractFrom: sl_remove(#Lh, #Rh);` — verify signature |
| `lib/std/types.bv:5-6` | `!> IsZero: _ == 0;` `!> IsOne: _ == 1;` | **audit**: these do not parse as any current metadata value — flag and remove only if truly dead, else convert to `op` |
| `benchmarks/nbody_newton_accel.bv`, `accel_min.bv` | `!> accel: try_all;` | keep `!>` (module-level dispatch directive, not per-type physical) — flagged |

**Docs** (all in the same commits as the features):

- `spec/SPEC.md`: §2.1 (Deferred Layout), §8.2 (pack, bit order, atomic
  modifier), §8.9 (spec supersedes physical `!>` keys; `!>` annotation-only),
  §8.x union, §8.x trap, §8.x atomic; remove Layout-DSL references; frozen
  descriptor §19.7 tied to `spec` names.
- `learn-briev/12-pragmas.md` (metadata section → spec), `15-custom-types.md`,
  `05-data-types.md` (structs, Bits).
- `docs/architecture/agent-reference.md` (spec vocabulary, Deferred Layout,
  layout rules), `docs/architecture/backend-architecture.md` (struct emission
  paths: `<{ }>`/`[N x i8]`/union overlay).
- `AGENTS.md`: rule 2 directive list gains `pack`; rule 11b baseline path
  corrected to `../briv-compiler-baseline`.
- Rename "Schrödinger" → "Deferred Layout"/"Boxed Cat Typing" in
  `docs/architecture/iterable-protocol.md` only (1 occurrence; the historical
  plan is untouched).
- `syntax-highlighter/syntaxes/rbv.tmLanguage.json` +
  `briev.tmLanguage.json`: add `spec`, `pack`, `atomic`, `trap`, `union`.

**Gates**: `cargo test --lib` green; Praetor per changed dir; full bench
harness after; `bash benchmarks/compare_baseline.sh <name>` A/B — expect
parity (additive paths only). Log the `Bits`-unit bug and the
`intrinsics.rs:724` pointer-width bug in `BUGS.md`.

## 4. Test matrix (contract-behavioral, rule 4)

| concern | where |
|---|---|
| spec parses in type/obj/struct; unknown name error; Endian value check | parser unit |
| canonical roundtrip: spec lines, pack/union prefixes, `Bits<N>` | canonical unit |
| size/alignment from spec (TypeDef + StaticStruct); Bytes overflow error | register-types unit |
| `Bits<4>` survives (not rounded); `Bits<0>` flexible | AST/parser unit |
| whole-byte packed IR `<{ }>` size/offsets; sub-byte `[N x i8]` mask/shift; Big vs Little | backend IR |
| packed interpreter equivalence (bit-slice read/write, both endians) | interpreter |
| trap: statement/guard/arm; `llvm.trap` in IR; never-type unification | parser+backend+typechecker |
| atomic: `load atomic`/`store atomic`/RMW; plain fields unchanged | backend IR |
| union: overlay size/align; field punning IR; C export; interpreter | backend+glue+interpreter |
| stdlib migration: full suite green, all benchmarks compile | suite + harness |

**Phase 1 status (2026-08-13):** implemented + committed. Lexer `Token::Spec`,
`spec` vocab entry, `Type::Bits(u64)` = bit count (sub-byte preserved), spec
parsing via `parse_metadata_clause`/`parse_spec_value`/`spec_name_to_key`
(type/obj/struct bodies; unknown spec + invalid Endian = errors), width-sweep
of all Bits sites to bit units (`i{n}`, `div_ceil(8)` at storage), pointer
element width fix (byte pointers are now i8; latent byte-era bitcast bug in
`glue/bridge.rs`), register_types `bytes` override (authoritative) + endian
surfaced on `properties`, `static_struct_resolved_ty` shared sizing helper
(Single authority; llvm/mod.rs StaticStruct arm delegates), canonical
`spec`/`!>` printers, SPEC §2.1 (Deferred Layout) + §8.9 (spec supersedes
physical `!>` keys). Tests: 19 new (parser spec forms/errors, canonical
round-trip + fixtures, register-types sizing TypeDef+StaticStruct, `Bits<4>`
sub-byte + `Bits<0>` flexible + byte-pointer i8). `cargo test --lib` 1803
green, build warnings unchanged (5 pre-existing), Praetor zero new diagnostics
(register_typedefs complexity actually reduced 37→33), full runtime suite 39
MATCH/PASS exit=0.

## 5. Commit sequence

1. `docs/plans/2026-08-13-layout-keywords.md` (this file) + baseline table.
2. Phase 1 (spec + Bits fix + consumers + canonical + SPEC §2.1/§8.9).
3. Phase 2 (pack + emission + interpreter + SPEC §8.2).
4. Phase 3 (DSL removal + SPEC cleanup).
5. Phase 4 (trap).
6. Phase 5 (atomic).
7. Phase 6 (union).
8. Phase 7 (stdlib migration + docs + highlighter + AGENTS.md fix + BUGS.md).

Each commit: `cargo test --lib` green, `cargo build` warning-free, Praetor on
changed dirs, architecture docs in the same commit.

## 6. Baseline table (filled at Phase 0 from the clean harness)

**Date:** 2026-08-13 · **Commit:** `4dccf5d9` + Phase 0 prerequisite fixes
(pp-types.bv frgn migration, tests `briefc`→`brievc` rename repair)
· **Worktree:** `../briv-compiler-spec`, branch `feat/spec-layout-keywords`
· **Harness:** `bash benchmarks/build_and_bench.sh --runtime`, BOUND=50000000
· **Log:** `/tmp/opencode/bench_baseline.log` · **Toolchain:** clang 18, llc 18

> **Phase 0 prerequisite fixes** (pre-existing HEAD regressions, landed with this
> baseline): (1) `pp-types.bv` used the removed `frgn ... as <sym> from ... fallback
> <e>` form (SPEC §19.1 `: sym`, §19.3 `fallback` removal) — the `--runtime` suite
> and `tests/pp_roundtrip_tests.rs`/`tests/c_driver_library.rs` were broken at HEAD;
> (2) both test files referenced the pre-rename `briefc` bin and `briv_*`/`BrivState`
> symbols (renamed at `62ae145d` "Massive rename"); the generated header is
> `briev_types.h`. 9/9 pp tests now pass. The remaining 7 stale integration-test
> files (`tests/c_driver_{cpp,callback,lua,java,node,boundary,go,csharp}.rs`,
> `tests/glue_integration.sh`) are recorded as a known HEAD regression in
> `BUGS.md`; they do not compile and were already dead at HEAD (not gated by
> `cargo test --lib`).

| Benchmark | Briev | C | Ratio | Winner | Correct |
|-----------|:-----:|:--:|:-----:|:------:|:-------:|
| ring_buffer | .0553s | .0496s | 1.11x | C | MATCH |
| float_math | .0455s | .0739s | .61x | Briev | MATCH |
| float_math_nonzero | .1609s | .1681s | .95x | Briev | MATCH |
| sparse_dispatch | .0489s | .0604s | .80x | Briev | MATCH |
| print_loop | .0346s | .0616s | .56x | Briev | MATCH |
| nbody_newton | 7.5469s | 8.9636s | .84x | Briev | MATCH |
| nbody_newton_accel | 1.0031s | .1373s | 7.30x | C | MATCH |
| nbody_sqrt | 2.4032s | 3.1360s | .76x | Briev | MATCH |
| nbody_sqrt_idio | 3.0421s | 3.9631s | .76x | Briev | MATCH |
| fasta | .2343s | .2236s | 1.04x | C | MATCH |
| fannkuch_redux | .0673s | .0677s | .99x | Briev | MATCH |
| mandelbrot | .7150s | .6913s | 1.03x | C | MATCH |
| kalman_filter_runtime | .1559s | .1818s | .85x | Briev | MATCH |
| knucleotide | .1909s | .1934s | .98x | Briev | MATCH |
| cancel_math | .0564s | .0643s | .87x | Briev | MATCH |
| bit_clear | .0002s | .0002s | 1.00x | ~tie | MATCH |
| queue_drain | .0352s | .0608s | .57x | Briev | MATCH |
| queue_drain_sym | .0347s | .0620s | .55x | Briev | MATCH |
| queue_drain_idio | .0364s | .0603s | .60x | Briev | MATCH |
| stack_push_pop | .0346s | .0611s | .56x | Briev | MATCH |
| interval_step | .0620s | .0620s | 1.00x | ~tie | MATCH |
| telemetry_stream | .1938s | .2013s | .96x | Briev | MATCH |
| pid_control | .3440s | .3516s | .97x | Briev | MATCH |
| matrix_pipeline | .4682s | .7466s | .62x | Briev | MATCH |
| accumulator_flush | .1242s | .1505s | .82x | Briev | MATCH |
| sweep_sparse | .2212s | .1558s | 1.41x | C | MATCH |
| sweep_mid | .2621s | .2377s | 1.10x | C | MATCH |
| sweep_dense | .4011s | .2678s | 1.49x | C | MATCH |
| sweep_arr | .4108s | .3515s | 1.16x | C | MATCH |
| series_converge | .0001s | .0003s | .33x | Briev | MATCH |
| global_lifetime | .0328s | .0701s | .46x | Briev | MATCH |
| deep_recursion | .0001s | .0004s | .25x | Briev | MATCH |
| arena_churn | .0882s | .1029s | .85x | Briev | MATCH |
| linked_list | 1.2131s | 1.7952s | .67x | Briev | MATCH |
| hash_ops | 1.0341s | 1.1215s | .92x | Briev | MATCH |
| hash_ops_idio | .0286s | .0517s | .55x | Briev | MATCH |
| enemy_swarm | .0964s | .1319s | .73x | Briev | MATCH |
| bridge_glue | done | | | | MATCH |
| bridge_multi | done | | | | PASS |

All 39 benchmarks: no MISMATCH, no FAIL. `bridge_glue`/`bridge_multi` build
+ run via Makefile (timed as "done" by the harness — python driver).

## 7. Risks & open checks

1. **Bits-unit sweep completeness** — the Rust type system surfaces missed
   sites as compile errors; the remaining risk is a *silent* semantic flip
   (a site that used to read bytes now reads bits). Mitigation: every `*8`/
   `/8`/`div_ceil` edit is a named, reviewed change; the sub-byte unit tests
   lock `Bits<4>` behavior; the `intrinsics.rs:724` pointer-width fix is
   asserted in a test.
2. **Packed field-offset audit** (rule 19) — verify GEP/alloca/array math
   against the actual linked binary before trusting `<{ }>` vs `[N x i8]`.
3. **`op ExtractFrom/InsertAt` migration** — each target's real signature must
   accept `#Lh, #Rh`; verify per binding before rewriting.
4. **`!> IsZero/IsOne`** (`lib/std/types.bv`) — confirm parse status; if the
   current parser rejects them, they are already dead (record in BUGS.md).
5. **union C-export shape** — `src/glue/export.rs` has no union emitter;
   verify the emitted C compiles.
6. **`spec Bytes` interaction with `pack`/sub-byte** — define: `spec Bytes` is
   the authoritative storage size; a packed struct's `div_ceil(Σbits,8)` must
   be ≤ `spec Bytes` (equal → padding to N; less → error per §2.1).

## 8. Definition of done

- All five keywords parse, typecheck, codegen, and interpret (rule 8).
- `Bits<N>` is N bits everywhere; `Bits<4>` round-trips the AST.
- The `<...>` DSL is gone; `grep` for its symbols returns zero.
- Stdlib and examples use `spec`/`op`; `!>` carries only annotation metadata.
- SPEC, tutorial, architecture docs, highlighter updated in-commit (rule 3).
- Full suite green; benchmarks at parity with the §6 baseline (rule 11/11b).
- `BUGS.md` records the Bits-unit bug, the `intrinsics.rs:724` pointer bug,
  and the `IsZero/IsOne` audit result.
