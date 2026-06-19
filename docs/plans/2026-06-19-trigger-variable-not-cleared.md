# Plan: Trigger Variable Not Cleared + step() Type Fix

**Date:** 2026-06-19  
**Status:** Planned, awaiting execution request

## Problem

Two bugs in officina's keyboard input:

### Bug 1: Trigger variable never reset (user-facing)

`officina.bv` declares `trg keypress: Char @stdin#;`. When the user types a character, the stdin epoll handler stores it to the `keypress` state field. The `process_input` txn reads it and appends to `current_input`, but **nothing ever resets `keypress` back to `'\0'`**.

The guard `[booted && keypress != '\0']` stays true forever for that character value. If any code path re-evaluates the guard (e.g., a timer tick, spurious epoll_wait return, or convergence loop), the same character gets appended again — producing `hhhhhhhhhhhh...`.

### Bug 2: `step()` emits wrong load/store type (latent)

`emit_trg_step` in `loop_engine.rs` hardcodes `load volatile i64` / `store volatile i64` for ALL trigger fields regardless of their actual LLVM type in `%State`:
- `Char` → `i32`
- `Bool` → `i8`
- `String` → `i8*`

With opaque pointers this reads/writes 8 bytes from a 4/1/8-byte field, potentially clobbering adjacent struct padding. Currently benign because the same value is written back (volatile read-modify-write), but is semantically wrong.

## Fix

### Fix 1: `loop_engine.rs:1258-1350` — type-correct load/store in `emit_trg_step`

Three locations need to match on the actual field type instead of hardcoding `i64`:

| Location | Lines | Current code | Fix |
|----------|-------|--------------|-----|
| Trigger volatile loads | 1264-1272 | `load volatile i64, i64*` | Match `self.field_types[idx]`: i32, i8, i8*, ptr, i64 |
| Dependency field loads | 1318-1328 | `load i64, i64*` | Match `self.field_types[dep_idx]` |
| Proxy store | 1329-1339 | `store i64, i64*` | Match `self.field_types[idx]` for destination |

Example for trigger loads:
```rust
let ty_str = &self.field_types[idx];
match ty_str.as_str() {
    "i32" => {
        writeln!(out, "  {} = load volatile i32, i32* {}, align 4", ld, gep).ok();
        writeln!(out, "  store volatile i32 {}, i32* {}, align 4", ld, gep).ok();
    }
    "i8" => {
        writeln!(out, "  {} = load volatile i8, i8* {}, align 1", ld, gep).ok();
        writeln!(out, "  store volatile i8 {}, i8* {}, align 1", ld, gep).ok();
    }
    "i8*" | "ptr" => {
        writeln!(out, "  {} = load volatile i8*, i8** {}, align 8", ld, gep).ok();
        writeln!(out, "  store volatile i8* {}, i8** {}, align 8", ld, gep).ok();
    }
    _ => {
        writeln!(out, "  {} = load volatile i64, i64* {}, align 8", ld, gep).ok();
        writeln!(out, "  store volatile i64 {}, i64* {}, align 8", ld, gep).ok();
    }
}
```

### Fix 2: `officina.bv:78-80` — clear keypress after consumption

Add `&keypress = '\0';` unconditionally before `term;` in `process_input`:

```brief
    &needs_redraw = true;
    &keypress = '\0';
    term;
};
```

This makes the guard `[booted && keypress != '\0']` converge to false after one firing, regardless of which guard branch executed.

## Verification

```bash
cargo test --lib                              # compiler tests pass
./target/release/brief-compiler build ~/Projects/officina-cli/officina.bv
printf "hello\x03" | timeout 2 ./officina     # "hello" appears once, not "hhhhhhhhheeee..."
```
