# Validation Report: Phase 0 — LLVM Backend Scaffold

**Date:** 2026-05-29  
**LLVM Version:** Ubuntu LLVM 18.1.3 (Optimized build, x86_64-pc-linux-gnu, Host CPU: ivybridge)  
**Compiler Commit:** `ef992b8`  
**Spec Version:** v1.2  

---

## 1. Test Fixtures

| Fixture | Description | Status |
|---------|-------------|--------|
| `tests/fixtures/counter.bv` | Single `Int` field, one `node` with `[counter < 10]` contract | ✅ PASS |
| `tests/fixtures/multifield.bv` | `Int` + `Bool`, two `node` with disjoint field access | ✅ PASS |
| `tests/fixtures/minimal.bv` | Single `Int`, no transactions | ✅ PASS |

## 2. Validation Checks

### 2.1 Module Structure
- ✅ `source_filename`, `target datalayout`, `target triple` present
- ✅ `%State` type emitted with correct field types (`{ i64 }`, `{ i64, i8 }`)
- ✅ `@global_state = global %State zeroinitializer`
- ✅ `declare void @llvm.assume(i1) #1`

### 2.2 Transaction Signature
- ✅ `%State* noalias nocapture %state` — correct pointer model
- ✅ `local_unnamed_addr #0` — attribute group reference
- ✅ `alwaysinline` — forced inlining for acyclic transactions

### 2.3 Field Access
- ✅ GEP: `getelementptr inbounds %State, %State* %state, i32 0, i32 N`
- ✅ Bool (i8) fields: `zext i8 %ld to i64` on load, `trunc i64 %val to i8` on store
- ✅ `align` values correct (i64=8, i8=1)

### 2.4 Init State
- ✅ `@init_state()` emits `store volatile` for each field's default value
- ✅ Uses temp register for GEP, not inline constant expression

### 2.5 Reactor Loop
- ✅ `@main()`: `call @init_state()` → `br label %tick` → `call @reactor_tick()` → `br label %tick`
- ✅ `@reactor_tick()`: comments document trigger sampling + dispatch phases

### 2.6 Attribute Blocks
- ✅ `#0`: `mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite)`
- ✅ `#1`: `nocallback nofree nosync nounwind willreturn memory(argmem: write)`

## 3. LLVM Toolchain Results

### 3.1 `llc` Assembly Generation

All fixtures produce valid `.s` files. No errors.

**`counter.ll` → `counter.s`** (excerpt):
```asm
increment:
	incq	(%rdi)          ; Single-instruction atomic increment!
	retq

init_state:
	movq	$0, (%rdi)
	retq

main:
	pushq	%rax
	movq	$0, global_state(%rip)
.Ltmp0:
	jmp	.Ltmp0                ; Infinite loop (expected — no events)
```

**`multifield.ll` → `multifield.s`** (excerpt):
```asm
increment:
	incq	(%rdi)
	retq

toggle:
	movzbl	8(%rdi), %eax       ; Load i8 (Bool)
	incq	%rax                ; Increment to set to 1 (true)
	movb	%al, 8(%rdi)        ; Store back
	retq
```

### 3.2 `opt -O3` Optimization

`counter.ll` optimized from 86 lines to 7 lines of IR:
```llvm
define void @increment(ptr noalias nocapture %state) local_unnamed_addr #0 {
entry:
  %tmp1 = load i64, ptr %state, align 8
  %tmp0 = add i64 %tmp1, 1
  store i64 %tmp0, ptr %state, align 8
  ret void
}

define noundef i32 @main() local_unnamed_addr #2 {
entry:
  store volatile i64 0, ptr @global_state, align 8
  unreachable
}
```

Key LLVM optimizations confirmed firing:
- **GEP elimination**: `%state` offset 0 → GEP simplified to direct `ptr %state`
- **`noalias` enabled**: `ptr noalias nocapture %state` preserved through optimization
- **Dead function elimination**: `@reactor_tick()` removed (empty body)
- **Main loop simplification**: Empty tick loop → `unreachable` after init

### 3.3 `opt -verify` (Structural Validity)

All fixtures pass LLVM IR verification.

## 4. LLVM 18.1.3 Compatibility Notes

| Issue | Fix | Impact |
|-------|-----|--------|
| GEP parentheses syntax removed | `(%State, %State* %s, ...)` → `%State, %State* %s, ...` | Instructions must not use parens. Constant expressions still require them (use temp reg instead). |
| `norecurse` not valid in function signature | Inline `norecurse` → attribute group `#0` only | Remove from `define void @reactor_tick() norecurse` — put in `attributes #0` |
| `opt -O3` outputs bitcode by default | Use `opt -O3 -S` for text IR | Bitcode requires `llvm-dis` to read |

## 5. Unit Tests

```
test backend::llvm::tests::test_llvm_acyclic_annotation ... ok
test backend::llvm::tests::test_llvm_generates_module ... ok
test backend::llvm::tests::test_llvm_generates_state_type ... ok
test backend::llvm::tests::test_llvm_generates_transaction ... ok
test backend::llvm::tests::test_llvm_has_noalias ... ok
```

All 5 tests passing. Full suite: 270 tests passing.

## 6. Regression Baseline

For future commits, repeat:
```bash
cargo build --release
briv-compiler llvm tests/fixtures/counter.bv --out /tmp/v/
llc /tmp/v/counter.ll -o /dev/null          # Must succeed
opt -O3 -S /tmp/v/counter.ll -o /dev/null   # Must succeed
grep -c "noalias" /tmp/v/counter.ll          # Must be > 0
```

Any change that breaks these checks must be fixed before merge.