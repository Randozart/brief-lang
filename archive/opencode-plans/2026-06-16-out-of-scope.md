# Out of Scope Decisions — 2026-06-16 String/FFI Session

Items discussed but explicitly deferred for separate work.

| Question | Decision | Why |
|----------|----------|-----|
| Buffer reuse for concat? | Not in scope | No ownership analysis exists in the compiler. `TypeError::OwnershipViolation` is a dead placeholder in `src/errors.rs:316` — never emitted. Building ownership/lifetime tracking is a separate analysis pass. |
| String content comparison? | Not in scope | LLVM already compared `i64` header pointer values. With `i8*`, it compares header pointer values. Both are identity comparisons — inconsistent with the interpreter's content comparison (`Value::String` compares Rust `String` contents). Fixing this requires runtime string comparison calls (or inline character-by-character comparison). |
| State field Bool storage? | i1 in SSA, i8 in memory | `trg_llvm_storage_ty` already returns `"i8"` for Bool. No change needed to storage — just eliminate the expression system's i64 boxing. Load: `load i8` → `trunc i8 to i1`. Store: `zext i1 to i8` → `store i8`. |
| Dead backends? | `#[allow(...)]` / `todo!()` | Per AGENTS.md — zero fixes for dead backends (C, VHDL, Verilog, COBOL, x86_64, AArch64, WASM, TCL). If a shared API change breaks them mechanically, use `_ => {}` or `todo!()` — do not implement the feature. |
| String interpolation `@{var}` or other prefix syntax? | `@` prefix on string literal means "no interpolation" | Per user direction. `@"hello {name}"` is literal. `"hello {name}"` interpolates. |
| officina generated artifacts (C backend)? | Delete from repo | The C backend is dead. Generated `officina.c`, `.o`, `.ll`, `.bc` files are build artifacts from a known-broken code path. Remove them. |
