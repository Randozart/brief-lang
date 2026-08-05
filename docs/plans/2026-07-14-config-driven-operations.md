# Config-Driven Operations: Replacing Type-Specific Intrinsics with Generic Operations + TOML Lowering

Date: 2026-07-14

## Problem

The compiler currently has 38 type-specific intrinsics (`AddI64#`, `FAddF64#`, `StringConcat#`, etc.) with hardcoded match arms in three places:
- `src/intrinsic_signatures.rs` — signature registry
- `src/interpreter/intrinsics.rs` — interpreter dispatch
- `src/backend/llvm/intrinsics.rs` — LLVM codegen dispatch

This is brittle: adding a new type (e.g., `Float16`) requires adding new intrinsics (`FAddF16#`, `FSubF16#`, ...) and match arms in all three files. The same pattern repeats across backends (webstack, CIRCT).

## Target State

1. **Generic operations** (~15 ops: `Add#`, `Sub#`, `Eq#`, `Sqrt#`, `Print#`, etc.) replace the 38 type-specific intrinsics.
2. **Config file** `config/llvm-ops.toml` maps `(operation, primitive, bytes) → LLVM IR template` — zero hardcoded match arms in Rust.
3. **Type-level overrides**: a type definition can say `op Add = my_custom_add` to override the config default.
4. **Backend dispatch**: check type override first, fall back to config file.

Let me write the plan.</think>

<｜DSML｜tool_calls>
<｜DSML｜invoke name="write">
<｜DSML｜parameter name="filePath" string="true">/home/randozart/Desktop/Projects/briv-compiler/docs/plans/2026-07-14-config-driven-operations.md