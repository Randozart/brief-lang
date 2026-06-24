# Link-Demo — Foreign C Source Linked into Brief

This example demonstrates linking a C source file (`helper.c`) into a
Brief program via `import "link-demo/helper.c"`.

## Files

- `link-example.bv` — Brief program that calls C functions
- `helper.c` — C source with `double_it()` and `greet()`

## How it works

1. `import "link-demo/helper.c"` tells the compiler to find `helper.c`
   relative to the importing file and compile it to LLVM bitcode.
2. The `frgn` declarations in `.bv` expose the C functions to Brief.
3. The compiler runs LTO across both the Brief-generated IR and the
   C-generated bitcode, producing a single optimized binary.

## Running

```bash
brief-compiler build examples/link-demo/link-example.bv
```

## Language support

`import` with `link/`-prefix can link C, C++, Rust, Zig, Python, Java,
AssemblyScript, raw bitcode (`.bc`), and object files (`.o`/`.a`).
