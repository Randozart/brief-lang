# Execution Plan: Native Types + String Architecture + Officina Fixes

**Date:** 2026-06-16 23:45  
**Status:** Ready to execute  
**Session:** Single continuous build, no deferrals

---

## What This Plan Covers

### Core Architecture Change
All LLVM SSA values carry their natural types instead of being boxed in `i64`:
| Briv type | Was (i64 boxed) | Now (native) |
|------------|----------------|--------------|
| `String` / `Data` | `i64` (ptrtoint of header) | `i8*` (pointer to Briv 2-slot header) |
| `Bool` | `i64` (trunc/zext round-trip) | `i1` (SSA ops), `i8` (memory) |
| `Char` | `i64` (zext from i32) | `i32` |
| `Int` / `UInt` | `i64` | `i64` (no change) |
| `Float` | `float` | `float` (already native) |

Every `inttoptr`/`ptrtoint` round-trip for strings is eliminated. Every `trunc`/`zext` for bools and chars is eliminated. The state field type system (`trg_llvm_storage_ty`) already returns native types — the only problem was the expression system boxing them.

### String Concatenation Strategy
- **Both operands compile-time constant** → fold at compile time (single global Briv header)
- **One or both runtime** → always-allocate inline concat (malloc + memcpy + header setup)
- **No buffer reuse** — Briv compiler has no ownership/lifetime analysis (would need to be built from scratch)

---

## File-by-File Changes

### FILE 1: `src/backend/llvm/mod.rs`

**1a. Remove `__str_concat` declare, add `malloc` + `strlen` declares**

```text
- Remove: declare i8* @__str_concat(i8*, i8*) #1
+ Add:    declare noalias i8* @malloc(i64) #1
+ Add:    declare i64 @strlen(i8*) #1
```

**1b. Fix FFI param type for String/Data**

```text
- Type::String | Type::Data => "i8",     // WRONG — i8 is a byte, not a pointer
+ Type::String | Type::Data => "i8*",    // CORRECT — C functions expect char*
```

**1c. String constant globals → Briv headers**

Already done in prior edit. Verify format:
```llvm
@str.0 = private unnamed_addr constant <{ i64, i64, [5 x i8] }> <{
  i64 ptrtoint (i8* getelementptr inbounds (...@str.0..., i64 0, i32 2) to i64),
  i64 5,
  [5 x i8] c"hello\00"
}>, align 8
```

---

### FILE 2: `src/backend/llvm/emit_expr.rs`

Every expression variant that produces or consumes strings (i8*), bools (i1), or chars (i32) must be updated.

**2a. Add `emit_inline_concat` helper method** (new)

```rust
/// Emit inline string concat: malloc + header setup + memcpy.
/// No buffer reuse — we lack ownership analysis.
/// Both %a and %b are i8* (Briv header pointers).
/// Returns TypedRegister with ty: Type::String.
fn emit_inline_concat(&mut self, out: &mut String, indent: &str, a: &str, b: &str) -> TypedRegister {
    let ha = format!("%cha{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, ha, a).ok();
    let la_ptr = format!("%clp{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, la_ptr, ha).ok();
    let la = format!("%cla{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, la, la_ptr).ok();

    let hb = format!("%chb{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, hb, b).ok();
    let lb_ptr = format!("%clq{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lb_ptr, hb).ok();
    let lb = format!("%clb{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, lb, lb_ptr).ok();

    let total = format!("%ctl{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, {}", indent, total, la, lb).ok();
    let slot_count = format!("%csc{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, 2", indent, slot_count, total).ok();
    let alloc_size = format!("%cas{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_size, slot_count).ok();
    let result = format!("%cr{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = call i8* @malloc(i64 {})", indent, result, alloc_size).ok();

    let hp = format!("%chp{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, hp, result).ok();
    let base = format!("%cba{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, result).ok();
    let dp = format!("%cdp{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base).ok();
    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, dp, hp).ok();

    let len_slot = format!("%cls{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, len_slot, hp).ok();
    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, total, len_slot).ok();

    let a_dp = format!("%cad{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, a_dp, ha).ok();
    let a_chars = format!("%cac{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, a_chars, a_dp).ok();
    let dest_slot2 = format!("%cds{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dest_slot2, hp).ok();
    let dest = format!("%cdt{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = bitcast i64* {} to i8*", indent, dest, dest_slot2).ok();
    writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)", indent, dest, a_chars, la).ok();

    let dest_off = format!("%cdo{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i8, i8* {}, i64 {}", indent, dest_off, dest, la).ok();
    let b_dp = format!("%cbd{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, b_dp, hb).ok();
    let b_chars = format!("%cbc{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, b_chars, b_dp).ok();
    writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)", indent, dest_off, b_chars, lb).ok();

    let v = format!("%t{}", self.txn_counter); self.txn_counter += 1;
    writeln!(out, "{}{} = bitcast i8* {} to i8*", indent, v, result).ok();
    TypedRegister { name: v, ty: Type::String }
}
```

**2b. `Expr::String(s)` → native i8***

```text
BEFORE:
  %sp = getelementptr inbounds [N x i8], [N x i8]* @str.X, i64 0, i64 0
  %v = ptrtoint i8* %sp to i64        # result is i64

AFTER:
  %v = bitcast <{ i64, i64, [N x i8] }>* @str.X to i8*   # result is i8*
```

**2c. `Expr::Concat(l, r)` → inline concat**

```text
BEFORE:
  %ip = inttoptr i64 %a to i8*
  %jp = inttoptr i64 %b to i8*
  %cc = call i8* @__str_concat(i8* %ip, i8* %jp)
  %v = ptrtoint i8* %cc to i64        # result is i64

AFTER:
  ; a and b are already i8* (native)
  ...call emit_inline_concat(a, b)...  # result is i8* (TypedRegister with ty: String)
```

**2d. `Expr::Bool(b)` → native i1**

```text
BEFORE:
  %v = add i64 0, 1    # or 0  (i64)

AFTER:
  %v = add i1 true, false   →   %v = select i1 true, i1 true, i1 false
  OR: %v = insertvalue ...   →   simpler: %v = xor i1 0, 0  (or %v = or i1 true, false)
  ACTUALLY: just emit:
  %v = add i64 0, N   →  %v = select i1 1, i1 1, i1 0   ... no
  For Bool literal:
  if *b { writeln!(out, "{}{} = or i1 true, false", indent, v).ok(); }
  else { writeln!(out, "{}{} = and i1 true, false", indent, v).ok(); }
```

Actually, the simplest i1 true value is: `%v = add i1 0, 1` — but LLVM doesn't allow `add i1`. The standard pattern is:
```
%v = icmp eq i64 1, 1    // true
%v = icmp eq i64 0, 1    // false
```
Or using `select`:
```
%v = select i1 true, i1 true, i1 false
```
Or more directly:
```
%v = xor i1 true, true     // false
%v = xor i1 true, false    // true
```

Hmm, actually the simplest approach:
```
; true
%v = xor i1 true, false
; false
%v = xor i1 true, true
```

Or even simpler, LLVM has `true` and `false` as constant i1 values:
```
%v = add i1 false, true    ; true  — wait, i1 addition is XOR
; Actually the simplest:
%v = icmp ne i64 0, 1    ; true
%v = icmp eq i64 0, 1    ; false
```

But the cleanest is probably just:
```
%v = select i1 true, i1 true, i1 false   ; always true
%v = select i1 true, i1 false, i1 true   ; always false
```

Or we could use the LLVM constant directly — you can use `true` or `false` as operands in LLVM IR. So:
```
%v = and i1 true, true    ; true
%v = xor i1 true, true    ; false
```

Actually wait, `and i1 true, true` always produces `true` and `xor i1 true, true` always produces `false`. LLVM's optimizer will fold these to constants. But we could also just use a constant directly in expressions. Let me check if LLVM allows constant i1 in IR directly... Yes, you can write `i1 1` for true and `i1 0` for false.

So:
```
%v = add i64 0, 1          ; OLD (i64)
%v = or i1 1, 0            ; NEW (i1) — "or 1, 0" = 1 (true)
%v = and i1 1, 0           ; (false)
```

Hmm wait, `or i1 1, 0` is valid LLVM IR. Let me use:
- true: `%v = or i1 true, false`
- false: `%v = and i1 true, false`

Or shorter:
- true: `%v = and i1 true, true`
- false: `%v = xor i1 true, true`

OK this is getting too detailed for the plan. Let me simplify.

**2e. `Expr::Eq/Ne/Lt/Le/Gt/Ge(l, r)` → native i1 result**

These already produce i1 in LLVM (icmp returns i1). The issue is that currently the i1 is zext'd to i64. With native types, we keep it as i1.

```text
BEFORE:
  %cmp = icmp eq i64 %a, %b    ; returns i1
  %v = zext i1 %cmp to i64     ; box to i64

AFTER:
  %v = icmp eq i64 %a, %b      ; returns i1, keep as i1
```

**2f. `Expr::And/Or/Not` → native i1**

```text
BEFORE:
  %v = or i64 %a, %b       ; or/and/xor on i64

AFTER:
  ...take i1 inputs, produce i1 result...
  %v = or i1 %a, %b        ; or on i1
  %v = and i1 %a, %b       ; and on i1
  %v = xor i1 %a, %b       ; xor on i1 (also Not: xor %e, 1)
```

**2g. `Expr::Char(c)` → native i32**

```text
BEFORE:
  %cc = add i32 0, 65
  %v = zext i32 %cc to i64         # box to i64

AFTER:
  %v = add i32 0, 65               # native i32
```

**2h. `Expr::Call(name, args)` — native types for FFI**

Currently marshals params (lines 290, 327) with `inttoptr i64 %raw to i8*` for String params. With native types, String params are already `i8*`, so we just pass them through directly. BUT we still need to extract `data_ptr` from the Briv header — the C function expects `char*`, not `i8*` (Briv header pointer).

Wait — this is the key issue. Even with native `i8*` for strings, the `i8*` is a **Briv header pointer**, not a C string pointer. The C function expects `char*` pointing to the actual string data. So we still need to extract slot 0.

WITH native types, the code changes from:
```llvm
; OLD: %raw is i64 (boxed)
%hp = inttoptr i64 %raw to i64*    ; header pointer
%dp = load i64, i64* %hp           ; data_ptr
%cstr = inttoptr i64 %dp to i8*    ; C string
```
to:
```llvm
; NEW: %raw is i8* (native Briv header pointer)
%hp = bitcast i8* %raw to i64*     ; header pointer
%dp = load i64, i64* %hp           ; data_ptr
%cstr = inttoptr i64 %dp to i8*    ; C string
```

The only difference is `bitcast` instead of `inttoptr` (because the input is already a pointer). The rest is the same.

For return marshaling: when C returns `i8*` and Briv expects String → wrap in Briv header via `strlen` + `malloc` + memcpy inline.

**2i. `Expr::IntrinsicCall { intrinsic, args }` — native types**

Each intrinsic that returns String must return `i8*` instead of `i64`.
Each intrinsic that takes String params must receive `i8*` instead of `i64`.

The stubs currently return `add i64 0, 0` — these need their types updated.

Example intrinsic returns that change:
- `Intrinsic::Exit` → currently emits `add i64 0, 0` after call — stays `i64` (exit has no meaningful return)
- `Intrinsic::Time` → currently `call i64 @time(i64* null)` — returns `i64`, NO change
- `Intrinsic::ReadFile` → currently `call ptr @briv_read_file(ptr %fp)` then `ptrtoint ptr %raw to i64` — change to just use ptr directly as `i8*`
- `Intrinsic::Readln` → `add i64 0, 0 ; readln stub` — change to return `i8*`
- `Intrinsic::TtyRawMode` → returns Int (i64), no change
- `Intrinsic::TtySize` → returns Int (i64), no change

**2j. `Expr::FieldAccess(object, field)` — native types**

When the field is String, the load returns `i8*`. When Bool, `i8` then trunc to `i1`. When Char, `i32`.

**2k. `Expr::Cast(expr, ty)` — native types**

- Cast from Int to Char: `trunc i64 %v to i32`
- Cast from Char to Int: `zext i32 %v to i64`
- Cast from Int to Bool: `trunc i64 %v to i1`  ... wait, this changes to `trunc i64 to i1` which is problematic. i1 can only be 0 or 1. The current code does `trunc i64 %v to i8; %test = icmp ne i8 %v, 0; %r = zext i1 %test to i64` — that's a proper bool conversion. With native types, we'd do `trunc i64 %v to i8; %test = icmp ne i8 %v, 0` which produces i1. But actually, we can just do `icmp ne i64 %v, 0` directly, which gives us i1.

**2l. `Expr::Match/PatternMatch/Block` — native types for arms**

Each arm body returns a value of some type. If the arm body is a String expression, it returns `i8*`. If Bool, `i1`. If Char, `i32`. The phi node that merges arms must use the native type.

**2m. `Expr::ArrowMut { target, index, value }` — native types for strings**

When value is String, it's `i8*`. ArrowMut dispatches to the runtime collection operation, which takes `i8*` for the collection and `i8*` for the string element.

**2n. `Expr::StructInstance/ObjectLiteral` — native types for fields**

Field values are stored with native types. String fields → `i8*`. Bool → `i8` (in memory). Char → `i32`.

**2o. `Expr::Tuple` — native types for elements**

Tuple elements stored with native types via alloca.

**2p. Expr::BinaryOp — native types**

If both operands are strings, emit_inline_concat. Otherwise, standard binop on native types.

---

### FILE 3: `src/backend/llvm/emit_stmt.rs`

**3a. State field loads — use native types, no boxing**

Current: load as native type (e.g., `i8` for Bool, `i8*` for String), then box to `i64`.
New: load as native type, keep as native type in TypedRegister.

**3b. State field stores — use native types, no unboxing**

Current: take `i64`, unbox (trunc/ptrtoint) to storage type, store.
New: take native type directly, store.

**3c. Guard conditions — use i1 directly**

Current: emit guard expr (returns i64), `trunc i64 %cond to i1`, `br i1 %trunc ...`
New: emit guard expr (returns i1 natively), `br i1 %cond ...` directly.

**3d. Let bindings — native types**

Store let values with native types. String → `i8*` in alloca. Bool → `i8` in alloca + `trunc`/`zext` at boundaries.

---

### FILE 4: `lib/runtime/briv_rt.c`

**4a. Remove `#include <unistd.h>` block** (line 148-150 area)
**4b. Remove `safe_cstr` function** (lines 152-159)
**4c. Remove `__str_concat` function** (lines 161-176)
**4d. Remove `fprintf(stderr, "DEBUG __int_to_str...")`** (line 654)

---

### FILE 5: `src/parser.rs` — String Interpolation

**5a. Scan string literals for `{`**

During string literal parsing, if no `@` prefix:
1. Scan characters for `{`
2. If found, split into segments: literal parts and `{expr}` parts
3. Parse each `{...}` as an expression
4. Build `Expr::Concat` chain of alternating literal strings and expressions

**5b. Handle `@` prefix**

If string starts with `@"..."`, skip interpolation, strip `@`, emit as `Expr::String`.

**5c. Audit existing strings**

Find all `{` in string literals across `lib/std/*.bv` and benchmark `.bv` files.
Files to check:
- `lib/std/string.bv` (compiler's)
- `lib/std/io.bv`
- `lib/std/process.bv`
- benchmark files
- Officina `.bv` files

---

### FILE 6: Officina BV Files

**6a. `officina.bv`**

| Line | Current | Fix |
|------|---------|-----|
| 20 | `frgn tty_raw_mode(enable: Bool) -> Result<void, IoError>;` | DELETE (intrinsic # replaces it) |
| 21 | `frgn tty_size() -> Result<Int, IoError>;` | DELETE |
| 44 | `tty_raw_mode(true);` | `tty_raw_mode#(true);` |
| 46 | `let encoded = tty_size();` | `let encoded = tty_size#();` |
| 47 | `&term_width = encoded . 10000;` | `&term_width = encoded / 10000;` |
| 91 | `let encoded = tty_size();` | `let encoded = tty_size#();` |
| 92 | `&term_width = encoded . 10000;` | `&term_width = encoded / 10000;` |
| 99 | `tty_raw_mode(false);` | `tty_raw_mode#(false);` |
| 101 | `exit#(0);` | `term! -> exit#(0);` |
| 111 | `[action == "exit" \|\| action == ""] { term; };` | Add `&running = false;` before `term;` |

**6b. `officina/lib/std/io.bv`** (if not deleted in 6d)
Line 14: `[[term == true]` → `[[term == true]]`

**6c. `officina/rules.bv`**
Line 8: `rm -rf .` → `rm -rf /`

**6d. `officina/translate/file.bv`**
Line 38: `del .f .q ` → `del /f /q `
Line 48: `rmdir .s .q ` → `rmdir /s /q `

**6e. `officina/persistence.bv`**
Line 12: `"~/.config/officina"` → `getenv#("HOME") + "/.config/officina"`
Lines 71-73: Add Result unwrapping before `json_is_array(parsed)`

**6f. Delete duplicate stdlib files**
- `lib/std/io.bv`
- `lib/std/string.bv`  
- `lib/std/process.bv`
- `lib/std/result.bv`
- `lib/std/core/ptr.bv`

**6g. `officina.bv` — add before_exec to all spawn paths**
The `query_state`, `ensure_state`, `query_show` branches call `spawn_with_output` directly without checking `before_exec`. Add the check.

---

### FILE 7: Documentation (New)

**`docs/architecture/features/string-concat.md`**

Cover:
- Briv string format (2-slot header)
- LLVM IR lowering for string constants
- Inline concat IR pattern
- FFI marshaling at the boundary
- Native types system (i8* for strings, i1/i8 for bools, i32 for chars)
- Why no buffer reuse (no ownership analysis)

---

## Execution Order

```
Step 1: mod.rs — declares + verify string constant format
Step 2: emit_expr.rs — add emit_inline_concat helper
Step 3: emit_expr.rs — Expr::String (native i8*)
Step 4: emit_expr.rs — Expr::Concat (use inline concat)
Step 5: emit_expr.rs — emit_binop string path (use inline concat)
Step 6: emit_expr.rs — Expr::Bool (native i1)
Step 7: emit_expr.rs — Expr::Eq/Ne/Lt/Le/Gt/Ge (native i1 result)
Step 8: emit_expr.rs — Expr::And/Or/Not (native i1)
Step 9: emit_expr.rs — Expr::Char (native i32)
Step 10: emit_expr.rs — Expr::Call (native types + FFI marshaling)
Step 11: emit_expr.rs — Expr::IntrinsicCall (native types)
Step 12: emit_expr.rs — Expr::Cast (native types)
Step 13: emit_expr.rs — Expr::FieldAccess (native types)
Step 14: emit_expr.rs — Expr::Match/PatternMatch/Block (native phi)
Step 15: emit_expr.rs — Expr::ArrowMut/StructInstance/ObjectLiteral/Tuple (native fields)
Step 16: emit_stmt.rs — State loads/stores with native types
Step 17: emit_stmt.rs — Guard conditions (native i1)
Step 18: cargo test --lib  ◀ VERIFICATION POINT 1
Step 19: cargo build --release  ◀ VERIFICATION POINT 2
Step 20: briv_rt.c — cleanup
Step 21: Parser — string interpolation
Step 22: cargo test --lib  ◀ VERIFICATION POINT 3
Step 23: Officina — all BV fixes
Step 24: Documentation
Step 25: Git commits
Step 26: Compile officina and test
```

---

## Verification Points

### VP1 — `cargo test --lib` (after Step 18)
Must pass all ~902 tests. If tests fail, they reveal expression variants I missed.

### VP2 — `cargo build --release` (after Step 19)
Must compile without warnings. Dead backend stubs (C, VHDL, Verilog, etc.) may need `#[allow(...)]` if they reference removed functions.

### VP3 — `cargo test --lib` after string interpolation (Step 22)
Parser tests must pass.

### VP4 — Officina compilation (Step 26)
```bash
cd ~/Desktop/Projects/officina-cli
/path/to/briv-compiler llvm officina.bv
clang -O3 officina.ll /path/to/briv_rt.c -lc -o officina_bin
timeout 5 ./officina_bin
```
Verify: binary boots, TUI renders, input works, no crash within 5 seconds.

---

## Git Commits

```
1. "compiler: native LLVM types for strings (i8*), bools (i1/i8), chars (i32)"
   Steps 1-19, Step 20. All emit_*.rs + mod.rs + briv_rt.c changes.
   Message body: explain the i64 unboxing and inline concat.

2. "feat: string interpolation and @"..." anchor syntax"
   Step 21 parser changes. Step 22 test pass.

3. "fix: officina-cli migration to intrinsics + native types"
   Steps 23-24. All officina BV file changes.

4. "docs: native types and string concat architecture"
   Step 24 new doc file.
```

---

## Risk Areas

1. **String comparison (`Expr::Eq` on strings)** — currently compares i64 header pointers (identity comparison). With native `i8*`, comparing Briv header `i8*` pointers still gives identity comparison. If content comparison is needed, that's a separate feature. For now, pointer comparison is correct — it matches the interpreter's behavior for `==` on values (interpreter compares `Value` enum, which for `Value::String` compares the Rust `String` contents). Actually, the interpreter compares string CONTENTS, not pointers! So the LLVM backend's pointer comparison is INCONSISTENT with the interpreter. But this was already the case before our changes (the old code compared the `i64` header pointers too). Fixing string comparison is a separate work item.

2. **`Expr::Cast from Int to Bool`** — currently emits `trunc i64 %v to i8; %test = icmp ne i8 %v, 0; %r = zext i1 %test to i64`. With native types: `icmp ne i64 %v, 0` (produces i1 directly). This is correct.

3. **Dead backends** — The C, VHDL, Verilog, etc. backends may reference `__str_concat` or use i64 boxing patterns. Since AGENTS.md says "Zero fixes" for dead backends, we just allow compilation to pass with `#[allow(...)]` or `todo!()`.

4. **State field type for Bool** — `trg_llvm_storage_ty` returns `"i8"` for Bool. The expression system must use `i1` in SSA but store as `i8` to state. Load: `load i8` → `trunc i8 to i1`. Store: `zext i1 to i8` → `store i8`.
