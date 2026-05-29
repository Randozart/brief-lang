# Brief LLVM Backend Specification

**Version:** 1.2  
**Date:** 2026-05-29  
**Status:** Active implementation plan

## Overview

The Brief LLVM Backend compiles Brief AST structures into optimized LLVM IR (`.ll` files). Two implementations are planned:

- **Rust backend** (`src/backend/llvm.rs`) — text IR emitter, currently 509 lines of working scaffold
- **Self-hosted backend** (`lib/compiler/backends/llvm.bv`) — Brief-in-Brief mirror emitting `.ll` via `StringBuilder`

## Design Philosophy

Brief enforces strict memory boundaries, transactional atomicity, and contract-driven constraints. The LLVM backend exploits these guarantees to bypass conservative compiler safety assumptions:

1. **Acyclic call graph** → direct calls, `norecurse`, full inlining into one SSA graph
2. **Isolated state transitions** → `noalias` + `nocapture` on every `%State*` pointer
3. **Bounded preconditions** → `!range` metadata on loads, `llvm.assume` for complex invariants
4. **No indirect pointers** → register promotion (struct members live in registers, not memory)
5. **Guarded control flow** → `select i1` instead of branches (no mispredicts)

## Document Index

| File | Content |
|------|---------|
| `01-ARCHITECTURE.md` | Pipeline: AST → Lowering → Codegen → `.ll` |
| `02-TYPE-MAPPING.md` | Brief types → LLVM types, alignment, ABI |
| `03-TRANSACTIONS.md` | txn → `define void @name(%State* noalias nocapture)` |
| `04-NOALIAS.md` | noalias/nocapture synthesis, register promotion |
| `05-CONTRACT-TO-METADATA.md` | !range, llvm.assume, constant propagation from contracts |
| `06-MATCH-TO-SWITCH.md` | match → `switch i64 %discriminant` with phi |
| `07-FFI-TO-DECLARE.md` | frgn → `declare @function`, C ABI marshaling |
| `08-REACTOR-LOOP.md` | main() → tick loop, trigger sampling, inlined dispatch |
| `08a-TRIGGERS.md` | trg → volatile double-buffering, 3 lowering models |
| `08b-TRANSITION-FUSING.md` | Sequential state composition (fusing guaranteed-sequential txns) |
| `08c-EQUILIBRIUM-SUSPENSION.md` | Event-driven sleep: replacing busy-spin with `__wait_for_event()` |
| `08e-AOT-SIZE-INFERENCE.md` | List→Vector[N] promotion via contract-bound analysis |
| `09-SIMD.md` | `<N x T>` vectors, `!llvm.loop.vectorize.enable` |
| `10-FULL-EXAMPLE.md` | Counter.increment from .bv → annotated .ll |
| `11-SELF-HOSTED.md` | Porting to Brief: StringBuilder IR emission |
| `12-IMPLEMENTATION-ORDER.md` | Phases 0-7 with deps and effort |
| `13-GPU-TARGET.md` | Future roadmap: NVPTX/SPIR-V, bank conflict elimination |
| `CHANGELOG-LLVM-SPEC.md` | Design journal tracking all spec revisions |