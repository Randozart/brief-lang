# Plan: ISR Handlers + Vector Tables (Phase 9 of C++-expressiveness)

**Date:** 2026-09-06
**Status:** Active — signed off (user: explicit `<>`, configured-capable, error-with-fix posture)
**Companion:** `docs/plans/2026-09-06-cpp-expressiveness.md` §4.5 (Decision 6) — this plan replaces
its "Open question 3" with the design below. Linker `section` placement (Phase 8) is scoped to the
`.isr_vector` table emission only; general `section` keywords remain deferred.

---

## 1. Origin and signed-off decisions

Phase 6 of the C++-expressiveness session closed pointer arithmetic, atomic ordering,
and portable SIMD. ISR handlers were deferred on one open question: does `isr` take a
target parameter? The user's calls:

1. **Explicit `<>`, configured-capable.** The mechanism rides in `isr<arm_cortex_m>`,
   per the `category<mechanism>` delimiter law (SPEC §8.1: `asm<chip>`,
   `#Link<name>`, `sync<group>`). Configuration must be able to supply the mechanism
   so explicit annotation is the exception, not the requirement.
2. **Posture: error with fix in the message.** When neither an explicit mechanism nor
   a target profile supply one, compilation fails with what/why/fix — the compiler
   never invents a vector table layout ("inventing hardware layout is silent
   wrongness").
3. **Compiler emits the calling convention.** Prologue/epilogue are compiler-owned
   per mechanism row — a handler with a hand-written prologue is two sources of
   truth for the ABI.

## 2. Key infrastructure findings (2026-09-06 investigation)

- **`asm<target>` is dead data** — parsed (`parser/definitions.rs:1700-1738`), stored
  (`ast/top.rs:1234-1241`), never validated (typechecker checks only non-empty,
  `typechecker/mod.rs:5118-5127`), never read by codegen (`emit_toplevel.rs:5244+
  ignores `af.target`). **The gap `isr` must not repeat.** A registry validation at
  typecheck time is the whole point of the explicit form.
- **`InterruptEntry`/`EmbeddedConfig` exist, unconstructed** —
  `src/backend/llvm/mod.rs:1288-1319` pre-allocates `interrupts: Vec<InterruptEntry>`
  with `vector: u32` + `trg_name: Option<String>`; nothing fills or reads it. The
  landing zone for collected handlers.
- **Board files exist** — `lib/boards/stm32f407/{addresses,registers,map}` + resolver
  `src/address_resolver.rs:37-85` map NAMES to hardware addresses. Named vectors
  (`@ TIM2`) ride the identical pattern (`interrupts.dbvl`: `TIM2: 28;`).
- **House config pattern** — `.dbvl` quoted-key line tables, `ConfigDb::from_str` +
  positional `db.field_string(&key, N)`, baked with `include_str!`
  (`config/targets.dbvl`, `config/protocols.dbvl` → `ProtocolConfig::load`,
  `src/target.rs:82-164`).
- **Restriction precedents** — `check_embedded_restrictions`
  (`src/backend/llvm/mod.rs:2094-2128`) bans threading intrinsics + unbounded
  recursion; `HardwareValidator` (`src/hardware_validator.rs:33-130`) upgrades
  severity under `is_embedded`. Body restriction proofs hook there.
- **`halt#`/`wfi` is documented but unimplemented** — out of scope here; a separate
  intrinsic slice after ISRs land.

## 3. Syntax and resolution order

```briev
// 1. EXPLICIT — mechanism named; overrides config (disclosed)
isr<arm_cortex_m> handler @ 0x1C: tim2_irq() {
    ack_timer();
};

// 2. CONFIGURED — named vector resolved through the board file
//    (lib/boards/<board>/interrupts.dbvl:  TIM2: 28;)
isr handler @ TIM2: tim2_irq() {
    ack_timer();
};

// 3. INFERRED — literal vector; mechanism from the active target profile
isr handler @ 0x1C: tim2_irq() { ... };
```

Resolution order (each step overrides the previous):

| Input | Mechanism | Vector |
|-------|-----------|--------|
| `isr<mech> ... @ lit` | literal (validated against registry) | literal |
| `isr<mech> ... @ Name` | literal | board table |
| `isr ... @ lit` | target profile's `isr_mechanism` row | literal |
| `isr ... @ Name` | target profile | board table |
| nothing active | **compile error** (what/why/fix) | — |

Named-vector resolution failure is also an error naming the board file and the
missing row.

## 4. Config surface

### `config/isr-targets.dbvl` (new, house pattern, include_str!)

```
# mechanism: entry_stride, sp_slot, thumb_bit, return_insn, fpu_context, max_frame
arm_cortex_m:   4; sp; 1; bx lr; lazy; 512;
arm_cortex_m0:  4; sp; 1; bx lr; none; 256;
riscv_machine:  4; none; 0; mret; none; 512;
x86_idt:        8; none; 0; iretq; none; 512;
```

Fields (positional, `ConfigDb` style):
- `entry_stride` — bytes per table slot
- `sp_slot` — `sp` = slot 0 is the initial stack pointer (ARM), `none` = no SP slot
- `thumb_bit` — OR into handler addresses (ARM Thumb)
- `return_insn` — the epilogue return (validation anchor + emission)
- `fpu_context` — `none` (reject Float in body), `lazy` (FPCCR.ASPEN/LSPEN), `eager`
- `max_frame` — stack frame bound in bytes; body frame larger = compile error

### Target profile key

`briev.toml` `[target.<name>]` gains `isr_mechanism = "arm_cortex_m"`. Absent +
no explicit `<>` = the error posture.

### `lib/boards/<board>/interrupts.dbvl` (new)

```
# interrupt name: vector number (slot index)
TIM2: 28;
USART1: 37;
```

Resolver extension in `src/address_resolver.rs` (same load/lookup shape as
addresses).

## 5. Compiler-derived proofs (inference matrix)

| Derived | Source | Failure mode |
|---------|--------|--------------|
| Table layout | mechanism row | impossible to misconfigure silently |
| Prologue scope | body register usage (frame analysis at emission) | never saves unused regs |
| Return instruction | mechanism row | — |
| Full vector table | declared `isr` set + reset entry; undeclared slots → default handler (mechanism row supplies the default name; compiler emits a spin loop if the board doesn't provide one) | table completeness is compiler-owned |
| FPU context need | body float usage × row's `fpu_context` | body uses Float + `none` row = error ("move the math out, or pick a mechanism that stacks FP context") |
| No allocation | existing effect analysis | `Malloc#` in body = compile error |
| No spawn/threading | `check_embedded_restrictions` ban list | compile error |
| Bounded frame | frame size ≤ `max_frame` | compile error with measured size |

**Programmer declares only:** which vectors the program uses (`@`), the body,
(optionally) the mechanism.

## 6. Implementation steps

| # | Step | Files |
|---|------|-------|
| 1 | `config/isr-targets.dbvl` + `IsrMechanism` struct + loader (`src/target.rs`, house pattern) | config/, src/target.rs |
| 2 | Lexer `Isr` token (+ vocab registration, Display, keyword list) | src/lexer.rs, src/vocab.rs |
| 3 | Parser: `parse_isr_handler` cloning `parse_asm_fn` shape; `@` vector = `Expr::Decimal` OR `Expr::Identifier` | src/parser/definitions.rs |
| 4 | AST `TopLevel::IsrHandler(IsrHandler)` — fields: `mechanism: Option<String>`, `vector: Expr`, `name`, `params`, `contract`, `body`, `span`; equality + display + canonical print | src/ast/top.rs, src/ast/canonical.rs |
| 5 | Typecheck: mechanism resolution (explicit → validate registry; else profile; else error-with-fix), named-vector resolution, duplicate-vector check, body restriction pre-pass | src/typechecker/mod.rs |
| 6 | Backend: fill `EmbeddedConfig.interrupts` during generate; emit per-mechanism vector table global + prologue/epilogue at handler definition; `section(".isr_vector")` on the table | src/backend/llvm/mod.rs, emit_toplevel.rs |
| 7 | Board `interrupts.dbvl` + resolver lookup | src/address_resolver.rs, lib/boards/stm32f407/ |
| 8 | Tests: parse round-trip (all three forms), registry errors, named-vector resolution, duplicate vector, restriction proofs, table golden layout per mechanism | parser tests, typechecker tests, backend tests |
| 9 | Docs: SPEC §13.2 (ISR declarations), agent-reference, backend-contracts, this plan updated | spec/, docs/architecture/ |

## 7. What does NOT change

- `asm<target>` — stays as-is this slice (its dead-target gap is noted; a later
  slice can apply the same registry pattern to it).
- Existing `trg @ addr` MMIO pins — unrelated surface; ISRs bind vectors, not
  addresses.
- Non-embedded targets — `isr` on a host build compiles only if a mechanism is
  active (x86_idt row exists for completeness); the default posture still errors
  without one.
- The interpreter — ISR bodies are typechecked; execution scheduling of hardware
  vectors is a target concern, not check-mode semantics. The table emission is
  data, not control flow.

## 8. Success criteria

- All three syntax forms compile; each resolves the mechanism per §3; the
  error posture fires with a what/why/fix message when no mechanism is
  available.
- Golden-file test per mechanism row: table layout bytes, prologue shape,
  return instruction.
- Body restrictions proven at compile time (alloc/spawn/float-per-row/frame
  bound), each with a targeted test.
- `cargo test --lib` green; no new Praetor diagnostics; docs updated in the
  same commit.
