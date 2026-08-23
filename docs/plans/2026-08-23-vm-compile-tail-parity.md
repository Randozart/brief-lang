# VM backend — compile-tail parity for bounty archives

**Date:** 2026-08-23
**Status:** active — §1.1 (arg-drop fix), §1.4 (diagnostics), §1.2
(parity harness + div/rem opcodes + bounty e2e asserting real output)
landed 2026-08-23; see BUGS.md for the match-drop root cause that gated
execution and its four-bug fix chain.
**Sequencing:** parallel branch; requires `2026-08-23-backend-scaffolding-foundation.md`
(Plan 0) merged first. Work confined to `src/backend/vm/`, `lib/tamer/`,
`lib/std/` (only if opcode support needs it), own tests, own doc sections.

## Charter

The VM exists to **finish compilation on any machine with a tamer**: macros
fully adapt to the target machine, and a single `.bounty` archive ships
everywhere instead of per-target binaries. The parity metric is therefore
**compile-tail parity**, not full-language runtime parity:

> Every macro stage block, derivation, and const-eval body produces the
> identical expansion under the tamer (from `.lair`) as under the host
> evaluator (`src/macros/eval.rs`) during development compiles.

## Baseline state (2026-08-23)

| File | Lines | State |
|------|-------|-------|
| `vm/mod.rs` | 177 | Entry; takes `_universe` and ignores it; struct field offsets via hand-rolled sizes |
| `vm/emit_toplevel.rs` | 106 | defn/txn/const emission subset |
| `vm/emit_stmt.rs` | 190 | Let/Assign/Guarded/Term subset |
| `vm/emit_expr.rs` | 297 | Int/Ptr literals+arith, If/Match(partial), Call/hcall, Index, Field, Deref |
| `vm/assembler.rs` | 539 | Opcode table 0x00–0xB0 + label fixups |

Reachability: only via `brievc bounty` (`main.rs:441`, hardcodes
`BackendKind::Vm`) — no file extension routes to it (`targets.dbvl:16-20`).

### Known correctness bugs

1. **Intrinsic argument dropping** — `vm/emit_expr.rs:198-204`: for
   intrinsic calls, *every* `Expr::Identifier` argument is skipped. Comment
   claims only "PascalCase strategy identifiers" are skipped; the code skips
   ALL identifier args. Any intrinsic call taking a variable argument
   (`Alloc#(buf)`, buffer-style host calls) receives wrong arguments at
   install time.
2. **Float literals trap silently** — `vm/emit_expr.rs:34-40`: push 0 +
   trap. No diagnostic, no source mapping.
3. **Match arms fall through** — non-literal patterns silently fall through
   to body (`vm/emit_expr.rs:164-169`); no-arm case pushes 0 (:175-177).
4. **Silent unknown-function trap** — `vm/emit_expr.rs:220-223`.

## Work items

### 1.1 Fix the arg-drop bug (correctness, do first)

Emit all arguments in reverse order. Strategy-tag skipping, if genuinely
needed by the bounty builder's host-call convention, must key on an explicit
marker (tagged literal or declared strategy param), never on expression
shape. Add regression tests: intrinsic call with Int local var arg must
pass the value.

### 1.2 Compile-tail corpus + parity harness

Build the fixture corpus: macro stage blocks / derivation blocks / const
evals exercising every `Sandbox` capability (`macros/eval.rs:105`) and
`NavValue` shape. Harness compares:

- Host path: `evaluate_stage_block` output
- Tamer path: same bodies compiled to `.lair` (VmBackend), executed by the
  tamer (self-hosted, `lib/tamer/*.bv` compiled native)

Diff expansions; mismatch = failure. This harness is the VM's benchmark
suite; wire it into `cargo test --lib` (fast subset) and an integration
script (full).

### 1.3 Opcode floor = what the corpus demands

Add opcodes ONLY when the corpus proves need. Each new opcode is a triplet,
landed together:
1. `assembler.rs` opcode const + emit method
2. tarm interpreter arm (`lib/tamer/vm.bv` et al.)
3. parity-harness test through both paths

Expected near-term needs from macro surface: string ops beyond PUSH_STR
(concat exists at language level via rewrite), float arithmetic if any
stage uses Float, bounded collection iteration if stages use List.
Anything beyond → capability error (Plan 0 matrix), never a silent trap.

### 1.4 Install-time diagnostics

Traps currently lose all provenance. Trap payload carries fn-index + PC;
the beastpack manifest maps back to source names so install-time failures
read like compiler errors: which macro/stage failed, why, what to do.
House diagnostics style (`src/errors.rs`): what/why/fix.

### 1.5 Determinism

Audit all HashMap iteration in `vm/*.rs` emission paths; sort by key
(house rule — SipHash seed varies per process). `mod.rs` field_offset_any /
collect_declarations iterates insertion-ordered Vec today but confirm;
struct_fields lookups fine, any future iteration must sort.

### 1.6 Debug reachability

Route `.vbv` extension → Vm in `targets.dbvl` (add line) so VM output is
buildable outside `brievc bounty` for debugging. Update target docs.

## Documentation maintenance

- `docs/architecture/backend-strategy.md`: VM section rewritten around the
  finish-compilation charter + parity harness (replaces implicit framing).
- Rationale comments preserved; arg-drop fix comment states the old bug,
  date, and why shape-guessing was wrong.

## Verification

1. Arg-drop regression tests fail-before/pass-after.
2. Parity harness green on full corpus (host == tamer expansions).
3. Bounty round-trip integration test: compile natively vs
   bounty→install-simulate → identical observable output on corpus.
4. `cargo test --lib` green; Praetor clean on `src/backend/vm`.
