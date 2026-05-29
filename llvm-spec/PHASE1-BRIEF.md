# Phase 1 Brief: Basic Transaction Emission

**Date:** 2026-05-29  
**Spec Reference:** `03-TRANSACTIONS.md`  
**Prerequisite:** Phase 0 complete (scaffold, LLVM 18 compatibility validated)  
**Estimated Effort:** 2-3 days  

---

## Goal

A single `txn` with `Int` fields, `let` bindings, `&field = expr` assignments, `term`, integer arithmetic, and `guarded` blocks compiles to correct, LLVM-verifyable IR.

## Concrete Deliverables

Each deliverable includes a test fixture, a validation command, and the expected output signature.

### 1. `let` Bindings → SSA Registers

**Brief:**
```brief
let x: Int = 42;
let y: Int = x + 1;
```

**Expected LLVM:**
```llvm
%x = add i64 0, 42
%y = add i64 %x, 1
```

**Validation:** `llc` must produce assembly with two register operations, no memory stores.

### 2. `&field = expr` Assignments → Store via GEP

**Brief:**
```brief
&count = count + 1;
```

**Expected LLVM:**
```llvm
%ptr = getelementptr inbounds %State, %State* %state, i32 0, i32 <idx>
%ld = load i64, i64* %ptr
%new = add i64 %ld, 1
store i64 %new, i64* %ptr
```

**Validation:** `llc` must produce a `store` instruction with correct offset.

### 3. `term` → `ret void`

**Brief:**
```brief
term;
```

**Expected LLVM:** `ret void`

### 4. `term expr` → `ret i64 %val`

**Brief:**
```brief
term result;
```

**Expected LLVM:** `ret i64 %result`

### 5. Integer Arithmetic

| Operation | LLVM | Test |
|-----------|------|------|
| Add | `add i64 %a, %b` | `a + b` |
| Sub | `sub i64 %a, %b` | `a - b` |
| Mul | `mul i64 %a, %b` | `a * b` |
| Div | `sdiv i64 %a, %b` | `a / b` |
| Mod | `srem i64 %a, %b` | `a % b` |

**Validation:** `llc` must produce arithmetic instructions without crashing.

### 6. Guarded Blocks → `br i1` + Basic Blocks

**Brief:**
```brief
[count < 100] {
    &count = count + 1;
};
```

**Expected LLVM:**
```llvm
%cond = icmp slt i64 %count, 100
br i1 %cond, label %then, label %end
then:
  ; body
  br label %end
end:
```

**Validation:** `opt -verify` must pass. `llc` must produce a conditional branch instruction.

### 7. Bool-to-i64 Coercion (Preconditions)

**Brief:**
```brief
rct txn increment [counter < 10] [...]
```

**Expected LLVM:** The precondition expression `counter < 10` emits `icmp slt` + `zext` to produce an `i64` from the `i1` comparison. The `zext` is needed because Brief's expression system uses `i64` for Bool values (1 = true, 0 = false).

## New Test Fixtures

Create `tests/fixtures/phase1/` with:

| Fixture | Tests |
|---------|-------|
| `let_binding.bv` | `let x: Int = 42; let y: Int = x + 1;` — SSA register emission |
| `arithmetic.bv` | `a + b`, `a - b`, `a * b`, `a / b`, `a % b` — all 5 ops |
| `guarded.bv` | `[x < 100] { &x = x + 1; };` — conditional branch + store |
| `term_expr.bv` | `term result;` — return value emission |
| `full_txn.bv` | Single transaction using all features above |

## Acceptance Criteria

All of the following must pass:

```bash
brief-compiler llvm tests/fixtures/phase1/full_txn.bv --out /tmp/p1/
llc /tmp/p1/full_txn.ll -o /dev/null          # Must succeed
opt -verify /tmp/p1/full_txn.ll -o /dev/null   # Must succeed
grep -c "add i64" /tmp/p1/full_txn.ll           # Integer arithmetic present
grep -c "store" /tmp/p1/full_txn.ll             # Store instructions present
grep -c "br i1" /tmp/p1/full_txn.ll             # Guarded branches present
```

## Implementation Checklist

- [ ] `Statement::Let` → emit SSA register, no memory store
- [ ] `Statement::Assignment` → GEP + load + compute + store (already done in Phase 0)
- [ ] `Statement::Term { values: [] }` → `ret void`
- [ ] `Statement::Term { values: [expr] }` → emit expr, then `ret i64 %val`
- [ ] `Statement::Guarded` → `icmp` cond → `br i1` → `then:` block → `br label %end` → `end:` label
- [ ] Chained expressions produce correct SSA register references
- [ ] All Phase 1 fixtures pass `llc` + `opt -verify`
- [ ] Phase 0 regression suite still passes (counter, multifield, minimal)

## Risks

- **Bool values are i64 in Brief's expression system** but i8 in `%State`. Transactions must `trunc` stores and `zext` loads. The Phase 0 implementation handles this, but the `let` binder must preserve the i64 type for chained arithmetic.
- **Precondition expressions** (`[counter < 10]`) emit `icmp` → `i1` → `zext` to `i64`. The `zext` register must not collide with the transaction's body registers. The `txn_counter` field handles this.