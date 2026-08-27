#!/usr/bin/env python3
"""Regenerate .expect for uart_extern (cbv-HW plan Slice A).

Models the FSM cycle-exactly like the other fixtures (preset init, reset
edges 0..1, TB prints ports in declaration order halt, check, <vars
sorted> after every edge >= 2):

  run node fires exactly once under [done == 0]: ticks 0->1, done 0->1;
  post [done == 1] provable on that single commit, so check holds. From
  the next edge halt rises and both vars hold — same row-count
  convention as the counter generator (CYCLES edges).
"""
CYCLES = 6

rows = []
for edge in range(2, CYCLES + 1):
    if edge == 2:
        halt, check, done, ticks = 0, 1, 1, 1
    else:
        halt = 1
    rows.append(f"{halt} {check} {done} {ticks}")
print("\n".join(rows))
