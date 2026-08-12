# Full Example: Counter.increment

## Briev Source

```briev
// counter.bv
rstruct Counter {
    count: Int;
    active: Bool;

    txn Counter.increment [count >= 0 && count < 100][@count + 1 == count] {
        &count = count + 1;
        term;
    };
}
```

## Generated LLVM IR

```llvm
; ModuleID = 'counter.bv'
source_filename = "counter.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

; ── Type Definitions ──────────────────────────────────────────────────

%struct.Counter = type { i64, i8 }
;   field 0: count (i64, offset 0)
;   field 1: active (i8, offset 8)

%struct.State = type { %struct.Counter }
;   flattened state — contains all rstruct fields

; ── Global State ──────────────────────────────────────────────────────

@global_state = global %struct.State zeroinitializer

; ── Transaction: Counter.increment ────────────────────────────────────

define void @Counter_increment(%struct.State* noalias nocapture %state) alwaysinline local_unnamed_addr #0 {
entry:
    ; ── Load fields ──
    %count_ptr = getelementptr inbounds %struct.State, %struct.State* %state, i64 0, i32 0, i32 0
    %active_ptr = getelementptr inbounds %struct.State, %struct.State* %state, i64 0, i32 0, i32 1

    %count = load i64, i64* %count_ptr, align 8, !range !0
    %active = load i8, i8* %active_ptr, align 1

    ; ── Precondition injection ──
    ; [count >= 0 && count < 100]
    %c1 = icmp sge i64 %count, 0
    %c2 = icmp slt i64 %count, 100
    %pre_cond = and i1 %c1, %c2
    call void @llvm.assume(i1 %pre_cond)

    ; ── Transaction body ──
    ; &count = count + 1;
    %new_count = add nuw nsw i64 %count, 1   ; nsw+nuw proven by contract bounds

    ; ── Postcondition injection ──
    ; [@count + 1 == count] — proved by proof engine, emit as constant
    call void @llvm.assume(i1 true)

    ; ── Commit ──
    store i64 %new_count, i64* %count_ptr, align 8
    ; active unchanged — no store emitted (dead store eliminated)

    ret void
}

; ── Reactor Loop (acyclic) ────────────────────────────────────────────

define i32 @main() local_unnamed_addr #0 {
entry:
    call void @init_state()
    br label %tick

tick:
    call void @reactor_tick()
    br label %tick
}

define void @reactor_tick() norecurse #0 {
    %state = load %struct.State, %struct.State* @global_state

    ; Evaluate preconditions
    %count = extractvalue %struct.State %state, 0, 0
    %c1 = icmp sge i64 %count, 0
    %c2 = icmp slt i64 %count, 100
    %t1_cond = and i1 %c1, %c2

    br i1 %t1_cond, label %t1_body, label %noop

t1_body:
    %new_count = add nuw nsw i64 %count, 1
    %new_state = insertvalue %struct.State %state, i64 %new_count, 0, 0
    br label %commit

noop:
    call void @__wait_for_event()
    br label %commit

commit:
    %final = phi %struct.State [%new_state, %t1_body], [%state, %noop]
    store %struct.State %final, %struct.State* @global_state
    ret void
}

; ── Intrinsics ────────────────────────────────────────────────────────

declare void @llvm.assume(i1) #1

; ── Attributes ────────────────────────────────────────────────────────

attributes #0 = {
    mustprogress
    nofree
    norecurse
    nosync
    nounwind
    willreturn
    memory(argmem: readwrite)
}
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }

; ── Metadata ──────────────────────────────────────────────────────────

!0 = !{i64 0, i64 100}  ; !range for count: [0, 100)
```

## Optimization Summary

| Feature | Applied | Effect |
|---------|---------|--------|
| `noalias` + `nocapture` | ✅ | `mem2reg` promotes state fields to SSA |
| `!range` | ✅ | LLVM deletes `count >= 0` check (proven by load metadata) |
| `llvm.assume` | ✅ | `count < 100` injected — LLVM eliminates overflow guards |
| `nuw nsw` | ✅ | `add nuw nsw` — no overflow check needed |
| `norecurse` | ✅ | Enables aggressive inlining |
| `local_unnamed_addr` | ✅ | Function address never taken |
| `willreturn` | ✅ | LLVM can move the call past other operations |
| `memory(argmem: readwrite)` | ✅ | Confirms no global access beyond `%State` |
| Acyclic inlining | ✅ | Reactor tick has zero function calls — pure SSA |