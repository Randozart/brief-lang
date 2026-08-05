# Phase 5.5 Briv: AST Completeness + Definition Support

**Date:** 2026-05-29  
**Goal:** The LLVM backend can compile every AST node that `lib/compiler/` emits, producing a `.ll` file that passes `llc`.  
**Estimated Effort:** 3 days  

## Gap Analysis

The litmus test (`briv-compiler llvm lib/compiler/main.bv`) revealed zero codegen crashes — all hits fell through to the `_ =>` fallback generating `add i64 0, 0`. Every file compiled cleanly. But the output is wrong for any file using the following AST nodes:

### Expr nodes missing (produce `; fallback` comments instead of real IR)

| Node | Used In | LLVM Strategy |
|------|---------|---------------|
| `Block(stmts, last)` | parser.bv line 1478 | Emit statements, then emit last expression |
| `Tuple(elems)` | typechecker.bv line 384 | Pack N i64 values into successive registers |
| `TupleDestructure(names, expr)` | parser.bv line 1577 | Emit expr, extract tuple elements by GEP-like offset |
| `Concat(l, r)` | parser.bv line 4188 | Emit both sides, `add i64` (string/IR assumption) |
| `Cast(expr, ty)` | parser.bv line 4396 | Emit inner expr, no-op (values are i64) |
| `Slice { value, start, end, ... }` | parser.bv line 5257 | Emit list + bounds, `getelementptr` |
| `MultiSlice { value, coordinates }` | parser.bv line 5353 | Recursive slice (like Slice) |
| `ForAll { var, expr }` | parser.bv line 4415 | `true` (always true in LLVM's model) |
| `Exists { var, expr }` | parser.bv line 4418 | `icmp ne i64 %len, 0` (non-empty test) |
| `StructInstance(name, fields)` | parser.bv line 4477 | Emit each field expression |
| `ObjectLiteral(fields)` | — | Emit fields as tuple |
| `Term` | parser.bv line 5116 | `add i64 0, 0` (value placeholder) |

### TopLevel nodes missing (skipped silently)

| Node | Used In | LLVM Strategy |
|------|---------|---------------|
| `Definition(defn)` | Every compiler file | `define i64 @name(params...)` |
| `Struct(sd)` | typechecker.bv | Define `%struct.Name = type { ... }` |
| `Constant(c)` | Various | `@constant_name = constant i64 N` |

## Implementation Order

| Step | What | Est. |
|------|------|------|
| 1 | `TopLevel::Definition` → `define i64 @name(i64 %arg0, ...)` | 0.5d |
| 2 | `Block(stmts, last)` → emit stmts + last expr | 0.25d |
| 3 | `Tuple` / `TupleDestructure` → sequential registers | 0.5d |
| 4 | `Concat`, `Cast` → trivial IR | 0.25d |
| 5 | `Slice` / `MultiSlice` → GEP-based array indexing | 0.5d |
| 6 | `ForAll` / `Exists` → boolean results | 0.25d |
| 7 | `StructInstance` / `ObjectLiteral` → field-by-field | 0.25d |
| 8 | Litmus test + regression | 0.5d |
| | **Total** | **~3d** |

## Litmus Test

```bash
# Step 1: Compile the Briv-in-Briv compiler to LLVM IR
briv-compiler llvm lib/compiler/main.bv --out /tmp/stage1/

# Step 2: Validate the IR with LLVM's assembler
llc /tmp/stage1/main.ll -o /tmp/stage1/main.o

# Step 3: Verify no fallback comments remain
grep -r "fallback" /tmp/stage1/ && echo "WARNING: fallback nodes remain"
```

## Regression

All 17 existing fixtures must still pass `llc`. 270 tests must pass.