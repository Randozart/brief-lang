// str_prepend.c — C reference for bridge benchmark
// 2026-07-22: Implements pp_type_bits ("Bits(" + s + ")") in C,
// matching the Briev pp_type_bits function.
//
// Compile: gcc -shared -fPIC -O2 -o libstr_prepend_c.so str_prepend.c

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

/// Echo a string — matching Briev's briev_test_cstr_roundtrip.
/// Input: C string pointer passed as int64_t
/// Output: Copy of input C string pointer as int64_t
/// Caller must free the returned pointer.
int64_t c_str_echo(int64_t s_ptr) {
    const char* s = (const char*)(uintptr_t)s_ptr;
    if (!s) return 0;
    size_t len = strlen(s);
    char* result = (char*)malloc(len + 1);
    if (!result) return 0;
    memcpy(result, s, len);
    result[len] = '\0';
    return (int64_t)(uintptr_t)result;
}
