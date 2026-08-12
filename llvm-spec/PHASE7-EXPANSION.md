# Phase 7 Expansion Plan: Full LLVM IR Emission in Briev

**Date:** 2026-05-29  
**Current state:** Foundation file exists at `lib/compiler/backends/llvm.bv` (285 lines, mostly stubs). Parses and typechecks. Blocked on Rust backend List-arg internal call fix for full `llc` validation.

## File Structure After Expansion

```
defn generate_llvm(program, cg) -> String        // Entry point — module header, loop items
defn emit_state_type(program) -> String           // %State = type { i64, i8, ... }
defn emit_init_state(program) -> String           // store volatile for each field
defn collect_field_info(program) -> (List<String>, List<String>, Map<String, Int>)
                                                  // names, types, name→index map
defn emit_txn(txn, field_idx) -> String           // define void @name(%State* noalias nocapture)
defn emit_defn(defn) -> String                    // define i64 @name(i64 %arg0, ...)
defn emit_body(body, field_idx) -> String         // Loop over statements
defn emit_stmt(stmt, field_idx) -> String         // Match on Statement variant, emit IR
defn expr_to_ir(expr, reg, field_idx) -> String   // Match on Expr variant, emit IR, return "%tmpN = instr"
defn expr_const(expr) -> String                   // Constant value string
defn emit_pre_func(txn) -> String                 // define internal i1 @pre_txn(%State*) 
defn emit_fused(a, b) -> String                  // Fused transaction body
defn emit_ffi_declare(binding) -> String          // declare i64 @fn(i8*, ...)
defn emit_reactor(txns, field_idx) -> String      // Dispatch chain + equilibrium + main()
defn emit_main() -> String                        // define i32 @main()
defn llvm_type(ty) -> String                      // Type::Int → "i64"
defn align_of(ty) -> Int                          // "i64" → 8
```

## Statement Emission Table

| Statement | Current | Planned IR |
|-----------|---------|------------|
| `StmtTerm(values, _)` | `ret void` / `ret i64 0` | `ret void` / `ret i64 %val` |
| `StmtEscape(expr)` | `ret void` / `ret i64 0` | `ret void` / `ret i64 %val` |
| `StmtAssign(lhs, rhs, _)` | `; assign` | GEP into %State + load + compute + store |
| `StmtLet(name, ty, init, _, addr)` | `; let` | SSA register via expr_to_ir, track in map |
| `StmtGuarded(cond, body)` | `; guard` | `icmp ne` + `br i1` + unique labels |
| `StmtUnification(name, pat, expr)` | `; uni` | `switch i64 %discriminant` |
| `StmtLocalTrigger(name, ty, expr)` | `; trg!` | `; await` comment placeholder |
| `StmtExpr(expr)` | calls expr_to_ir | Calls expr_to_ir, drops result |
| `StmtAlka(block)` | raw content | Raw content passthrough |
| `StmtOnExit(body, _)` | `; on_exit` | Register in pending_cleanup |

## Expression Emission Table

| Expression | Current | Planned IR |
|------------|---------|------------|
| `ExprInt(n)` | `add i64 0, N` | `add i64 0, N` |
| `ExprBool(b)` | `add i64 0, 1/0` | `add i64 0, 1/0` |
| `ExprFloat(f)` | fallback | `fadd float 0.0, F` |
| `ExprString(s)` | fallback | `alloca` + `ptrtoint` to i64 |
| `ExprChar(c)` | fallback | `add i32 0, C` + `zext` to i64 |
| `ExprTerm` | fallback | `add i64 0, 0` |
| `ExprVar(name)` | `add i64 0, 0` | Lookup in let_bindings then field GEP + load |
| `ExprPriorState(name)` | fallback | Comment placeholder |
| `ExprBinOp(op, l, r)` | fallback | Match op to add/sub/mul/div/mod/icmp |
| `ExprUnaryOp(op, e)` | fallback | not → `xor`, neg → `sub 0` |
| `ExprCall(name, args)` | fallback | Uppercase → alloca+ptrtoint, else `call i64 @name` |
| `ExprFieldAccess(obj, f)` | fallback | Comment placeholder |
| `ExprIndex(arr, idx)` | fallback | `inttoptr` + `getelementptr` |
| `ExprSlice(arr, start, end, s, m)` | fallback | `inttoptr` + `getelementptr` |
| `ExprMultiSlice(arr, coords, m)` | fallback | Recursive slice |
| `ExprList(elems)` | fallback | `alloca` + `ptrtoint` |
| `ExprTuple(elems)` | fallback | Sequential registers |
| `ExprTupleDestructure(names, e)` | fallback | Destructure comment |
| `ExprCast(e, ty)` | fallback | Passthrough |
| `ExprBlock(stmts)` | fallback | Emit statements |
| `ExprForAll(var, body)` | fallback | `add i64 0, 1` (always true) |
| `ExprExists(var, body)` | fallback | `icmp ne` |

## Data Structures

The backend needs state across calls. In Briev, this uses closure-passing patterns (unlike Rust's struct fields):

```briev
// Field tracking — passed as parameter
let field_names = collect_field_names(program);
let field_types = collect_field_types(program);
let field_idx_map = build_field_name_to_idx(program);

// Let binding tracking — passed as parameter
// Use the event loop pattern: carry a Map<String, Int> through calls
```

## Git Strategy

All changes go into a single commit with message:

```
Phase 7: expand llvm.bv to full IR emission
    
- Statement emission: assign, let, guarded, uni, term, escape
- Expression emission: all 20+ Expr variants to real IR
- Contract support: precondition extraction, !range metadata
- FFI declare/call with ABI marshaling
- Reactor loop: trigger sampling, precondition dispatch, equilibrium
- Metadata emission: !range nodes at module footer
- Wire llvm dispatch into main.bv
```

## Acceptance Criteria (when Rust List-arg fix lands)

```bash
# All existing fixtures still pass
briev-compiler llvm tests/fixtures/counter.bv --out /tmp/r/
llc /tmp/r/counter.ll -o /dev/null

# llvm.bv compiles through itself
briev-compiler llvm lib/compiler/backends/llvm.bv --out /tmp/r/
grep -c "fallback" /tmp/r/llvm.ll  # Should be 0
llc /tmp/r/llvm.ll -o /dev/null
```