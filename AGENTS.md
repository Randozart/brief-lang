# Brief Compiler - Agent Guidelines

See CLAUDE.md for complete documentation. This file ensures OpenCode picks up the same guidelines.

## Quick Reference

### Commands
- **Build**: `cargo build`
- **Test**: `cargo test --lib`
- **Test backend registry**: `cargo test --lib -- backend::tests`
- **Compile RBV**: `./target/release/brief-compiler rbv <file.rbv>`
- **Selfhost**: `cargo run --bin brief-compiler -- selfhost <file.bv>`

### File Types
- **.bv** - Brief (standard Brief file)
- **.rbv** - Rendered Brief (Brief + View, compiles to web frontend. Designed for web specifically. Like `.tsx` is to `.ts`)
- **.ebv** - Embedded Brief (Brief with less OS based abstractions, and more oriented towards bare metal and embedded programming)
- **.dbv/.dbvs/.dbvl** - Data Brief (configuration with schema and lines, think `.xml` compared to `.xmls` and `.json` compared to `.jsonl`)

### Critical Philosophy

**CONTRACT-FIRST**: Contracts are the source of truth. Never weaken contracts to match lazy code.

**NO MAGIC**: Never add hardcoded Rust string matches as "built-in" functions.
  - If a `.bv` file needs `is_digit`, the fix is `import char from "std/char.bv"` — NOT a Rust match arm.
  - If a `.bv` file needs `None`, the fix is `import option from "std/option.bv"` — NOT pre-populating state.
  - The FFI system (`frgn from "..."`) is the transparent path. Use it.
  - The standard library exists as a dependency source. Import from it.

**SELF-DOCUMENTING FAILURE**: Before fixing any issue:
  1. Understand WHY the fix works (not just THAT it works)
  2. Document the root cause in BUGS.md
  3. Ensure the fix doesn't violate Contract-First or No Magic

### Anti-Patterns (NEVER DO)
- Changing `[product > 0]` to `[true]` because code doesn't set product
- Using generic contracts like `[true]` that pass everything
- Adding postconditions that don't guarantee specific outcomes
- Adding Rust string-match built-ins when the standard library or import system should be used
- Pre-populating interpreter state with enum constants (None, Some, Ok, Err) — let stdlib handle it

### Correct Approach
- Keep contract `[product > 0]` 
- Fix code: make buttons call product-specific transactions like `add_laptop`, `add_keyboard`
- If interpreter raises `UndefinedForeignFunction("is_digit")`, add `import char from "std/char.bv"` to the calling .bv file
- If import resolver can't find a standard library file, fix the search path, not the interpreter

## For OpenCode

This project uses OpenCode. When making changes:
1. Read CLAUDE.md for full context
2. Follow Contract-First Philosophy
3. Never weaken contracts - fix code instead
4. Test with `cargo test --lib` before committing
5. Document bugs and root causes in BUGS.md
6. Never add Rust built-ins for things the standard library should provide

## Self-Hosting Pipeline

The Brief-in-Brief compiler lives in `lib/compiler/`. The Rust interpreter runs it via:
```
brief-compiler selfhost <file.bv>
```

**Known gaps in interpreter** (add these legitimately, not as magic):
- `Expr::Block`, `Expr::Tuple`, `Expr::TupleDestructure` — properly implemented in eval_expr
- `Expr::ForAll`, `Expr::Exists`, `Expr::MultiSlice` — properly implemented in eval_expr
- `Statement::Unification` — properly implemented (looks up state, matches variant, executes block)

**Do NOT add as built-ins**: `is_digit`, `is_alpha`, `is_alphanumeric`, `is_upper`, `is_lower`, `is_space`, `char_to_string`, `None`, `Some`, `Ok`, `Err`. These are in `lib/std/` and should be imported.