// 2026-07-25: Debug version of vm_execute with trace

#include "interp.h"
#include <stdio.h>

uint64_t vm_execute_dbg(VmState* vm, uint32_t fn_idx, int depth) {
    vm->has_error = 0;
    if (fn_idx >= vm->function_count) {
        snprintf(vm->error_buf, sizeof(vm->error_buf), "fn index %u out of range", fn_idx);
        vm->has_error = 1; return 0;
    }
    LairFunction* fn = &vm->function_table[fn_idx];
    // Push initial frame
    vm->locals_len = 0;
    vm->frame_count = 0;
    if (push_frame(vm, fn->local_count) != 0) return 0;
    // Copy arguments from stack into local slots
    for (uint16_t i = 0; i < fn->arg_count; i++) {
        uint64_t arg = stack_pop(vm);
        vm->locals[vm->locals_len - fn->local_count + fn->arg_count - 1 - i] = arg;
    }
    const uint8_t* pc = vm->bytecode + fn->bytecode_offset;
    const uint8_t* end = pc + fn->bytecode_len;
    while (pc < end && !vm->has_error) {
        uint8_t op = read_u8(pc);
        switch (op) {
            case OP_CALL: {
                uint16_t callee_idx = read_u16(pc + 1);
                if (callee_idx >= vm->function_count) { vm->has_error = 1; return 0; }
                LairFunction* callee = &vm->function_table[callee_idx];
                Frame* cur = &vm->frames[vm->frame_count - 1];
                cur->return_pc = pc + 3;
                cur->return_frame_idx = vm->frame_count - 1;
                if (push_frame(vm, callee->local_count) != 0) return 0;
                for (uint16_t i = 0; i < callee->arg_count; i++) {
                    uint64_t arg = stack_pop(vm);
                    vm->locals[vm->locals_len - callee->local_count + callee->arg_count - 1 - i] = arg;
                }
                // Debug
                fprintf(stderr, "%*sCALL fn=%u (args=%u, locals=%u) stack=%zu frame_count=%zu\n",
                        depth*2, "", callee_idx, callee->arg_count, callee->local_count,
                        vm->stack_len, vm->frame_count);
                // Recurse
                uint64_t result = vm_execute_dbg(vm, callee_idx, depth + 1);
                fprintf(stderr, "%*sRET from fn=%u = %lu\n", depth*2, "", callee_idx, (unsigned long)result);
                if (!vm->has_error) stack_push(vm, result);
                pc = pc + 3; // Skip past call instruction to continue
                continue; // Don't execute, we already recursed and got result
            }
            case OP_RET: {
                uint64_t retval = 0;
                if (vm->stack_len > 0) retval = stack_pop(vm);
                pop_frame(vm);
                return retval;
            }
            default:
                // Unhandled opcode — just skip (1 byte)
                pc += 1;
                break;
        }
    }
    return 0;
}
