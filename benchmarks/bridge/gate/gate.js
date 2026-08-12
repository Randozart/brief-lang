// gate.js — Gate A + B for Node. Native feature_hash uses BigInt (JS numbers
// are doubles; bitwise ops truncate to 32 bits) with a 64-bit mask — the same
// 64-bit wrapping semantics as Briev's FNV-1a.
const b = require(process.argv[2]);

const MASK = (1n << 64n) - 1n;
function nativeFh(count, seed) {
    let h = BigInt(seed);
    for (let i = 0n; i < count; i++) {
        h = ((h ^ (i * 2654435761n)) * 1099511628211n) & MASK;
    }
    return h;
}
function nativeAdd(a, c) { return a + c; }

const r = BigInt(process.argv[3]);
const N = 200000, N2 = 2000000;
let sink = 0n;
b.feature_hash(1000, 42);
let t0 = process.hrtime.bigint();
for (let i = 0; i < N; i++) sink += BigInt(b.feature_hash(1000, 42));
console.log('BRIEV_FH ' + (Number(process.hrtime.bigint() - t0) / N).toFixed(1));
nativeFh(1000, 42);
t0 = process.hrtime.bigint();
for (let i = 0; i < N; i++) sink += nativeFh(1000, 42n + BigInt(i));
console.log('NATIVE_FH ' + (Number(process.hrtime.bigint() - t0) / N).toFixed(1));
b.add(3, 4);
t0 = process.hrtime.bigint();
for (let i = 0; i < N2; i++) sink += BigInt(b.add(3, 4));
console.log('BRIEV_ADD ' + (Number(process.hrtime.bigint() - t0) / N2).toFixed(2));
nativeAdd(3, 4);
t0 = process.hrtime.bigint();
for (let i = 0; i < N2; i++) sink += BigInt(nativeAdd(3, i % 8));
console.log('NATIVE_ADD ' + (Number(process.hrtime.bigint() - t0) / N2).toFixed(2));
