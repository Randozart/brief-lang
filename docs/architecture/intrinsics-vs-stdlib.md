# Intrinsics vs Stdlib — The Dividing Line

## The Rule

**Everything that MUST hold with no stdlib loaded is an intrinsic.**
**Everything else is stdlib.**

If `rm -rf lib/std && compiler --no-stdlib` still type-checks and compiles
`let x: Int = 5`, it's an intrinsic. If a user could write a `.bv` file that
achieves the same thing, it belongs in stdlib.

## Three-Layer Architecture

| Layer | File | Role | Validated |
|-------|------|------|-----------|
| **Contract** | `src/intrinsic_signatures.rs` | Declares what `#` intrinsics exist and their type signatures | Frontend (typechecker) rejects calls to unknown intrinsics |
| **Implementation** | `config/llvm-ops.toml` | Maps (op, primitive, bytes) → LLVM IR template | Backend finds template or falls through to `emit_external_call` |
| **Binding** | `lib/std/types/bootstrap.bv` | Maps operator symbols (`+`, `<`) to per-type op bindings | Typechecker resolves operator via `get_operator_intrinsic` |

The frontend can optionally cross-check the config to give early errors when
the backend has no template for a given type+width combination. Currently this
check does not exist — the backend silently falls through to external call.

## What Lives Where

### Intrinsics (`intrinsic_signatures.rs` — compiler must have)

| Category | Examples | Reason |
|----------|----------|--------|
| Primitive types | `Int`, `Float`, `Bool`, `Void` | Compiler needs these to type-check anything |
| Arithmetic | `Add#`, `Sub#`, `Mul#`, `Div#`, `Rem#`, `Neg#`, `Abs#` | Operator sugar desugars to these; no stdlib needed |
| Comparison | `Eq#`, `Neq#`, `Lt#`, `Gt#`, `Le#`, `Ge#` | Same — operators desugar to these |
| Bitwise | `BitAnd#`, `BitOr#`, `BitXor#`, `Shl#`, `Shr#`, `BitNot#` | Same |
| Logical | `Not#` | Unary, no short-circuit concern |
| Pointers | `Deref#`, `AddrOf#` | `*ptr` and `&x` desugar to these; needed for memory model |
| Index | `Index#` | `a[b]` desugars to this |
| Memory | `Malloc#`, `Free#`, `Load#`, `Store#`, `Copy#`, `Fill#` | Heap operations are language primitives |
| Math hardware | `Sqrt#`, `Sin#`, `Cos#`, `Fabs#`, `Ceil#`, `Floor#`, `Pow#` | Cannot be expressed in Brief without hardware access |
| I/O | — | Migrated to stdlib. `!Print`/`!PrintLn` dispatched via Front plugin; `!GetEnv`/`!GetEnvInt` resolved to pure-Brief environ scan. All use `SysCall#(Write, ...)` or `Load#` underneath. |
| Atomic | `AtomicLoad#`, `AtomicStore#`, `AtomicCas#`, etc. | Hardware memory model primitives |

### Stdlib (`lib/std/` — can be absent, feels native)

| Category | Examples | How it hooks in |
|----------|----------|-----------------|
| Collection types | `RingBuffer<T>`, `List<T>`, `HashMap<K,V>` | `InsertAt <~ fn(#L, #R)` property |
| Strategy functions | `ring_push`, `ring_pop` | Brief `defn` with Ptr arithmetic |
| Derived collections | `Stack<T>`, `Queue<T>` | Wraps RingBuffer/List |
| Terminal I/O | `print_int`, `print_str`, `print_char`, `print_float` | `!Print`/`!PrintLn` dispatched by Front plugin → typed stdlib functions using `SysCall#(Write, ...)` |
| Environment | `getenv`, `get_env_int` | `!GetEnv`/`!GetEnvInt` dispatched by Front plugin → pure-Brief environ scan via `Load#` |
| File I/O | `FileRead#`, `FileWrite#` | Already intrinsics (observable) |
| Threading | | Uses atomic intrinsics |

## The Test

```
// This must work with --no-stdlib:
let x: Int = 5;
let y: Int = x + 3;
let p: Ptr<Int> = &x;
let v: Int = *p;
```

```
// This is stdlib — compiler doesn't know about it:
let rb: RingBuffer<Int> = init_ring_buf(1024);
rb <- 42;              // Works because InsertAt property resolves
let v <- &rb;           // Works because ExtractFrom property resolves
```

## Why Not Make Everything Stdlib?

Some operations cannot be expressed in Brief at all:
- `a + b` on hardware integers maps to a single `add` instruction — no Brief
  function can emit that without compiler help.
- `Sqrt#(x)` maps to `@llvm.sqrt.f32` — no Brief function can call LLVM
  intrinsics directly.
- `Malloc#(size)` maps to `@malloc` — the ABI convention for heap allocation
  is platform-specific and baked into the compiler.

These must be intrinsics. Everything else is negotiable.

## Why Make Everything Else Stdlib?

The `ring_push` case proves the pattern: 15 lines of Brief with Ptr arithmetic
replaces a compiler-intrinsic + Rust match arm. A user writing `MyQueue<T>`
with `InsertAt <~ my_push(#L, #R)` gets the same `<-` syntax without touching
the compiler. The only hardcoded Rust code is the strategy dispatch reading the
property value — and even that is generic.
