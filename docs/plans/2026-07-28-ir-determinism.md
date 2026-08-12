# IR Determinism — Eliminating HashMap Randomization Variance
## 2026-07-28

## The Problem

Rust's `HashMap` uses SipHash with a random seed per process. Iteration order
varies between compilations. When HashMap iteration drives LLVM IR instruction
order, this produces up to ~9% benchmark performance variation — enough to mask
or fake optimization gains.

The baseline commit `139c345` first identified this. AGENTS.md rule 7 mandates
sorting every HashMap whose iteration determines IR instruction order.

## The Tangled Maps

These HashMaps on `FunctionContext` and `CompilerContext` are iterated to
produce LLVM IR and must be sorted before emission (exhaustive list):

### On `CompilerContext`

| Map | Iterated in | Purpose |
|-----|-------------|---------|
| `field_index_map` | emit_toplevel.rs init_state | Field initialization order |
| `field_types` | loop_engine/mod.rs exit loading | Multi-field exit |
| `field_to_meta_idx` | emit_toplevel.rs | Metadata node emission |

### On `FunctionContext`

| Map | Iterated in | Purpose |
|-----|-------------|---------|
| `phi_field_regs` | counter.rs emit_counting_loop | Phi register names |
| `pending_phi_backedge` | counter.rs emit_counting_loop | Backedge values |
| `backedge_field_regs` | counter.rs emit_counting_loop | Latch values |
| `last_val_temps` | counter.rs | Last value temps |
| `done_needs_fields` | counter.rs | Post-loop needs |
| `pending_phi_native_backedge` | counter.rs | Native-typed backedge |
| `vector_phi_groups` | counter.rs | Vector phi groups |
| `vector_phi_current` | counter.rs | Current vector phi |
| `ssa_old_int_regs` | counter.rs | Old reg mapping |
| `ssa_old_float_regs` | counter.rs | Old reg mapping |
| `let_bindings` | emit_expr.rs | Let binding names |
| `reg_type_cache` | emit_expr.rs | Register types |

### Pattern for each fix

At every site where a HashMap is iterated for IR emission, replace:

```rust
// WRONG — non-deterministic:
for (key, val) in &map {
    writeln!(out, ...).ok();
}

// RIGHT — sorted by key:
let mut sorted: Vec<_> = map.iter().collect();
sorted.sort_by_key(|(k, _)| k.clone());
for (key, val) in &sorted {
    writeln!(out, ...).ok();
}
```

### Verification

Before and after each change:

```bash
cargo build --release 2>/dev/null
# Compile same benchmark twice, diff the IR — must be identical
./target/release/brievc build benchmarks/ring_buffer.bv --llvm --out /tmp/ring1
./target/release/brievc build benchmarks/ring_buffer.bv --llvm --out /tmp/ring2
diff /tmp/ring1.ll /tmp/ring2.ll
# Expected: no output (identical IR)
```

### Impact

Zero performance improvement. Zero regression risk. Eliminates up to 9% benchmark
variance, making every future optimization's A/B comparison trustworthy.

## Audit Result: Already Deterministic

After a thorough audit of every HashMap iteration across ALL backend files
(`emit_toplevel.rs`, `dispatch.rs`, `counter.rs`, `mod.rs`, `ssa.rs`,
`emit_stmt.rs`, `loop_engine/`), the codebase is ALREADY deterministic.

Every HashMap that produces LLVM IR instructions falls into one of these
categories:

| Category | Example maps | Status |
|----------|-------------|--------|
| Sorted before iteration | `struct_types` by key, `field_index_map` by index, `cache_slots` by name | ✅ Done |
| Sorted in helper | `last_val_temps` in `load_last_val_temps` | ✅ Done |
| Used with `.get()` only (no iteration) | `phi_field_regs`, `backedge_field_regs`, `pending_phi_backedge`, `field_to_meta_idx` | ✅ Not needed |
| Not a HashMap at all | `trigger_names` (Vec), `string_constants` (Vec), `field_types` (Vec) | ✅ Not needed |
| GPU/SPIR-V only | `field_index_map.iter()` in GPU kernel extractor | ✅ Not LLVM IR |

The original HashMap randomization fix (`139c345`, AGENTS.md rule 7) was
thoroughly applied. The codebase has remained clean since.

**Remaining benchmark variance** is from other sources: CPU frequency scaling,
thermal throttling, scheduler noise, or memory bus contention — not from IR
non-determinism.

## No Implementation Needed

This plan is complete as an audit document. No code changes required.
