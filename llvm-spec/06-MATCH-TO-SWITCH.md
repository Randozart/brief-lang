# Match Expression → LLVM switch

## Overview

Both `uni` (single-arm) and `match` (multi-arm) pattern matching lower to LLVM's `switch` instruction. The pattern is consistent:

```llvm
switch i64 %discriminant, label %default [
    i64 0, label %arm_0
    i64 1, label %arm_1
    ...
]
```

## Discriminant Layout

Every enum in the `%State` type has a discriminant field (i64, 0-indexed):

```briev
enum Option<Int> { Some(Int), None }
// discriminant: 0 = None, 1 = Some
```

```llvm
%disc = extractvalue %struct.State %state_val, <discriminant_index>
; or
%disc_ptr = getelementptr inbounds %struct.State, %struct.State* %state, i64 0, i32 <disc_index>
%disc = load i64, i64* %disc_ptr
```

## Match Arm Translation

```briev
match val {
    Ok(x) => handle_ok(x),
    Err(e) => handle_err(e),
    _ => handle_default,
}
```

```llvm
; Load discriminant
%disc = ... ; from val's enum discriminant

; Switch on discriminant
switch i64 %disc, label %arm_default [
    i64 0, label %arm_ok      ; Ok variant
    i64 1, label %arm_err     ; Err variant
]

arm_ok:
    ; Extract field(s)
    %x = extractvalue %struct.Result_Int_String %val, 1, 0  ; offset 1 = payload, offset 0 = first field
    ; Call handler
    %result = call i64 @handle_ok(i64 %x)
    br label %merge

arm_err:
    %e_ptr = getelementptr ... ; string pointer
    %e = load i64, i64* %e_ptr ; or string handling
    %result2 = call i64 @handle_err(i64 %e)
    br label %merge

arm_default:
    %result3 = call i64 @handle_default()
    br label %merge

merge:
    %final = phi i64 [%result, %arm_ok], [%result2, %arm_err], [%result3, %arm_default]
```

## Field Extraction

For a variant with fields (`Ok(Int)`), the payload struct is embedded in the enum:

```llvm
; Variant type: %struct.Result_Ok = type { i64 }
; Enum type:    %struct.Result_Int_String = type { i64, %struct.Result_Ok, %struct.Result_Err }
;                                                  ^disc     ^payload 0            ^payload 1

; Extract value from Ok variant:
%val = extractvalue %struct.Result_Int_String %result_val, 1, 0
;                               1 = Ok payload slot  0 = first field of Ok struct
```

## Expression vs Statement Match

**Expression match** (returns a value): Phi node at the merge point. Each arm produces an SSA value, and the phi selects the right one.

```llvm
%result = phi i64 [%arm0_val, %arm_0], [%arm1_val, %arm_1], [%default_val, %arm_default]
```

**Statement match** (void return, like `term;` in each arm): No phi. Each arm branches to a common continuation or returns void.

## Guard Support

```briev
match val {
    Ok(x) if x > 0 => handle_positive(x),
    _ => handle_other,
}
```

```llvm
arm_ok:
    %x = extractvalue ...
    %guard_cond = icmp sgt i64 %x, 0
    br i1 %guard_cond, label %arm_ok_body, label %arm_ok_fail

arm_ok_body:
    %r1 = call i64 @handle_positive(i64 %x)
    br label %merge

arm_ok_fail:
    ; Fall through to next arm (or default)
    br label %arm_default_or_next

arm_default_or_next:
    %r2 = call i64 @handle_other()
    br label %merge
```

## The Exhaustiveness Guarantee

The typechecker requires `_ =>` if not all variants are covered. The LLVM backend can therefore:

1. Use `switch` with a known discriminant range → LLVM generates a jump table
2. If `_ =>` exists, the `default` label points to it
3. If all variants are covered and no `_ =>`, LLVM can mark `default` as `unreachable`

```llvm
; All 3 variants covered, no default → unreachable
switch i64 %disc, label %unreachable [
    i64 0, label %arm_0
    i64 1, label %arm_1
    i64 2, label %arm_2
]

unreachable:
    unreachable  ; LLVM can optimize this entire switch to a known-true branch
```

## Implementation Priority

1. **`match { Variant(x) => body, _ => default }`** — single variant + wildcard (simplest)
2. **`match { V1 => b1, V2 => b2 }`** — multi-variant, no wildcard (exhaustive known)
3. **Guards**: `if condition` on arms
4. **Nested patterns**: `match { V1(inner) if inner > 0 => ... }`