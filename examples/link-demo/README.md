# Link-Demo — Foreign C Source Linked into Briev

This example demonstrates linking a C source file (`helper.c`) into a
Briev program via `import "link-demo/helper.c"`.

## Files

- `link-example.bv` — Briev program that calls C functions
- `helper.c` — C source with `double_it()` and `greet()`

## How it works

1. `import "link-demo/helper.c"` tells the compiler to find `helper.c`
   relative to the importing file and compile it to LLVM bitcode.
2. The `frgn` declarations in `.bv` expose the C functions to Briev.
3. The compiler runs LTO across both the Briev-generated IR and the
   C-generated bitcode, producing a single optimized binary.

## Running

```bash
briev-compiler build examples/link-demo/link-example.bv
```

## Language support

`import` with `link/`-prefix can link C, C++, Rust, Zig, Python, Java,
AssemblyScript, raw bitcode (`.bc`), and object files (`.o`/`.a`).
