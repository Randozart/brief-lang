# Issues Log

**Project:** briv-compiler
**Purpose:** Approaches that failed and required different solutions

---

## Issue Template

```markdown
### YYYY-MM-DD HH:MM - Issue Title

**Problem:** What was being attempted
**Approach Tried:** What was tried that failed
**Why It Failed:** Root cause
**Solution:** How the problem was ultimately solved
```

---

## 2026-04-27

### 2026-04-27 20:25 - C backend called run_arm instead of run_c

**Problem:** CLI `c` command was executing ARM backend instead of C backend
**Approach Tried:** The match arm `"c" | "cc"` was dispatching to `run_arm()`
**Why It Failed:** Copy-paste error - the ARM handler code was duplicated for C without updating the function call
**Solution:** Added proper `run_c()` function call in the `"c" | "cc"` match arm

---

### 2026-04-27 20:15 - Expression translation bug in Rust backend

**Problem:** `self.count = (count + 1);` - missing `self.` prefix on `count`
**Approach Tried:** Simple identifier replacement without state field prefix
**Why It Failed:** `Expr::Identifier` returned just the name without `self.` prefix, but identifiers in Rust state methods should be `self.field`
**Solution:** Changed `Expr::Identifier(n) => n.clone()` to `Expr::Identifier(n) => format!("self.{}", n)` in `rust.rs`

---

### 2026-04-27 19:50 - CLI command conflict with `c` alias

**Problem:** Wanted to add `c` command for C backend but `c` was already aliased to `check`
**Approach Tried:** Different command names (cc, c-code, native-c)
**Why It Failed:** Breaks convention - most compilers use `c` for C compilation (gcc, clang)
**Solution:** Changed `check` alias from `c` to `ck`, freeing `c` for C backend

---

### 2026-04-27 19:30 - Inline assembly expression translation

**Problem:** C backend expression `state->count = 0 /* expr not implemented */;`
**Approach Tried:** Default fallthrough to `"0 /* expr not implemented */"` for binary expressions
**Why It Failed:** `Expr::Add`, `Expr::Sub`, etc. were not handled in `expr_to_c()`
**Solution:** Added binary operator cases to `expr_to_c()` - Add, Sub, Mul, Div, Eq, Ne, Lt, Le, Gt, Ge, And, Or, Not, Neg

---

### 2026-04-27 19:20 - Rust asm template vs real asm

**Problem:** Wanted to generate real `asm!` blocks but browser/WASM doesn't support inline asm
**Approach Tried:** Generate `asm!` directly in Rust backend
**Why It Failed:** `asm!` requires nightly Rust, and WASM target can't use it anyway
**Solution:** Rust backend generates commented template with `#![feature(asm)]` note; WASM backend generates comment-only output

---

## Earlier Issues

### YYYY-MM-DD HH:MM - Issue Title

**Problem:**
**Approach Tried:**
**Why It Failed:**
**Solution:**
