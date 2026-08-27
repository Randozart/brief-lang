# Foreign Hardware Imports + MMIO Pins for .cbv

**Date:** 2026-08-27
**Status:** APPROVED — pending implementation
**Parent set:** `docs/plans/2026-08-23-backend-scaffolding-foundation.md`
(CIRCT toolchain validation → parallel track)
**Owner backend surface:** `src/backend/circt/` + parser/normalizer + LLVM
intrinsic staging
**Sync note (session 2026-08-27):** foundation holds 6 unmerged commits
awaiting main's B1.1 protocols.bv bodies; this plan builds on top and syncs
with that batch.

---

## Motivation

Two capabilities the hardware target lacks, both blocking real deployment:

1. **Foreign HDL import** — vendor/legacy Verilog/VHDL (UARTs, FIFOs,
   PHY cores) cannot be referenced from `.cbv`; non-cell calls error
   (`src/backend/circt/mod.rs:1092-1109`). The ONLY extern mechanism today
   is the seq.firmem workaround: `tools/patch_generated.py` rewrites macro
   ops into `hw.module.extern`, `memory_companions()` writes reference
   `.sv`, the harness globs them in. That is the skeleton — it must be
   generalized.
2. **MMIO pins** — `.ebv` addresses registers via `trg x @ 0xADDR`
   volatile inttoptr loads; `.cbv` has NO path: the `mmio_vars` field is
   declared then never read or written anywhere else in the file
   (`src/backend/circt/mod.rs:29-30, 177`). Same program cannot be
   host-simulated on embedded AND pinned into fabric.

## Design decisions (recorded 2026-08-27, user-approved)

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Import surface | New declarative item `extern Name(ports) -> outs from "path";` — NOT metadata-on-cell, NOT bare-path form | Sibling of `frgn … from` (software FFI): importing foreign code is a grammatical act, not a tuning fact. `!>` carries layout facts only. Bare `extern "file";` was rejected because this toolchain's circt-translate has no `--import-verilog`/slang, so nothing could derive name+ports; explicit ports are the typed contract enforced at every instance site |
| Internal representation | Desugars to a CellDef-shaped record (+ `extern_source`) | Typechecker sealing, port checks, and the instance emitter already work on this header shape; zero consumer forks |
| Clock/reset on blackboxes | Implicit injection matches defined cells (`in %clock, in %reset` hardcoded at emission) | Defined cells get compiler-injected clocking (`emit_cell_module`); declaring them per-user would allow mis-declaration; symmetric injection makes blackbox ↔ defined module port sets identical |
| MMIO syntax | SAME declarative channel as `.ebv`: `@ 0xADDR` on triggers/state, board TOML as shared numeric source | One contract, two lowerings (pin vs memory); no second address language |
| Pin ordering | Address-sorted top-module port emission (deterministic rule) | Cousin of the HashMap determinism rule; separately compiled partitions agree on bus layout without communication |
| Dynamic addressing boundary | Runtime/computed addresses = `VolatileLoad#`/`VolatileStore#` over `Ptr` on native/embedded targets; on CIRCT a dynamic-address trigger is a capability ERROR ("hardware pins are static") | Synthesis requires static pin lists; honest boundary between worlds, disclosed via house diagnostics |
| Range reservation | DEFERRED until constraint-file/linker-script generation exists; then ONE declarative item (`reserve [a..b];`) proved once in frontend analysis | Overlap checking is meaningless before downstream tools consume ranges; see Rejected Alternatives |
| Volatile intrinsics naming | **`VolatileLoad#(Ptr<T>) -> T`, `VolatileStore#(Ptr<T>, T)`** — PascalCase + `#` per Golden Rule 4 | Snake-case names in current docs violate Rule 4; worse, they are DOC-ONLY — zero occurrences in src/, lib/, SPEC (functional volatility comes from internal `fun.volatile_read` register paths). Doc-only intrinsics are rot; they must be implemented |

## Rejected alternatives (revisitable)

**`Mmio#` binding intrinsic** (`Bind#(addr)`-style). Decomposed need:

| Need | Right home | Status |
|------|-----------|--------|
| Named static bind | `@ addr` declarations + board TOML | Track B below |
| Anonymous/dynamic bind | Deref triggers / volatile pointer access | Exists on `.ebv`: `LinkRef::Deref` takes computed ptr exprs with null-guarded volatile load (`src/backend/llvm/emit_toplevel.rs:1099-1118`) |
| Range reservation | Frontend analysis proof gate, declarative item | Deferred (above) |
| Cross-backend sync | Board TOML consumed by both backends | By construction |

A call-form intrinsic would re-parse its arguments into exactly what the
`@` parser already records — two channels for one fact invites disagreement
(single-source-of-truth violation), and per rule 14 ergonomic wrappers live
in stdlib `.bv`, not Rust arms.

---

## Slice C (first — smallest, unblocks B's embedded half):
## Implement `VolatileLoad#` / `VolatileStore#`

**Problem:** documented but nonexistent (`docs/architecture/features/ptr.md:14,173,192-193`,
`target-import.md:24-26` are lies relative to the binary).

**Work items:**
1. Intrinsic signatures in the typechecker registry:
   `VolatileLoad#(Ptr<T>) -> T`, `VolatileStore#(Ptr<T>, T)`.
2. LLVM emission: reuse the existing volatile machinery — the Deref-trigger
   null-check pattern becomes the shared helper (guard applies to NULL-ptr
   reads/writes identically); alignment from element width via casting graph.
3. Capability staging: true on LLVM/embedded/GPU; FALSE elsewhere (CIRCT
   errors naming `trg @ addr` pins instead).
4. Tests: typecheck reject/give-shape cases, IR structural assertions
   (`load volatile`, guard blocks), interpreter parity where meaningful.
5. Docs migration: rename every snake-case occurrence to PascalCase forms;
   add implementation notes (ptr.md §pointer ops table gains "since"
   column).

**Acceptance:** `brievc build` of a small fixture doing
`VolatileLoad#(reg_ptr)` emits the volatile-load block; interpreter and
contract stories unchanged.

---

## Slice A: Extern hardware declarations

### Syntax (final)

```briev
extern UartTop(rx: Bit<8>) -> byte_out: Bit<8> from "rtl/uart.v";

// generic foreign modules:
extern Fifo<T>(width: Int, push: T) -> pop: T from "rtl/fifo.sv";
```

Semicolon-terminated, bodyless (there IS no body); VHDL legal as any
tool-resolvable unit by name (`from "rtl/core.vhd"` consumes through
vendor synthesis, sim harness runs SV-only).

### Work items

1. **Parser** (`src/parser/definitions.rs`): new top-level arm after
   `frgn` handling — lex `extern` (add token + Canonical vocab entry),
   parse `Ident <type_params>? (ports_in)? -> ports_out? from string`,
   desugar into `CellDef { name, ports_in, ports_out, metadata: {"extern":
   Quoted(path)}, fields/members empty }`. All validation stays in the one
   cell pipeline.
2. **Normalizer**: extern cells join the keep-set unchanged (they ARE
   CellDefs).
3. **CIRCT emission** (`circt/mod.rs`): in the module walk,
   CellDefs carrying the `extern` marker emit
   `hw.module.extern @Name(in %clock, in %reset, ports…)` using the SAME
   port-signature computation defined cells use — one code path, branch
   only on body-vs-blackbox.
4. **Capability honesty across targets**: an extern cell reached by an
   LLVM/software build is a hard error — software binaries have no RTL
   linkage (`why:` hardware blackboxes have no object-code image; `fix:`
   compile the enclosing program for a circuit/synth target, or model the
   device in Briev). Implemented via the existing capability-gate walker.
5. **Companion collection** (`compile.rs` Circt arm): collect every
   `extern` source path, copy beside output next to memory companions;
   extend `memory_companions()` region into a generalized companion
   registry so future slices inherit the plumbing.
6. **Harness** (`tools/hw_harness.sh`): FIX the Vivado leg gap found
   during research (:111/:116 pass only the top `.sv` — forward the whole
   file list incl. rtl sources); new fixture `uart_extern` exercising
   verilator sim parity against a trivial hand-written uart stub.
7. **Tests**: parser unit (shape + generics + missing-from errors);
   structural MLIR asserts (blackbox emitted once; instance ports match);
   negative: extern under LLVM backend → capability error text; duplicate
   extern + defined cell same name → parser/type checker collision error.

### Visual contract (locked examples from design session)

```
src/main.cbv                build/main.mlir                      link step
extern UartTop(rx..)->…  ⇒  hw.module.extern @UartTop(...)    ⇒  verilator/vivado
cell Debounce(..){..}       hw.module @Debounce {body}           reads BOTH files
UartTop(debounced)          hw.instance "…" @UartTop(...)        port-for-port identical
Debounce(raw)               hw.instance "…" @Debounce(...)       call sites indistinguishable
```

---

## Slice B: MMIO pins on .cbv

### Syntax (shared with .ebv — no new language)

```briev
import "target" as board;                    // boards/<name>.toml constants

node run [false][true] {
    trg sensor: Int @ board.sensor_in;       // explicit numeric also fine
    level = when sensor > threshold { 1 } else { 0 };
};
```

### Work items

1. **Parser**: accept `@ LinkRef` on node/cell-local state + trigger decls
   for `.cbv` sources (AST path `LinkRef::Explicit(u64)` exists — verify no
   gating rejects it pre-backend; wire board-TOML symbol resolution same as
   targets do today).
2. **Normalizer keep-set**: preserve trigger address metadata into analysis
   results (frontend-driven dispatch pattern: MMIO surface computed ONCE in
   frontend, backends consume).
3. **CIRCT backend consumption**:
   - top-module input ports per addressed trigger, ADDRESS-SORTED emission
     (deterministic order rule from Decisions);
   - thread port wires into the register/wire environment so txn bodies
     read them like state (wire-map v2 register-first pattern);
   - optional v1.s: named state decls with `!> expose` metadata become
     OUTPUT pins — cut from v1 if pressure demands; document decision.
   - `LinkRef::Deref` on a circuit target → capability error: "hardware
     pins are static — dynamic addressing targets the native/embedded
     build" (name the fix; mirrors Decisions boundary).
4. **Dead-code policy (rule 3)**: while touching this area, either fold
   vestigial `mmio_vars` (`circt/mod.rs:29-30,177`) INTO the new consumed
   flow or delete it outright; likewise decide LLVM-side fate of unused
   `extract_target_addresses` (`hardware/handoff.rs:304`, test-only caller)
   — no silent zombie fields survive the slice.
5. **Board-contract tests**: one TOML drives fixture through BOTH
   extensions; assert identical resolved numerics surface in IR (inttoptr
   constant) and MLIR (port present, sorted position).

### Dual-target visual

```mlir
;; .cbv lowering (new)
hw.module @top(in %clock: !seq.clock, in %reset: i1,
               in %sensor: i64,                     ;; ← trg @ addr
               out halt: i1, out check: i1, out level: i64)
```

```llvm
; .ebv lowering (exists — unchanged)
%level = load volatile i64, ptr inttoptr (i64 0x40010000 to ptr)
```

Same number enters both from `boards/<name>.toml`.

---

## Verification matrix

| Check | Mechanism |
|-------|-----------|
| Slice C | new cargo tests (typecheck + IR structural); doc/table sweep green |
| Slice A | parser units; MLIR structural; `uart_extern` harness fixture e2e through verilator; Vivado leg passes full file list |
| Slice B | dual-target fixture asserts (numerics equal across `.ll` const & `.mlir` port); sorted-order test; Deref-rejection diagnostic test |
| Regression | all HW fixtures still emit valid MLIR (`--no-std` path), conformance sweep 209-green stays, parity + HW harnesses green post-B1.1-sync |
| Praetor/Kani | changed-file gates per commit checklist; Kani harness on reserve/address overlap logic IF slice lands |

## Documentation updates (same-commit rule)

- `spec/SPEC.md`: §8-analogous addition for `extern` items; §trigger/§19.7
  MMIO sections extended with pin semantics + static-pin boundary
- `learn-briev/07-ffi.md` gets the hardware half; new short section in
  `05-data-types.md` cross-linking extern cells
- `docs/architecture/features/ptr.md`, `target-import.md`: PascalCase
  intrinsics, implemented-now status
- `docs/architecture/backend-contracts.md` §6: blackbox invariant +
  sorted-pin invariant added alongside wire-map rules
- syntax highlighter: `extern` keyword entry when landing slice A

## Sequencing

**Slice C → Slice A → Slice B.** Rationale: C is self-contained and stops
the doc-lies bleeding; A introduces companion plumbing that B's fixtures
reuse for constraint outputs; each slice ships independently green.

## Out of scope (recorded)

- Port DERIVATION from HDL (`briev link rtl/x.sv` generating extern items
  via external yosys/slang) — v2+ tooling land, stays out of compiler
- `reserve` range items until constraint/linker generation exists
- Non-SV simulation of VHDL in the parity harness (synthesis-through-vendor
  only)
