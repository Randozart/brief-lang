# Backend CLI Cleanup + LLVM Build Simplification

**Date:** 2026-06-12
**Status:** In Progress

## Goal

Remove non-essential transpilation targets from CLI, keep only LLVM, Webstack, VHDL, and SystemVerilog. Make `briv build` default to LLVM backend with sensible defaults.

## Phase 1 — Remove CLI subcommands (main.rs only)

Remove these match arms from the command dispatch:

| Command | Lines | Status |
|---|---|---|
| `rust` | 3948-3977 | Remove |
| `c` / `cc` | 4067-4106 | Remove |
| `cobol` / `cbl` | 4108-4136 | Remove |
| `arm` / `a` | 4138-4166 | Remove |
| `wasm` | 4500-4539 | Remove |

Also in `run_compile_unified()` (line 1676-1754):
- Remove `"c"`, `"rust"`, `"cobol"`, `"wasm"` from backend dispatch match
- Keep `"llvm"`, `"verilog"`, `"vhdl"`, `"react"`

Update `print_usage()` (line 314-386):
- Remove `wasm`, `rust`, `c`, `arm`, `cobol` lines
- Keep `verilog`, `vhdl`, `webstack`

## Phase 2 — Make `briv build` default to LLVM

Current `run_build()` for `.bv/.sbv` transpiles to Rust then `rustc`. Change to:
- Run LLVM backend with sensible defaults
- Derive output binary name from input filename
- No flag noise (no PGO, no hw-handoff, default optimize_budget=256)

`.rbv/.srbv` path (WASM+JS) stays unchanged.
`.ebv/.sebv` path remains error with explicit target suggestion.

## Not Changing

- Backend source modules (C, Rust, COBOL, WASM, x86_64, aarch64 stay in `src/backend/`)
- `supported_hashtags()` in `backend/mod.rs`
- `llvm` subcommand (kept for advanced users who want raw IR)
