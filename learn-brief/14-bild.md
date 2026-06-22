# BILD — Brief's Low-Level Escape Hatch

BILD (**B**rief's **I**nop **L**LVM **D**ialect) is the language inside
`inop`/`inop!` declaration bodies. It gives you direct access to LLVM's
entire instruction set — including inline assembly — without modifying the
compiler.

Think of it as Brief's "unsafe escape hatch" that is still safe because:
- The compiler does the lowering (`term → ret`, parameter binding)
- `llc` catches malformed IR at compile time
- You choose the fallback expression for the interpreter and other backends

## When to Use BILD

| You need... | Use... | Example |
|-------------|--------|---------|
| OS calls (open, read, write) | `frgn` + libc | `frgn read(...)` |
| High-performance math | `#`-intrinsics | `sqrt#(x)` |
| GPU compute | `.abv` (Accelerated Brief) | `abv` file |
| **LLVM IR or inline asm** | **inop + BILD** | `inop sadd(a,b)` |
| Everything else | Pure Brief | `defn add(a,b) { a+b }` |

BILD is for situations where you need an instruction that LLVM knows about
but Brief doesn't have a built-in intrinsic for. Common cases:
- Custom atomic operations (CAS, fetch-add with specific ordering)
- SIMD intrinsics (`add <2 x double>`, shufflevector)
- CPU feature detection (`cpuid`)
- Compiler barriers and fences
- Direct register access (e.g., reading the thread pointer)

## 1. Your First BILD Program

Let's write a saturating addition — add two integers but cap at `MAX_INT`
instead of wrapping:

```brief
// examples/inop-sadd.bv
inop sadd(a: Int, b: Int) -> Int {
  %res = add i64 %a, %b;
  term %res;
} fallback a + b;
```

Let's walk through this:

| Part | What it does |
|------|-------------|
| `inop` | Declares a **pure** intrinsic (no side effects) |
| `sadd(a: Int, b: Int) -> Int` | Name, params with types, return type |
| `{ %res = add i64 %a, %b; term %res; }` | **BILD body** — the LLVM IR |
| `fallback a + b` | Fallback for interpreter and other backends |

Key observations:
- Parameter `a` becomes `%a` in BILD, `b` becomes `%b` — named parameters,
  not positional `%arg0` / `%arg1`
- `term %res` is lowered to `ret i64 %res` — you don't write `ret` directly
- `inop` (no `!`) marks it as pure — the optimizer can fold, reorder, or
  precompute calls

### Calling your inop

Inops are called via the same `name#(args)` syntax as built-in intrinsics:

```brief
let r = sadd#(10, 20);    // r == 30
```

The parser first checks built-in intrinsics (`Intrinsic::from_name("sadd")`),
fails, then falls back to `Intrinsic::UserDefined("sadd")`.

### A complete working program

```brief
inop sadd(a: Int, b: Int) -> Int {
  %res = add i64 %a, %b;
  term %res;
} fallback a + b;

let tick: Int = 0;

rct txn run [tick < 1][tick == 1] {
    [tick == 0] {
        let x: Int = sadd#(10, 20);
        term! -> print_int#(x);
    };
    &tick = tick + 1;
    term;
};
```

Save this as `sadd.bv` and compile:

```bash
./target/release/brief-compiler build sadd.bv
./sadd
# Output: 30
```

## 2. Type Mapping in BILD

Brief parameters arrive in BILD with their native LLVM types:

| Brief type | LLVM type | BILD name | Example instruction |
|------------|-----------|-----------|-------------------|
| `Int` | `i64` | `%x` | `add i64 %x, %y` |
| `Float` | `float` | `%f` | `fadd float %f, %g` |
| `Bool` | `i8` | `%b` | `trunc i8 %b to i1` |
| `Char` | `i32` | `%c` | `add i32 %c, 1` |
| `String` | `i8*` | `%s` | `call i64 @strlen(i8* %s)` |
| `Data` | `i8*` | `%buf` | `load i8, i8* %buf` |
| `Ptr<T>` | `i8*` | `%p` | `load i64, i8* %p` |

### Working with Float

```brief
inop fadd3(a: Float, b: Float, c: Float) -> Float {
  %t1 = fadd float %a, %b;
  %res = fadd float %t1, %c;
  term %res;
} fallback a + b + c;
```

### Working with Pointers

```brief
inop! peek(ptr: Ptr<Int>) -> Int {
  %res = load i64, i8* %ptr;
  term %res;
} fallback 0;
```

## 3. Inline Assembly in BILD

Since BILD IS LLVM IR, and LLVM IR supports inline assembly via the `asm`
keyword inside `call`, BILD inherits inline assembly automatically:

```brief
// Atomic exchange via xchg instruction
inop! xchg(ptr: Ptr<Int>, val: Int) -> Int {
  %res = call i64 asm "xchg %1, %0" : "=r"(%ret) : "r"(%val), "m"(inttoptr i64 %ptr to ptr) : "memory";
  term %res;
} fallback 0;
```

The LLVM asm syntax is:
```
call <ret-ty> asm "<instructions>" : "<output-constraints>"(<registers>)
                               : "<input-constraints>"(<registers>)
                               : "<clobbers>"
```

### another example: CPUID

```brief
inop cpuid(eax_in: Int) -> Int {
  %eax = call i64 asm "cpuid" : "=a"(%a), "=b"(%b), "=c"(%c), "=d"(%d) : "a"(%eax_in) : "memory";
  term %eax;
} fallback 0;

let vendor: Int = cpuid#(0);    // vendor string in ebx, ecx, edx
```

### Inline assembly rules

1. **Clobber `"memory"` if the asm reads or writes memory** — LLVM needs to
   know not to reorder loads/stores around the asm
2. **Use `inop!` (not `inop`) if the asm has side effects** — this prevents
   the optimizer from eliminating or reordering the call
3. **Match constraint letters to register sizes** — `"=r"` for i64, `"=a"`
   for rax, `"=D"` for rdi, etc.

## 4. Side Effects: `inop` vs `inop!`

| Keyword | `has_side_effects` | Optimizer behavior |
|---------|-------------------|-------------------|
| `inop` / `inop#` | `false` | Can fold, reorder, eliminate, precompute |
| `inop!` / `inop#!` | `true` | Must preserve — never eliminated |

Rule of thumb: if your BILD body calls inline asm or an LLVM instruction that
reads/writes memory (load, store, atomicrmw, cmpxchg), use `inop!` unless the
operation is a pure computation like `add`, `mul`, `fadd`, `sqrt`, etc.

```brief
// Pure — safe to fold:
inop sadd(a: Int, b: Int) -> Int { ... }

// Side-effecting — must preserve:
inop! write_buf(ptr: Ptr<Byte>, len: Int) -> Int { ... }
```

The side-effect flag is propagated to the transition graph: `inop!` bodies
prevent pure-body optimization and keep their transaction loop running.

## 5. Compiling and Running

```
.bv file → brief-compiler → .ll → opt → llc → .o → cc → executable
                                            ↑
                                       BILD body
                                       emitted here
```

Compile with:

```bash
# Full pipeline to binary:
./target/release/brief-compiler build my_prog.bv

# Emit LLVM IR only (inspect the BILD → LLVM lowering):
./target/release/brief-compiler build my_prog.bv --emit-llvm /tmp/
```

## 6. Debugging BILD

### Inspect the emitted LLVM

```bash
./target/release/brief-compiler build my_prog.bv --emit-llvm /tmp/
cat /tmp/my_prog.ll | grep -A5 "define i64 @my_inop"
```

### Verify the IR

```bash
opt -passes=verify /tmp/my_prog.ll -o /dev/null
```

### Common errors

| Error | Likely cause |
|-------|-------------|
| `expected instruction opcode` | Missing `;` between BILD statements |
| `use of undefined value %x` | Using a register before defining it |
| `Instruction does not dominate all uses` | SSA violation — register defined in wrong block |
| `error: expected ';'` | LLVM version mismatch in the `.ll` output |
| `invalid redefinition of function 'main'` | User `defn main` renamed to `brief_main` automatically |

### Watch for precomputation

If your inop has all compile-time-constant inputs and no side effects
(`inop`, not `inop!`), the compiler may precompute the entire call and
fold the result. The binary will have the answer hardcoded. This is
correct behavior — if you need runtime execution, make at least one
argument runtime-determined (e.g., via `getenv_int#("BOUND")`).

## 7. When to Choose BILD vs Alternatives

| Approach | Latency | Safety | Portability | Best for |
|----------|---------|--------|-------------|----------|
| Pure Brief | Medium | Highest | All targets | Most code |
| `#`-intrinsic (built-in) | Low | High | All targets | OS, math, IO |
| **inop + BILD** | **Lowest** | **Medium** | **LLVM + fallback** | **LLVM instructions, inline asm** |
| `frgn` + C | Low | Low | Wherever C compiles | Large C libraries |
| `.abv` (GPU) | Variable | Medium | GPU backends | GPU compute |

BILD is the answer when you need an instruction that the compiler doesn't
have a built-in intrinsic for: a specific SIMD opcode, a CPU feature flag,
a hardware register, or a custom atomic sequence.

## See also

- `docs/architecture/features/inop.md` — the `inop` declaration system
- `docs/architecture/features/bild.md` — BILD dialect reference (grammar, type mapping, lowering)
- `examples/inop-sadd.bv` — complete working example
- `tests/fixtures/inop_sadd.bv` — E2E test fixture with LLVM verification
