# Error Messages + Highlighter Updates

**Date:** 2026-06-25
**Status:** Active

## Motivation

Phase 1+2 (Ptr<T>, volatile_load#/volatile_store#, asm target {} in BILD)
added a whole class of low-level features without proper error messages or
syntax highlighting. Users get silent type mismatches (Void), generic
fallthrough errors, or no errors at all. This plan fixes both gaps.

## Part 1: Error Messages

### A. CRITICAL — volatile_load# typechecker silently returns Void

**File:** `src/typechecker.rs:2010-2025`
**Problem:** When `volatile_load#` gets a non-Ptr<T> argument, the return
type silently becomes `Type::Void`. This produces confusing downstream
errors instead of a clear diagnostic at the call site.

**Fix:** Add a `TypeError::TypeMismatch` diagnostic when the argument is
not `Ptr<T>`:
```
volatile_load# requires a Ptr<T> argument, got <actual_type>
```
Also validate arg count — require exactly 1 argument.

### B. CRITICAL — volatile_store# typechecker validates nothing

**File:** `src/typechecker.rs:2027`
**Problem:** `Intrinsic::VolatileStore => Type::Bool` accepts any argument
types without validation.

**Fix:** Validate:
- First arg is `Ptr<T>`
- Second arg type matches `T`
- Emit `TypeError::TypeMismatch` with clear message:
  ```
  volatile_store# requires (Ptr<T>, T) arguments, got (<arg1_type>, <arg2_type>)
  ```
- Wrong arg count → `"volatile_store# requires exactly 2 arguments, got N"`

### C. HIGH — Ptr in binary_op_type_scalar falls to "unknown"

**File:** `src/typechecker.rs:2813-2838`
**Problem:** `binary_op_type_scalar` has no Ptr arms, so `Ptr + Float`,
`Ptr + Bool`, `Ptr + String` in nested contexts (List<Ptr<T>>) produce
`Type::Custom("unknown")` silently.

**Fix:** Add Ptr+Int arm returning the Ptr type, and Ptr+nonInt arm
emitting a `TypeError::TypeMismatch`:
```
cannot perform arithmetic on Ptr<T> with <other_type>
```

### D. MEDIUM — binary_op.rs error hides operand types

**File:** `src/features/binary_op.rs:116`
**Problem:** The catch-all `_ =>` error is `"binary op {:?}"` which shows
the operator but not the operand types.

**Fix:** Include types:
```
"binary op {:?} on ({:?}, {:?})"
```
This helps users immediately see what types conflicted.

### E. MEDIUM — bild_asm.rs unclosed block silent

**File:** `src/analysis/bild_asm.rs:247-250`
**Problem:** An unclosed `asm target {` block silently keeps the original
BILD lines, which then fail at LLVM assembly time with a confusing error.

**Fix:** Use `eprintln!` to warn:
```
"warning: unclosed asm target block — expected '}' at end"
```
This matches the existing warning pattern in this file.

### F. LOW — LLVM backend volatile_load/store validation

**File:** `src/backend/llvm/emit_expr.rs:2444,2482`
**Problem:** No compile-time validation before emitting `inttoptr` on
arbitrary input. Typechecker should catch this first, but defensive
validation prevents UB in edge cases.

**Fix:** Add a debug_assert or `unreachable!()` if the register type is
not Ptr. The typechecker path is the primary defense — this is a
safety net for latent bugs.

### G. LOW — Ptr :> projection validation

**File:** `src/typechecker.rs:2346-2348`
**Problem:** The catch-all arm `_ => Ptr<typeof(x)>` wraps any type in
Ptr without checking whether the projection makes sense.

**Fix:** Add a `TypeError::TypeMismatch` diagnostic when the source type
is not a pointer, list, string, or struct:
```
".#Ptr projection requires Ptr, List, String, or struct, got <type>"
```

## Part 2: Highlighter Updates

**File:** `syntax-highlighter/syntaxes/briev.tmLanguage.json`

### Keywords (keyword.declaration)

Add to the existing `\b(...)\b` pattern:
- `inop` / `INOP` — user-defined intrinsic declaration
- `bild` / `BILD` — BILD block
- `type` / `TYPE` — type definition
- `fallback` / `FALLBACK` — inop fallback clause
- `target` / `TARGET` — asm target / import target (new keyword.modifier)

### Modifiers (storage.modifier)

New patterns:
- `mmio` / `MMIO` — annotation keyword
- `wake` / `WAKE` — trigger modifier
- `nowake` — trigger modifier

### Types (support.type.primitive)

Add to `\b(Int|Float|String|Bool|Data|Void|UInt)\b`:
- `Ptr` — pointer type
- `Bits` — type universe built-in

### Intrinsic functions (support.function.intrinsic)

New pattern for `identifier#` calls:
- Match `\b[a-z_][a-zA-Z0-9_]*#` scope as `support.function.intrinsic.briev`
- This catches `volatile_load#`, `volatile_store#`, `strlen#`, `bytes#`, etc.

### Operators

Add:
- `:>` — `keyword.operator.projection.briev`
- `<-` — `keyword.operator.push.briev`
- `!` after `term`/`escape` — scope as part of control flow

## Implementation Order

1. Write plan document ✓
2. Fix volatile_load# typechecker — emit diagnostic instead of silent Void ✓
3. Fix volatile_store# typechecker — validate args ✓
4. Fix binary_op_type_scalar — add Ptr arm + diagnostic ✓
5. Fix binary_op.rs error message — include operand types ✓
6. Fix bild_asm.rs — error for unclosed asm target block ✓
7. Fix LLVM backend defensive validation ✓
8. Fix Ptr :> projection typecheck validation ✓
9. Update highlighter grammar (keywords, types, intrinsics, operators) ✓
10. Run `cargo test --lib` — all pass ✓

## Part 3: VS Code Extension — Add .abv and .cbv Languages

### Icon files

Copy from `assets/` to `syntax-highlighter/images/`:
- `assets/a-briev-icon.svg` → `images/a-briev-logo.svg` (Accelerated Briev)
- `assets/c-briev-icon.svg` → `images/c-briev-logo.svg` (Circuit Briev)

All icon files in `images/` follow the `{prefix}-briev-logo.svg` naming convention.

### package.json changes

1. **Main Briev language icon**: change from `./images/briev-logo.svg` to
   `./images/extension-logo.svg` (tighter crop, viewBox="242 242 540 540")

2. **ActivationEvents**: append `onLanguage:abv` and `onLanguage:cbv`

3. **New language: abv** (Accelerated Briev, `.abv`):
   - Icon: `./images/a-briev-logo.svg`
   - Grammar: `source.briev` → `./syntaxes/briev.tmLanguage.json`

4. **New language: cbv** (Circuit Briev, `.cbv`):
   - Icon: `./images/c-briev-logo.svg`
   - Grammar: `source.briev` → `./syntaxes/briev.tmLanguage.json`

5. **Grammar entries**: add `abv → source.briev` and `cbv → source.briev`

### briev.tmLanguage.json

Add `<:` (subtype operator) pattern before `<` to prevent partial matching:
```
"keyword.operator.subtype.briev" → match "<:"
```

## Part 4: Future

- Fix `[pre]]` dangling bracket highlight (waiting on user screenshot)
- Consider file icon theme for more detailed file-type differentiation

