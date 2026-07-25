// brief_bridge — Brief WASM bridge (auto-generated)
// Overhead: ~75ns per call

import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import path from 'path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

let _exports = null;

export async function init() {
    const wasmPath = path.join(__dirname, 'brief_bridge.wasm');
    const wasm = readFileSync(wasmPath);
    const mod = new WebAssembly.Module(wasm);
    const instance = new WebAssembly.Instance(mod);
    instance.exports.init_state();
    _exports = instance.exports;
}

export function add(a0, a1) {
    return _exports.add(a0, a1);
}

export function mul(a0, a1) {
    return _exports.mul(a0, a1);
}

