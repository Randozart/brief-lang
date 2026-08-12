# Event Model: LLVM IR Lowering

**Date:** 2026-05-29
**Status:** Specification
**Version:** 1.0

This document specifies how the LLVM backend (src/backend/llvm.rs) compiles
the Briev event model into LLVM IR. It is the compilation counterpart to
specs/EVENT-MODEL.md.

## 1. `@ link sym` → External Global Declaration

For every `trg name: Type @ link sym;` in the program:

### LLVM Module Header

```llvm
; Bool triggers → i8 (stored as zero/non-zero)
@sym = external global i8, align 1

; Int triggers → i64
@sym = external global i64, align 8

; String triggers → i8* (pointer to runtime-managed buffer)
@sym = external global i8*, align 8
```

### Implementation in generate()

In `LlvmBackend::generate()`, after collecting all trigger declarations into
`self.triggers`, iterate and emit:

```rust
for (name, trg) in &self.triggers {
    if let LinkRef::Linked(sym) = &trg.address {
        let llvm_ty = self.llvm_type_for_trg(&trg.ty); // i8, i64, or i8*
        let align = self.align_of(llvm_ty);
        writeln!(out, "@{} = external global {}, align {}", sym, llvm_ty, align).ok();
    }
}
```

**Fixes bug 4B** — currently `@sym` is referenced in `load volatile` but never
declared.

### MMIO Triggers (`@ 0x...`)

No module-level declaration needed. The address is materialized inline via
`inttoptr`:

```llvm
%ptr = inttoptr i64 0x40001000 to i8*
%val = load volatile i8, i8* %ptr, align 1
```

## 2. Trigger Sampling in `reactor_tick()`

All triggers are sampled exactly once at the top of `reactor_tick()`, before
any precondition evaluation:

```llvm
define void @reactor_tick() local_unnamed_addr #0 {
entry:
    ; ── SAMPLE PHASE ──
    ; MMIO trigger:
    %tr0 = load volatile i8, i8* inttoptr (i64 0x40001000 to i8*), align 1
    %tr0_s = zext i8 %tr0 to i64

    ; @ link trigger:
    %tr1 = load volatile i8, i8* @__io_pending, align 1
    %tr1_s = zext i8 %tr1 to i64

    ; ── EVALUATE PHASE ──
    ; ... preconditions reference %tr0_s, %tr1_s, never raw loads ...
```

The sampled values (`%tr0_s`, `%tr1_s`) are immutable SSA registers. All
precondition evaluations and transaction bodies within this tick reference the
sampled values, never the raw volatile pointer. This enforces deterministic
execution within a single tick.

## 3. `trg` Identifier References

When `Expr::Identifier(name)` resolves to a trigger name (found in
`self.trigger_names`), the compiled code must reference the SAMPLED register
from the sampling phase — NOT emit a new `load volatile`.

### Implementation

The sampling phase emits a sampled value into a persistent register. For each
trigger `name`, the sampling code writes to a reusable register pattern:

```llvm
; Sampling phase:
%s_<name> = load volatile i8, i8* @<sym>, align 1
%sz_<name> = zext i8 %s_<name> to i64
```

When `Expr::Identifier(name)` is evaluated later in the tick, it emits:

```rust
let sampled = format!("%sz_{}", name);
writeln!(out, "{}{} = add i64 0, {}", indent, v, sampled).ok();
```

### Current Bug

The current code (lines 532-545) emits a NEW `load volatile` at the
`Expr::Identifier` site instead of referencing the sampled value. This:

1. Breaks the single-sample guarantee (a `trg` read twice in one tick gets
   different values)
2. Duplicates the `@sym` reference without declaring the global

**Fix:** The `emit_reactor` function must pre-emit all trigger loads into
named `%sz_<name>` registers. The `emit_expr` for trigger identifiers must
reference these pre-sampled registers, not emit new loads.

## 4. Pump Transaction Compilation

The pump transaction (`node __io_pump [__io_pending] { ... term; }`) is
compiled as a normal `node`. No special treatment.

### Dispatch Order

The pump transaction must be the first transaction evaluated in the dispatch
chain. This is guaranteed by:

1. The pump's precondition is `[__io_pending]` — it only fires when the
   runtime signals an event
2. The user's consumer transactions gate on `[__io_ready]` — which the pump
   sets AFTER it completes
3. In a single tick, the pump and consumer cannot both fire (the pump clears
   `__io_pending` and sets `__io_ready`; the consumer fires on a SUBSEQUENT
   tick)

If the pump and consumer DID need to fire in the same tick, the dispatch chain
would need priority ordering: pump first (priority 0 in `dispatch` vector),
then consumers. The current backend sorts dispatch by `dispatch` vector order
(all non-fused txns appended after fused ones). A priority field could be
added to `ReactiveTransaction` to guarantee pump-first ordering.

## 5. `__wait_for_event()` — Removed, Replaced by Library Pattern

The `__wait_for_event()` call was removed from the backend's equilibrium path
(Phase F of PHASE-REOPT-LLVM.md). The backend no longer emits any call in the
no-op dispatch chain — it simply does:

```llvm
    ; If no precondition was true:
    ret void
```

The user (or stdlib) provides sleep as a regular `frgn` + `node [true]`:

```briev
// In std/io.bv:
frgn __wait_for_event() -> Void from "libruntime";

node __io_pump [__io_pending] { &buf = __raw_poll(); &ready = true; term; };
node __io_sleep [true]        { __wait_for_event(); term; };
```

Because `__io_sleep` is declared last, it fires only when no other precondition
is true. `__wait_for_event()` blocks the thread. On return, the tick loop
re-evaluates all preconditions. Zero magic.

## 6. `__raw_poll()` FFI Call

The `__raw_poll()` foreign function is called inside the pump transaction body.
It is a standard `call` instruction:

```llvm
; Inside @__io_pump:
%buf = call %Vector_u8 @__raw_poll()
```

The return type matches the `frgn` declaration in `io.bv`. The call is
guaranteed non-blocking because the runtime has already determined (via the
interrupt/event that woke `__wait_for_event`) that data is available.

## 7. `trg!` Statement

The `Statement::LocalTrigger` is emitted as a comment:

```rust
Statement::LocalTrigger { name, .. } => {
    writeln!(out, "{}; trg! {} — use top-level trg + node instead", indent, name).ok();
}
```

No LLVM instructions are emitted. The statement is a no-op for code generation
and serves only as documentation.

## 8. MMIO Trigger Compilation

For `trg name: Type @ 0xABCD;`:

### Sampling Phase

```llvm
%ptr = inttoptr i64 0xABCD to <ty>*
%sampled = load volatile <ty>, <ty>* %ptr, align <align>
```

### Type Mapping

| Briev Type | LLVM Type | Alignment |
|-----------|-----------|-----------|
| Bool | i8 | 1 |
| Int | i64 | 8 |
| UInt | i64 | 8 |
| Float | float | 4 |
| Char | i32 | 4 |

## 9. Dispatch Ordering

The dispatch chain in reactor_tick() evaluates preconditions in priority order
using a **fall-through chain** (not first-true-wins return). Each transaction
body branches to the next precondition check, never to `ret void`. The final
`ret void` only executes after ALL preconditions have been evaluated:

```llvm
; Evaluate pump
%pr_pump = call i1 @pre___io_pump(%State* @global_state)
br i1 %pr_pump, label %b_pump, label %ck_next

; Pump body — executes, then falls through to check next precondition
b_pump:
call void @__io_pump(%State* @global_state)
br label %ck_next

; Evaluate consumer
ck_next:
%pr_t1 = call i1 @pre_handle_input(%State* @global_state)
br i1 %pr_t1, label %b_t1, label %ck_t1

; Consumer body — executes, then falls through
b_t1:
call void @handle_input(%State* @global_state)
br label %ck_t1

; ... etc ...
ck_t1:
; No precondition was true — tick ends
ret void
```

This matches the interpreter model (reactor.rs) where all dirty transactions
are evaluated sequentially in one tick, with each transaction's side effects
visible to the next transaction's precondition. This is essential for the
pump/consumer pattern: `__io_pump` sets `io_ready`, and the consumer reads it
in the same tick.

The dispatch chain in reactor_tick() evaluates preconditions in priority order.
The order is:

1. Fused transactions (from resolve_fusable_pairs)
2. Pump transaction (if present, priority 0)
3. All remaining non-fused transactions (in declaration order)

Each precondition evaluation branches to the transaction body if true, or
falls through to the next check:

```llvm
; Evaluate pump
%pr_pump = call i1 @pre___io_pump(%State* @global_state)
br i1 %pr_pump, label %b_pump, label %ck_next

; Evaluate user transaction 1
ck_next:
%pr_t1 = call i1 @pre_handle_input(%State* @global_state)
br i1 %pr_t1, label %b_t1, label %ck_t1

; ... etc ...

; No precondition was true:
noop:
call void @__wait_for_event()
ret void
```

## 10. Fused Transaction Interaction with Pump

Fused transactions must never include the pump. The pump is a `node` with
precondition `[__io_pending]`, and fusing it with a downstream consumer would
break the buffer-caching guarantee (the pump's result would be consumed in the
same tick, but the downstream consumer expects the buffer to be stable).

The existing fusion inhibition rules already prevent this: if the pump and a
consumer write to overlapping state (`__io_buffer`, `__io_ready`), the WAW
hazard check rejects fusion. This is correct and requires no change.

## 11. Summary: New Bugs Discovered

The event model analysis reveals one additional LLVM backend issue not
previously cataloged:

| # | Lines | Bug | Fix |
|---|-------|-----|-----|
| E1 | 532-545 | Trigger identifiers emit fresh `load volatile` per reference instead of using pre-sampled register | Move trigger sampling to reactor_tick prologue; Expr::Identifier references pre-sampled `%sz_<name>` registers |
| E2 | 805-812 | Trigger sampling happens in reactor_tick but values are not stored in reusable named registers | Emit `%sz_<name> = zext i8 %raw to i64` for each trigger, store in a map for Expr::Identifier lookup |
