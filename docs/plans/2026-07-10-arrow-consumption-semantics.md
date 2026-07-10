# Arrow Consumption Semantics: `&` as Consumption Marker on RHS of `<-`

## Core rule

`&` before a collection on the RHS of `<-` = **consumption**.
Without `&` = **no consumption** (source preserved).

| Syntax | Semantics | `&` on RHS |
|--------|-----------|:---:|
| `list <- value` | Push | No |
| `value <- &list` | Pop (remove one) | Yes |
| `value <- list` | Peek (read without removing) | No |
| `dest <- &source` | Transfer (move all) | Yes |
| `dest <- source` | Copy (preserve source) | No |
| `<- &list` | Drain | Yes |
| `letter <- "word"` | Char peek | No |
| `letter <- &"word"` | Char pop | Yes |
| `letter <- &"word"[;="d"]` | Filtered char pop | Yes |

## AST changes

Add `consume: bool` to `ArrowMut` and `ArrowTransfer`:

```rust
// Before:
ArrowMut { dir: ArrowDir, target, index, value }
ArrowTransfer { dest, source, filter }

// After:
ArrowMut { dir: ArrowDir, consume: bool, target, index, value }
ArrowTransfer { dest, source, filter, consume: bool }
```

When `consume: true`:
- `Pop` → removes one element (current behavior)
- `Transfer` → moves all elements from source (current behavior)

When `consume: false`:
- `Pop` → peeks one element (new: load last without removal)
- `Transfer` → copies all elements (new: deep copy, source unchanged)

## Parser changes (mod.rs:6377-6440)

Current dispatch (broken after LHS-`&` cleanup):

```rust
// 1. Both have & → transfer
// 2. LHS has & → push
// 3. RHS has & → pop assignment
// 4. Neither → error
```

New dispatch:

```rust
// 1. RHS has & → consume = true
//    a. LHS has & → transfer (backward compat: &dest <- &source)
//    b. otherwise → pop or transfer (decided by codegen: value <- &list or dest <- &source)
// 2. LHS has & → push (&list <- value)
// 3. Neither → consume = false
//    a. LHS is collection → push (list <- value)
//    b. otherwise → peek or copy (decided by codegen: value <- list or dest <- source)
```

## Codegen changes

### emit_arrow_pop (arrow.rs)
When `consume: true` (current): load and remove element.  
When `consume: false` (new): load element, DON'T remove it. Skip the pending_phi_backedge update for the list field — the list size remains unchanged.

### emit_arrow_transfer (arrow.rs)
When `consume: true` (current): move elements, empty source.  
When `consume: false` (new): deep copy elements into dest, source unchanged.

### emit_arrow_push (arrow.rs)
No change — push is always non-consuming.

## Flat control flow mandate

Every match arm must be max 2 levels deep. Use guard clauses (`if let`, `?`, `let...else`) not nested matches.

## Files changed

| File | What | Lines |
|------|------|-------|
| `src/ast.rs` | Add `consume: bool` to `ArrowMut`, `ArrowTransfer` | ~4 |
| `src/parser.rs` | Reorder arrow dispatch, accept no-`&` forms | ~15 |
| `src/features/arrow.rs` | Update interpreter for `consume` flag | ~10 |
| `src/backend/llvm/expr/arrow.rs` | Peek/copy codegen paths | ~40 |
| `src/backend/llvm/expr/rest.rs` | Update dispatch for `consume` flag | ~4 |
| `src/features/arrow.rs` (eval) | Update evaluate for `consume` flag | ~10 |
| All `*.bv` files | Update `&dest <- &source` → `dest <- &source` | 1 file |

## Verification

1. `cargo test --lib` — 1444+ tests pass
2. `bash benchmarks/build_and_bench.sh --runtime` — all benchmarks MATCH
3. `dest <- &source` parses correctly as transfer (with consumption)
4. `value <- list` parses correctly as peek (without consumption)
