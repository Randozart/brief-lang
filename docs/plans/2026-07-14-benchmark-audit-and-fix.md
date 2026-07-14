# Benchmark Audit & LLVM Optimization Restoration

## Benchmark Issues

| Priority | File | Issue | Fix |
|----------|------|-------|-----|
| High | `benchmarks/test_ring_buffer.bv` | `#!exit` pragma line 8 | Remove line |
| High | `benchmarks/let-order.bv` | `#!exit` pragma line 2 | Remove line |
| High | `benchmarks/async_counters_idio.bv` | `#!exit` pragma line 25 | Remove line |
| Medium | `benchmarks/nbody_sqrt_idio.bv` | Line 132: `dx34` → `dy34` (copy-paste bug) | Fix variable name |
| Medium | `benchmarks/fannkuch_redux_sym.bv` | Lines 34-36 dead LCG code | Move or remove |
| Medium | `benchmarks/test_ring_buffer.bv` | `let buf: Int = 0;` wrong init | Fix init syntax |
| Low | `benchmarks/gpu/saxpy.bv` | Empty file (0 bytes) | Remove |

## LLVM Optimization Gaps

| Gap | Location | Fix | Impact |
|-----|----------|-----|--------|
| `emit_binary_op` no `nsw` on Int Add/Sub/Mul | `emit_expr.rs:288-306` | Add `nsw` flag | Enables LLVM nsw-based optimizations |
| `config/llvm-ops.toml` no `fast` on float ops | `config/llvm-ops.toml` | Add `fast` to float templates | Matches actual codegen |

No optimizations were lost from the old codebase — both gaps are net improvements over the pre-refactor state.

## Execution Order

1. Remove `#!exit` from 3 benchmark files
2. Fix `nbody_sqrt_idio.bv` dx34→dy34
3. Fix `fannkuch_redux_sym.bv` dead LCG code
4. Fix ring buffer init syntax
5. Add `nsw` to `emit_binary_op`
6. Add `fast` to float op config templates
7. `cargo test --lib`
8. `bash benchmarks/build_and_bench.sh --correctness`
