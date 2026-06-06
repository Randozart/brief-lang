# Brief FFI Architecture — Zero-Cost Multi-Language Interop

## Core Insight

`frgn` is just a `call` instruction. Nothing more. No marshaling, no context switch, no runtime boundary. The exact same LLVM `call` that Brief uses for its own `defn` functions is used for foreign functions.

## How It Works

```
Brief source:    frgn __print_int(n: Int) -> Result<Bool, Error>;
                         ↓
Parser:          stores name="__print_int", params=[(n, Int)], return=Result<Bool,Error>
                         ↓
LLVM codegen:    declare i64 @__print_int(i64)     ← just a symbol declaration
                         ↓
LLVM call site:  %result = call i64 @__print_int(i64 %n)   ← same as any function call
                         ↓
LTO link:        llvm-link program.bc brief_rt.bc          ← merges IR modules
                         ↓
Inlining:        opt -O3                                    ← inlines across language boundaries
```

## `import "link/..."` — The Bridge

`import "link/brief_rt.c"` tells the compiler:

| Step | Command | What happens |
|------|---------|-------------|
| 1 | `clang -c -emit-llvm -O2 brief_rt.c` | Compile C to LLVM bitcode |
| 2 | `llvm-as program.ll` | Compile Brief to LLVM bitcode |
| 3 | `llvm-link program.bc brief_rt.bc` | Merge both into one module |
| 4 | `opt -O3 program_merged.bc` | Inline across language boundary |
| 5 | `llc -filetype=obj -O3` | Generate native code |

The file extension tells the compiler which toolchain to use:

| Extension | Language | Compiler | Convention |
|-----------|----------|----------|------------|
| `.c` | C | `clang -emit-llvm` | C ABI |
| `.cpp` | C++ | `clang++ -emit-llvm` | C++ ABI |
| `.rs` | Rust | `rustc --emit=llvm-ir` | Rust ABI |
| `.zig` | Zig | `zig build-obj --emit-llvm-ir` | C ABI |
| `.bc` | LLVM IR (any) | (already bitcode) | Inferred |

## `from "lang"` — Disambiguation Only

`from` is **not required** on `frgn` declarations. The compiler resolves symbols by scanning all `import "link/..."` targets:

```
frgn __print_int(n: Int) -> Result<Bool, Error>;  
  // ^ found in brief_rt.c → uses C convention
```

Only needed when two link targets export the same symbol:

```
import "link/posix.c";
import "link/windows.c";
frgn write(fd: Int, buf: Data, len: Int) -> Result<Int, Error> from "posix";
frgn write(fd: Int, buf: Data, len: Int) -> Result<Int, Error> from "windows";
```

## Zero-Cost Inlining

Before `opt -O3`:
```llvm
define i64 @main() { call i64 @__print_int(i64 42) }
define i64 @__print_int(i64 %n) { call i64 @fprintf(stderr, "%lld\n", %n) }
```

After `opt -O3`:
```llvm
define i64 @main() { call i64 @fprintf(stderr, "%lld\n", i64 42) }
```

The call to `@__print_int` is inlined. The C function body is pasted directly into `main`. **Zero overhead.**

## Error Handling

| Syntax | Semantics |
|--------|-----------|
| `frgn foo() -> Result<T, Error>` | Returns `Result<T, Error>` — caller MUST handle both Ok and Err |
| `frgn! foo()` | Fire-and-forget — return discarded, error panics |

No `-> Void` syntax. `frgn!` IS the void case.

## Languages Without LLVM Backends

Python, JavaScript, Java, etc. cannot produce LLVM IR. They are **interpreter-only** — compiled backends emit an error: *"`from "python"` has no LLVM backend — can only be called via interpreter."*

## No Magic

| Bad (old) | Good (new) |
|-----------|------------|
| `from "libruntime"` (parsed and discarded) | `import "link/brief_rt.c"` + optional `frgn name()` |
| Hardcoded `emit_declares("__rt_init")` | `frgn __rt_init()` declared in `std/rt.bv`, imported explicitly |
| Interpreter match on `"insert"` string | Type-based dispatch on `Value::HashMap` — same native code |
| `"None"`/`"Err"` => discriminant 0 | Enum declaration drives discriminant |

The FFI is transparent. Every function name you see is the actual symbol the linker resolves. No hidden name mapping, no string matching, no magic destinations.
