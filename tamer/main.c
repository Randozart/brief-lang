// 2026-07-25: tamer — Brief install-time compiler system tool.
// Reads a .bounty file, extracts .lair + .beastpack, executes the
// compilation passes via the VM, and produces a native binary.
//
// Usage: tamer <bounty_file> [-o <output_dir>]
//
// This is the platform-specific component in the Bounty pipeline.
// Installed once per platform via package manager (apt, brew, winget).

#include "interp.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/wait.h>

// ── .bounty format constants ─────────────────────────────────────────────
#define BOUNTY_MAGIC       "BOUNDATA\0"
#define BOUNTY_MAGIC_LEN   9
#define SECTION_LAIR       1
#define SECTION_BEASTPACK  2
#define SECTION_MANIFEST   3
#define SECTION_USER_LAIR  4

// ── Host FFI stubs (MVP) ─────────────────────────────────────────────────
// These will be replaced with real LLVM/linker bindings in a future phase.
// For now, they log and return reasonable defaults.

static void host_log(uint64_t* args, int n) {
    // args[0] = string table index (the message)
    fprintf(stderr, "[tamer] host_log: arg0=%lu (n=%d)\n", (unsigned long)args[0], n);
}

static void host_cpuid(uint64_t* args, int n) {
    // Return CPU feature bitmask: assume x86_64 with basic features
    args[0] = 0x07; // bits: SSE, SSE2, AVX
    (void)n;
}

static void host_os_abi(uint64_t* args, int n) {
    // Return OS identifier: 0 = Linux
    args[0] = 0;
    (void)n;
}

// ── Host LLVM IR emission buffer ────────────────────────────────────────
// Accumulates LLVM IR text segments and writes to .ll file at the end.

#define LLVM_IR_BUFFER_SIZE (1024 * 1024)  // 1MB buffer
static char ir_buffer[LLVM_IR_BUFFER_SIZE];
static size_t ir_buffer_pos = 0;

static void host_llvm_emit(uint64_t* args, int n) {
    // args[0] = pointer to IR text string (null-terminated)
    const char* text = (const char*)(uintptr_t)args[0];
    size_t len = strlen(text);
    if (ir_buffer_pos + len < LLVM_IR_BUFFER_SIZE) {
        memcpy(ir_buffer + ir_buffer_pos, text, len);
        ir_buffer_pos += len;
    }
    (void)n;
}

static void host_llvm_flush(uint64_t* args, int n) {
    // args[0] = pointer to output path string (null-terminated)
    const char* output_path = (const char*)(uintptr_t)args[0];
    FILE* f = fopen(output_path, "w");
    if (f) {
        fwrite(ir_buffer, 1, ir_buffer_pos, f);
        fclose(f);
        printf("[tamer] Wrote LLVM IR: %s (%zu bytes)\n", output_path, ir_buffer_pos);
    } else {
        fprintf(stderr, "[tamer] Error: cannot write '%s'\n", output_path);
    }
    ir_buffer_pos = 0;
    (void)n;
}

static void host_invoke_clang(uint64_t* args, int n) {
    // args[0] = pointer to argv array (null-terminated pointer array)
    // args[1] = pointer to environment (null)
    // Forks and exec's clang, waits for completion.
    // Returns exit status or -1 on error.

    char** argv = (char**)(uintptr_t)args[0];
    pid_t pid = fork();
    if (pid == 0) {
        // Child: exec clang
        execvp(argv[0], argv);
        _exit(127);
    } else if (pid > 0) {
        // Parent: wait for child
        int status;
        waitpid(pid, &status, 0);
        if (WIFEXITED(status)) {
            args[0] = WEXITSTATUS(status);
        } else {
            args[0] = -1;
        }
    } else {
        args[0] = -1;  // fork failed
    }
    (void)n;
}

// ── Section reading ──────────────────────────────────────────────────────
// Reads the .bounty section table and extracts a section by type.

static const uint8_t* find_section(const uint8_t* data, size_t size,
                                    uint8_t type, size_t* out_size) {
    if (size < 21) return NULL;
    if (memcmp(data, BOUNTY_MAGIC, BOUNTY_MAGIC_LEN) != 0) return NULL;

    uint32_t section_count;
    memcpy(&section_count, data + 17, 4);
    size_t table_start = 21;

    for (uint32_t i = 0; i < section_count; i++) {
        size_t entry_off = table_start + i * 17;
        if (entry_off + 17 > size) return NULL;

        if (data[entry_off] == type) {
            uint64_t sec_offset, sec_size;
            memcpy(&sec_offset, data + entry_off + 1, 8);
            memcpy(&sec_size, data + entry_off + 9, 8);
            if (sec_offset + sec_size > size) return NULL;
            *out_size = (size_t)sec_size;
            return data + sec_offset;
        }
    }
    return NULL;
}

// ── Main ─────────────────────────────────────────────────────────────────

int main(int argc, char** argv) {
    if (argc < 2) {
        fprintf(stderr, "Usage: tamer <bounty_file> [-o <output_dir>]\n");
        return 1;
    }

    const char* bounty_path = argv[1];
    const char* output_dir = ".";
    for (int i = 2; i < argc; i++) {
        if (strcmp(argv[i], "-o") == 0 && i + 1 < argc) {
            output_dir = argv[i + 1];
            i++;
        }
    }

    printf("[tamer] Loading .bounty: %s\n", bounty_path);

    // 1. Read .bounty file
    FILE* f = fopen(bounty_path, "rb");
    if (!f) {
        fprintf(stderr, "[tamer] Error: cannot open '%s'\n", bounty_path);
        return 1;
    }
    fseek(f, 0, SEEK_END);
    long file_size = ftell(f);
    fseek(f, 0, SEEK_SET);
    uint8_t* file_data = (uint8_t*)malloc((size_t)file_size);
    if (!file_data) { fclose(f); return 1; }
    fread(file_data, 1, (size_t)file_size, f);
    fclose(f);

    // 2. Extract sections
    size_t lair_size = 0;
    const uint8_t* lair = find_section(file_data, (size_t)file_size, SECTION_LAIR, &lair_size);
    if (!lair) {
        fprintf(stderr, "[tamer] Error: .lair section not found\n");
        free(file_data);
        return 1;
    }
    printf("[tamer]   .lair: %zu bytes\n", lair_size);

    size_t beastpack_size = 0;
    const uint8_t* beastpack = find_section(file_data, (size_t)file_size, SECTION_BEASTPACK, &beastpack_size);
    if (!beastpack) {
        fprintf(stderr, "[tamer] Error: .beastpack section not found\n");
        free(file_data);
        return 1;
    }
    printf("[tamer]   .beastpack: %zu bytes\n", beastpack_size);

    size_t manifest_size = 0;
    find_section(file_data, (size_t)file_size, SECTION_MANIFEST, &manifest_size);
    printf("[tamer]   manifest: %zu bytes\n", manifest_size);

    // 3. Initialize VM
    VmState vm;
    if (vm_init(&vm) != 0) {
        fprintf(stderr, "[tamer] Error: VM initialization failed\n");
        free(file_data);
        return 1;
    }

    // 4. Register host functions
    vm_register_host(&vm, 0, host_log);
    vm_register_host(&vm, 1, host_cpuid);
    vm_register_host(&vm, 2, host_os_abi);
    vm_register_host(&vm, 3, host_llvm_emit);
    vm_register_host(&vm, 4, host_llvm_flush);
    vm_register_host(&vm, 5, host_invoke_clang);

    // 5. Load .lair bytecode
    if (vm_load_lair(&vm, lair, lair_size) != 0) {
        fprintf(stderr, "[tamer] Error: failed to load .lair: %s\n", vm_error(&vm));
        vm_free(&vm);
        free(file_data);
        return 1;
    }
    printf("[tamer]   functions: %zu\n", vm.function_count);

    // 6. Find the tame function by name
    int tame_idx = vm_find_function(&vm, "tame");
    if (tame_idx < 0) {
        fprintf(stderr, "[tamer] Error: 'tame' function not found in .lair\n");
        vm_free(&vm);
        free(file_data);
        return 1;
    }
    printf("[tamer]   tame function: index %d\n", tame_idx);

    // 7. Push .beastpack + user .lair + output dir as arguments to tame()
    size_t user_lair_size = 0;
    const uint8_t* user_lair_data = find_section(file_data, (size_t)file_size, 4, &user_lair_size);
    if (user_lair_data) {
        printf("[tamer]   user .lair: %zu bytes\n", user_lair_size);
    }
    // tame(lair_data, lair_len, beastpack_data, beastpack_len)
    vm.stack[vm.stack_len++] = (uint64_t)(uintptr_t)(user_lair_data ? user_lair_data : lair);
    vm.stack[vm.stack_len++] = (uint64_t)(user_lair_data ? user_lair_size : lair_size);
    vm.stack[vm.stack_len++] = (uint64_t)(uintptr_t)beastpack;
    vm.stack[vm.stack_len++] = (uint64_t)beastpack_size;

    // 8. Execute tame function
    printf("[tamer] Taming the beast...\n");
    uint64_t result = vm_execute(&vm, (uint32_t)tame_idx);

    if (vm.has_error) {
        fprintf(stderr, "[tamer] Error during execution: %s\n", vm_error(&vm));
        vm_free(&vm);
        free(file_data);
        return 1;
    }

    // 8. Report success
    printf("[tamer] Taming complete! (exit code: %lu)\n", (unsigned long)result);
    printf("[tamer] Output directory: %s\n", output_dir);

    vm_free(&vm);
    free(file_data);
    return 0;
}
