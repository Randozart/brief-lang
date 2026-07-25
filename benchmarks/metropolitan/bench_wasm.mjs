// WASM bridge benchmark — Tier 2: WebAssembly (gen_wasm output)
// 2026-07-24: Measures per-call latency of Brief export via WASM.
// Usage: node bench_wasm.mjs

import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import path from 'path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const wasmPath = path.join(__dirname, 'out', 'bench_add.wasm');

// Load WASM directly (same as generated bridge.mjs does)
const wasm = readFileSync(wasmPath);
const mod = new WebAssembly.Module(wasm);
const instance = new WebAssembly.Instance(mod);
instance.exports.init_state();

const add = instance.exports.add;

// Warmup
let warm = add(3, 4);
if (warm !== 7) { console.error('wrong result:', warm); process.exit(1); }

// Native baseline
function native_add(a, b) { return a + b; }

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
console.log("Metropolitan FFI Benchmark — WASM");
console.log("=".repeat(60));

console.log("\n[Pure Node.js]");
run("native add", native_add, 100000, 3, 4);

console.log("\n[WASM (gen_wasm)]");
run("wasm add", add, 50000, 3, 4);

console.log("\n[Correctness]");
let n = native_add(3, 4);
let w = add(3, 4);
if (n === w) {
    console.log("  ✅ All match");
} else {
    console.log("  ❌ MISMATCH");
}
