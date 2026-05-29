# Transactions: txn → LLVM Function

## Basic Pattern

Every `txn` becomes an LLVM `define` function receiving the global state pointer:

```brief
rstruct Counter {
    count: Int;

    txn Counter.increment [count < 100][@count + 1 == count] {
        &count = count + 1;
        term;
    };
}
```

```llvm
define void @Counter_increment(%struct.State* noalias nocapture %state) local_unnamed_addr #0 {
entry:
    ; 1. Load fields from state
    %count_ptr = getelementptr inbounds %struct.State, %struct.State* %state, i64 0, i32 0
    %count = load i64, i64* %count_ptr, align 8, !range !0

    ; 2. Precondition injection (see CONTRACT-TO-METADATA.md)
    %pre_cond = icmp slt i64 %count, 100
    call void @llvm.assume(i1 %pre_cond)

    ; 3. Transaction body
    %new_count = add nuw nsw i64 %count, 1

    ; 4. Postcondition (proved at compile time → assume true)
    call void @llvm.assume(i1 true)

    ; 5. Commit
    store i64 %new_count, i64* %count_ptr, align 8
    ret void
}
```

## Transaction Body Translation Rules

| Brief Statement | LLVM IR |
|----------------|---------|
| `&count = count + 1;` | `%new = add i64 %old, 1` + `store i64 %new, i64* %ptr` |
| `let x: Int = expr;` | `%x = ...` (SSA value) |
| `[guard] { ... }` | `%cond = ...` / `br i1 %cond, label %body, label %end` |
| `term;` | `ret void` |
| `term expr;` | `ret i64 %expr` |
| `uni val(Variant(f)) = { ... };` | `switch i64 %discriminant ...` (see MATCH-TO-SWITCH.md) |
| `match expr { ... };` | `switch i64 %discriminant ...` (see MATCH-TO-SWITCH.md) |

## Function Attributes

Transactions always receive these attributes (enabling maximum LLVM optimization):

```llvm
attributes #0 = {
    mustprogress       ; always terminates
    nofree             ; no memory allocation
    norecurse          ; no recursion (proven by call graph analysis)
    nosync             ; no synchronization instructions
    nounwind           ; no exceptions
    willreturn         ; always returns
    memory(argmem: readwrite)  ; only accesses memory via pointer args
}
```

## Acyclic Optimization

If the call graph has no cycles (`cg.has_cycle() == false`):

1. All `txn` and `defn` functions get `norecurse` + `willreturn`
2. The reactor loop inlines all transaction bodies — no `call` instructions at all
3. LLVM sees the entire state machine as one SSA graph → maximum optimization

If the call graph has cycles, functions get `recursive` (no `norecurse`) and the reactor loop uses `call` instructions with a dispatch table.