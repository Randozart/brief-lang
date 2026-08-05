# Link Dependencies — Foreign Source Compilation

**Date:** 2026-06-24
**Phase:** TBD
**Status:** Fully implemented

## Overview

Link dependencies tell the Briv compiler to compile foreign source files and link them into the final binary. They use the `import` keyword with a `link/`-prefixed path, which differentiates them from regular Briv module imports.

## Syntax

```briv
import "link/path/to/file.c";
import "link/some_library.o";
import "link/xxhash/xxhash.c";
```

The path after `link/` is relative to the search paths (see Resolution below). The file extension determines the source language and which compiler is used.

## Supported Languages

| Extension | Language | Compiler |
|-----------|----------|----------|
| `.c` | C | `clang -c -emit-llvm -O2` |
| `.cpp`, `.cc`, `.cxx` | C++ | `clang++ -c -emit-llvm -O2` |
| `.rs` | Rust | `rustc --emit=llvm-bc -C opt-level=3` |
| `.zig` | Zig | `zig build-obj --emit-llvm-ir -O ReleaseFast` |
| `.py` | Python | `codon build --emit-llvm -O3` |
| `.java` | Java | `javac` then `native-image --llvm --emit-llvm-bc` |
| `.ts`, `.as.ts` | AssemblyScript | `asc` then `wasm2llvm` |
| `.bc` | LLVM Bitcode | Copied directly |
| `.o`, `.a` | Object file | Warning: cannot LTO |

## Resolution

The compiler searches for linked files in this order:
1. Project source directory (relative to current file)
2. `lib/runtime/<path>`
3. `lib/std/<path>`
4. `lib/std/c/<path>`
5. `BRIEF_STDLIB_PATH` env var
6. Absolute path

## LTO Pipeline

All linked bitcode files are merged with the program's IR module, then `opt -O3` runs on the merged module before `llc` produces the final object. This enables cross-language LTO — a C file linked via `import "link/file.c"` gets the same optimization as Briv code.

## Common Patterns

```briv
// Link the runtime C support library
import "link/briv_rt.c";

// Link a third-party library
import "link/xxhash/xxhash.c";

// Link a precompiled object
import "link/vendor_library.o";

// Link a Rust library compiled to bitcode
import "link/rust_lib.bc";
```

## Restrictions

Named imports are rejected for link dependencies:
```briv
import { someFunc } from "link/file.c";  // ERROR: cannot name-import from link target
```

## Examples

See `examples/link-demo/link-example.bv` for a complete walkthrough linking C code.
