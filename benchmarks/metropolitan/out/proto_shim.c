// proto_shim — Protocol bridge shim (auto-generated)
// Compile: gcc -O2 -o proto_shim proto_shim.c -ldl

#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <dlfcn.h>

static void* lib_handle = NULL;

typedef int64_t (*add_fn_t)(int64_t, int64_t);
static add_fn_t add_fn = NULL;
typedef int64_t (*mul_fn_t)(int64_t, int64_t);
static mul_fn_t mul_fn = NULL;

int main(int argc, char** argv) {
    (void)argc;
    char* slash = strrchr(argv[0], '/');
    char so_path[4096];
    if (slash) { snprintf(so_path, sizeof(so_path), "%.*s/bench_add.so", (int)(slash - argv[0]), argv[0]); }
    else { snprintf(so_path, sizeof(so_path), "./bench_add.so"); }

    lib_handle = dlopen(so_path, RTLD_LAZY | RTLD_LOCAL);
    if (!lib_handle) { fprintf(stderr, "dlopen: %s\n", dlerror()); return 1; }

    add_fn = (add_fn_t)dlsym(lib_handle, "add");
    if (!add_fn) { fprintf(stderr, "dlsym add: %s\n", dlerror()); return 1; }
    mul_fn = (mul_fn_t)dlsym(lib_handle, "mul");
    if (!mul_fn) { fprintf(stderr, "dlsym mul: %s\n", dlerror()); return 1; }

    char line[1024];
    while (fgets(line, sizeof(line), stdin)) {
        char fn[256]; int64_t a = 0, b = 0; int n = sscanf(line, "%255s %ld %ld", fn, &a, &b);
        if (n < 1) continue;
        if (strcmp(fn, "add") == 0) { printf("%ld\n", add_fn(a, b)); fflush(stdout); continue; }
        if (strcmp(fn, "mul") == 0) { printf("%ld\n", mul_fn(a, b)); fflush(stdout); continue; }
        printf("-1\n"); fflush(stdout);
    }
    dlclose(lib_handle);
    return 0;
}
