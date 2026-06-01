# Per-Field SSA Register Promotion — Replace Struct SSA with Per-Field Load/Store

## Problem
The Kalman filter benchmark (12 float fields + 2 int fields = 14 total) runs
**1.87× slower** than C (1.214s vs 0.649s). Root cause: the current struct-SSA
approach (`load %State` → `extractvalue`/`insertvalue` chains → `store %State`)
forces LLVM's code generator to rematerialize per-field GEP loads from
`@global_state` inside the hot loop (69 loads vs C's 9), causing 36 stack spills.

The IIR benchmark (4 float fields) has no spill → parity with C. The boundary
is between 4 and 12 fields.

## Solution
Replace the single `%State` struct load/store with individual per-field loads
and stores. Each field becomes its own SSA value with its own phi node.
LLVM's `mem2reg` pass promotes these naturally — no struct aliasing, no
`volatile` interference, no spill.

### Data structure changes
```
- ssa_state_reg: Option<String>
+ field_ssa_regs: HashMap<String, String>   // field name → current SSA register
+ field_ssa_dirty: HashSet<String>           // fields modified since last store
```

### Three-phase body emission (emit_folded_loop SSA body path)

**Phase 1 — Scan body for field usage** (before emitting any IR):
Walk statements left-to-right, stop at `Term`/`Escape` (honors `terminated`).
Collect:
- `reads: HashSet<String>` — fields on RHS of any expression
- `writes: HashSet<String>` — fields on LHS of `&x = ...`
- `loop_carried = reads ∩ writes` — need phi nodes across back-edge

**Phase 2 — Emit per-field loads** (in the entry block, before loop):
```
%x0_init = load float, float* getelementptr %State @global_state, 0, 2
%count_init = load i64, i64* getelementptr %State @global_state, 0, 1
```
Populate `field_ssa_regs` with all read fields.

**Phase 3 — Emit loop header with per-field phis** (for loop-carried only):
```
hdr:
  %x0 = phi float [%x0_init, entry], [%x0_next, body]
  %count = phi i64 [%count_init, entry], [%count_next, body]
```

Then emit body (uses/updates `field_ssa_regs`, marks dirty in
`field_ssa_dirty`), then after loop exit emit per-field stores:
```
done:
  store float %x0_next, float* getelementptr %State @global_state, 0, 2
  store i64 %count_next, i64* getelementptr %State @global_state, 0, 1
```

### emit_ssa_main changes
Tick-tock structure with per-field phi merge after precondition guards:
```
  tick:
    ; Individual per-field loads
    %x0 = load float, float* getelementptr %State @global_state, 0, 2
    %count = load i64, i64* getelementptr %State @global_state, 0, 1
    field_ssa_regs = {"x0": %x0, "count": %count, ...}

    ; Txn with precondition → branch + body + per-field phi merge
    br %cond, %body, %skip
    body:  ... update field_ssa_regs ...
    skip:
    %x0_merge = phi float [%x0_body, %body], [%x0_pre, %skip]
    %count_merge = phi i64 [%count_body, %body], [%count_pre, %skip]
    field_ssa_regs = {"x0": %x0_merge, "count": %count_merge, ...}

    ; Per-field stores
    store float %x0_merge, float* getelementptr %State @global_state, 0, 2
    store i64 %count_merge, i64* getelementptr %State @global_state, 0, 1

    ; Exit check
    %done = icmp ...  →  br %done, %tick, done
```

### emit_expr changes (line 1734)
```
- if ssa_state_reg.is_some() && field_index_map.contains(name):
-     extractvalue + bitcast + zext  (6 instructions per field read)
+ if field_ssa_regs.contains(name):
+     return field_ssa_regs[name]    (0 instructions — register already holds the value)
```

### emit_stmt changes (line 1556)
For `&x = expr`:
```
- if ssa_state_reg.is_some() && field_index_map.contains(&fname) && !is_volatile:
-     trunc/bitcast → insertvalue → chain ssa_state_reg
+ if field_ssa_regs.contains(&fname) && !is_volatile:
+     trunc/bitcast field value to native type
+     field_ssa_regs.insert(fname, new_reg)
+     field_ssa_dirty.insert(fname)
+     return  // defer GEP + store to loop exit
```

For `Guarded`:
```
- if statements.len() == 1 && ssa_state_reg.is_none()
+ if statements.len() == 1 && field_ssa_regs.is_empty()
```

### Edge cases
| Case | Handling |
|------|----------|
| Read-only field | Load once, use throughout, no phi, no dirty, no store |
| Write-only field | No initial load, just dirty mark + final store |
| No precondition (unconditional body) | Skip phi — no merge needed |
| Term/Escape mid-body | Scan stops at first term; writes after it not counted |
| emit_ssa_main multi-txn | Union of writes across ALL reactive txns determines phi set |
| Guard→select | Disabled when `field_ssa_regs` non-empty |

### Acceptance Criteria
1. Kalman: **<0.75s** (<15% of C's 0.649s)
2. IIR: stays at **parity** (0.156s)
3. All other benchmarks unchanged (ring_buffer 0.001s, async_counters 0.001s)
4. **Zero GEP loads from @global_state in Kalman hot loop** (vs current 69)
5. 362 tests pass
6. No regression in `#!exit` warnings
