# BILD — Brief's Inop LLVM Dialect

**Date added:** 2026-06-22
**Status:** Stable (Phase 1)

## Purpose

BILD (**B**rief's **I**nop **L**LVM **D**ialect) is the low-level intermediate
language used inside `inop`/`inop!` declaration bodies. It is a subset of LLVM
IR with Brief-flavored syntax conventions, designed to give systems programmers
direct access to LLVM's instruction set (including inline assembly) without
modifying the compiler.

The name "Inop" pulls double duty: **In**trinsic **Op**eration (what it
declares) and **I**ntermediate **R**epresentation **Op** (how the body is
expressed).

## Grammar (Informal)

A BILD body is a sequence of statements wrapped in `{ }`:

```ebnf
bild-body    = "{" { bild-stmt ";" } "}"
bild-stmt    = ( bild-assign | bild-term )
bild-assign  = "%" ident "=" llvm-instruction
bild-term    = "term" [ "%" ident ]
             | "term!"
```

Each `llvm-instruction` is standard LLVM IR: `opcode type operands`,
including LLVM's inline assembly syntax (`call <ret-ty> asm "..." ...`).

## Type Mapping

Brief parameters are mapped to their native LLVM types by name:

| Brief type | LLVM type | BILD name | Example |
|------------|-----------|-----------|---------|
| `Int` | `i64` | `%param_name` | `%a` |
| `Float` | `float` | `%param_name` | `%x` |
| `Bool` | `i8` | `%param_name` | `%flag` |
| `Char` | `i32` | `%param_name` | `%c` |
| `String` | `i8*` | `%param_name` | `%s` |
| `Data` | `i8*` | `%param_name` | `%buf` |
| `Ptr<T>` | `i8*` | `%param_name` | `%p` |

The parameter name in Brief becomes the LLVM register name directly — there
is no positional `%argN` mapping. A parameter `(a: Int, b: Int)` is available
as `%a` and `%b`.

## `term` Lowering

The `term` keyword inside BILD is transformed by the backend:

| Context | BILD | Lowered LLVM |
|---------|------|--------------|
| Standalone inop | `term %res` | `ret i64 %res` |
| Standalone inop (float) | `term %f` | `ret float %f` |
| Reactive txn | `term!` | `br %done` |
| Callable txn | `term %res` | `store; br %post` |

`term!` with a swan song (`term! -> expr`) runs the swan song before the
terminator. Swan songs are Brief expressions (not BILD) and may contain
`print_int#`, `frgn` calls, or other side-effecting operations.

## Inline Assembly

BILD supports LLVM's inline assembly syntax directly, since BILD IS LLVM IR:

```bild
inop! xchg(ptr: Ptr<Int>, val: Int) -> Int {
  %res = call i64 asm "xchg %1, %0" : "=r"(%ret) : "r"(%val), "m"(inttoptr i64 %ptr to ptr) : "memory";
  term %res;
} fallback 0;
```

This is the escape hatch: LLVM IR includes inline asm as a first-class
construct (`call <ret-ty> asm "..." : constraints`), so BILD inherits it
automatically. No special syntax, no extra keywords — same `call` instruction
with the `asm` keyword.

## Statement Separators

Statements are terminated by `;`. Newlines are lexer whitespace — they are
discarded by the Brief lexer. This means you can write BILD on a single line
or across multiple lines:

```bild
// Multi-line:
{
  %res = add i64 %a, %b;
  term %res;
}

// Single-line (same body):
{ %res = add i64 %a, %b; term %res; }
```

## Limitations (Phase 1)

| Limitation | Detail | Future work |
|------------|--------|-------------|
| No `%state` access | Parameters only — cannot read/write global state | Phase 2 |
| Single output | Exactly one return type | Phase 2 |
| No recursion guard | Structural recursion checker operates on fallback only | Phase 2 |
| LLVM-only | Other backends (Webstack, CIRCT) use fallback expression | Polyglot bodies Phase 2 |
| No SMT-LIB | LLVM IR → SMT-LIB bvlogic translator not yet built | Phase 3 |

## Error Model

Malformed BILD is caught by `llc` at compile time — the user's test suite
catches it before production. There is no BILD-specific linter in Phase 1.

Common errors:
- **Undefined register**: using `%x` before defining it (LLVM SSA violation)
- **Type mismatch**: `add i64 %a, %b` where `%a` is actually `float`
- **Metadata collision**: duplicate metadata `!N` IDs in complex bodies
- **Inline asm constraint mismatch**: wrong number/type of operands
- **`%state` not available**: the state pointer is not passed to inop bodies

## File format

BILD bodies are stored as `Vec<String>` on `InopDeclaration.llvm_body`, one
string per semicolon-delimited statement (with the semicolon stripped). The
LLVM backend reassembles them with `;` terminators before emission.

## See also

- `docs/architecture/features/inop.md` — the `inop`/`inop!` declaration syntax and pipeline
- `learn-brief/14-bild.md` — tutorial: writing your first BILD program
