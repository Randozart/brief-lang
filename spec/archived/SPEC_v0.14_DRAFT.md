# Briv Language Specification — v0.14.0 Draft Additions

**Date:** 2026-05-16  
**Status:** Draft for discussion  
**Based on:** v0.13.0

---

## §10. Alka Escape Hatch

The `alka` block embeds raw Alka Drop packets inside a Briv transaction or definition body. Like `asm`, it is an opaque text passthrough — Briv validates brace-matching and extracts the content for the Alka backend to compile into binary Drop packets, but does not parse or validate the Alka syntax itself.

### 10.1 Grammar

```bnf
alka_block ::= "alka" "!"? "{" alka_line* "}" ";"
alka_line  ::= [^;]+ ";"
```

- `alka { ... };` — safe coordination Drop (fences, signals, watches)
- `alka! { ... };` — dangerous hardware pulse (doorbell rings, raw register writes). The `!` denotes a break from Briv's normal control flow, consistent with `asm!`

### 10.2 Placement

`alka` blocks may appear inside any body block — `node`, `defn`, struct methods. They are not valid at top-level (use `.alka` files for standalone Alka recipes) and are not expressions (Drops are fire-and-forget, not values).

### 10.3 Target Behavior

| Backend | `alka {}` handling |
|---------|--------------------|
| C / Rust / WASM | Emitted as `/* alka: ... */` comment or no-op (configurable via build flag) |
| Alka binary (Metrod) | Extracted and compiled into `.alkas` 32-byte Drop packets |
| SystemVerilog / VHDL | Error — no Alka concept in hardware targets |

When targeting C, a build flag (`--emit-alka-ffi`) can change the emitted code from a comment to a call to an external `alka_emit()` function declared via FFI.

### 10.4 Examples

```briv
// Safe coordination Drop — signal completion to orchestrator
node push_expert [src != 0][state == RESIDENT] {
    ce_dma(src, dst, size);
    alka {
        FENCE GPU_MAIN.METAPAGE == 1;
        SIGNAL EXPERT_READY;
    };
};

// Dangerous hardware pulse — ring the GPU doorbell
node ring_doorbell [gpput_valid == true] {
    alka! {
        PULSE DOORBELL @ 0x90;
    };
};
```

---

## §11. Hashtag Modifier System

Hashtags are inline modifiers that attach semantics to variables, assignments, terms, and block definitions. Each backend declares which tags it supports. Unknown advisory tags produce a warning; unsupported mandatory tags produce a compile error.

### 11.1 Grammar

```bnf
hashtag     ::= "#" identifier ("(" expression ")")?          // value tag
              | "#" identifier "{" statement* "}" ";"         // block pragma
              | "#!" identifier ("(" expression ")")?         // mandatory tag
              | "#!" identifier ("|" identifier ("(" expression ")")?)+  // fallback chain
              | "#" "[" identifier "]" hashtag                // scoped tag
```

Scoped tags (`#[target]tag`) restrict the modifier to a specific target backend.

### 11.2 Block Pragmas

The `#on_exit { ... };` block pragma registers a cleanup handler within the enclosing body. When the body exits (normally via `term` or via early exit), the cleanup executes. Multiple `#on_exit` blocks stack in LIFO order.

```briv
node claim_gpu {
    &CLAIMED = true;
    #on_exit {
        &CLAIMED = false;
    };
    dma_work();
};
```

In strict mode, the proof engine verifies that `#on_exit` cleanup does not invalidate the transaction's postcondition. If a cleanup could break the proven post-state, it is a compile error.

### 11.3 Tag Levels

| Syntax | Meaning | Backend behavior |
|--------|---------|-----------------|
| `#tag` | Advisory hint | Apply if supported. Warn if unrecognized. |
| `#!tag` | Mandatory requirement | **Error** if backend does not support this tag. |
| `#!A|B|C` | Fallback chain | Try A. If unsupported, try B. If unsupported, try C. **Error** if none supported. |
| `#[cpp]volatile` | Scoped | Only applies when targeting C++ backend. Other backends ignore. |

### 11.4 Valid Positions

Hashtags may appear in the following positions (and only these — `#` on standalone expressions is a parse error to prevent ambiguity):

1. **After `let` declarations**, before `:` type, `@` address, or `=` initializer
   ```briv
   let DOORBELL @ 0x90 #volatile : UInt32;
   let buf : Byte[4096] #!aligned(4096);
   ```

2. **After `&` assignment expressions**, before `;`
   ```briv
   &DOORBELL = token #!sfence;
   &REG = val #!sfence|volatile;
   ```

3. **After `term`**, before `;`
   ```briv
   term #gold;
   term #retry;
   ```

4. **After `}`** closing a block-based definition
   ```briv
   node push [pre][post] {
       // ...
   } #vessel;
   ```

5. **Before a variant body** (applies to that body only)
   ```briv
   node push
       [use_ce] #!direct_ce { build_pushbuffer(); }
       [use_cpu]             { memcpy(); }
       [post data_ready];
   ```

### 11.5 Backend Registry

Each backend declares its supported tags. Example:

| Backend | Supported tags |
|---------|---------------|
| C | `volatile`, `sfence`, `lfence`, `mfence`, `aligned(N)`, `packed` |
| Rust | `volatile`, `sync`, `aligned(N)`, `repr(C)`, `packed` |
| Alka (Metrod) | `direct_ce`, `p2p`, `bar1_window`, `thermal_sense` |
| SystemVerilog | `clock`, `register`, `gate`, `posedge`, `negedge` |

### 11.6 Strict Mode Interaction

In `.sbv`/`.sebv`:
- `#` advisory tags are still warnings if unrecognized
- `#!` mandatory tags are errors if unrecognized
- Scoped `#[target]` tags that don't match the current target produce an informational note (not a warning)

---

## §12. Multiple Bodies with Per-Body Preconditions

Transactions, definitions, types, and structs may have multiple body blocks, each with an optional `[pre]` condition. The semantics differ by definition kind.

### 12.1 Grammar

```bnf
variant ::= contract? "{" (statement | member)* "}"
multi   ::= variant+ ";"
```

### 12.2 Transactions and Definitions — Runtime Dispatch

For `node` and `defn`, the `[pre]` conditions are evaluated **at runtime** in declaration order. The first matching precondition executes its body.

- A shared `[post]` is declared on the first body and must be provably satisfied by all bodies
- A body without `[pre]` is the catch-all (must be last)
- Strict mode requires exhaustive coverage — the proof engine must verify all possible states are handled

```briv
node transfer_expert
    [loc == SSD] [post ready] {
        dma_from_ssd(expert);
    }
    [loc == RAM] {
        dma_from_ram(expert);
    }
    {
        // catch-all: already resident
        term;
    };
```

#### Contract Rules

- `[post]` may only appear on the first body. It is an error to repeat it on subsequent bodies.
- Each body's `[pre]` must be non-trivial in strict mode (not `[true]`).
- Bodies inherit the transaction's parameters — no special parameter syntax.
- Local `[guard] { ... }` statements inside a body work as before.

### 12.3 Types and Structs — Discriminant Variants

For `type` and `struct`, the `[pre]` condition is a **discriminant value** set at construction. The variant is selected once and fixed for the instance's lifetime.

- The discriminant value is also a member of the variant
- Use `+` to add members, `-` to remove members from the base type
- The base type definition is the default variant (when no discriminant matches)

```briv
type GPU {
    vendor: UInt16;
    bar0: Ptr;
}
[has_ce] {
    has_ce: Bool = true;
    + ce_engine: NV_C0B5;
};
```

```briv
// Base variant — no CE engine
let gpu1: GPU { vendor: 0x10DE, bar0: 0xE0000000 };

// has_ce variant — automatically includes ce_engine
let gpu2: GPU { vendor: 0x10DE, bar0: 0xE0000000, has_ce: true };
```

#### Member Access

Accessing a member that does not exist in the selected variant is a compile-time error:

```briv
let engine = gpu1.ce_engine;  // Error: ce_engine not in base GPU variant
let engine = gpu2.ce_engine;  // OK: has_ce variant includes ce_engine
```

#### Strict Mode

In `.sbv`/`.sebv`, accessing a variant-only member through a code path where the discriminant is not guaranteed is a proof error:
```briv
[gpu.has_ce] {
    let engine = gpu.ce_engine;  // OK: guarded
};
let engine = gpu.ce_engine;       // Proof error: has_ce not guaranteed here
```

---

## §13. Dynamic Address Binding

Physical address binding currently supports literal hex addresses (`@0xADDR`) and named virtual spaces (`@virtual:ADDR`). Dynamic binding extends this to runtime-resolved addresses via expressions.

### 13.1 Grammar

```bnf
address ::= "@" expression          // NEW: runtime-resolved
          | "@" hex_literal        // literal (existing)
          | "@virtual:" expression // named virtual (existing)
          | "@stack:" expression   // stack offset (existing)
          | "@heap:" expression    // heap offset (existing)
```

When `expression` is a literal (number or hex literal), the address is static and resolved at compile time. When `expression` is a variable, the address is resolved at runtime when the `@`-mapped variable is first accessed or dereferenced.

### 13.2 Runtime Resolution

```briv
let userd_ptr: Ptr = discover_userd_page();  // obtained at runtime

// Dynamic binding — the address is resolved at first access
let USERD @ userd_ptr #volatile : {
    GPGET:  UInt32,
    GPPUT:  UInt32,
    DOORBELL: UInt32
};
```

The compiler emits a pointer dereference to `userd_ptr` rather than an immediate constant. If `userd_ptr` changes between accesses, the mapping follows the current value.

### 13.3 Strict Mode Requirements

In `.sbv`/`.sebv`, the expression must be provably non-null before the mapped variable is accessed:

```briv
node use_userd [userd_ptr != 0] {
    let USERD @ userd_ptr #volatile : { ... };
    &USERD.DOORBELL = token #!sfence;
};
```

Accessing a dynamically-bound variable without a non-null guard is a proof error in strict mode.

### 13.4 Alignment Requirements

If a `#!aligned(N)` modifier is present on a dynamically-bound variable, the proof engine must verify the alignment constraint at access time:

```briv
let PB @ pb_ptr #!aligned(4096) : UInt32[128];
```

This generates a runtime alignment check in non-strict mode, or a proof obligation in strict mode.

### 13.5 Target Behavior

| Backend | Dynamic `@` |
|---------|-------------|
| C / Rust | Emitted as `type *ptr = (type*)expr;` with volatile if tagged |
| Alka | Resolved via Alka SHIFT / FLOW address operands |
| SystemVerilog | Error — FPGA synthesis requires static addresses |

---

---

## Appendix: Source Documents

This draft is based on analysis of the following existing documents:

### Briv Compiler (`briv-compiler/`)
- `spec/SPEC.md` — Master specification v0.13.0 (canonical reference)
- `spec/LANGUAGE-TUTORIAL.md` — Step-by-step language guide
- `spec/QUICK-REFERENCE.md` — Syntax reference
- `spec/old_docs/language_specs/` — Archived spec versions (v4–v8)
  - `EMBEDDED_BRIV_2.0_SPEC.md`, `EMBEDDED_BRIV_2.1_SPEC.md` — Embedded Briv (.ebv) design
- `spec/old_docs/ffi_design/` — FFI system evolution
- `spec/old_docs/hardware_design/` — Hardware validation and config guides
- `spec/old_docs/design/` — Individual design decisions (guard blocks, symbolic execution, etc.)

### VITRIOL (`VITRIOL/`)
- `docs/ARCHITECTURE.md` — Core architecture
- `docs/COPY_ENGINE_PLAN.md` — NV_C0B5 Copy Engine implementation plan
- `docs/ALKA_EXECUTOR_DESIGN.md` — Executor + ABI documentation
- `docs/VITRIOL_PROBE_SPEC.md` — Hardware probe specification
- `alka/vials/vitriol_rig.alkavl` — Hardware topology vial
- `alka-executor/vitriol_copy_engine.h` — NV_C0B5 register definitions
- `alka-executor/executor.c` — Alka stream executor
- `vitriol-daemon/vitriol.c` — Kernel module implementation

### Alka Language (`alka-lang/`)
- `docs/SPECv5.md` — Alka spec v5.1 (authoritative)
- `docs/HANDBOOK.md` — Practitioner's guide
- `docs/ROADMAP.md` — Implementation roadmap
- `docs/ALKA_INTEGRATION.md` — VITRIOL integration guide
- `docs/INSIGHTS_ARCHITECTURE.md` — Architectural insights
- `src/athanor/` — Kernel module source (C)
- `src/compiler/` — Alka compiler (Zig)

---

## Summary of Token Registry

| Token | Location | Grammar |
|-------|----------|---------|
| `alka` | Statement (inside bodies) | `"alka" "!"? "{" [^;]+ ";"* "}" ";"` |
| `#tag` | Modifier | `"#" identifier ("(" expr ")")?` |
| `#!tag` | Mandatory modifier | `"#!" identifier ("(" expr ")")?` |
| `#!A|B\|C` | Fallback chain | `"#!" id ("|" id)+` |
| `#[t]tag` | Scoped modifier | `"#[" id "]" hashtag` |
| `#on_exit { ... };` | Block pragma (inside bodies) | `"#" identifier "{" stmt* "}" ";"` |
| `@ expr` | Dynamic address binding | `"@" expression` |
| `[pre] { body }` | Multi-body variant | `"[" expr "]" "{" ... "}"` |
| `+ member` | Differential add | `"+" field_decl` |
| `- member` | Differential remove | `"-" identifier` |
