#include "../interp.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

// Test: function with arguments.
// Build a .lair with function 0 = add(a, b) -> a + b.
// Bytecode: load_local 0 (a), load_local 1 (b), add, ret
static uint8_t* build_add_lair(size_t* out_size) {
    uint8_t bc[] = {
        OP_LOAD_LOCAL, 0,   // a
        OP_LOAD_LOCAL, 1,   // b
        OP_ADD,
        OP_RET,
    };
    size_t bc_len = sizeof(bc);

    LairFunction fn;
    fn.name_idx = 0;
    fn.bytecode_offset = 0;
    fn.bytecode_len = (uint32_t)bc_len;
    fn.local_count = 2;
    fn.arg_count = 2;

    const char* strings = "add\0";
    size_t str_len = 4;

    size_t hdr = LAIR_HEADER_SIZE;
    size_t str_off = hdr;
    size_t fn_off = str_off + str_len;
    size_t fn_sz = sizeof(fn);
    size_t bc_off = fn_off + fn_sz;
    size_t total = bc_off + bc_len;

    uint8_t* buf = (uint8_t*)calloc(total, 1);
    memcpy(buf, "LAIR", 4);
    uint32_t ver = 1; memcpy(buf + 4, &ver, 4);
    buf[8] = LAIR_ENDIAN_LE;
    memcpy(buf + 16, &str_off, 8);
    uint64_t tmp = str_len; memcpy(buf + 24, &tmp, 8);
    memcpy(buf + 32, &fn_off, 8);
    tmp = fn_sz; memcpy(buf + 40, &tmp, 8);
    memcpy(buf + 48, &bc_off, 8);
    tmp = bc_len; memcpy(buf + 56, &tmp, 8);
    memcpy(buf + str_off, strings, str_len);
    memcpy(buf + fn_off, &fn, fn_sz);
    memcpy(buf + bc_off, bc, bc_len);

    *out_size = total;
    return buf;
}

int main(void) {
    size_t sz;
    uint8_t* lair = build_add_lair(&sz);
    assert(lair);

    VmState vm;
    assert(vm_init(&vm) == 0);
    assert(vm_load_lair(&vm, lair, sz) == 0);

    // Push arguments (right-to-left): b=4, a=3
    vm.stack[vm.stack_len++] = 4;
    vm.stack[vm.stack_len++] = 3;

    uint64_t result = vm_execute(&vm, 0);
    assert(!vm.has_error && vm_error(&vm) == NULL);
    assert(result == 7 && "expected add(3, 4) = 7");
    printf("test_add PASSED: add(3, 4) = %lu\n", (unsigned long)result);

    vm_free(&vm);
    free(lair);
    return 0;
}
