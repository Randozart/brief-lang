# Plan: Rename `rct txn` → `node` + Benchmark Fixes

This plan covers two stages:

1. **Stage 1** — Rename `rct txn` to `node` throughout the entire
   codebase, update the lexer/parser/AST/backend to accept `node` as a
   keyword, and ensure `node`, `txn`, and `defn` are correctly
   distinguished with the right semantics and optimization paths.

2. **Stage 2** — Fix all benchmarks identified during investigation to
   compile and run correctly, at parity with C.

---

## Stage 1: Rename `rct txn` → `node`

### 1.1 Overview

The keyword `rct` is removed. The construct formerly written `rct txn`
is now written `node`. The AST field `is_reactive: bool` becomes
`is_node: bool` (or may be kept as `is_reactive` if preferred — see
§1.6).

**Grammar change:**

```
// Before:
transaction ::= ("rct")? "txn" identifier ... contract body
construct  ::= "rct" ... (reactive, no args/return)
             | "txn" ... (callable, args/return)
             | "defn" ... (pure function)

// After:
construct  ::= "node" identifier ... contract body    // reactive, no params/return
             | "txn" identifier ... contract body      // callable, params/return
             | "defn" identifier ... body              // pure function
```

A `node` has no parameters and no return value. A `txn` may have
parameters and a return value. A `defn` is pure (no state access).

### 1.2 Lexer Changes

**File: `src/lexer.rs`**

1. Rename token variant: `Rct` → `Node` (line 57-58,
   `#[token("rct")]` → `#[token("node")]`).
2. Update display impl: `Token::Rct => write!(f, "rct")` →
   `Token::Node => write!(f, "node")` (line 532).
3. The `async` modifier needs handling. Currently `rct async txn`
   parses `rct` keyword then checks for `async`. After rename:
   `node async NAME [pre][post] { ... }`. The lexer already has
   `#[token("async")]  Async,` — no lexer change needed for async.
   Only the parser's order changes (see §1.3).

**Fuzzing keyword list** (`src/fuzzing/ast_generator.rs:384`):
   Replace `"rct"` with `"node"`.

### 1.3 Parser Changes

**File: `src/parser/definitions.rs`**

1. Rename `parse_reactive_transaction()` → `parse_node()` (line 279).
2. Update dispatch: `Some(Token::Rct)` → `Some(Token::Node)` (line 27).
   This triggers `parse_node()` which parses `node NAME [pre][post] { body }`.
3. Update error message (line 293):
   ```
   // Before:
   "expected 'txn' after 'rct', found '{}'"
   // After:
   "expected node name after 'node', found '{}'"
   ```
4. In `parse_node()`: consume `Token::Node`, then check for optional
   `async` modifier, then expect the identifier name (no `"txn"` keyword
   is expected after `node` — the node keyword replaces both `rct` and
   `txn`). Set `is_node: true` on the Transaction.
5. The `"txn"` keyword path (line 333+) remains unchanged — it parses
   callable transactions with `is_node: false`.

**Parse flow after rename:**

```
// Parsing "node NAME [pre][post] { body }"
Token::Node => {
    self.pos += 1; // consume 'node'
    let is_async = if self.check_identifier("async") {
        self.pos += 1;
        true
    } else { false };
    // expect identifier name
    let name = self.parse_identifier()?;
    // parse contracts, body
    let txn = Transaction { is_node: true, is_async, name, ... };
    TopLevel::Transaction(txn)
}

// Parsing "txn NAME(args) [pre][post] -> Ret { body }" — unchanged
```

**File: `src/parser/helpers.rs`**

Update token display: `Token::Rct => "rct".into()` →
`Token::Node => "node".into()` (line 230).

**File: `src/parser/definitions.rs`** — also update the identifier
detection (line 333):
```
// Before: if self.check_identifier("txn") || self.check_identifier("rct") {
// After:  if self.check_identifier("txn") || self.check_identifier("node") {
```

### 1.4 AST Changes

**File: `src/ast/top.rs`**

Rename field: `pub is_reactive: bool` → `pub is_node: bool` (line 94).

**File: `src/ast/display.rs`**

Update output text (line 355):
```rust
// Before:
let prefix = if txn.is_reactive { "rct txn" } else { "txn" };

// After:
let prefix = if txn.is_node { "node" } else { "txn" };
```

### 1.5 Internal Field Name: `is_reactive` → `is_node`

The boolean field on `Transaction` appears in ~60 locations. Every
occurrence must be updated:

| File | Current | New |
|------|---------|-----|
| `src/ast/top.rs:94` | `is_reactive: bool` | `is_node: bool` |
| `src/ast/display.rs:355` | `txn.is_reactive` | `txn.is_node` |
| `src/annotator.rs:311` | `if txn.is_reactive` | `if txn.is_node` |
| `src/analysis/transition_graph.rs:27` | `is_reactive: bool` | `is_node: bool` |
| `src/analysis/transition_graph.rs:155` | `is_reactive: txn.is_reactive` | `is_node: txn.is_node` |
| `src/analysis/provenance.rs:204` | `is_reactive: bool` | `is_node: bool` |
| `src/reactor.rs:39` | `txn.is_reactive` | `txn.is_node` |
| `src/reactor.rs:427` | `fn make_rct_txn` | `fn make_node` |
| `src/backend/llvm/mod.rs:1682` | `!t.is_reactive` | `!t.is_node` |
| `src/backend/llvm/mod.rs:2247` | `!txn.is_reactive` | `!txn.is_node` |
| `src/backend/llvm/mod.rs:2368` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/mod.rs:2375` | `!t.is_reactive` | `!t.is_node` |
| `src/backend/llvm/mod.rs:2442` | `graph.nodes[0].is_reactive` | `graph.nodes[0].is_node` |
| `src/backend/llvm/mod.rs:3180` | `node.is_reactive` | `node.is_node` |
| `src/backend/llvm/emit_toplevel.rs:1270` | `txn.is_reactive` | `txn.is_node` |
| `src/backend/llvm/helpers.rs:494` | `!ta.is_reactive \|\| !tb.is_reactive` | `!ta.is_node \|\| !tb.is_node` |
| `src/backend/llvm/hazard.rs:264` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/hazard.rs:359` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/dispatch.rs:57` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/dispatch.rs:300` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/optimizer.rs:85` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/optimizer.rs:192` | `!txn.is_reactive` | `!txn.is_node` |
| `src/backend/llvm/optimizer.rs:247` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/optimizer.rs:254` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/loop_engine/ssa.rs:42` | `!txn.is_reactive` | `!txn.is_node` |
| `src/backend/llvm/loop_engine/ssa.rs:327` | `t.is_reactive` | `t.is_node` |
| `src/backend/llvm/loop_engine/ssa.rs:364` | `txn.is_reactive` | `txn.is_node` |
| `src/backend/webstack.rs:336` | `txn.is_reactive` | `txn.is_node` |
| `src/plugin/intrinsics.rs:260` | `txn.is_reactive` | `txn.is_node` |
| `src/fuzz_checker/mod.rs:59` | `txn.is_reactive` | `txn.is_node` |
| `src/fuzzing/ast_generator.rs:77-80` | `is_reactive` | `is_node` |
| `src/beast/serialize.rs:65` | `t.is_reactive` | `t.is_node` |
| `src/beast/deserialize.rs:222-240` | `is_reactive` | `is_node` |
| `src/lsp.rs:808` | `t.is_reactive` | `t.is_node` |

**Rust test files** — all `is_reactive: true` → `is_node: true`,
all `is_reactive: false` → `is_node: false`:

| File | Count |
|------|-------|
| `src/reactor.rs` | ~15 test sites |
| `src/analysis/transition_graph.rs` | ~4 test sites |
| `src/analysis/dependency_graph.rs` | ~1 |
| `src/analysis/region.rs` | ~1 |
| `src/analysis/range.rs` | ~1 |
| `src/analysis/call_graph.rs` | ~1 |
| `src/analysis/watchdog.rs` | ~1 |
| `src/backend/llvm/tests.rs` | ~33 test sites |
| `src/backend/webstack.rs` | ~2 |
| `src/backend/circt.rs` | ~1 |
| `src/plugin/intrinsics.rs` | ~3 |
| `src/fuzzing/concolic.rs` | ~1 |
| `src/hardware_validator.rs` | ~1 |

**Recommended command for mechanical rename:**
```bash
# Rename struct field and all references in Rust source
find src -name '*.rs' -exec sed -i \
  -e 's/is_reactive/is_node/g' \
  -e 's/make_rct_txn/make_node/g' \
  {} +
```

### 1.6 BEAST Serialization

**File: `src/beast/serialize.rs`** (line 65):
```
// Before: if t.is_reactive { children.push(atom(":reactive")); }
// After:  if t.is_node { children.push(atom(":node")); }
```

**File: `src/beast/deserialize.rs`** (lines 222-240):
```
// Before:
":reactive" => { is_reactive = true; i += 1; }
// After:
":node" => { is_node = true; i += 1; }
```

**Backward compatibility:** Accept both `:reactive` and `:node` during
a transition period, emit only `:node`. Add a fallthrough:
```rust
":reactive" | ":node" => { is_node = true; i += 1; }
```

### 1.7 Annotator

**File: `src/annotator.rs:311`**:
```
// Before:
let rct = if txn.is_reactive { "rct " } else { "" };

// After:
let kind = if txn.is_node { "node " } else { "txn " };
```

### 1.8 LSP

**File: `src/lsp.rs`**

1. Keyword list (line 565): Replace `"rct"` with `"node"`.
2. Hover display (line 808):
   ```
   // Before:
   if t.is_reactive { " rct" } else { "" }
   // After:
   if t.is_node { " node" } else { " txn" }
   ```

### 1.9 Plugin

**File: `src/plugin/intrinsics.rs`**

Update all comments (lines 231, 256) referencing `rct txn` → `node`.
The `CheckReactive` plugin name in comments → `CheckNode`.

### 1.10 Rust Comments

Every Rust doc comment or inline comment referencing `rct txn`:

| File | Line(s) | Change |
|------|---------|--------|
| `src/parser/definitions.rs` | 279-293 | Doc comments + error messages |
| `src/backend/llvm/mod.rs` | 458 | Doc comment |
| `src/backend/llvm/optimizer.rs` | 289 | Comment: `rct async txn` → `node async` |
| `src/backend/llvm/loop_engine/ssa.rs` | 9-10 | Doc comments on SSA paths |
| `src/plugin/intrinsics.rs` | 231, 256 | Doc comments |

Command:
```bash
find src -name '*.rs' -exec sed -i \
  -e 's/rct txn/node/g' \
  -e 's/rct async txn/node async/g' \
  -e 's/rct/node/g' \
  {} +
```

**CAUTION:** `sed -i 's/rct/node/g'` may match `rct` inside words or
identifiers. Review with `git diff` afterward. Better to use more
specific patterns:
```bash
sed -i 's/\brct txn\b/node/g'
sed -i 's/\brct async txn\b/node async/g'
```

### 1.11 Stdlib `.bv` Files

Replace `rct txn` → `node` in:

| File | Lines |
|------|-------|
| `lib/std/system.bv` | 13 (comment), 26, 34, 42 |
| `lib/std/console.bv` | 10 (comment), 21 (comment), 42 |
| `lib/std/ffi/system.bv` | 13 (comment), 26, 34, 42 |

Simple `sed -i 's/rct txn/node/g'` on each file.

### 1.12 Self-hosting Compiler

**File: `lib/compiler/token.bv:176`:**
```
// Before:
uni tok(KeywordRct) = "rct";
// After:
uni tok(KeywordNode) = "node";
```

**File: `lib/compiler/lexer.bv:148`:**
```
// Before:
[text == "rct"] { term (KeywordRct, state); };
// After:
[text == "node"] { term (KeywordNode, state); };
```

**Verification:** The self-hosting compiler must produce the same
output as the Rust compiler for the same input. Test:
```bash
# Compile a test file with Rust compiler
./target/release/brief-compiler build tests/fixtures/counter.bv -o /tmp/rust_out

# Compile the self-hosting compiler with itself (once self-hosting works)
```

### 1.13 Example `.bv` Files

Replace `rct txn` → `node` in all 27 example files:

```bash
find examples -name '*.bv' -exec sed -i 's/rct txn/node/g' {} +
find examples -name '*.bv' -exec sed -i 's/rct async txn/node async/g' {} +
```

**Note:** `examples/complex_workflow.bv` has 5+ node declarations.
`examples/volatile-io.bv` has a node at line 92. These are all
straightforward replacements.

### 1.14 Test Fixture `.bv` Files

Replace `rct txn` → `node` in all 22+ test fixture files:

```bash
find tests -name '*.bv' -exec sed -i 's/rct txn/node/g' {} +
find tests -name '*.bv' -exec sed -i 's/rct async txn/node async/g' {} +
```

**Note:** `tests/fixtures/event_model.bv` has multiple nodes (lines
14, 20, 26). `tests/fixtures/multifield.bv` has nodes at lines 5, 12.
All are straightforward.

### 1.15 Benchmark `.bv` Files

Replace `rct txn` → `node` in all 31+ benchmark files:

```bash
find benchmarks -name '*.bv' -exec sed -i 's/rct txn/node/g' {} +
find benchmarks -name '*.bv' -exec sed -i 's/rct async txn/node async/g' {} +
```

**Special case: async modifier**
```
// Before:
rct async txn fetch [pre][post] { ... }

// After:
node async fetch [pre][post] { ... }
```

### 1.16 Documentation Files

**Strategy:** Use `sed` for bulk replacement, then manual review for
edge cases.

```bash
# Architecture docs
find docs -name '*.md' -exec sed -i \
  -e 's/rct txn/node/g' \
  -e 's/rct async txn/node async/g' \
  {} +

# Spec docs
find spec -name '*.md' -exec sed -i \
  -e 's/rct txn/node/g' \
  -e 's/rct async txn/node async/g' \
  {} +

# Learn-brief docs
find learn-brief -name '*.md' -exec sed -i \
  -e 's/rct txn/node/g' \
  -e 's/rct async txn/node async/g' \
  {} +

# Root docs
sed -i 's/rct txn/node/g' AGENTS.md AGENTS_HISTORY.md AGENTS_HISTORY_2.md BUGS.md README.md
sed -i 's/rct async txn/node async/g' AGENTS.md AGENTS_HISTORY.md AGENTS_HISTORY_2.md BUGS.md README.md
```

**Edge cases to review manually after bulk replace:**

| Pattern | Where it appears | Manual check |
|---------|-----------------|--------------|
| `"rct"` (quoted keyword) | `spec/SPEC.md` grammar rules | Change to `"node"` |
| `Rct` (PascalCase in tables) | spec tables, milestone docs | Change to `Node` |
| `rct` (bare, meaning the construct) | philosophy docs, agent history | Change to `node` |
| `rct` as part of a larger identifier | discussion text, bug reports | Review individually |
| `circt` (contains "rct" as substring) | backend references in plans | **DO NOT TOUCH** — `circt` is a separate backend name |

**For `circt`:**
The substring `circt` contains `rct` but is NOT related to the
`rct txn` keyword. **Do NOT use `sed -i 's/rct/node/g'`** without
word boundaries. Use `sed -i 's/\brct\b/node/g'` or better, use
separate targeted patterns for `rct txn`, `rct async txn`, and `rct`
as a standalone word.

The `docs/architecture/txn-semantics.md` file is already updated — it
uses `node` throughout.

### 1.17 Validation After Stage 1

```bash
# 1. Build
cargo build

# 2. Run tests
cargo test --lib

# 3. Check for remaining 'rct' references (excluding circt, archived)
git grep -n -i '\brct\b' -- '*.rs' '*.bv' '*.md' \
  | grep -v 'circt' \
  | grep -v 'archived' \
  | grep -v 'old_docs'

# 4. Compile a simple file to verify parsing
./target/release/brief-compiler build examples/simple_contract.bv -o /tmp/test_node

# 5. Run the compiled binary
/tmp/test_node

# 6. Compile all benchmarks
for b in benchmarks/*.bv; do
  BOUND=50000000 ./target/release/brief-compiler build "$b" --out benchmarks 2>&1 \
    | grep -v "warning:" || echo "FAIL: $b"
done
```

Planned find-and-replace commands:

```bash
# ── Rust source ──
find src -name '*.rs' -exec sed -i \
  -e 's/\bis_reactive\b/is_node/g' \
  -e 's/\bmake_rct_txn\b/make_node/g' \
  {} +

# ── .bv files ──
find lib benchmarks tests examples -name '*.bv' -exec sed -i \
  -e 's/rct txn/node/g' \
  -e 's/rct async txn/node async/g' \
  {} +
find lib/compiler -name '*.bv' -exec sed -i \
  -e 's/KeywordRct/KeywordNode/g' \
  -e 's/"rct"/"node"/g' \
  {} +

# ── Markdown files ──
find docs spec learn-brief -name '*.md' -exec sed -i \
  -e 's/\brct txn\b/node/g' \
  -e 's/\brct async txn\b/node async/g' \
  -e 's/\brct\b/node/g' \
  {} +
sed -i \
  -e 's/\brct txn\b/node/g' \
  -e 's/\brct async txn\b/node async/g' \
  AGENTS.md AGENTS_HISTORY.md AGENTS_HISTORY_2.md BUGS.md README.md

# ── Lexer token ──
sed -i 's/#\[token("rct")\]/#[token("node")]/' src/lexer.rs
```

---

## Stage 2: Benchmark Fixes

### 2.1 fasta — 105× regression

**Root cause:** `__print_char()` in `lib/runtime/brief_rt.c:163` calls
`fflush(stdout)` after every non-newline character. 50M fflush syscalls
vs C's buffered `fputc`.

**Fix:** Remove `fflush(stdout)` from `__print_char`.

**File: `lib/runtime/brief_rt.c`** (lines 163-171):

```c
// BEFORE:
int64_t __print_char(int64_t c) {
    if (c == 10) {
        puts("");
    } else {
        putchar((int)c);
        fflush(stdout);    // ← DELETE THIS LINE
    }
    return 0;
}

// AFTER:
int64_t __print_char(int64_t c) {
    if (c == 10) {
        puts("");
    } else {
        putchar((int)c);
    }
    return 0;
}
```

**Verification:**
```bash
cargo build --release
BOUND=50000000 ./target/release/brief-compiler build benchmarks/fasta.bv --out benchmarks
hyperfine -w 2 'BOUND=50000000 benchmarks/fasta > /dev/null' \
               'BOUND=50000000 benchmarks/fasta_c > /dev/null'
```

Expected ratio: ~1.0× (both buffered). Previous: 105×.

---

### 2.2 queue_drain — build failure

**Root cause (two-part chain):**

1. `collect_state_identifiers` in `src/analysis/transition_graph.rs`
   has no match arm for `Expr::AddrOf` or `Expr::Deref`. The arrow ops
   `<- &queue` and `&queue <- count` parse their operand as `AddrOf`,
   so `queue` is invisible to field-reference analysis.

2. `apply_field_modes` in `src/backend/llvm/mod.rs` eliminates `queue`
   as "unreferenced," reducing `%State` from 7 to 6 fields. But
   `arena_ptr_idx`, `arena_end_idx`, `arena_base_idx` hold stale
   indices (4, 5, 6), producing GEP `i32 6` on a 6-field struct.

**Fix A: Add `Expr::AddrOf`/`Expr::Deref` handling.**

**File: `src/analysis/transition_graph.rs`** (after line ~1044):

```rust
// 2026-07-21: Handle &state_field and *state_field so arrow ops
// (e.g. <- &queue, &queue <- count) don't hide state references
// from field-liveness analysis.
Expr::AddrOf(inner) | Expr::Deref(inner) => {
    collect_state_identifiers(inner, state_fields, out);
}
```

**Fix B: Update stale arena indices after field rebuild.**

**File: `src/backend/llvm/mod.rs`** (~line 3690):

After the rebuild loop in `apply_field_modes` that reconstructs
`field_index_map`, add:

```rust
// 2026-07-21: Update stale arena/ringbuf indices after field
// elimination may have shifted them. The rebuild loop above
// reconstructs field_index_map with eliminated fields removed,
// so we must re-read indices from the new map.
self.arena_ptr_idx = self.ctx.field_index_map.get("__arena_ptr").copied();
self.arena_end_idx = self.ctx.field_index_map.get("__arena_end").copied();
self.arena_base_idx = self.ctx.field_index_map.get("__arena_base").copied();
self.ringbuf_data_idx = self.ctx.field_index_map.get("__ringbuf_data").copied();
self.ringbuf_head_idx = self.ctx.field_index_map.get("__ringbuf_head").copied();
self.ringbuf_tail_idx = self.ctx.field_index_map.get("__ringbuf_tail").copied();
self.ringbuf_mask_idx = self.ctx.field_index_map.get("__ringbuf_mask").copied();
```

**Note on `Option<usize>` vs `usize`:** If `arena_ptr_idx` and friends
are `Option<usize>`, the `.copied()` from `.get()` works directly. If
they are `usize`, use `.unwrap_or(0)`. Check the field types in the
backend struct before writing.

**Verification:**
```bash
cargo test --lib
BOUND=50000000 ./target/release/brief-compiler build benchmarks/queue_drain.bv --out benchmarks
BOUND=50000000 ./target/release/brief-compiler build benchmarks/queue_drain_idio.bv --out benchmarks
BOUND=50000000 bash -c 'diff <(benchmarks/queue_drain) <(benchmarks/queue_drain_c)'
```

---

### 2.3 float_math_nonzero — 2.21× regression

**Root cause:**

Two distinct issues:

**A (p22 phi bug):** The `p22` state variable is loaded as a constant
`0.0` and never loop-carried through the SSA phi in the main
convergence loop. The `p22 += Q22` computation produces a value used
only in the print guard — it never feeds back into the next tick.
This is a correctness bug (p22 accumulation is lost) and also prevents
proper optimization.

**B (sequential dependency chain):** The txn body assigns `x0`, `x1`,
`x2` sequentially. Since all RHS read pre-tick state (atomicity rule),
these three assignments are independent — no read-after-write chain.
But the current codegen emits them as sequential load→compute→store
sequences, creating artificial data dependencies. LLVM's scheduler
cannot break them because the store to state looks like a memory
barrier.

**Fix A: Add p22 to loop-carried phis.**

**File: `src/backend/llvm/mod.rs`** — convergence loop emission.

Find where the loop-carry phi nodes are built for the main convergence
loop (the function that emits the `%cm_header` → `%cm_body` →
`%cm_latch` pattern). The phi nodes are built from the set of state
fields. Ensure that ALL mutable state fields (including p00, p11, p22)
are included in the phi set, not just the ones referenced in branch
conditions.

Specifically, look for where `visited_fields` or `done_needs_fields`
are populated and ensure `p22` doesn't fall through a gap. The bug is
likely that p22 is not detected as a "live" field because the
analysis only tracks fields referenced in the transaction body's
expressions, but p22's write (`p22 = p22 + Q22`) is detected, while
its read in the print guard `[count % 5000000 == 0] { ... p22 ... }`
is in a guarded block that may be handled separately.

**Fix B: Emit concurrent-style code for independent assignments.**

This is a larger optimization. The compiler should detect that within
a node/txn body, assignments to state fields from pre-tick reads can
be reordered for parallel execution.

**Implementation sketch:**

1. In the txn body emission pass, collect all assignments to state
   fields.
2. For each assignment, check if its RHS references only pre-tick
   state fields (no `let` bindings that depend on earlier state writes).
3. If all RHS references are to pre-tick state, split the body into:
   - **Load phase:** Load all referenced pre-tick state fields.
   - **Compute phase:** Compute all new values (no ordering constraints).
   - **Store phase:** Store all new values to state.
4. Emit the compute phase with no memory barriers between independent
   computations, allowing LLVM to schedule them on separate execution
   units.

**Priority:** Fix A (p22 phi) is a correctness bug — do it first.
Fix B is a performance optimization — do it second, after verifying
that A resolves the correctness issue and measuring the remaining gap.
The benchmark rewrite with `let` temporaries (see §2.3 Alternative)
achieves the same effect without Fix B.

**Alternative (benchmark-only fix):**

Rewrite `benchmarks/float_math_nonzero.bv` to use explicit
temporaries that capture pre-tick values:

```
node tick [count < total][count == total] {
    let nx0 = A00*x0 + A01*x1 + A02*x2;
    let nx1 = A10*x0 + A11*x1 + A12*x2;
    let nx2 = A20*x0 + A21*x1 + A22*x2;
    x0 = nx0; x1 = nx1; x2 = nx2;
    p00 = p00 + Q00;
    p11 = p11 + Q11;
    p22 = p22 + Q22;
    count = count + 1;
    [count % 5000000 == 0] {
        let trace: Float = p00 + p11 + p22;
        PrintLn!(x0 + x1 + x2 + trace);
    };
    term;
};
```

This avoids the sequential dependency chain because `nx0`, `nx1`,
`nx2` are independent `let` bindings that all read pre-tick state.
The compiler emits them with no read-after-write chain, and LLVM
schedules them in parallel.

**Verification:**
```bash
cargo test --lib
BOUND=50000000 ./target/release/brief-compiler build benchmarks/float_math_nonzero.bv --out benchmarks
BOUND=50000000 bash -c 'diff <(benchmarks/float_math_nonzero) <(benchmarks/float_math_nonzero_c)'
hyperfine -w 2 'BOUND=50000000 benchmarks/float_math_nonzero' \
               'BOUND=50000000 benchmarks/float_math_nonzero_c'
```

Expected ratio after Fix A + benchmark rewrite: ~1.0×.

---

### 2.4 ring_buffer — 1.77× regression (low priority)

**Analysis:** After `clang -O3` (the actual compilation pipeline), the
Brief hot loop body is structurally identical to C's — 9 instructions
including the `mul` for modulus. Both DCE the buffer store as dead
code (no FFI reads the buffer). The 1.77× ratio is likely measurement
noise at sub-60ms absolute runtimes.

**Minor issue:** Raw IR emits `srem` instead of `urem`. The
`clang -O3` pipeline's `instcombine` converts `srem` to `urem`, so
this has no effect on the final binary. But if anyone uses `llc`
directly without `opt`, the `srem` would produce a hardware `idivq`
instruction instead of a multiplication-by-magic `mulq`.

**Fix (optional):** In the operator emission code that lowers `%` and
`mod`, detect non-negative operands and emit `urem` instead of `srem`.
This is a ~3 line change in the LLVM backend's binary operator
emission.

**Priority:** Lowest — only worth doing if other fixes are complete
and the 1.77× persists after all other changes.

**Verification:**
```bash
BOUND=50000000 ./target/release/brief-compiler build benchmarks/ring_buffer.bv --out benchmarks
hyperfine -w 2 'BOUND=50000000 benchmarks/ring_buffer' \
               'BOUND=50000000 benchmarks/ring_buffer_c'
```

---

## Dependency Order

```
Stage 1 (rename)
  ├── 1.2 Lexer
  ├── 1.3 Parser
  ├── 1.4 AST
  ├── 1.5 Field rename (~60 sites)
  ├── 1.6 BEAST
  ├── 1.7 Annotator
  ├── 1.8 LSP
  ├── 1.9 Plugin
  ├── 1.10 Rust comments
  ├── 1.11 Stdlib .bv
  ├── 1.12 Self-hosting
  ├── 1.13 Examples .bv
  ├── 1.14 Tests .bv
  ├── 1.15 Benchmarks .bv
  ├── 1.16 Documentation
  └── 1.17 Validation

Stage 2 (benchmarks)
  ├── 2.1 fasta (fflush)           ← independent, can start after Stage 1
  ├── 2.2 queue_drain (AddrOf)     ← independent
  ├── 2.2 queue_drain (indices)    ← independent
  ├── 2.3 float_math (p22 phi)     ← independent
  ├── 2.3 float_math (concurrent)  ← independent (or benchmark rewrite)
  └── 2.4 ring_buffer (srem/urem)  ← lowest priority
```

All Stage 2 fixes are independent of each other and can be implemented
in any order after Stage 1 is complete. The only dependency is that
Stage 2 benchmarks use the renamed `node` syntax, so they must be
updated in Stage 1 first.

---

## Commands Summary

```bash
# ── Rust source rename ──
find src -name '*.rs' -exec sed -i \
  -e 's/\bis_reactive\b/is_node/g' \
  -e 's/\bmake_rct_txn\b/make_node/g' \
  {} +
sed -i 's/#\[token("rct")\]/#[token("node")]/' src/lexer.rs

# ── .bv files ──
find lib benchmarks tests examples -name '*.bv' -exec sed -i \
  -e 's/rct txn/node/g' \
  -e 's/rct async txn/node async/g' \
  {} +
find lib/compiler -name '*.bv' -exec sed -i \
  -e 's/KeywordRct/KeywordNode/g' \
  -e 's/"rct"/"node"/g' \
  {} +

# ── Markdown files ──
find docs spec learn-brief -name '*.md' -exec sed -i \
  -e 's/\brct txn\b/node/g' \
  -e 's/\brct async txn\b/node async/g' \
  -e 's/\brct\b/node/g' \
  {} +
sed -i \
  -e 's/\brct txn\b/node/g' \
  -e 's/\brct async txn\b/node async/g' \
  AGENTS.md AGENTS_HISTORY.md AGENTS_HISTORY_2.md BUGS.md README.md

# ── Build and test ──
cargo build
cargo test --lib

# ── Verify no remaining rct references (excluding circt and archived) ──
git grep -n -i '\brct\b' -- '*.rs' '*.bv' '*.md' \
  | grep -v 'circt' \
  | grep -v 'archived' \
  | grep -v 'old_docs'
```

---

## Stage 3: Rename `.beast` → `.beast` (Brief Expressive AST)

The BEAST intermediate representation is renamed to BEAST — Brief Expressive
AST. File extension `.beast`, CLI flag `--emit-beast`, module path
`src/beast/`.

### 3.1 Overview

```
beast → beast    # Extension
Bvir → Beast    # PascalCase (enum, type names)
beast → beast    # snake_case (function names, field names, module)
--emit-beast → --emit-beast   # CLI flag
```

### 3.2 Module directory rename

```bash
mv src/beast src/beast
sed -i 's/pub mod beast/pub mod beast/' src/lib.rs
```

### 3.3 Rust source changes

**File: `src/lib.rs`** (line 32):
```
pub mod beast;  →  pub mod beast;
```

**File: `src/main.rs`:**

| Line(s) | Change |
|---------|--------|
| 60 | `--emit-beast` → `--emit-beast` (help text) |
| 85 | `--emit-beast [stage]` → `--emit-beast [stage]` (doc comment) |
| 94 | `emit_beast: Vec<BeastStage>` → `emit_beast: Vec<BeastStage>` |
| 127-139 | `--emit-beast` → `--emit-beast`, `BeastStage::*` → `BeastStage::*` |
| 201 | `emit_beast_stages: emit_beast` → `emit_beast_stages: emit_beast` |

**File: `src/compile.rs`:**

| Line(s) | Change |
|---------|--------|
| 28 | Comment: `--emit-beast` → `--emit-beast` |
| 56-57 | `pub emit_beast_stages: Vec<BeastStage>` → `pub emit_beast_stages: Vec<BeastStage>` |
| 112,130,152 | `emit_beast_snapshot` → `emit_beast_snapshot` |
| 217 | `emit_beast_stages: vec![]` → `emit_beast_stages: vec![]` |
| 380 | Doc comment: `--emit-beast` → `--emit-beast` |
| 382 | `fn emit_beast_snapshot` → `fn emit_beast_snapshot` |
| 389 | `opts.emit_beast_stages` → `opts.emit_beast_stages` |
| 397 | `brief_compiler::beast::to_beast` → `brief_compiler::beast::to_beast` |
| 399 | `{}.beast.{}` → `{}.beast.{}` (file extension in output path) |

**File: `src/beast/mod.rs`** (formerly `src/beast/mod.rs`):

| Line(s) | Change |
|---------|--------|
| 3 | Comment: `.beast` → `.beast` |
| 11 | `pub use serialize::to_beast` → `pub use serialize::to_beast` |
| 12 | `pub use deserialize::from_beast` → `pub use deserialize::from_beast` |

**File: `src/beast/serialize.rs`** (formerly `src/beast/serialize.rs`):

| Line(s) | Change |
|---------|--------|
| 2 | Comment: `.beast` → `.beast` |
| 11 | `pub fn to_beast` → `pub fn to_beast` |
| 282-292 | `crate::beast::from_beast` → `crate::beast::from_beast`, `to_beast` → `to_beast` |
| 293,339-340 | Same function/trait renames |
| 355-357 | `crate::beast::sexpr` → `crate::beast::sexpr` |

**File: `src/beast/deserialize.rs`** (formerly `src/beast/deserialize.rs`):

| Line(s) | Change |
|---------|--------|
| 2 | Comment: `.beast` → `.beast` |
| 11 | `pub fn from_beast` → `pub fn from_beast` |

**File: `src/beast/sexpr.rs`** (formerly `src/beast/sexpr.rs`):

| Line(s) | Change |
|---------|--------|
| 2 | Comment: `.beast` → `.beast` |

**File: `src/beast/pattern.rs`** (formerly `src/beast/pattern.rs`):

| Line(s) | Change |
|---------|--------|
| 586,655 | `crate::beast::sexpr` → `crate::beast::sexpr` |

**Files referencing `crate::beast::` or `beast::`**:

| File | Change |
|------|--------|
| `src/plugin/intrinsics.rs:23` | `use crate::beast` → `use crate::beast` |
| `src/plugin/intrinsics.rs:152,157-163` | `beast::` → `beast::` (14 occurrences) |
| `src/backend/llvm/normalizer.rs:229,273` | `crate::beast::layout` → `crate::beast::layout` |
| `src/ast/layout.rs:3` | Comment: `src/beast/layout.rs` → `src/beast/layout.rs` |

### 3.4 Documentation

| File | Change |
|------|--------|
| `docs/plans/2026-07-15-compiletime-meta-and-plugin-architecture.md` | `.beast` → `.beast`, `BEAST` → `BEAST` (~15 refs) |
| `docs/plans/2026-07-14-beast-plugin-midend.md` | Rename file to `...beast-plugin-midend.md` + all `.beast` → `.beast` |
| `docs/plans/2026-07-19-extensible-number-types.md` | `beast::layout` → `beast::layout` |

### 3.5 Self-hosting compiler references

If the self-hosting compiler (`lib/compiler/`) mentions `.beast` or `beast`,
update those references as well (search first — likely none, since the
`.beast` format is a Rust-side serialization detail).

### 3.6 Suggested commands

```bash
# Rename module directory
mv src/beast src/beast

# Rust source (mechanical replacements)
find src -name '*.rs' -not -path '*/beast/*' -exec sed -i \
  -e 's/\bbvir\b/beast/g' \
  -e 's/\bBvir\b/Beast/g' \
  -e 's/\bto_beast\b/to_beast/g' \
  -e 's/\bfrom_beast\b/from_beast/g' \
  -e 's/\bemit_beast\b/emit_beast/g' \
  -e 's/--emit-beast/--emit-beast/g' \
  {} +

# The beast module itself (already moved, avoids self-rename)
find src/beast -name '*.rs' -exec sed -i \
  -e 's/\bbvir\b/beast/g' \
  -e 's/\bBvir\b/Beast/g' \
  -e 's/\bto_beast\b/to_beast/g' \
  -e 's/\bfrom_beast\b/from_beast/g' \
  {} +

# Docs
find docs -name '*.md' -exec sed -i \
  -e 's/\.beast/.beast/g' \
  -e 's/\bBEAST\b/BEAST/g' \
  -e 's/\bbvir\b/beast/g' \
  {} +

# Root docs
sed -i \
  -e 's/\.beast/.beast/g' \
  -e 's/\bBEAST\b/BEAST/g' \
  -e 's/\bbvir\b/beast/g' \
  AGENTS.md AGENTS_HISTORY.md AGENTS_HISTORY_2.md BUGS.md README.md
```

### 3.7 Validation

```bash
cargo build
cargo test --lib
git grep -n '\.beast\|BEAST\|to_beast\|from_beast\|emit_beast' -- '*.rs' '*.md' \
  | grep -v archived | grep -v old_docs
# Should return no results
```
```
