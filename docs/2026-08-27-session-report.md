# Session Report — 2026-08-27 (backend scaffolding tail + cbv foreign HW/MMIO)

**Author surface:** backend-foundation worktree sessions (opencode/GLM).
**End state:** `backend-foundation` @ `24bf3eef` — canonical, 1988 tests
green. This document is the pickup point for the next agent.

---

## 1. End-state map (UPDATED after single-tree consolidation)

| Tree | Commit | State |
|------|--------|-------|
| `briev-lang` (branch `main`) | `e11af56a` + this doc commit | **THE tree — single source of truth.** All plans implemented; 1991 tests green |

**Single-tree consolidation (same day):** the axiom agent's base
(`f189fc38`) + their in-flight authority fields were rescued and
completed on main (`60f9d8b9` — Contract.post_authority /
OperatorDef.trusted_lemmas / CastEdge.trusted_axiom + the 232-site
initializer sweep they hadn't finished; 1991 green). Foundation was
merged into main (`e11af56a`, `-X ours` on axiom-sweep files where the
sync daemon had hunk-flipped) and both trees converged, after which ALL
auxiliary worktrees/branches were retired as verified ancestors of
main: `backend-foundation`, `compiler-in-brief`, `feat/accel-gpu`,
`feat/out-observability`, `bits-strings`, `feat/language-decluttering`,
`feat/derivation-synthesis`, `feat/data-brief`, `feat/spec-layout-keywords`,
plus the dead dirs (dogfood, out, accel, spec, `briev-compiler-baseline`
— its snapshot `0f97c1c7` verified ancestor) and `/tmp/opencode` scratch.

**Kept:** `briev-lang/` (canonical) and `briv-compiler-baseline/` —
AGENTS rule 12b's persistent A/B baseline worktree, genuinely important
for compare_baseline.sh regression work.

Every branch named above is a verified ancestor of main: no stale code
seeped, none was lost that wasn't already in the tree.

## 2. Session ledger (commit index)

**Completed plans:**

- **VM compile-tail parity — COMPLETE** (`20a6fe24`, `4d4d8433`): §1.3
  resolved WITHOUT new opcodes — the corpus demanded top-level const
  REFERENCES; `VmBackend.const_values` resolves transitive const-to-const
  (cycle-rejecting) and inlines as PUSH_I64. Tamer self-package
  (`brievc build lib/tamer/main.bv --backend vm`) works for the first
  time; parity harness passes end-to-end. §1.5 determinism audit: one
  real fix (`field_offset_any` sorted iteration); all other emission
  paths audited Vec/order-safe.
- **SPIR-V kernel emission — COMPLETE** (`1b26d9bd`, `7c18fcf6`,
  `2286f09d`): §2.3 Load#/Store# as SSBO ADDRESS EXPRESSIONS (numeric
  addresses cannot exist in Vulkan space — capability errors name the
  fix); §2.4 universe-driven scalars via a casting-graph SPIR-V table
  (Bool = OpTypeBool, Int signedness=1 fixing latent SDiv/SLessThan
  violations; heap categories + out-of-range widths = capability
  errors); §2.5 spirv-val + spirv-dis structural sweep helper,
  Vulkan-runner smoke probe-gated; §2.6 strategy-doc v2 table.
- **Enum variant construction — COMPLETE** (`4bb965cb`, `38943d5c`;
  migrations verified done `2026-08-23`): bare + qualified
  `Enum::Variant` construction/patterns, ambiguity detection, LLVM
  handle ABI (multi-payload), stdlib chains green (result/option/
  process/string), json.bv archived with blocknotes preserved.
  Conformance sweep recount: **209 active sources, 0 failures**.

**Bug-ledger closures:**

- `cbd2f87b` — plugin-disable flag was a silent no-op under extension
  filters (`active_plugins` early-returned from `enabled_only` before
  consulting `disabled`); `--no-std`/`--disable-plugin prelude` now
  actually work. Found via the CIRCT probe.
- `3849e140` — CIRCT call fallthrough instantiated modules that were
  never defined (plain fns, enum ctors). Now: cells only; anything else
  records the house capability error.
- `cf7840f9` — sweep recount; stale "40 enum-gated chains" note closed.
- `4165965b` — bug sweep B1.2: protocol round-trip backend skip arm →
  hard error (what/why/fix). Interpreter side was main's `717b3a34`.
- `2dfc8d75` — axiom agent's half-wired WIP completed forward (not my
  feature; kept their semantics).

**cbv foreign hardware + MMIO plan (all three slices):**

- `7f071f22` plan; `d59d1ecf` Slice C (VolatileLoad#/VolatileStore#
  implemented for real — they were DOC-ONLY snake_case fiction before;
  Rule-4 PascalCase, typechecker Ptr gate, boxed-ABI LLVM emission,
  VirtualHeap interpreter arms); `fe5dadb7` Slice A (extern HDL
  imports: lexer/vocab/parser/capabilities/blackbox emission/
  companion copying); `f0092994` uart_extern fixture + extern grammar
  corrected from upstream `.td` (one-paren-list, bare output names, no
  `-> (outs)` wrapper); `cd03cce0` Slice B (MMIO pins: read-only
  trigger VALUES, address-sorted `mmio_vars` port emission, static-pin
  capability boundary, LLVM volatile-load reads, value-pins excluded
  from event dispatch); `a19cbe44` numeric-address fixtures; `24bf3eef`
  language-truth docs (SPEC §13.1, learn-briev/07 §4b, contracts §6
  invariants, highlighter `extern`).

**Docs:** planned-features-tracker.md (master status), AGENTS.md index
rows, backend-strategy.md (SPIR-V v2 table, VM emission facts), plan
docs per feature. See tracker for deferred-work index with trigger
conditions.

## 3. Working discipline for THIS machine (environmental hazard)

BUGS.md carries the hazard entry ("external process reverted in-flight
edits"); this section is the countermeasure playbook the next agent
should adopt from the start:

1. **Symptom:** freshly edited files returned to pre-edit content
   minutes after a verified green build; mtimes showed the snapshot's
   ORIGINAL timestamps. Selective (some files, not others). Suspected
   bidirectional sync/mirror service on `~/Desktop/Projects`.
2. **Countermeasures that worked:**
   - Source edits + builds + tests inside an isolated clone
     (`/tmp/opencode/slice_a`); push to the real worktree via
     `git fetch /tmp/… && git merge --ff-only` only.
   - **Absolute paths** for every cross-repo operation; never trust a
     bare `cd` to persist between tool invocations (observed cwd drift
     into `briev-lang` repeatedly — that is how stray files landed on
     main once, requiring the `6839fd7f → bc1dc653/2afcdd3b` rescue).
   - Tiny commits immediately after each verified step; the foundation
     worktree is only ever a ff-merge target, never an edit surface.
   - md5-compare before deciding a file "lost" an edit (the revert can
     be partial — some files restored, siblings intact).
3. **One structural gotcha:** bash tool cwd resets to
   `briev-lang` between invocations regardless of intent — pass
   `workdir` explicitly on EVERY command that touches a tree.
4. **Post-convergence note (2026-08-27 evening):** once main and
   foundation converged to identical content, the mirroring stopped
   flipping hunks (identical trees = idempotent mirror) — and the aux
   trees were then retired entirely. If edits mysteriously revert on
   the SINGLE tree, the §2 playbook applies unchanged; also re-check
   whether any new worktree/sync service appeared.

## 4. Known-live gates (do not misread as regressions)

1. **B1.2 protocol gate** — any real-pipeline build importing the
   stdlib prelude (tamer packaging, HW/parity harness compile steps,
   examples) hard-errors naming the missing conversion bodies
   (`ascii_to_utf8`/`utf8_to_ascii` for ASCII, `utf16_to_utf8` for
   UTF16, Posit32 pair) until the axiom agent lands B1.1 in
   `lib/std/protocols.bv`. The diagnostic is the designed behavior —
   land the bodies, everything goes green.
2. **HW harness loud-SKIP** — CIRCT toolchain binaries are MISSING from
   both worktrees (only `tools/circt-src` remains; `tools/circt/bin/`
   vanished — see BUGS.md hazard suspicion). Rebuild with
   `tools/install-circt.sh`; the harness then runs the full corpus
   including `uart_extern` (linkage fixture) and the firmem companions.
   The extern-blackbox grammar was corrected from upstream `.td`
   sources (`HWStructure.td`/`ModuleImplementation.cpp`) in lieu of
   runtime validation — the structural test locks the shape; one
   circt-opt parse run post-install is the residual verification.
3. **`cargo test --lib` is the green baseline** (1988) — the suites
   above are pipeline-level and sit behind gate (1).

## 5. Deferred-work index (with trigger conditions)

| Item | Trigger |
|------|---------|
| Collections Phase E modifiers (`seq`/`vol`/`async`/`sync<g>` + concurrency gate) | User deferred (large track) |
| `reserve [a..b];` range declarations | When constraint-file/linker-script generation exists |
| Board-symbol `.cbv` addresses (`trg x @ board.sensor`) | When target-import symbol resolution reaches the circuit surface |
| Typed trigger ports (`trg x: Int @ …`) | Trigger grammar gains a type position |
| Host-sim address remapping | When host-side MMIO simulation is wanted (real addresses fault host processes by design) |
| Accel eligibility for Load#/Store# bodies | GPU offload corpus demands it (lowering locked by direct-shape tests) |
| Vulkan runner smoke fixture | A runner (vkm/vkrunner) is installed |
| phase2b2 dynamic component counts | `docs/plans/2026-08-11-phase2b2-instance-state.md` owns it |
| Port derivation from HDL (`briev link rtl/x.sv`) | v2 tooling; stays out of the compiler (rule 14) |

## 6. First tasks for the next agent

1. ~~ff to main~~ DONE — main converged past `24bf3eef` through
   `e11af56a` (axiom base + rescued fields included).
2. Tracker hygiene: items 2/3/7 and bug #1's partial-line still show
   stale "Remaining" text; actual states are DONE/verified-in-sweep
   (superseded by this report — fix if the tracker is still the
   status surface when you read this).
3. `bash tools/install-circt.sh` then `bash tools/hw_harness.sh` —
   expect full parity including `uart_extern`, and (post-B1.1) zero
   compile failures.
4. After B1.1 lands: re-run `tools/parity_harness.sh` end-to-end and
   update tracker bug #1 to RESOLVED.
