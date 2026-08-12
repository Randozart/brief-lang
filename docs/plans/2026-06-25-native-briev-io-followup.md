# Native Briev I/O: Completion Plan (Phase 3–6 + Ext B–D)

**Date:** 2026-06-25
**Status:** Planned
**Previous:** `docs/plans/2026-06-25-native-briev-io.md` (Phases 1–2, Ext A done)

## What was implemented in commit 3c607d4

| Item | Status |
|------|--------|
| Phase 1: `Ptr<T>` type + ops + casts | ✅ Done |
| Phase 2: `volatile_load#`/`volatile_store#` | ✅ Done |
| Extension A: `asm target { }` in BILD | ✅ Done |
| Extension D (partial): call/asm/load/store/alloca/GEP → Opaque | ⚠️ Partial |

---

## Remaining work (ordered by dependency)

### Item 1: Extension D — complete symexec fallthrough (prerequisite)

**Files:** `src/analysis/bild_symexec.rs:299,311`

Change the two catch-all `_ => Err(SymExecError::UnsupportedOpcode(...))` arms to
return `Opaque` instead of erroring. This makes `inttoptr`, `ptrtoint`, `bitcast`,
`phi`, `br`, `switch`, `extractvalue`, `insertvalue`, and all other LLVM IR opcodes
tolerable in BILD symbolically. Contract verification falls through to the `fallback`
expression.

Lines to change:
```
// Line 299: in parse_bild_instruction
_ => Ok(SymExpr::Opaque(instr.to_string()))

// Line 311: outer catch-all
_ => Ok(SymExpr::Opaque(instr.to_string()))
```

**Tests:** Verify that a BILD body containing `inttoptr`, `phi`, `br` no longer
errors during symbolic execution.

**Docs:** None needed — this is internal.

---

### Item 2: Extension B — `fn(T) -> U` function pointer type + `&f` address-of

#### 2a. `Type::Fn` variant in AST

**File:** `src/ast.rs`

Add to the `Type` enum:
```
Fn(Vec<Type>, Box<Type>)  // Fn(param_types, return_type)
```
Update `Type::display()` for diagnostics. Update `Type::eq()` / `Type::hash()` /
any partial_eq/eq/hash derives. Update `type_universe.rs` to handle it.

The parser already creates `Type::Applied("Fn", ...)` for fn type syntax — route
this in `parse_type_inner` to produce `Type::Fn(params, ret)` instead of going
through `Applied`. This keeps the type system clean and avoids string-based
dispatch on `"Fn"`.

#### 2b. `Expr::AddressOf(Box<Expr>)` in AST

**File:** `src/ast.rs`

Add to the `Expr` enum:
```
AddressOf(Box<Expr>),
```

**Parser** (`src/parser.rs`): In `parse_unary`, when `&` is followed by an
identifier that is a known function/`defn`/`inop!` name, produce
`Expr::AddressOf(Box::new(Expr::Identifier(name)))` instead of
`Expr::OwnedRef(name)`. The disambiguation: `&identifier` when there is no
`<-` (arrow push) or assignment context produces `AddressOf`.

For now, `&` only works on `defn`/`inop!` names (static address-of).
Later: `&local_var` for stack pointers.

#### 2c. Typechecker support

**File:** `src/typechecker.rs`

- `Expr::AddressOf(inner)`: infer the inner expression's type, wrap in
  `Type::Fn(...)` for a `defn`/`inop!`.
- `Expr::Identifier` when it resolves to a function pointer variable:
  type is `Type::Fn(...)`.
- `Expr::Call` with a non-identifier callee (i.e. `Expr::AddressOf` or
  `Expr::Identifier` with `Type::Fn`): check parameter types, infer return type.

Currently `Expr::Call(name, args)` takes a `String` name. Either:
- Add a new `Expr::IndirectCall(Box<Expr>, Vec<Expr>)` variant, or
- Change `Expr::Call` to `Expr::Call { callee: Box<Expr>, args: Vec<Expr> }`
  with a compat path for the string overload.

Use the second approach — it's cleaner and less AST bloat.

#### 2d. Interpreter support

**File:** `src/interpreter.rs`

- `Expr::AddressOf(inner)`: evaluate inner to get the function name, return
  `Value::Int(addr_of_fn)` (for now, return `Value::Int(0)` — interpreter
  cannot produce real addresses but it tracks what was addressed).
- Indirect call: when `Expr::Call` has a non-string callee, evaluate the callee
  to get the `Value::Int(fn_ptr)` and dispatch via looking up the named function
  if the address corresponds to a known function. For unknown addresses, call
  the FFI registry.

#### 2e. LLVM backend

**File:** `src/backend/llvm/emit_expr.rs`

- `Expr::AddressOf(inner)`: emit `ptrtoint <fn_type> @fn_name to i64` to get
  the function pointer as an integer.
- Indirect call: emit `call i64 %fn_ptr(i64 %arg1, ...)` with appropriate
  `bitcast` to the expected function signature.

#### 2f. Tests

- Parser: `&my_fn` produces `Expr::AddressOf`
- Typechecker: `fn(Int) -> Int` type inference
- Interpreter: address-of then call (smoke test)
- LLVM: indirect call emission matches expected IR

#### 2g. Docs and examples

- `docs/architecture/features/fn-ptr.md` — function pointer type, `&f`, indirect calls
- `examples/function-pointers.bv` — address-of, indirect call, fn pointer type

---

### Item 3: Extension C — `#section("name")` on `inop!`

#### 3a. Section field on InopDeclaration

**File:** `src/ast.rs`

```
pub struct InopDeclaration {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub outputs: Vec<Type>,
    pub contract: Contract,
    pub llvm_body: Vec<String>,
    pub llvm_body_spans: Vec<Span>,
    pub fallback: Option<Expr>,
    pub has_side_effects: bool,
    pub has_state_access: bool,
    pub section: Option<String>,   // NEW
    pub span: Option<Span>,
}
```

#### 3b. Parser

**File:** `src/parser.rs`

Before parsing `inop!` / `inop`, check for a `#section("name")` directive.
If present, consume it and set the `section` field.

Lexer: either reuse `#!` parsing or add `HashSection` token. Simplest:
handle in the `parse_inop_decl` preamble by checking for `#section("...")`
as a keyword-consuming prefix.

Could also parse as a `#!` pragma: `#!section(".init_array") inop! ...`.
Reuse the existing `#!` pragma machinery.

#### 3c. LLVM backend

**File:** `src/backend/llvm/emit_toplevel.rs` (the `emit_inop` function)

When `inop.section.is_some()`, emit `section "..."` after the return type and
before `local_unnamed_addr`:

```
define i64 @name(...) section ".init_array" local_unnamed_addr #0 {
```

#### 3d. Tests

- Parser: `#section(".init_array") inop! ...` parses correctly
- LLVM: verify section attribute in emitted IR
- Example file: `examples/section-attr.bv`

#### 3e. Docs and examples

- Update `docs/architecture/features/bild.md` — add `#section` to BILD reference
- `examples/section-attr.bv` — inop! with section attribute

---

### Item 4: Phase 3 — BILD-inline syscalls

**Trivial — infrastructure is already in place** (`bild_asm.rs`, `asm target { }`).

#### 4a. Create `lib/std/syscall.bv`

```briev
inop! syscall6(nr: Int, a1: Int, a2: Int, a3: Int, a4: Int, a5: Int, a6: Int) -> Int
    [nr > 0][nr < 512]
{
    %res = asm target {
        [arch("x86_64")]:
            "syscall"
            : "={rax},{rax},{rdi},{rsi},{rdx},{r10},{r8},{r9}"
            : (i64 %nr, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5, i64 %a6);
        [arch("aarch64")]:
            "svc #0"
            : "={x0},{x8},{x0},{x1},{x2},{x3},{x4},{x5}"
            : (i64 %nr, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5, i64 %a6);
        default:
            "ud2"
            : "={rax},{rax},{rdi},{rsi},{rdx},{r10},{r8},{r9}"
            : (i64 %nr, i64 %a1, i64 %a2, i64 %a3, i64 %a4, i64 %a5, i64 %a6);
    };
    term %res;
} fallback -1;
```

Plus convenience wrappers for common syscalls (1–3 args): `syscall1` through `syscall3`.

#### 4b. Integration test

Compile a `.bv` file that imports `syscall.bv` and calls `SYS_write`.
Verify it produces runnable LLVM IR with `call asm` for syscall.

#### 4c. Docs

- `docs/architecture/features/bild.md` — already exists; add note about syscall pattern
- The `examples/bild-asm-target.bv` (from Item 7 below) covers this

---

### Item 5: Phase 4 — `#!cfg` conditional compilation

#### 5a. AST

Add `CfgGuard` struct:
```
pub struct CfgGuard {
    pub condition: CfgCondition,
    pub items: Vec<TopLevel>,
}

pub enum CfgCondition {
    Eq(String, String),           // target_os == "linux"
    Ne(String, String),           // target_arch != "x86_64"
    And(Box<CfgCondition>, Box<CfgCondition>),
    Or(Box<CfgCondition>, Box<CfgCondition>),
    Not(Box<CfgCondition>),
    Bool(bool),
}
```

Add to `TopLevel`:
```
TopLevel::Cfg(CfgGuard),
```

#### 5b. Lexer/parser

**File:** `src/lexer.rs`, `src/parser.rs`

Lex `#!cfg` as a pragma token. In the parser, treat `#!cfg(condition)` as
a block guard around subsequent definitions (or a single one).

Condition parser: `target_os == "linux"`, `target_arch == "x86_64"`,
`board == "stm32f407"`, with `&&`, `||`, `!`.

Evaluation at parse/import time: the pragma processing in `parse_toplevel`
evaluates the condition against the compiler's target configuration. If false,
skip the guarded items (produce no AST nodes).

#### 5c. Compiler CLI

**File:** `src/main.rs` (or the CLI entry point)

Add flags:
- `--board <name>` — sets `board` condition variable
- `--os <name>` — sets `target_os` (default: auto-detect from target triple)
- `--target <triple>` — already exists; extract `target_arch` from it

#### 5d. Tests

- Parser: `#!cfg(target_os == "linux") defn foo() -> Int { term 1 };` parses
- Evaluation: `#!cfg(target_os == "freestanding")` skips under Linux target
- Integration: conditional include works correctly

#### 5e. Docs and examples

- `docs/architecture/features/cfg.md` — `#!cfg` syntax and condition reference
- `examples/cfg-guards.bv` — conditional definitions for Linux vs freestanding

---

### Item 6: Phase 5 — DBS/DBL device address maps

#### 6a. Schema files

Create `lib/devices/uart.dbvs`, `lib/devices/gpio.dbvs`, etc. with entry schemas
defining register layouts.

Create `lib/boards/peripheral.dbvs` for validating board descriptions.

Create `lib/boards/stm32f407.dbvl`, `lib/boards/kv260.dbvl` with peripheral
instances and their base addresses.

#### 6b. `import "target"` resolver

**File:** `src/import_resolver.rs` (or new `src/analysis/target_import.rs`)

When the parser encounters `import "target"`:
1. Resolve board from `--board` flag or target spec
2. Load `lib/boards/{board}.dbvl`
3. For each `PeripheralEntry`, load the referenced `.dbvs` file
4. Parse register name/offset/type from schema strings
5. Populate a compile-time `HashMap<String, StructInstance>` namespace
6. Each peripheral becomes a typed struct constant with `Ptr<T>` fields
7. Contract bounds auto-derived from schema address ranges

#### 6c. Reuse `hardware_validator.rs` machinery

Convert DBL to the internal alias map at import time. The existing validator
already checks memory overlaps and validates schema imports.

#### 6d. Tests

- Load a `.dbvl` file and verify peripheral addresses
- `import "target"` produces correct `Ptr<T>` constants
- Memory overlap validation on loaded DBL

#### 6e. Docs and examples

- `docs/architecture/features/target-import.md` — DBL-based board import
- `examples/target-import.bv` — import "target" + MMIO access

---

### Item 7: Missing example files from Phase 1–2 + Ext A

Create these to satisfy the plan's documentation commitment:

| Example file | Content |
|---|---|
| `examples/volatile-io.bv` | MMIO-style read/write with contracts |
| `examples/bild-asm-target.bv` | Multi-arch asm dispatch in BILD inop! |

---

### Item 8: Phase 6 — Rewrite stdlib I/O using syscalls

#### 8a. Per-intrinsic migration

For each intrinsic below, create a syscall-based `inop!` version in
`lib/std/<arch>/<os>/` guarded by `#!cfg`, alongside the old C-calling version:

| Intrinsic | Syscall |
|---|---|
| `PrintInt` | `SYS_write(1, buf, len)` |
| `PutChar` | `SYS_write(1, &c, 1)` |
| `TtyReadKey` | `SYS_read(0, buf, 1)` |
| `Open`/`Read`/`Write`/`Close` | Respective `SYS_*` |
| `GetEnvInt` | `SYS_getenv` → parse |
| `Exit` | `SYS_exit` |
| `Sleep`/`NanoSleep` | `SYS_nanosleep` |

#### 8b. Guarding with `#!cfg`

```briev
#!cfg(target_os == "linux")
inop! print_int(n: Int) -> Bool {
    // syscall SYS_write(1, buf, 8) — BILD body with asm target { }
} fallback false;
```

The C-calling `frgn` version remains as a `#!cfg(target_os == "freestanding")`
fallback until the `volatile_store#(UART_TX, ...)` path is also implemented.

#### 8c. Remove C runtime functions

After each intrinsic's native version is stable, remove the corresponding
function from `briev_rt.c`. The C runtime shrinks from ~1744 lines toward ~200.

#### 8d. Tests

Each migrated intrinsic must pass existing tests (they rely on the interpreter,
which has its own implementation). The LLVM backend tests must verify the new
codegen path.

---

## ❌ CANCELLED — Item 2: Extension B (fn pointers + &f)

This item has been **replaced** by `docs/plans/2026-06-25-function-lens-properties.md`.

The `fn(T) -> U` type, `Expr::AddressOf`, and `&f` address-of operator are
cancelled. Instead, function metadata is accessed via the existing `:>` lens:

```briev
let addr: Int = add :> Address;   // replaces &f
```

Rationale:
- Zero parser changes (existing `:>` parsing works)
- Zero AST additions (new `ProjectionTarget` variants only)
- Zero `OwnedRef` match site changes (204 sites untouched)
- Idiomatic with Briev's lens philosophy
- Opens additional metadata (name, arity, docs, hash, etc.) for free

## Effort estimate

| Item | Est. time | Dependencies |
|------|-----------|-------------|
| 1. Extension D (symexec) | **15 min** | Done ✅ |
| 2. **CANCELLED** — see `function-lens-properties.md` | — | — |
| 3. Extension C (`#section`) | **1–2 hr** | Done ✅ |
| 4. Phase 3 (syscall.bv) | **30 min** | Done ✅ |
| 5. Phase 4 (`#!cfg`) | **4–6 hr** | Done ✅ |
| 6. Phase 5 (DBS/DBL) | **6–8 hr** | None |
| 7. Missing examples | **1 hr** | Done ✅ |
| 8. Phase 6 (stdlib rewrite) | **8–12 hr** | Items 3–7 |
| **NEW:** Function lens properties | **2–3 hr** | None — see `function-lens-properties.md` |

## Per-commit checklist

- `cargo test --lib` — all tests pass
- `cargo build` — no warnings
- Praetor on new/changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6)
- Update architecture docs if API contracts changed
- Update `learn-briev/` for any user-facing syntax change
- Create or update example `.bv` file for every new construct
- Kani harnesses for all safety-critical code
- `_ => return None;` fallthrough unchanged in all optimization passes
- No weakening of existing optimization paths
