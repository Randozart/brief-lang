# Plan: GPU backend hardening — `brievc run`, the resident-launch gate, and the f16 fault hunt

**2026-09-01.** Three tracks, ordered small → soundness → perf-unlock.

## Track A — `brievc run x.abv` (small, unlocks iteration)

A single subcommand: parse → typecheck → analyze → emit kernels → launch
through the RT in-process → download → report. No generated runner file,
no gcc round trip.

1. `brievc run <file.abv>` in main.rs: reuse `compile.rs`'s pipeline
   through kernel emission, then:
2. `briev_accel_init(&desc)` with the emitter's field table (the runner
   generator's `ssbo_layout` output drives both — one layout, one source).
3. Fire the phase machine exactly as the generated runner does (declaration
   order, fast-forward, sync launches).
4. Report: per-node dispatch lines + final counter values; `--verify`
   flag: derive expected outputs the interpreter computes for
   small shapes (256³ max) and diff — the .abv-level correctness gate
   every GPU leg has lacked.
5. Gates: `brievc run examples/gpu/gemm.abv` reproduces the runner's
   numbers; `brievc run --verify` passes on gemv/gemm/gemm_small.

## Track B — `.bv` resident-launch wrapper gate (soundness)

The runtime ABI (resident launches, dirty-scalar sync) is reachable from
`.bv` programs via the accel descriptors — but a `.bv` program is only
safe for resident launches when every field READ is provably performed by
a kernel, never by host code between launches (the staging window is
stale by design between launches). The gate: an analysis over the `.bv`
program's accel usage proving **all-readers-are-kernels** per field.

1. **Design first** (half the leg): enumerate what makes a field
   resident-safe — written by the kernel, read by the kernel, host reads
   only at download points (end of transaction / explicit sync). Map the
   existing accel analysis structures (KernelShape read/write sets, host
   reads through the field table).
2. **The gate**: analysis pass producing per-field
   `{kernel-only | host-read-ok}`; `.bv` accel programs with a
   host-read-between-launches field get resident OFF (full-copy launch —
   correct, slower) with a diagnostic naming the field and the host read.
3. **Tests**: a kernel-only `.bv` program → resident ON; a program that
   peeks a field between nodes → resident OFF + diagnostic names the
   field; the gate never flips a program to incorrect behavior (the
   fallback is always the exact full-copy path).
4. This unblocks shipping the resident path for `.bv` (currently only
   `.abv` uses it).

## Track C — f16 tensor fault hunt (M2.2 close-out)

The f16 tensor kernel writes ~25% of the output then dies (variant- and
size-independent; f32 coopmat smoke fully correct). Plan (cheapest first,
pure-SPIR-V steps before tooling):

1. **Y-fill workgroup tracing**: pre-store y-fill (kernel writes a
   sentinel `wgid` into y before the loop); the post-run pattern shows
   exactly which workgroups RAN, which stored, and where death struck —
   isolates fault-vs-never-scheduled.
2. **Feature probe**: `shaderFloat16`/`storageBuffer16BitAccess` support
   queried before chaining (the driver currently chains unconditionally;
   a missing feature = creation failure = the 25% symptom).
3. **Shape reduction**: does a 16×16 f16 mma with K=16, ONE iteration
   (no loop) fault? Bisect: loop vs mma vs FConvert vs store.
4. **DXC cross-check** (external tool, last): compile the equivalent
   HLSL coopmat kernel with DXC and diff the SPIR-V against ours.
5. On root-cause: fix, flip `spirv_coopmat`, measure vs the ggml anchor
   (10.9ms / 12.6 TFLOP/s same GPU). If the fault is a driver/NVVM bug
   beyond our control: document the device constraint, keep the knob off,
   record the finding.

## Gates throughout

2025+ lib tests green · Float32 tiers bit-identical · spirv-val clean ·
exact correctness (f16 tolerance documented on tensor rows only) ·
Praetor clean on changed files · every track committed separately.

## Deliverable

A GPU backend where: `.bv` programs safely use the resident path
(Track B), iterating on GPU kernels takes one command (Track A), and the
tensor-core rung is either closed (root-cause found) or precisely
characterized with the blocking party identified (Track C).
