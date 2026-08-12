// Node.js bridge benchmark — Tier 2: koffi ESM (gen_node output)
// 2026-07-24: Measures per-call latency of Briev export via koffi FFI.
// Usage: node bench_node.mjs

import koffi from 'koffi';
import { fileURLToPath } from 'url';
import path from 'path';
import fs from 'fs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const outDir = path.join(__dirname, 'out');
const soPath = path.join(outDir, 'bench_add.so');

const lib = koffi.load(soPath);
const add = lib.func('add', 'int64_t', ['int64_t', 'int64_t']);

// Warmup
let warm = add(3, 4);
if (Number(warm) !== 7) { console.error('wrong result:', warm); process.exit(1); }

// Native baseline
function native_add(a, b) { return a + b; }

// Benchmark
function run(name, fn, iterations, ...args) {
    let result = fn(...args);
    let times = [];
    for (let i = 0; i < iterations; i++) {
        const t0 = process.hrtime.bigint();
        fn(...args);
        const t1 = process.hrtime.bigint();
        times.push(Number(t1 - t0));
    }
    times.sort((a, b) => a - b);
    const median = times[Math.floor(times.length / 2)];
    console.log(`  ${name.padEnd(30)}  median=${median.toString().padStart(8)}ns  result=${result}`);
}

console.log("=".repeat(60));
console.log("Metropolitan FFI Benchmark — Node.js");
console.log("=".repeat(60));

console.log("\n[Pure Node.js]");
run("native add", native_add, 100000, 3, 4);

console.log("\n[koffi FFI (gen_node)]");
run("koffi add", add, 50000, 3, 4);

console.log("\n[Correctness]");
let n = native_add(3, 4);
let k = add(3, 4);
console.log(`  native:  ${n}`);
console.log(`  koffi:   ${k}`);
if (n === Number(k)) {
    console.log("  ✅ All match");
} else {
    console.log("  ❌ MISMATCH");
}
