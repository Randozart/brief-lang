#!/usr/bin/env python3
"""Generate a SystemVerilog testbench for an emitted CIRCT module
(2026-08-25, Plan 3.5 tier 3 — simulation parity).

Reads the port list from the hw.module @top header of the .mlir, emits
`module tb` that drives every input (clock toggles; reset starts high,
drops after edge 1; other inputs sit at 0), clocks the DUT CYCLES times,
then prints "<cycle> <output ports in declaration order>" after every
edge >= 2. Output format is locked by the per-fixture .expect files
(see tmp_fixtures/hw/*.expect.gen.py).

Usage: hw_sim_tb.py <file.mlir> <cycles>
"""
import re
import sys

def main() -> None:
    mlir_path, cycles = sys.argv[1], int(sys.argv[2])
    header = ""
    with open(mlir_path) as f:
        for line in f:
            if line.startswith("hw.module @top("):
                header = line
                break
    outs = re.findall(r"out ([A-Za-z_][A-Za-z_0-9]*): i([0-9]+)", header)
    if not outs:
        sys.exit("no output ports found in module header")
    ins = re.findall(r"in %([A-Za-z_][A-Za-z_0-9]*): ([^,)]+)", header)

    lines: list[str] = ["module tb;", "  reg clock = 0;"]
    driven = {"clock"}
    for name, ty in ins:
        if name in driven:
            continue  # clock reg already declared
        m = re.fullmatch(r"i([0-9]+)", ty.strip())
        width = (int(m.group(1)) - 1) if m else 0
        init = "1'b1" if name == "reset" else "'0"
        rng = f"[{width}:0] " if width else ""
        lines.append(f"  reg {rng}{name} = {init};")
        driven.add(name)
    for name, width in outs:
        lines.append(f"  wire [{int(width) - 1}:0] {name};")
    conns = ", ".join(
        [f".{n}({n})" for n, _ in ins] + [f".{n}({n})" for n, _ in outs]
    )
    lines.append(f"  top dut({conns});")
    lines.append("  integer i;")
    lines.append("  initial begin")
    lines.append(f"    for (i = 0; i < {cycles}; i = i + 1) begin")
    lines.append("      #4 clock = 1;")
    lines.append("      #4 clock = 0;")
    lines.append("      if (i == 1) reset <= 1'b0;")
    fmt = " ".join(["%0d"] * (len(outs) + 1))
    args = ", ".join(["i"] + [n for n, _ in outs])
    lines.append(f'      if (i >= 2) $display("{fmt}", {args});')
    lines.append("    end")
    lines.append("    $finish;")
    lines.append("  end")
    lines.append("endmodule")
    print("\n".join(lines))

if __name__ == "__main__":
    main()
