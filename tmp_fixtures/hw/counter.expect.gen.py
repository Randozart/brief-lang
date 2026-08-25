#!/usr/bin/env python3
"""Regenerate .expect files for the HW corpus (2026-08-23, Plan 3.5).

Models the emitted FSM cycle-exactly: registers power on at preset
(init), reset holds init for edges 0..1, the testbench prints ports in
declaration order (halt, check, <vars sorted>) after every edge >= 2.

Interpreter cross-check (Rule 5 — interpreter IS reference):
  counter: txn step fires once per cycle -> counter_k = k until pre
           [counter < 255] refuses; refusal HOLDS state (contract
           gating) and raises halt. Sim must show 0,1,...,255 then hold,
           halt rising the cycle after the bound is hit. Post is
           [counter <= 255]: one application under pre commits at most
           255, so check never drops (inconsistent pairs DO drop it —
           that's the violation signal).
  adder:   a += 3 per accepted cycle under [a < 87]: 0,3,...,87 then
           hold + halt. Post [a < 90] provable on every accepted commit
           (max committed = 86+3=89), so check never drops.
Run from repo root: python3 tmp_fixtures/hw/counter.expect.gen.py
"""
CYCLES = 270

def gen(name, vars_init, step_fn, wd=None):
    # wd = (cond_fn, bound): liveness watchdog model. Counter reloads on
    # cond, saturates at 0; tmo printed value = NOT cond AND cnt==0 taken
    # on the state BEFORE the edge (same sampling as halt), matching the
    # registered tmo port. Port order: halt, check, wd_tmo, <vars>.
    lines = []
    state = dict(vars_init)
    halt, check = 0, 1
    cnt = wd[1] if wd else 0
    for i in range(CYCLES):
        reset = i <= 1
        pre_ok, committed = step_fn(state)
        cond = wd[0](state) if wd else True
        tmo_val = int((not cond) and cnt == 0)
        if reset:
            state = dict(vars_init)
            halt, check = 0, 1
        else:
            state.update(committed)
            halt = int(not pre_ok)
            check = 1
            if wd:
                cnt = wd[1] if cond else max(cnt - 1, 0)
        if i >= 2:
            vals = [halt, check, tmo_val] if wd else [halt, check]
            for k in sorted(state):
                # arrays flatten to lanes in index order (port order)
                vals += list(state[k]) if isinstance(state[k], tuple) else [state[k]]
            lines.append(f"{i} " + " ".join(str(v) for v in vals))
    open(f"tmp_fixtures/hw/{name}.expect", "w").write("\n".join(lines) + "\n")

# counter: c = c+1 gated by [c < 255]; post [c <= 255] always holds
def step_counter(s):
    ok = s["counter"] < 255
    return ok, {"counter": s["counter"] + 1} if ok else {}

# adder: a = a+b gated by [a < 87]; b never written
def step_adder(s):
    ok = s["a"] < 87
    return ok, {"a": s["a"] + s["b"]} if ok else {}

# array: buf[idx] = idx under [idx < 4]; lanes init 7, hold+halt at bound
def step_array(s):
    ok = s["idx"] < 4
    if not ok:
        return False, {}
    b = list(s["buf"])
    b[s["idx"]] = s["idx"]
    return True, {"buf": tuple(b), "idx": s["idx"] + 1}

# watchdog: beat ticks to 6 then holds; ![beat < 5] within 2cyc breaches
# once the condition has been false for the whole budget.
def step_wd(s):
    ok = s["beat"] < 6
    return ok, {"beat": s["beat"] + 1} if ok else {}

gen("counter", {"counter": 0}, step_counter)
gen("adder", {"a": 0, "b": 3}, step_adder)
gen("array", {"buf": (7, 7, 7, 7), "idx": 0}, step_array)
gen("watchdog", {"beat": 0}, step_wd, wd=(lambda s: s["beat"] < 5, 2))
print("expect files regenerated")
