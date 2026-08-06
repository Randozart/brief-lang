# 14. Process Boundaries and GPU Deferral (`endprogram`, `beginprogram`, `accel`)

Three keywords control the program's process boundary and where its work runs.

## `endprogram` — exit the process

`endprogram` completes the process boundary — it genuinely exits, unlike
`term`, which only ends the current transaction.

```briv
node report [count == bound][true] {
    endprogram println!(px[0]);   // print, then exit 0
};

node fail [bad][true] {
    endprogram 2;                 // exit with code 2
};
```

- `endprogram;` exits 0; `endprogram code;` exits with `code`.
- Runs `defer` cleanup via the runtime (`__exit` in `briv_rt.c`).
- A node whose precondition stays true cannot re-fire after `endprogram` —
  the process is gone. This is why `report [count == bound][true]` above is
  safe: the first firing exits before the precondition re-checks.
- `endprogram` replaced `exit program` (2026-08-06), which had been conflated
  with `term` in every backend — a node with an always-true precondition used
  to loop forever.

## `beginprogram` — the program entry (entry loop)

`beginprogram` is an optional precondition marker, true exactly once at
program start. A node `[beginprogram && <state>][<goal>]` is an **entry
loop**: it is entered once when its state conditions hold, then the body
loops until the goal. The precondition is evaluated once at entry, never
re-checked during the loop.

```briv
let startingnumber: Int = get_env_int!("env_var");
let i: Int = 0;

node entry [beginprogram && startingnumber == 1][i == 4] {
    i = i + 1;
    term;
};
```

The compiler enforces, at compile time:
- **Goal reachability** — the body must provably progress toward the goal
  (a counter the body advances, or `[true]` for a single pass). An
  unreachable goal is a compile error.
- **Entry conflict** — at most one `beginprogram` node may be eligible at
  program start; overlapping entry conditions are a compile error.

`beginprogram` is scoped to `node`. It replaces the implicit "whichever node
fires first" entry with an explicit one.

## `accel` — defer a counted loop to the GPU

`accel` marks a native counted loop as a **parallel map over work-items**.
The work-item index is a real state counter — no virtual variables.

```briv
let i: Int = 0;
accel node force [i < nb][i == nb] {
    dv[i] = force_on(i);     // per-work-item compute (disjoint affine write)
    i = i + 1;               // counted-loop advance
    term;
};
```

- `[i < N]` is the loop bound; `[i == N]` is the goal ("loop until true").
- The compiler **proves** the map (disjoint per-`i` writes, counter advance,
  pure, flat types). If the proof fails, the body runs on CPU with a remark.
- On a GPU it launches N work-items once and fast-forwards the counter; on
  CPU the counted loop runs natively (each firing = one work-item).
- **Verifiable speedup only**: in `try` mode the compiler verifies the GPU
  path is faster (statically for known N, or via a runtime auto-tuning probe
  that measures both lanes and checks output equality) before deferring.
- Module shortcut: `!> accel: try_all;` makes every eligible body a candidate;
  `force;` requires keyword-marked bodies to offload (errors on ineligible);
  `try_all_force;` combines both. See SPEC §9.7.

```briv
!> accel: try_all;

let i: Int = 0;
accel node init_bodies [beginprogram && i < nb][i == nb] {
    px[i] = i as Float * 0.1 + 0.5;
    i = i + 1;
    term;
};
```

Here `beginprogram` marks the entry, and `accel` offloads the init pass.
