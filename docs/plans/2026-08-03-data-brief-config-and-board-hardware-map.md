# Plan: Data Brief as Universal Config + Board-Owned Hardware Map

**Date:** 2026-08-03
**Status:** Phase 2 (board-owned hardware map) DONE — 2026-08-03; Phase 3 next
**Branch:** `feat/data-brief-config` (new)

**This is the AUTHORITATIVE record** for two coupled streams that share one
mechanism: (1) making **Data Brief** (`.dbv`/`.dbvl`, via `src/dbrief/v2.rs`)
the universal config and board-description format, replacing the `config/*.toml`
layer; and (2) giving **`.cbv` and `.ebv` a single, board-owned shared hardware
map** they both resolve against, so firmware and hardware agree on MMIO
addresses by construction.

## Plan map (how the plans relate)

| Plan | Role | Status |
|---|---|---|
| **[This plan](2026-08-03-data-brief-config-and-board-hardware-map.md)** | The authoritative record — the DB-as-config migration, the board-owned hardware map, the shared address contract, and the `.ebv` follow-up handoff. All design research lives here. | Active |
| [2026-07-30-meta-circular-tamer.md](2026-07-30-meta-circular-tamer.md) | The freestanding `.ebv` stress test (VM, pointers, bytecode in Brief). Depends on `.ebv` having a real no-OS runtime; this plan provides the board/address foundation it builds on. | Design → foundation |
| [2026-08-01-consumptive-operators-lifetime-and-c-surface.md](2026-08-01-consumptive-operators-lifetime-and-c-surface.md) | The C-surface reduction master plan. This plan carries its `.ebv` thread forward by making `.ebv` config-driven (board map) rather than hardcoded. | Superseded for config |

---

## Part I — Design research (the "why")

### 1. The coordination problem: how do `.cbv` and `.ebv` agree on an address?

The embedded tier has two backends reading the *same* device:

- **`.cbv` → CIRCT** — hardware synthesis. `AddressOf#` at
  `src/backend/circt.rs:392` calls `crate::address_resolver::resolve_address`,
  emitting a `hw.constant` that drives the address bus. State vars with `@`
  addresses become external MMIO ports.
- **`.ebv` → LLVM** — freestanding firmware. `AddressOf#` at
  `src/backend/llvm/intrinsics.rs` and trg loads at `emit_toplevel.rs:575`
  (`LinkRef::Explicit(addr)` → `inttoptr` → volatile `load`) call the *same
  `resolve_address`*.

They already share one `address_resolver::resolve_address(id)`. Today that
resolver's backing data is a **hardcoded table + `config/address-map.toml`**.
The agreement between hardware and firmware therefore exists but is
**unowned** — buried in a compiler config nobody single-sources.

**The fix:** the *board* — not the compiler — owns the map. The compiler reads
it. That inverts the dependency: a board's `.dbv`/`.dbvl` becomes the single
source of truth both backends resolve against, and automation (CMSIS-SVD import,
vendor-provided files) can regenerate it because it is just data.

### 2. Why Data Brief, now

`config/*.toml` is the *last* configuration in the system not expressed in a
Brief-native form. Data Brief (`.dbv`/`.dbvl`) is already shipped — `src/dbrief/
v2.rs` parses it (`;` separator, `>` directives, bare tokens default), and
`src/dbrief/bridge.rs::document_to_program` converts it to the Brief AST. It is
already consumed for board files (`import "target"` → `lib/boards/<board>.dbvl`
in `src/import_resolver.rs:275-321`) and for GLUE registries
(`lib/glue.dbvl`). The hardware-map seam for using it as the universal config is
therefore already built and battle-tested.

Notably, quotes (`"..."`) **are** legal Data Brief — they are opt-in via the
parser flag (spec §3.4) and `parse_document_quoted` in `v2.rs`. This is what
makes even the IR-template-heavy `alloc-strategies.toml` migratable as quoted
string values.

### 3. `.dbv` vs `.dbvl` — the split

Data Brief family is two formats. They are complementary, not interchangeable:

| Format | Strength | Weakness | Use here |
|---|---|---|---|
| `.dbv` | nested blocks, keyed entries, schemas, multiple `as` groups | brace-laden for trivial tables | **Rich, nested register/device detail** (`map.dbv`) |
| `.dbvl` | one entry per line, single-pass parser, schema via `>` directive | no nesting | **Flat `KEY: address;` tables** both backends resolve |

The board ships both: a `.dbv` carrying the fixed register layout
(offsets/sizes/access/reset — the stm32f407 nesting today), and a `.dbvl`
carrying the flat `CAPITALIZED_CONSTANT → address` table. The `.dbvl` is what
`address_resolver` queries.

### 4. The intrinsic-runtime thread (explicitly deferred)

The `.ebv` no-OS runtime divergence (`Malloc#`→ bump allocator, `Print#`→ pure
Brief formatting + `write`, `Now#`→ freestanding clock) is **out of scope for
this plan** and becomes its own follow-up plan (included in §Phase 4 as a
hand-off doc). The reason for the split: the board/address context (this plan)
is the *precondition* the intrinsic-runtime work reads (a device lives at an
address in a board's map; the runtime maps intrinsics to *that*). Verification
is cleaner if the two are decoupled.

---

## 5. Design

### 5.1 The board-owned hardware map

> **2026-08-03 (format audit):** the existing `lib/boards/stm32f407.dbvl` is
> **obsolete pre-v2 syntax** and does not parse under `src/dbrief/v2.rs`. Three
> violations: (1) `schema lib/devices/uart.dbvs;` uses the removed `.dbvs`
> extension and the non-directive `schema X;` form, which fails
> `parse_schema`'s `expect_char('{')`; (2) `uart1 { ... }` key-without-colon
> nested rows match no dispatch arm in `parse_data_entry_in_group`; (3) nested
> `{ }` in a `.dbvl` contradicts the line-oriented spec (§7/§12.4). Phase 2
> therefore **rewrites** (not merely relocates) this file into valid v2 DB.

> **2026-08-03 (parser verification, empirical):** probing `src/dbrief/v2.rs`
> with candidate board forms proved that **nested `{ }` register blocks do NOT
> parse** in the current v2 parser — neither `>`-prefixed (`{ > 0; rw; }`) nor
> bare (`{ 0; 9; rw; }`), nor the spec's own §11.3 example verbatim. The
> verifiably-working forms are **flat**:
> - `.dbvl` line-tables with `>schema <Name> from "<path>"` + `KEY: value;` lines
> - `.dbv` flat keyed `as Device { uart1: 0x40011000; 0x18; }` entries
>
> The board format below is therefore **flat line-oriented data only**. Hex
> literals parse as `String` (not `Int`), so the resolver treats addresses as
> strings. Nesting may be revisited as a parser enhancement; it is not required
> for the address coordination this plan delivers.

A board directory owns both files; the compiler only reads them:

```
lib/boards/<board-name>/
├── map.dbv          ← schemas only (Device, Register), no data
└── addresses.dbvl   ← flat CAPITALIZED_CONSTANT → address (+size) lines
```

`map.dbv` (schema-only — data lives in the `.dbvl`):

```
// map.dbv
schema Device {
    base_addr: String;
    size: Int;
};

schema Register {
    name: String;
    offset: Int;
    size: Int;
    access: String;
};
```

`addresses.dbvl` (the resolver's backing table):

```dbvl
>schema Device from "map.dbv"
UART0: 0xFFE01000; 0x18;
UART1: 0x40004400; 0x18;
GPIOA: 0x40020000; 0x400;
TIMER: 0xFE002000; 0x4;
```

Register detail (optional second table, same flat line form):

```dbvl
>schema Register from "map.dbv"
UART0_DR: 0x00; 9; rw;
UART0_SR: 0x01; 9; ro;
```

### 5.2 The shared `resolve_address` contract (unchanged API)

Neither backend changes its call site. `address_resolver::resolve_address(id)`
keeps its exact signature. The change is *only* in its backing:

1. **New:** board-provided `lib/boards/<board>/addresses.dbvl` wins.
2. **New:** `config/address-map.toml` becomes a deprecated alias (or is removed).
3. **Fallback:** only on *unknown* board/absent board file, the hardcoded
   table remains as a **warning** (never silent — rule: an unowned default must
   say so) plus the `0xFE000000` base.

The `.cbv` and `.ebv` backends therefore stay agreeing *by construction* — both
call the same `resolve_address`, now backed by the board's data.

### 5.3 Validation against the map

The existing `hardware_validator` overlap machinery (`B4006`, the
banks/sections logic in `src/hardware_validator.rs:178`) becomes the guard: a
`.cbv` MMIO placement resolves against the board map and is checked for
bank-bounds / overlap, so hardware synthesis cannot silently collide with
firmware's assumptions.

---

## Phases

### Phase 1 — DB-read routing (shared loader + parity harness)

- Add a small Rust loader (in `src/dbrief/` or a new `src/config_db.rs`) that
  dispatches `.dbv`/`.dbvl` content through `v2::parse_document` /
  `parse_document_quoted` and exposes keyed lookup by **capitalized constant**.
- Wire it so `--config-dir`/profile resolution (in
  `src/config_resolver.rs`) can point at `.dbv`/`.dbvl` files.
- Build a **config-parity test harness**: for each migrated config, golden-
  compile a representative set before/after the migration and assert identical
  emitted LLVM IR + runtime behavior. `cargo test --lib` must stay green with a
  clean diff.

### Phase 1b — stm32f407 round-trip golden test (format proof)

Before the loader and real board rewrite, **prove the format**: add a unit test
in `src/dbrief/v2.rs` (tests module) that feeds the *intended* v2 flat
`map.dbv` (schemas only) + `addresses.dbvl` + `registers.dbvl` content through
`parse_document` and asserts a well-formed `DbriefDocument`: the `Device`/
`Register` schemas resolve, every `.dbvl` `KEY → address/fields` pair lands
with the schema-derived key and the expected field values (hex addresses read
as `String`). This locks the format **before** any loader dependency, so Phase
2 rewrites against a verified grammar.

### Phase 2 — Board-owned hardware map

- **Rewrite** the obsolete `lib/boards/stm32f407.dbvl` (see §5.1 audit) into
  `lib/boards/stm32f407/map.dbv` (schemas only) and add
  `lib/boards/stm32f407/addresses.dbvl` (flat constant table) and
  `lib/boards/stm32f407/registers.dbvl` (flat register detail), validated by the
  Phase 1b golden test.
- Retarget `src/address_resolver.rs` to read the board `.dbvl` first,
  config TOML second, hardcoded-with-warning last.
- Add an `import "target"`-driven path so a board is selected the existing way
  (`--board` mechanism) and its map is the source.
- Run the hardware-validator overlap checks against the board map.

> **2026-08-03 (Phase 2 complete):** the board is rewritten as a directory
> (`lib/boards/stm32f407/{map.dbv,addresses.dbvl,registers.dbvl}`) and
> `resolve_address` is retargeted: active-board `addresses.dbvl` (via
> `ConfigDb`) → config TOML (deprecated alias) → hardcoded **with warning** →
> `0xFE000000`. `set_active_board` is driven by `import "target"` (thread-local,
> set in `resolve_target_import`), so the interpreter, LLVM, and CIRCT agree by
> construction — all route through the one function. Three latent divergences
> were fixed while wiring:
> 1. **`flatten_peripheral_constants` hex-as-String** (`bridge.rs`): the v2
>    parser yields hex literals as `DataValue::String`, but flattening only
>    accepted `DataValue::Int` → zero per-key constants. Now `data_value_as_u64`
>    radix-parses `0x…` strings. This was a silent dead path — the old board
>    file never parsed, so nothing noticed.
> 2. **`>schema Name from "path"` tagged with file stem** (`v2.rs`): the
>    directive set `current_schema` to `path`'s stem ("map"), so `.dbvl` groups
>    were tagged "map" and flattening could not find the declared `Device`
>    schema. The directive names the schema explicitly; `current_schema` is now
>    the NAME. Matches `docs/architecture/data-brief.md` semantics.
> 3. **Board schema carrier re-emitted as an import**: `>schema Device from
>    "map.dbv"` pushed "map.dbv" into `doc.imports`; the bridge turned it into a
>    literal `import "map.dbv"` that failed resolution. `resolve_target_import`
>    now drops the merged `map.dbv` from `doc.imports`.

### Phase 3 — Migrate all six TOML configs → DB

For each, a DB schema + loader + parity test. The TOML files are deleted only
after the DB path provably produces identical output.

| Config | DB form | Quoted-value risk? |
|---|---|---|
| `address-map.toml` | `.dbvl` `KEY: addr;` | none (Phase 2) |
| `targets.toml` | `.dbv` `[extension] → backend/plugins` | none |
| `protocols.toml` | `.dbv` `#word → ABI` | none |
| `module-registry.toml` | `.dbv` `name → path` | none |
| `encodings.toml` | `.dbv` schemas | none |
| `ir-lowering.toml` | `.dbv` tunables | none |
| `alloc-strategies.toml` | `.dbv` with **quoted** LLVM IR templates | **yes** — templates carry `{}`, `;`, `:`. Solved by `parse_document_quoted`: store the template as `template: "call @llvm.memset..."`. Escape `"`/`\`. |

`alloc-strategies` is Phase 3-*last* and gated behind the quoted-string test,
so the five clean ones are not blocked by the IR one.

### Phase 4 — Documentation + `.ebv` runtime hand-off

- Update `docs/architecture/data-brief.md`: note it is now the universal config
  format (`schema RegistryEntry` / `as` per migrated config).
- Update config-loading docs (the `config_resolver.rs` chain).
- Write the follow-up-plan outline for the **per-target intrinsic runtime**
  (`.ebv` `Malloc#`/`Print#`/`Now#` symbol-map and no-C runtime), referencing the
  Phase 3 DB convention.

---

## Exceptions to Standards

This plan follows AGENTS.md Plan Directives throughout:

- ✓ **Flat control flow** — all resolver changes use guard clauses; no new
  nesting.
- ✓ **CONTRACT-first** — `resolve_address` API is untouched; only its backing
  source changes. Never weaken an existing contract.
- ✓ **Comments get 2026-08-03 provenance** with the when/why/undo each.
- ✓ **Behavioral tests, not literal tests** — parity asserts *behavior/IR*, not
  config text.
- ✓ **Documentation is code** — DB spec + config docs update in the same commit
  as each phase.
- ✓ **Additive only** — fallthrough / warning defaults preserved; TOML removal
  only after parity proves equivalence.
- ✓ **DRY** — the DB lookup is one loader, reused by all six configs; no
  duplicated parse-and-divide logic.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `alloc-strategies` IR templates parse surprisingly under quotes | Medium | Phase-3-last slips | Gate it behind the quoted-string parity test; fall back to a documented `"..."`. |
| Board-map resolver changes break `.cbv`/`.ebv` address agreement | Medium | High | `address_resolver` contract unchanged; parity harness covers `AddressOf#`; overlap checks added. |
| **Existing board file is non-parsing pre-v2** | Certain ($5.1 audit) | Low (it is dead weight today) | Phase 1b golden test proves the intended v2 grammar before Phase 2 rewrites the board; no parser regression is possible because the old syntax is excluded. |
| `.dbvl` flat table loses register nesting | None | Low | **Resolved 2026-08-03 (empirical):** the v2 parser rejects nested `{ }` in all forms (verified in Phase 1b probes); the flat line-table carries register rows instead, so nothing is lost. A parser nesting enhancement is out of scope. |
| TOML removal breaks `--config-dir` users | Low | Medium | `config_resolver` reads DB from the same resolved dir; parity harness covers it |

## Timeline

| Phase | Effort | Owns |
|---|---|---|
| 1a | DB-read routing (shared loader + parity harness) | Me |
| 1b | stm32f407 round-trip golden test (format proof) | — |
| 2 | Board rewrite (map.dbv + addresses.dbvl) + resolver retarget | — |
| 3 | Five easy TOML → DB | — |
| 4 | alloc-strategies quoted | — |
| 5 | docs + hand-off | — |

Migrate each atop the last; commit per phase with `cargo test --lib` green.