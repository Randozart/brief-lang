## Top-Level __init: Scripting with Atomic Boot Safety

**What**: Allowed executable statements directly at global scope, which the
compiler automatically wraps in a synthesized `rct txn __init` transaction
at compile time.

**Why it matters**: Eliminates boilerplate for simple scripts while retaining
Brief's transactional safety guarantees. `println#("hello")` at top level is
valid Brief. If the startup fails (an FFI error triggers `escape`), the entire
boot transaction atomically rolls back — zero partial state, no half-configured
program.

**How**: `TopLevel::Statement(Box<Statement>)` is added to the AST. The parser
enforces that all declarations (let, const, struct, txn, defn) must precede
executable statements — no interleaving. `Program::synthesize_init_txn()`
collects all `TopLevel::Statement` items, creates a collision-avoiding
`__booted_N` state flag, and synthesizes:
```
let __booted_N: Bool = false;
rct txn __init [!__booted_N][__booted_N] {
    // all top-level statements in order
    &__booted_N = true;
    term;
};
```
The synthesized transaction fires once on program start. Escape inside the
boot sequence triggers a clean abort with rolled-back state.
