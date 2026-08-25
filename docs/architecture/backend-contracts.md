# Backend Contracts & Decision Record

**2026-08-23** · Distills every architectural decision from the backend
scaffolding effort (plans `2026-08-23-backend-scaffolding-foundation.md`
through `-webstack-v2-completion`, all CLOSED). This is the normative
reference for what each backend IS, how it integrates, and which emission
invariants are load-bearing. **Read this before touching any
`src/backend/<name>/` code.**

Regression prevention rule: if you change something documented here,
update this document in the same commit and say why.

---

## 1. Per-backend charters

| Target | Extension | Charter |
|--------|-----------|---------|
| **LLVM** | `.bv` `.ebv` | Reference implementation. Native binaries, embedded mode (`halt#`→`wfi`), wasm32 for webstack. Full language surface. |
| **VM** *(emit mode)* | none — `--backend vm` | Finish compilation on ANY machine with a tamer: one `.bounty` archive ships everywhere; macros adapt at install time. Output is `.lair` bytecode consumed by the self-hosted tamer (`lib/tamer/`) driven by `tamer/install_sim.c`. |
| **SPIR-V** | `.abv` | Standalone GPU kernels: valid Vulkan/OpenCL compute binaries validated by spirv-val. Kernel selection/bodies come from the frontend accel analysis. NOT related to `!> accel` offload (that is `BackendKind::Gpu` through LLVM). |
| **CIRCT** | `.cbv` | Synthesizable register-level hardware: MLIR (HW+Comb+Seq) that real CIRCT accepts, Verilog-exportable, simulable, synthesizable. |
| **Webstack** | `.rbv` | Rendered Briev → wasm32 via `LlvmBackend` + `GlueWebGenerator` JS shim (`src/glue/web_generator.rs`). v2 only — the TS emitter is deleted. |

Routing truth lives in `config/targets.dbvl`; dispatch in `src/compile.rs`
`codegen()`. The VM has no file extension BY DESIGN — it is an emit mode
of the same language, reachable via `--backend vm` or `brievc bounty`.

---

## 2. Uniform integration contract

1. **Analysis once.** `analyze_program` runs in the pipeline
   (`compile.rs codegen()`), never inside a backend. Every backend
   consumes the same `AnalysisResults` (dependency graph, accel entries,
   …). Backends CONSUME frontend decisions; they do not re-derive them.
2. **Capability matrix required.** Any backend without the full surface
   declares `CAPABILITIES` (see `src/backend/capabilities.rs`) and the
   pipeline runs `validate_program` BEFORE codegen. Out-of-surface
   constructs are compile errors with what/why/fix — never runtime traps,
   never silent drops.
   - ⚠️ LLVM claims full surface and skips the gate. That claim must stay
     true: the Statement::Match regression proved an unguarded "full"
     claim hides holes. If LLVM gains a gap, either fix it or shrink the
     declaration honestly.
3. **Errors channel pattern.** Constructs discovered unsupported DURING
   emission go into a backend error accumulator
   (`VmBackend.errors`, `CirctBackend.errors` RefCell); the pipeline
   hard-errors non-empty after generate. Pattern: record_unsupported()
   with dedup.
4. **Determinism law.** ANY HashMap iteration whose order feeds emitted
   output OR program rewriting must be sorted by key (or use BTreeMap).
   Violations produced different binaries per process (BUGS.md
   "rbv nondeterminism", CIRCT cell modules).

---

## 3. Cross-cutting emission laws

Hard-won in this effort; each one corresponds to a real defect class.

| Law | Failure it prevents |
|-----|---------------------|
| Result ids exist only when a result type exists | Phantom operands on result-less ops (OpUnreachable wc=2) |
| Terminators via TYPED builder methods only | Raw inserts leave selected_block open → NestedBlock panic |
| Decorations carry EXPLICIT operand lists | dr::Builder::decorate silently dropped params on the wire |
| SPIR-V globals section ordered: annotations → types/constants → variables | §2.4 layout violations ("invalid layout section") |
| Function-scope OpVariables are the FIRST instructions of entry block | "All OpVariable instructions…first block" |
| LoopMerge targets: BOTH merge and continue blocks always exist; continue carries the unconditional back-edge | Undefined forward refs / zero-back-edge loops |
| Guards: PRESET the slot, then ONE conditional set. Never two sequential whens where the second reads the mutated slot | EQ returned 0 for equal values (tamer), Abs-style logic corruption |
| Struct-of-array element reads LOAD; address-handles only for genuine structs | Pointers pushed onto value stacks |
| Contract placeholder wires forbidden — the condition op IS the definition | Duplicate result definitions |

---

## 4. VM / tamer specification

### 4.1 `.lair` format (writer: `vm/assembler.rs`; reader: `tamer/install_sim.c`, `lib/tamer/loader.bv`)

```
header: "LAIR"(4) ver(u32)@4 endian(u8)+rsvd@8 flags(u32)@12
        then u64 section offsets/sizes @16..80:
        str_off str_size fn_off fn_size bc_off bc_len host_off host_size
strings: NUL-separated name table
fn entry: 20 B = name_idx(u32) bc_offset(u64, SECTION-relative)
          + bc_len(u32) local_count(u16) arg_count(u16)
host entry: 12 B = name_idx(u32) id(u32) arity(u32)
bytecode: opcode stream (opcodes in src/backend/vm/assembler.rs)
header total: 96 bytes
```

- **Addressing is BASE-ABSOLUTE**: readers index from the lair pointer;
  fn-table `bc_offset`s are section-relative and converted by the
  producer (manifest `entry_bc` ships ABSOLUTE).
- Multi-byte readers stitch BYTES via read_u8 (unaligned-safe).
- Host-table lookup lives in C (`briev_rt.c briev_host_arity_of`) — the
  Briev-side struct walk across the defn boundary was unreliable.

### 4.2 Canonical host-service ids

`Print#=0, Log#=1` (src/backend/vm/mod.rs). Unknown services ≥1000,
first-use order among themselves, rejected loudly at run time
(`briev_host_fail`). Names ride in the string table for diagnostics only.
Arity is recorded per call site into the 12-byte host entry — the
interpreter pops exactly that many slots.

### 4.3 Export ABI (Briev → native)

EVERY exported defn takes a LEADING `%state` pointer. C drivers calling
exports MUST declare it; omitting shifts every argument by one register
and produces phantom behavior. State writes from defn bodies do NOT
round-trip through the %state view — C owns memory/tables; Briev exports
pure functions (the `step()` pattern).

### 4.4 Bounty archive

Magic `BOUNDATA\0`(9) + ver/flags/count (u32 ×3 = 21 B header);
per-section `type(1)+offset(u64)+size(u64)` = 17 B entries; data at
absolute file offsets. Manifest JSON includes `entry_bc` (absolute byte
offset of the user entry function — user fns emit AFTER prelude helpers).

### 4.5 Conformance metric

`tools/parity_harness.sh` + `parity_expected_values_match_independent_evaluation`.
Corpus: `tmp_fixtures/parity/*.bv` (OWNED by this plan — sweeps must not
delete). One fixture per opcode family as opcodes grow. EXPECT lines are
locked against an independent evaluator, so host semantics == baked
contract == tamer behavior.

---

## 5. SPIR-V specification

- **Typed dr::Builder emission ONLY.** Raw insert_types_global_values of
  types/decorations/constants caused a seven-defect stack (duplicate ids,
  dropped decorate params, misordered layout). TypeCache replaced by a
  Briev-type dedup map over typed helpers; single id space.
- **build() buckets the globals section**: decorations → types/constants →
  variables (SPIR-V §2.4 puts annotations in their own section).
- `instr()` allocates a result id ONLY when a result type exists.
- LoopMerge names merge AND continue; both blocks always emitted; the
  continue block carries the unconditional back-edge (exactly one).
- Function-scope OpVariables pre-declared in ENTRY (locals pre-scanned via
  `collect_locals`); values stored later.
- Kernel selection/bodies from `AnalysisResults.accel`: eligible
  AccelEntries only; body = `shape.kernel_stmts` (PROVEN statements);
  `shape.index_var` binds to GetGlobalId(0). A GLCompute invocation IS one
  work item — no induction loop.
- Entry-point interface lists every Input variable + the SSBO variable.
- State = ONE Block-decorated StorageBuffer: fields sorted by name,
  explicit MemberOffset layout, ArrayStride on arrays, descriptor set 0
  binding 0. Reads/writes via AccessChain (+Load/Store).
- Canonical host ids mirror the VM table (`GetGlobalId#` etc. lower to
  BuiltIn inputs here).
- Validation: `test_scale_kernel_passes_spirv_val` (spirv-val) +
  structural assertions on the in-memory module. OPEN: Vulkan smoke test
  needs a runner.

---

## 6. CIRCT specification

- **Ports:** clock is `!seq.clock` (never i1); triggers/state render via
  mlir_type.
- **Sequential semantics (wire-map v2, 2026-08-25 — registers FIRST):**
  hw.module bodies are MLIR graph regions; circt-opt accepts
  use-before-def (probed). Phase A — init constant + `seq.firreg` per
  output var, each register consuming a forward-named `<var>_next` wire;
  the wire-map points at REGISTER OUTPUTS from the start. Phase B — txn
  bodies emit against live register outputs; assignments compute new
  wires and repoint pending (reads see cycle-start state = NBA
  semantics); guarded bodies mux on condition. Phase C — defines the
  forward-referenced `<var>_next = mux(%reset, init, pending-or-reg)`
  wires. Reset forces init; unwritten vars hold.
  The previous scheme (bodies read init constants) folded guards to
  compile-time constants and fired transitions unconditionally — a
  guarded counter ran past its bound, diverging from the interpreter at
  the bound cycle. See BUGS.md (2026-08-25).
- **Contract obligations as ports (2026-08-25):** pre-guard evaluated on
  live state GATES the commit: `commit = mux(pre_ok, computed_next,
  current_reg)` — refusal holds state and raises `halt` (registered OR
  of refusals). Post-guard is evaluated on COMMITTED next values via a
  shadow wire-map, as an implication `¬pre ∨ post` (a refused txn
  carries no post obligation), ANDed into the `check` output port.
  Toolchain findings driving this form: pinned build has NO `seq.assert`;
  module-level `sv.assert` is rejected ("non-procedural region");
  procedural `sv.assert` inside `sv.alwaysff (posedge %i1)` works but
  would need an extra i1 event input — ports need none and are
  simulatable everywhere. An inconsistent contract pair (post falsified
  by an accepted commit) shows up as a check drop in simulation — that
  is the violation signal, and sim parity caught exactly this on the
  counter fixture's original `[c<255][c<255]` pair.
- **Multi-txn arbitration:** several txns writing one var resolve by
  program order (last commit gate wins). Single-txn-per-var is the
  tested surface; beyond it stays honest via this documented rule.
- **Type lowering:** universe-driven (rule 19) — width from rt.bytes,
  signedness from protocol category (`Cast.Int`→siN, `Cast.UInt`→uN,
  `Cast.Float`→fN); BitRange widths honored. REQUIRES the normalizer
  keep-set to preserve `Cast.*` properties (they were being stripped —
  that was the "everything renders i64" bug). Top-level `let` IS state
  and must reach var_types.
- **Honest comb subset ONLY:** add/sub/mul/divu/divs/modu/mods/shl/shru/
  and/or/xor/parity/mux/icmp/neg/hw.constant. comb has NO ctpop/ctlz/
  cttz/rev/sqrt/sin/cos/pow/floor/ceil — those arms emitted unparsable
  MLIR and are deleted; unknown intrinsics record capability errors.
  Abs# lowers honestly (neg+icmp+mux).
- **No silent drops:** every unsupported construct records into
  `errors` (RefCell) → pipeline hard-error. cell_defs iterated SORTED.
- **Contracts:** see "Contract obligations as ports" above — pre gates
  the commit (refusal ⇒ hold + halt), post is an implication on
  committed values into `check`.
- `UInt[N]` source syntax is an ELEMENT ARRAY (Vector), not a sized
  scalar; Constrained/BitRange types are currently programmatic-only
  (parser gap, logged).
- Validation (2026-08-25, all tiers LIVE): probe-gated `circt-opt`
  round-trip (`test_emitted_module_parses_under_circt_opt`,
  `circt_tools_available()`); full chain in `tools/hw_harness.sh`
  (parse → lower-seq-to-sv + export-verilog → verilator lint →
  SIMULATION PARITY — 270-cycle verilator --binary run diffed against
  locked `.expect` sequences derived from interpreter semantics,
  generators in `tmp_fixtures/hw/*.expect.gen.py` → optional Vivado);
  synthesis via `tools/vivado_check.sh` on KV260 xck26-sfvc784-2LV-c —
  generated counter FSM synthesizes to 21 LUTs
  (`VIVADO_SYNTH=1 VIVADO_REPORT_DIR=... tools/hw_harness.sh`).

---

## 7. Webstack specification

v2 only: `LlvmBackend` (wasm32-wasi target) emits the module;
`GlueWebGenerator` emits the JS shim; flush batching implemented in
`emit_stmt.rs` (`__web_flush_buf` / count-parameterized
`__web_flush_state` at term boundaries). AddressOf# is implemented in
LLVM intrinsics (eprintln diagnostics remain an opportunistic sweep).
There is no TS emitter.

---

## 8. Known limitations (open, deliberate)

1. **Convergent-txn loop engine** — a convergent txn whose guard never
   changes across iterations may spin (param-slot stores not persisting
   was observed pre-refactor; superseded architecture means it is
   UNVERIFIED today). Re-run the lib/tamer/vm.bv vm_loop repro before
   trusting either verdict. Related: Version-DAG phi fix (CLOSED).
2. **Sized-scalar source syntax** — BitRange types unreachable from the
   parser (programmatic-only). Spec decision needed.
3. **Vulkan smoke test** — needs an installed runner.
4. **LLVM diagnostics sweep** — eprintln warnings (AddressOf#, main.rs
   unused) remain an opportunistic house-style pass.
