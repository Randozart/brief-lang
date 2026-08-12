# Briev Self-Hosting Status

> Extracted from the README (2026-07-31).

The Briev compiler can now:
- Parse itself
- Type-check itself
- Verify its own contracts
- Generate code for itself (3 canonical backends)
- Run shared analysis (CallGraph, range inference) in both Rust and Briev

**Implementation:**
- 3 canonical backends (LLVM, CIRCT, Webstack)
- 300+ standard library functions
- ~126,000 lines of Rust bootstrap compiler
- ~21,000 lines of Briev in lib/ (includes stdlib + self-host)
- ~75,000 lines of documentation
- 1,279 passing tests (cargo test --lib)

**Key v0.18.0 additions:**
- **GLUE Protocol Bridge**: TOML-driven cross-language FFI, protocol-path BFS optimization, `briev export` subcommand
- **Protocol-driven type mapping**: `type_map`/`c_type_map`/`conversions` replaced by `protocols` mapping protocol categories to language types
- **Full backend export**: `briev export` uses `LlvmBackend::generate()` (no `ret i64 0` stubs)
- **Round-trip FFI tests**: 8 integration tests verifying full pipeline from `.bv` to FFI call
- **Bridge benchmark**: Python ↔ C vs Briev via ctypes (C 5988ns, Briev 6203ns, ✅ all match)
- **Dynamic GLUE config**: `#[serde(flatten)]` language discovery — zero hardcoded language names in Rust
- **C-compatible string format**: `[length][data]` format matching `briev_rt.c`
- **`emit_protocol_chain`**: Real LLVM IR emission for Bitcast, MeldShuffle, ProtocolTransform kinds
- **Arena allocator budget control**: `--optimize-budget 0` uses direct `malloc`
- **Configurable arena size**: `arena_initial_size` field replaces magic 65536 constant

**Performance improvements (v0.16 → v0.17):**
| Benchmark | v0.16 | v0.17 | Winner |
|-----------|-------|-------|--------|
| nbody_newton | 1.35x | **1.05x** | **~tie** |
| fannkuch_redux | 1.31x | **0.95x** | **Briev** |
| fasta | 1.23x | **1.10x** | ~tie |
| ring_buffer | 1.45x | **1.10x** | ~tie |
| float_math_nonzero | 2.21x | **0.94x** | **Briev** |

**See:** [docs/plans/2026-07-21-rct-txn-to-node-rename-and-benchmark-fixes.md](docs/plans/2026-07-21-rct-txn-to-node-rename-and-benchmark-fixes.md) for the comprehensive plan and current benchmark results.
