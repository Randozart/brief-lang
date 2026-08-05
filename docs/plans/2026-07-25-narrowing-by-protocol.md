# Int Narrowing by Protocol — Correctness Fix
## 2026-07-25

## Problem

The Int narrowing pass has three bugs:

1. **Protocol vs name matching** — The pass matches on type name `"Int"` instead
   of `#Int` protocol membership. This excludes user-defined `#Int` types and
   makes `Int64` (which IS `Int { bits <~ 64 }`) eligible for narrowing despite
   its `bits <~ 64` cap.

2. **Narrowed width not propagated to SSA values** — The pass sets the function
   header return type correctly (e.g., `define i8 @test`) but the body still
   emits `i64` for all constants, identifiers, and operations. The LLVM IR
   type system rejects `add i8 %i64, %i64` and `ret i8 %i64`.

3. **`binop_int_type()` mismatch** — Binary operation templates use the narrowed
   width (`add i8`) while operands remain `i64`, producing invalid IR.

## Fix Strategy

Propagate the narrowed bit width through every `TypedRegister` so that SSA
values are emitted at the correct width from definition to return. Narrowing
operates on `#Int`/`#UInt` protocol membership, not type names.

## Changes

### 1. `src/optimizer/narrow_int.rs` — Protocol-based narrowing

Change the type check from name-matching to protocol-matching:

```rust
// Before:
if name == "Int" || name == "UInt" { ... }

// After:
let universe = self.ctx.type_universe.as_ref();
if universe.map_or(false, |u| u.find_protocol("Int", ty))
    || universe.map_or(false, |u| u.find_protocol("UInt", ty))
{
```

Then respect `bits <~ N` metadata to cap the floor: if a type has `bits <~ 64`,
even a proven range of 6 bits doesn't narrow below 64.

### 2. `src/backend/llvm/emit_expr.rs` — Propagate narrowed width

**a) Add `emit_int()` helper** — emits `add iN 0, imm` at the proven width:

```rust
fn emit_int(&mut self, out: &mut String, v: &str, imm: i64, indent: &str) -> TypedRegister {
    let bits = self.fun.narrowed.get("ret").copied()
        .map(|b| if b <= 1 { 8 } else { b.next_power_of_two().min(64) })
        .unwrap_or(64);
    let llvm_ty = format!("i{}", bits);
    writeln!(out, "{}{} = add {} 0, {}", indent, v, llvm_ty, imm).ok();
    TypedRegister { name: v.to_string(), ty: Type::from_bits(bits) }
}
```

**b) Change `Expr::Decimal`** — replace `add i64 0, N` with `emit_int()`.

**c) Change `Expr::Identifier` for integer bindings** — load at narrowed width
when the binding name is in `self.fun.narrowed`.

**d) Remove `binop_int_type()`** — instruction type derives from operand
widths. Operands are already at the correct width from steps b-c.

**e) Remove `ret_ty` override** at lines 1809-1813 — no longer needed since
all SSA values are at the correct width.

### 3. `src/backend/llvm/emit_toplevel.rs` — Fix `llvm_type()` narrowing

Change the narrowing check (lines 359-370) from name-matching to
protocol-matching, same logic as step 1.

### 4. `src/backend/llvm/emit_stmt.rs` — Keep `ret` truncation as safety net

The existing `trunc i64 %val to iN` at `ret` is kept as a safety net.
It should rarely fire since all values are already at the correct width.

### 5. `AGENTS.md` — Document protocol-based narrowing

Updated item 20 to describe the intended behavior.

## Verification

```bash
# 1. Simple constant emits correct IR
cat > /tmp/t1.bv << 'EOF'
defn f() -> Int { term 42; }
EOF
brivc build --backend llvm /tmp/t1.bv 2>&1
grep "define.*@f" /tmp/t1.ll        # → define i8 @f
grep "add.*0.*42" /tmp/t1.ll        # → add i8 0, 42
grep "ret" /tmp/t1.ll               # → ret i8 %tX
llc -O0 /tmp/t1.ll -o /dev/null     # passes

# 2. SysCall# with Int64 wrapper
cat > /tmp/t2.bv << 'EOF'
defn test() -> Int64 {
    let fd: Int64 = SysCall#(2, path, 0, 0, 0, 0, 0);
    term fd;
}
EOF
brivc build --backend llvm /tmp/t2.bv 2>&1
llc -O0 /tmp/t2.ll -o /dev/null     # passes

# 3. ShellCmd# compiles and runs
brivc build --backend llvm /tmp/shell.bv 2>&1
clang /tmp/shell.ll lib/runtime/briv_rt.c -o shell
./shell                     # works
```
