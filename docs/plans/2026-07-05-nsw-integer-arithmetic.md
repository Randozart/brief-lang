# Add `nsw` Flags to Integer Arithmetic

Date: 2026-07-05
Status: Complete
Target: mandelbrot 1.10x → ≤1.00x (actual: 0.99x)

## 1. Problem

mandelbrot's hot loop has **8 `idivq`** instructions vs 0 in C: 6 `sdiv i64 %val, 100`
(SCALE divisions) and 2 `srem i64 %val, 139968` (LCG modulo). LLVM's strength
reduction needs `nsw` (no signed wrap) to convert `sdiv` → `mul` + magic constant
(~30→~3 cycles). The compiler emits zero `nsw` flags.

## 2. Changes

### `expr/math.rs` (6 lines)

| Line | Current | Fixed (final) |
|------|---------|---------------|
| 26 | `"add"` | `"add nsw"` |
| 30 | `"sub"` | `"sub nsw"` |
| 34 | `"mul"` | `"mul nsw"` |
| 38 | `"sdiv"` | NO CHANGE — LLVM 18 rejects nsw on sdiv |
| 47 | `"srem i64"` | NO CHANGE — LLVM 18 rejects nsw on srem |
| 81 | `"sub i64 0"` | `"sub nsw i64 0"` |

### `helpers.rs` (2 changes)

1. `op_str_to_rune`: strip `"nsw "` suffix (after opcode) before matching
2. Peephole constant folding: strip `"nsw "` suffix before matching

### Deviation from plan

LLVM 18 requires `add nsw` (flag AFTER opcode), not `nsw add` (before).
LLVM 18 rejects `nsw` entirely on `sdiv`/`srem`.  The initial plan was
written for LLVM 14 syntax; updated to match LLVM 18 reality.

## 3. Verification

1. `cargo test --lib`
2. `bash benchmarks/build_and_bench.sh --runtime`
3. Check `opt -O2 -S /tmp/mb/mandelbrot.ll -o /tmp/mb/opt.ll` for
   strength-reduced `mul` patterns instead of `sdiv`/`idivq`