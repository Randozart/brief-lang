#!/bin/bash
# run_gate.sh — the zero-friction FFI gate.
# Builds bench.bv, then runs Gate A (Brief feature_hash vs native) and
# Gate B (Brief add vs native internal) for every host whose toolchain is
# present. Toolchains: cc/g++/go (PATH or ~/brief-tools/go), javac (PATH or
# ~/brief-tools/jdk-*), lua (~/brief-tools/lua-*/src/lua), python3, node.
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
SEED=42

briefc="$ROOT/target/debug/briefc"
[ -x "$briefc" ] || briefc="$ROOT/target/release/briefc"
BV="$ROOT/examples/glue-host/bench.bv"

echo "== build bridge =="
BC() { (cd "$ROOT" && "$briefc" "$@") || exit 1; }
BC build "$BV" --library --out "$WORK"
BC bindings "$BV" c --out "$WORK" >/dev/null
for lang in go java lua python node; do
    BC export "$BV" "$lang" --out "$WORK" >/dev/null 2>&1
done
BC extension "$BV" python --out "$WORK" >/dev/null 2>&1
BC extension "$BV" node --out "$WORK" >/dev/null 2>&1
BC extension "$BV" java --out "$WORK" >/dev/null 2>&1
BC extension "$BV" lua --out "$WORK" >/dev/null 2>&1

median() { # median of the numbers on stdin (one per line)
    sort -n | awk '{a[NR]=$1} END{print (NR%2?a[(NR+1)/2]:(a[NR/2]+a[NR/2+1])/2)}'
}
collect() { # label; then the driver prints BRIEF_FH etc. Runs in $WORK. 3 interleaved rounds, median.
    local label="$1"; shift
    local bf nf ba na
    for r in 1 2 3; do
        local out
        out=$( (cd "$WORK" && "$@") 2>/dev/null)
        echo "$out" | awk '/BRIEF_FH/{print $2}' >> "$WORK/m.bf"
        echo "$out" | awk '/NATIVE_FH/{print $2}' >> "$WORK/m.nf"
        echo "$out" | awk '/BRIEF_ADD/{print $2}' >> "$WORK/m.ba"
        echo "$out" | awk '/NATIVE_ADD/{print $2}' >> "$WORK/m.na"
    done
    bf=$(median < "$WORK/m.bf"); nf=$(median < "$WORK/m.nf")
    ba=$(median < "$WORK/m.ba"); na=$(median < "$WORK/m.na")
    printf "%-6s %8s %8s %6.2f %8s %8s %6.2f\n" \
        "$label" "$bf" "$nf" "$(awk "BEGIN{print $bf/$nf}")" "$ba" "$na" "$(awk "BEGIN{print $ba/$na}")"
    rm -f "$WORK/m.bf" "$WORK/m.nf" "$WORK/m.ba" "$WORK/m.na"
}

echo "== gate (Brief vs native, ns/call) =="
printf "%-6s %9s %9s %7s %9s %9s %7s\n" "host" "BriefFH" "NatFH" "FHratio" "BriefAdd" "NatAdd" "Addratio"

# C
if command -v cc >/dev/null && command -v clang >/dev/null; then
    cc -O3 -o "$WORK/gate_c" "$HERE/gate_c.c" "$WORK/libbench.a" && collect C "$WORK/gate_c" "$SEED"
fi
# C++
if command -v g++ >/dev/null; then
    g++ -O3 -o "$WORK/gate_cpp" "$HERE/gate_cpp.cpp" "$WORK/libbench.a" && collect C++ "$WORK/gate_cpp" "$SEED"
fi
# Go — Brief side (cgo) + native side (pure Go, CGO=0; a cgo-linked binary
# produced bogus sub-1ns/iter native numbers).
go="go"; [ -x "$HOME/brief-tools/go/bin/go" ] && go="$HOME/brief-tools/go/bin/go"
if command -v "$go" >/dev/null 2>&1; then
    mkdir -p "$WORK/gogo" && cp "$WORK/libbench.a" "$WORK/gogo/" && cp "$HERE/gate.go" "$WORK/gogo/" \
      && printf 'module gatego\n\ngo 1.22\n' > "$WORK/gogo/go.mod" \
      && (cd "$WORK/gogo" && CGO_ENABLED=1 "$go" run . "$SEED" | awk '/BRIEF_FH|BRIEF_ADD/{print}') > "$WORK/go_out" 2>/dev/null \
      && mkdir -p "$WORK/gonat" && cp "$HERE/gate_go_native.go" "$WORK/gonat/" \
      && printf 'module gonat\n\ngo 1.22\n' > "$WORK/gonat/go.mod" \
      && (cd "$WORK/gonat" && CGO_ENABLED=0 "$go" run . "$SEED" | awk '/NATIVE_FH|NATIVE_ADD/{print}') > "$WORK/gonat_out" 2>/dev/null \
      && bf=$(awk '/BRIEF_FH/{print $2}' "$WORK/go_out") && nf=$(awk '/NATIVE_FH/{print $2}' "$WORK/gonat_out") \
      && ba=$(awk '/BRIEF_ADD/{print $2}' "$WORK/go_out") && na=$(awk '/NATIVE_ADD/{print $2}' "$WORK/gonat_out") \
      && printf "%-6s %8s %8s %6.2f %8s %8s %6.2f\n" "Go" "$bf" "$nf" "$(awk "BEGIN{print $bf/$nf}")" "$ba" "$na" "$(awk "BEGIN{print $ba/$na}")"
fi
# Java
JAVAC=javac; JAVA=java
J=$(ls -d "$HOME"/brief-tools/jdk-*/bin 2>/dev/null | head -1)
if [ -n "$J" ]; then JAVAC="$J/javac"; JAVA="$J/java"; fi
if [ -x "$JAVAC" ]; then
    cp "$HERE/Gate.java" "$WORK/" \
      && (cd "$WORK" && "$JAVAC" Gate.java && "$JAVA" -Djava.library.path="$WORK" bench "$SEED") > "$WORK/java_out" 2>/dev/null \
      && bf=$(awk '/BRIEF_FH/{print $2}' "$WORK/java_out") && nf=$(awk '/NATIVE_FH/{print $2}' "$WORK/java_out") \
      && ba=$(awk '/BRIEF_ADD/{print $2}' "$WORK/java_out") && na=$(awk '/NATIVE_ADD/{print $2}' "$WORK/java_out") \
      && printf "%-6s %8s %8s %6.2f %8s %8s %6.2f\n" "Java" "$bf" "$nf" "$(awk "BEGIN{print $bf/$nf}")" "$ba" "$na" "$(awk "BEGIN{print $ba/$na}")"
fi
# Lua
LUA=$(ls "$HOME"/brief-tools/lua-*/src/lua 2>/dev/null | head -1)
if [ -n "$LUA" ] && [ -x "$LUA" ]; then
    collect Lua "$LUA" -e "package.cpath='$WORK/?.so'" "$HERE/gate.lua" "$WORK" "$SEED"
fi
# Python
if command -v python3 >/dev/null; then
    cp "$WORK/bench.cpython"*.so "$WORK/bench.so" 2>/dev/null
    collect Py python3 "$HERE/gate.py" "$SEED" "$WORK"
fi
# Node
if command -v node >/dev/null; then
    collect Node node "$HERE/gate.js" "$WORK/bench.node" "$SEED"
fi
