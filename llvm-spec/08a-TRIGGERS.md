# Triggers: trg → Volatile Sampling & Synchronization

## 1. The Hazard: Mid-Cycle Fluctuation

A `trg` is mutated asynchronously by an external source (hardware interrupt, separate thread, WASM host environment). Reading a raw `trg` multiple times within a single `reactor_tick()` is dangerous:

```llvm
; THE BUG: Reading raw volatile trigger directly in preconditions
%cond1 = load volatile i1, i1* @button_trg_ptr
br i1 %cond1, label %txn1, label %check_txn2

check_txn2:
%cond2 = load volatile i1, i1* @button_trg_ptr ; If button is pressed EXACTLY here,
                                                ; both or neither transaction could execute!
```

## 2. The Solution: Volatile Double-Buffering (Sampling)

A volatile `trg` is loaded exactly **once** at tick entry. This value is saved in a local, immutable SSA register. All downstream precondition evaluations and transaction bodies use the **sampled SSA register**, never the raw pointer.

```llvm
@volatile_button_ptr = external global i8, align 1

define void @reactor_tick() norecurse #0 {
entry:
    ; 1. SAMPLE PHASE: Load volatile trigger exactly once
    %raw_trg = load volatile i8, i8* @volatile_button_ptr, align 1
    %trg_sampled = icmp ne i8 %raw_trg, 0

    ; 2. EVALUATE PHASE: Use stable, immutable %trg_sampled
    %state = load %struct.State, %struct.State* @global_state
    %count = extractvalue %struct.State %state, 0, 0

    ; Precondition: [button_trg && count < 100]
    %c1 = icmp slt i64 %count, 100
    %t1_cond = and i1 %trg_sampled, %c1

    br i1 %t1_cond, label %t1_body, label %noop
    ...
```

**The Rule:** A volatile `trg` variable is loaded exactly **once** per tick, in the prologue of `reactor_tick()`, before any state is loaded or any precondition is evaluated. All uses within that tick reference the SSA sample.

## 3. Trigger Lowering Models

The backend chooses a model based on the `trg` declaration:

### Model A: Memory-Mapped I/O (MMIO — Bare-Metal / FPGAs)

When a trigger is bound to a physical address (`let button: Bool @ 0x40001000;`):

```llvm
%ptr = inttoptr i64 1073745920 to i8* ; 0x40001000
%sampled = load volatile i8, i8* %ptr, align 1
```

The `volatile` qualifier prevents LLVM from hoisting, merging, or eliminating the load.

### Model B: Polled FFI (OS / WebAssembly)

Triggers are updated by calling a platform polling function before the tick:

```llvm
; In main() reactor loop:
tick:
    call void @__poll_triggers() ; Updates shared memory trigger state
    call void @reactor_tick()
    br label %tick
```

### Model C: Shared Memory Spinlocks (Metropolitan Protocol)

For concurrent IPC channels, the backend emits a spinlock with a hardware yield:

```llvm
spin:
    %status = load volatile i32, i32* %status_word_ptr, align 4
    %ready = icmp eq i32 %status, 1
    br i1 %ready, label %process, label %yield

yield:
    tail call void @llvm.x86.sse2.pause() ; Hardware pause — yield CPU
    br label %spin
```

## 4. Synthesis with Transition Fusing

When the **Transition Fusing** pass evaluates whether to fuse `Txn_A` → `Txn_B`:

- If `Txn_B`'s guard depends on a volatile `trg`, fusion is **refused** — a `trg` can mutate between ticks, but fusing would force `Txn_B` to use the stale sample from `Txn_A`'s tick.
- Only fuse when all intermediate guards rely strictly on **deterministic, internal state**, never on `trg` inputs.
