// brief_bridge — Brief koffi bridge (auto-generated)
// Runtime: ~280ns per call

import koffi from 'koffi';

const lib = koffi.load('./export_add.so');

const _add = lib.func('add', 'int64_t', ['int64_t', 'int64_t']);
export function add(a0, a1) {
    return _add(a0, a1);
}

const _mul = lib.func('mul', 'int64_t', ['int64_t', 'int64_t']);
export function mul(a0, a1) {
    return _mul(a0, a1);
}

