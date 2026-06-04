# print_loop — Brief 0.65× (35% faster)

## What it tests

FFI output in a hot loop: increment counter, conditionally call `__print_int` every
100K ticks. 50M iterations, 500 print calls. Tests whether the FFI path is
overhead and prevents pure-counter fold elimination.

## Result

| | Time | Ratio |
|---|------|-------|
| Brief | 0.0399s | — |
| C | 0.0606s | 0.65× |

## Why Brief wins

**LTO inlines `__print_int` into the hot loop.** Brief's `frgn __print_int` is
a plain C function in `brief_rt.c` — `fprintf(stderr, "%lld\n", n)`. During LTO,
`clang -c -emit-llvm` compiles `brief_rt.c` to LLVM bitcode, `llvm-link` merges
it with the program IR, and `opt -O3` inlines `__print_int` into the reactor tick
body.

C's `printf` / `fprintf` is a varargs libc function — it cannot be inlined by any
optimizer. It must go through the PLT (Procedure Linkage Table) as an out-of-line
call. The call overhead + libc dispatch for every 100K-th iteration adds up.

### Key assembly evidence

**Brief** — `__print_int` is inlined. No `call` instruction for the print path.
The integer division (`ops % 100000`) uses multiply-by-reciprocal (`imul` +
`movabs` + shift), and the output goes to `fprintf` which IS called, but only
after LTO-inlined argument preparation:
```asm
; reactor_tick inline: no fprint call setup, integer div via imul
imul   %rdi,%rax
movabs $0xa7c5ac471b460,%rcx     ; multiply-by-reciprocal for 100000
```

**C** — `fprintf` is an out-of-line PLT call:
```asm
jmp    *0x2fca(%rip)              ; fprintf@plt — indirect jump through PLT
```

The difference: Brief's LTO pipeline (`clang -c -emit-llvm` → `llvm-link` →
`opt -O3` → `llc`) can see across the FFI boundary. C's `fprintf` is opaque.

### Secondary factor: structural liveness

`statement_contains_ffi` in `compute_effectively_pure` detects the `__print_int`
call and prevents the pure-counter fold. The compiler knows this loop produces
output, so it emits the full body instead of replacing it with `store i64 N`.
This is correct — it's what the programmer intended.

## Compiler optimization paths

| Pass | Brief | C |
|------|-------|---|
| FFI inlining | ✅ LTO inlines `__print_int` | ❌ `fprintf` is varargs/libc |
| Structural liveness | ✅ `statement_contains_ffi` keeps loop live | ✅ `volatile`-free output |
| Integer division | ✅ Multiply-by-reciprocal (`imul`+shift) | ✅ Same pattern |
| Loop optimization | ✅ Unified folded loop with SSA | ✅ Standard while-loop |

## Reproduce

```bash
cargo build --release --bin brief-compiler
bash benchmarks/build_and_bench.sh print_loop
```
