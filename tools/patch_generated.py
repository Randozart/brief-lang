#!/usr/bin/env python3
"""Patch SeqToSV output for the brievc pipeline (seq-firmem plan, 2026-08-25).

`circt-opt --lower-seq-to-sv` lowers `seq.firmem` ops to instances of an
EXTERNALLY-generated module (`hw.module.generated ... @FIRRTLMem`). Upstream,
the macro body is emitted by firtool's flow — standalone circt-opt REJECTS
the op at export (BUGS.md 2026-08-25). We rewrite each generated op into a
plain `hw.module.extern` with the identical port list; brievc emits the
reference implementation as a companion `.sv`, linked by the harness.

No-op when the IR has no generated modules (all non-mem fixtures).
"""
import re
import sys

def main() -> None:
    src = open(sys.argv[1]).read()
    pat = re.compile(
        r"hw\.module\.generated @(\w+), @\w+\(([^)]*)\)\s+attributes\s*\{[^}]*\}"
    )
    out, n = pat.subn(lambda m: f"hw.module.extern @{m.group(1)}({m.group(2)})", src)
    open(sys.argv[2], "w").write(out)
    if n:
        print(f"patched {n} generated module(s)")

if __name__ == "__main__":
    main()
