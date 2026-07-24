// Node.js bridge benchmark via koffi FFI
// Usage: node benchmarks/multi_lang/bench_node.mjs

import koffi from 'koffi';
import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import path from 'path';
import fs from 'fs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const buildDir = path.resolve(__dirname, '../../target/multi_lang');
const soPath = path.join(buildDir, 'export_add.so');
const shimPath = path.join(buildDir, 'proto_shim');

// ── 1. Direct FFI ──────────────────────────────────────────────────────

let lib;
try {
    lib = koffi.load(soPath);
} catch (e) {
    console.error(`Failed to load ${soPath}:`, e.message);
    process.exit(1);
}

const add_ffi = lib.func('add', 'int64_t', ['int64_t', 'int64_t']);

function bench_ffi(a, b) {
    return add_ffi(a, b);
}

// ── 2. Protocol bridge subprocess ──────────────────────────────────────

function bench_protocol(a, b) {
    return new Promise((resolve, reject) => {
        const child = spawn(shimPath, [], { stdio: ['pipe', 'pipe', 'pipe'] });
        let stdout = '';
        child.stdout.on('data', (data) => { stdout += data.toString(); });
        child.on('close', (code) => {
            if (code !== 0) reject(new Error(`shim exited ${code}`));
            else resolve(parseInt(stdout.trim(), 10));
        });
        child.on('error', reject);
        child.stdin.write(`add ${a} ${b}\n`);
        child.stdin.end();
    });
}

// ── 3. Native reference ────────────────────────────────────────────────

function bench_native(a, b) {
    return a + b;
}

// ── Benchmark runner ────────────────────────────────────────────────────

function run_bench(name, fn, iterations = 10000, ...args) {
    // Warmup
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
    const mean = times.reduce((s, v) => s + v, 0) / times.length;
    const min = times[0];
    const max = times[times.length - 1];
    const fmt = (n) => n.toString().padStart(8);

    console.log(`  ${name.padEnd(30)}  median=${fmt(median)}ns  mean=${fmt(mean)}ns  min=${fmt(min)}ns  max=${fmt(max)}ns  result=${result}`);
}

async function run_bench_async(name, fn, iterations = 100, ...args) {
    let result = await fn(...args);

    let times = [];
    for (let i = 0; i < iterations; i++) {
        const t0 = process.hrtime.bigint();
        await fn(...args);
        const t1 = process.hrtime.bigint();
        times.push(Number(t1 - t0));
    }

    times.sort((a, b) => a - b);
    const median = times[Math.floor(times.length / 2)];
    const mean = times.reduce((s, v) => s + v, 0) / times.length;
    const min = times[0];
    const max = times[times.length - 1];
    const fmt = (n) => n.toString().padStart(8);

    console.log(`  ${name.padEnd(30)}  median=${fmt(median)}ns  mean=${fmt(mean)}ns  min=${fmt(min)}ns  max=${fmt(max)}ns  result=${result}`);
}

async function main() {
    console.log("=".repeat(65));
    console.log("Multi-Language Bridge Benchmark — Brief export defn from Node.js");
    console.log("=".repeat(65));

    const a = 3, b = 4;

    // Native reference
    console.log("\n[Pure Node.js]");
    run_bench("native add", bench_native, 50000, a, b);

    // Direct FFI via koffi
    console.log("\n[Node.js koffi FFI]");
    run_bench("koffi add", bench_ffi, 10000, a, b);

    // Protocol bridge subprocess
    if (fs.existsSync(shimPath)) {
        console.log("\n[Protocol Bridge (subprocess)]");
        await run_bench_async("proto_shim add", bench_protocol, 100, a, b);
    } else {
        console.log(`\n  (no proto shim at ${shimPath}, skipping)`);
    }

    // Correctness
    const ffi_result = bench_ffi(a, b);
    const native_result = bench_native(a, b);
    console.log(`\n[Correctness]`);
    console.log(`  koffi:   ${ffi_result}`);
    console.log(`  native:  ${native_result}`);
    if (fs.existsSync(shimPath)) {
        const proto_result = await bench_protocol(a, b);
        console.log(`  proto:   ${proto_result}`);
        if (ffi_result === native_result && ffi_result === proto_result) {
            console.log("  ✅ All match");
        } else {
            console.log("  ❌ MISMATCH");
        }
    } else {
        if (ffi_result === native_result) {
            console.log("  ✅ koffi == native");
        } else {
            console.log("  ❌ MISMATCH");
        }
    }
}

main().catch(console.error);
