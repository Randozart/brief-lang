# SPIR-V backend — real kernel emission for standalone `.abv`

**Date:** 2026-08-23
**Status:** active — §2.1 core landed 2026-08-23: real statement/
expression lowering over locals + invocation-id builtins + one SSBO
binding for indexed state (src/backend/spirv/lower.rs); capability
errors replace silent drops; structural tests on the in-memory module.
Assembled-binary validation FIXED + spirv-val PASSES (BUGS.md closed —
seven stacked emission bugs documented there). §2.2 LANDED 2026-08-23: compile_spirv consumes AnalysisResults.accel —
selection = eligible entries; body = the PROVEN kernel_stmts (not raw);
index_var binds to get_global_id(0); entry_name validated ("main" =
wildcard). The [idx<N] sniffer and the induction-loop skeleton are gone —
a GLCompute invocation IS one work item. Remaining: Vulkan smoke test when
a runner is available.
**Sequencing:** parallel branch; requires Plan 0
(`2026-08-23-backend-scaffolding-foundation.md`) merged first. Work confined
to `src/backend/spirv/`, own tests, own doc sections.

## Charter

`.abv` compiles to a **valid SPIR-V binary** loadable by Vulkan/OpenCL
compute: kernels with real bodies, correct memory model, validated by the
SPIRV-Tools toolchain. The accel GPU-offload path (`!> accel` metadata →
`BackendKind::Gpu` → LLVM emitter reuse, plan `2026-08-06-accel-gpu-offload.md`)
is a separate mechanism and is NOT touched by this plan.

## Baseline state (2026-08-23)

| File | Lines | State |
|------|-------|-------|
| `spirv/mod.rs` | 92 | `compile_spirv(items, entry_name)`; walks txns, `is_kernel` filter; `entry_name` never used |
| `spirv/kernel.rs` | 138 | Emits loop skeleton only; **body block = placeholder `Op.Return`** (:91-97) |
| `spirv/types.rs` | 164 | Small type map; `_ => Err("unsupported type")` |
| `spirv/intrinsics.rs` | ~60 | Only GetGlobalId#, GetLocalId#, WorkgroupSize# |
| `spirv/builder.rs` | ~90 | rspirv `dr` module builder |
| `spirv/normalizer.rs` | 106 | Wired (compile.rs:861) |

### Known falsehoods / gaps

1. `mod.rs:2-4` doc claims "Load#, and Store# intrinsics" supported — they
   are not implemented (`intrinsics.rs` has three emitters, `_ => Err`).
2. Kernel detection is ad-hoc `[idx < N]` precondition sniffing with a
   constant bound (`kernel.rs:12-14,129-138`) — ignores the frontend's
   accel analysis entirely.
3. Body block emits nothing (placeholder `Op.Return`, kernel.rs:91-97).
4. One test asserts non-empty bytes (`mod.rs:57-91`); no spirv-val.
5. No statement/expression lowering exists at all.

## Work items

### 2.1 Statement/expression lowering (the core)

Implement real body emission over rspirv structured blocks:

- **Expressions:** Int arithmetic (IAdd/ISub/IMul/SDiv/SRem), bitwise
  (BitwiseAnd/Or/Xor/Not), shifts, comparisons (SLessThan etc.), Bool ops
  (LogicalAnd/Or/Not), Select for If-expressions, constants.
- **Statements:** Let/Assign via Function-storage variables or SSA with
  OpPhi at merges; Guarded → OpSelectionMerge + OpBranchConditional;
  Term/EndProgram → OpReturn/OpReturnValue.
- **Loop shape:** reuse the existing LoopMerge skeleton from kernel.rs,
  feeding it the lowered induction + body instead of the placeholder.
- **State access:** kernel-visible state in `StorageClass::StorageBuffer`
  behind a Block-decorated struct; OpAccessChain loads/stores; descriptor
  set/binding decorations emitted deterministically (sorted by field name).

### 2.2 Frontend-driven kernel selection

Replace `[idx < N]` sniffing with `AnalysisResults.accel` entries consumed
via Plan 0's `BackendContext` — the same proof the LLVM offload path uses.
Honor `entry_name`: compile.rs:1907 currently hardcodes `"main"`; pass the
resolved entry through and error helpfully when the named entry has no
accel proof. Keep `is_kernel` as a fallback ONLY for direct-API callers
with a deprecation path — pipeline always uses analysis.

### 2.3 Load#/Store# + honest intrinsic surface

Implement Load#/Store# over StorageBuffer access chains (making the mod.rs
doc true). Extend per need with capability-matrix entries (Plan 0);
unsupported intrinsics produce matrix errors, not ad-hoc strings.

### 2.4 Universe-driven types

types.rs lowering derives integer widths/floating kinds from the
TypeUniverse/casting graph (protocol category properties), not from growing
name matches (rule 19). Unsupported type → capability error naming the fix.

### 2.5 Validation harness

Tests run `spirv-val` on every emitted binary (installed locally; probe +
skip-with-loud-warning pattern if absent). `spirv-dis` structural checks:
entry point present, loop structure, storage buffer declarations. Optional
gated smoke test executing a fixture kernel when a Vulkan runner exists
(probe-gated, skipped otherwise).

### 2.6 Doc truth

Rewrite `spirv/mod.rs` header to describe exactly what v2 supports after
this plan; remove aspirational claims.

## Documentation maintenance

- New section or update in `docs/architecture/backend-strategy.md` SPIR-V
  part (branch-owned edit; Plan 0 fixes only the routing tables).
- Rationale comments dated 2026-08-23 where placeholder code is replaced.

## Verification

1. Fixture kernels (vec-add style mirroring an accel benchmark shape)
   pass `spirv-val`; disassembly shows real loop + body + buffer access.
2. Kernel selection comes from accel analysis (fixture with non-kernel txn
   alongside kernel proves filtering).
3. `cargo test --lib` green; Praetor clean on `src/backend/spirv`.
