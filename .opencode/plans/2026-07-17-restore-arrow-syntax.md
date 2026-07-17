# Restore `<-` Arrow Syntax

## Design Principles

1. **General solution, not benchmark-specific.** The arrow operations must work
   for ANY collection type with `insert_at`/`extract_from` properties, not just
   RingBuffer inline fields.

2. **Two paths, selected by type properties:**
   - **Fast path (inline RingBuffer):** When `ringbuf_inline` has entries — direct
     %State GEP access for the 4 inline fields (data, head, tail, mask).
   - **Slow path (arena-backed List<T>):** When the collection is a heap-allocated
     list — use the arena infrastructure (`emit_arena_alloc()`, `arena_slots`) that
     survived the refactoring for bump allocation, falling back to `@malloc`+`@free`.

3. **No transfer syntax** — `&dest <- &source` is removed. Only push, pop, discard.

4. **No feature dispatch** — unified Expr/Statement variants, direct codegen in
   `emit_stmt.rs`/`emit_expr.rs`. The dead `src/features/arrow.rs` is removed.

---

## Step 1: AST Variants (`src/ast/expr.rs`)

### `Expr::AddrOf(Box<Expr>)`
For `&queue`. Marks a collection target for push/pop/discard.

### No new Statement variants
Push reuses `Statement::Assign(lhs, rhs)` — LHS is `AddrOf(target)`.
Pop reuses `Statement::Assign(lhs, rhs)` — RHS is `AddrOf(source)`.
Discard reuses `Statement::Expression(expr)` — expr is `AddrOf(source)`.

---

## Step 2: Parser (`src/parser/expressions.rs` + `src/parser/statements.rs`)

### Parse `&` unary operator
In `parse_unary()`: `Some(Token::Ampersand)` → advance → parse inner →
`Expr::AddrOf(Box::new(inner))`.

### Parse `<-` prefix (discard)
In `parse_statement()`: `Some(Token::ArrowLeft)` → parse target, expect `;`,
return `Ok(Statement::Expression(Expr::AddrOf(target)))`.

### Parse `<-` infix (push / pop)
In `parse_expression_statement()`: after parsing LHS and checking for `Eq`,
also check for `Token::ArrowLeft`:
```
let rhs = self.parse_expression()?;
self.expect(Token::Semicolon)?;
// &target <- value → Assign(AddrOf(target), value)  [push]
// value <- &source → Assign(Ident(value), AddrOf(source))  [pop]
```

Codegen distinguishes by checking which side has `AddrOf`:
- `Assign(AddrOf(target), value)` → push
- `Assign(Ident(x), AddrOf(source))` → pop
- `Assign(AddrOf(_), AddrOf(_))` → invalid (removed syntax)

---

## Step 3: LLVM Codegen (`src/backend/llvm/emit_stmt.rs` + `emit_expr.rs`)

### Push: `Assign(AddrOf(target), value)`
1. Extract target variable name from target addr-of expr
2. Call `check_insert_strategy(target)` → returns `"ring_push"` or `"list_push"`
3. **If RingBuffer with inline fields:** Look up `ringbuf_inline[name]` for the
   4 %State GEP indices. Emit direct GEP+load+store for the ring buffer push
   (data[tail & mask] = value; tail = (tail+1) & mask).
4. **If heap-allocated list:** Use the arena-backed List<T> path:
   - Check `field_prealloc_info` for preallocated capacity
   - If capacity available: fast path (GEP buf[len+2], store, increment len)
   - If not: slow path (`emit_arena_alloc()` for new buffer + `memcpy` old elements)
5. Register stores in `pending_phi_backedge` for loop convergence

### Pop: `Assign(ident(x), AddrOf(source))`
1. Call `check_extract_strategy(source)` → returns `"ring_pop"` or `"list_pop"`
2. **Inline RingBuffer path:** GEP+load data[head & mask]; head = (head+1) & mask
3. **Arena list path:** Load head element, advance pointer, free old buffer if no arena
4. Store popped result to `x`

### Discard: `Expression(AddrOf(source))`
Same as pop but skip the final store to `x`.

### `Expr::AddrOf(inner)` when used in expression context
Emits a GEP into `%State` for the addressed state field. Returns the field's i64
register (ptrtoint of heap data for lists, or the raw i64 handle for RingBuffers).

---

## Step 4: Remove dead `src/features/arrow.rs`

The file is orphaned (not in `features/mod.rs`), imports non-existent types,
and its `ExprCodegenLLVM` implementations are no-op stubs. Delete it.

---

## Step 5: Cleanup

- Update comment at `mod.rs:739` about arrow ops now being implemented
- `collect_push_targets()` at `mod.rs:743` — remove or rewrite to scan for
  `Assign(AddrOf(...), ...)` LHS patterns

---

## Files Changed

| File | Change |
|------|--------|
| `src/ast/expr.rs` | Add `Expr::AddrOf(Box<Expr>)` variant |
| `src/parser/expressions.rs` | Parse `&` as `Expr::AddrOf` in `parse_unary()` |
| `src/parser/statements.rs` | Parse `<-` prefix (discard) and infix (push/pop) |
| `src/backend/llvm/emit_stmt.rs` | Codegen for push/pop/discard (both inline RingBuffer and arena List paths) |
| `src/backend/llvm/emit_expr.rs` | Handle `Expr::AddrOf` — emit %State GEP for state fields |
| `src/backend/llvm/mod.rs` | Update arrow comment; update `collect_push_targets()` |
| `src/features/arrow.rs` | **Remove** — orphaned dead code |
