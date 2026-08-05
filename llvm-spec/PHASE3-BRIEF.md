# Phase 3 Briv: Match Expression → switch

**Date:** 2026-05-29  
**Spec Reference:** `06-MATCH-TO-SWITCH.md`  
**Prerequisite:** Phase 2.5 complete (trigger sampling, fusing)  
**Estimated Effort:** 2-3 days  

## Goal

`match val { V1(x) => ..., V2 => ..., _ => ... }` generates `switch i64 %discriminant` with payload extraction via `extractvalue` and `phi` for expression-returning matches.

## AST Nodes

Three constructs lower to `switch`:

### 1. `Statement::Unification`
```briv
uni Some(value) = opt_result { ... };
// Single-arm match: if discriminant matches "Some", extract value and execute body
```

**AST fields:** `name: String`, `pattern: String`, `expr: Expr`

### 2. `Expr::Match`
```briv
let result = match val {
    Ok(x) => handle_ok(x),
    Err(e) => handle_err(e),
    _ => 0,
};
// Returns a value — phi at merge point
```

**AST fields:** `value: Box<Expr>`, `arms: Vec<MatchArm>` where each arm has `pattern: MatchPattern` (Wildcard or Variant { name, fields })

### 3. `Expr::PatternMatch` (guard context)
```briv
[value Variant(field1, field2)] { ... }
// Boolean guard: true if value's discriminant matches Variant
```

**AST fields:** `value: Box<Expr>`, `variant: String`, `fields: Vec<String>`

## LLVM switch Instruction

```llvm
switch i64 %discriminant, label %default [
    i64 0, label %arm_0
    i64 1, label %arm_1
]
```

### Discriminant Loading
The enum type is embedded in `%State` or is a local SSA value. Discriminant is always `i64`:
- From state field: `extractvalue %struct.State %val, <payload_idx>, 0`
- From SSA register: `extractvalue %struct.Enum_Type %val, 0`

### Payload Extraction
```briv
enum Option<Int> { Some(Int), None }
// Layout: { i64, { i64 } } — discriminant + Some payload
```

```llvm
; Extract the Int from Some variant:
%payload = extractvalue %struct.Option_Int %val, 1  ; slot 1 = Some payload
%field = extractvalue %struct.Some_Payload %payload, 0  ; first field of payload
```

### Phi for Expression Matches
```llvm
arm_ok:
  %r1 = call i64 @handle_ok(i64 %x)
  br label %merge
arm_err:
  %r2 = call i64 @handle_err(i64 %e)
  br label %merge
arm_default:
  %r3 = add i64 0, 0
  br label %merge
merge:
  %result = phi i64 [%r1, %arm_ok], [%r2, %arm_err], [%r3, %arm_default]
```

### Exhaustiveness
If all variants are covered and no wildcard, `default` = `unreachable`:
```llvm
switch i64 %disc, label %unreachable [
    i64 0, label %arm_0
    i64 1, label %arm_1
]
unreachable:
  unreachable
```

## Test Fixtures

| Fixture | Tests |
|---------|-------|
| `match_simple.bv` | `match x { Some(v) => v, None => 0 }` — single variant + wildcard |
| `match_multi.bv` | `match x { A => 1, B => 2, C => 3 }` — multi-variant, exhaustive |
| `match_guard.bv` | `[value Variant(f)] { ... }` — pattern match in guard |
| `unify_simple.bv` | `uni Some(val) = expr { &out = val; };` — unification with body |

## Acceptance Criteria

```bash
for f in tests/fixtures/phase3/*.bv; do
  briv-compiler llvm "$f" --out /tmp/p3/
  llc /tmp/p3/$(basename "$f" .bv).ll -o /dev/null  # Must succeed
done
grep "switch" /tmp/p3/match_simple.ll         # switch instruction present
grep "phi" /tmp/p3/match_multi.ll              # phi for multi-arm return
grep "extractvalue" /tmp/p3/match_simple.ll    # payload extraction
grep "unreachable" /tmp/p3/match_multi.ll       # exhaustive match -> unreachable
```

## Implementation Checklist

- [ ] Determine enum type layout (`EnumDefinition` from program items) for discriminant offset
- [ ] `Expr::Match` → emit switch, per-arm basic blocks, phi merge
- [ ] `Statement::Unification` → single-arm switch + body execution
- [ ] `Expr::PatternMatch` → `extractvalue` discriminant + `icmp eq` for guard
- [ ] `extractvalue` for payload field extraction at correct offsets
- [ ] Wildcard default → label, exhaustive default → `unreachable`
- [ ] Phase 0-2.5 regression fixtures still pass