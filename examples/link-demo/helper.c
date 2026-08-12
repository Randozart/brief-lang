// helper.c — C source linked into a Briev program
// Compiled via clang -c -emit-llvm -O2, then LTO'd with Briev IR

#include <stdio.h>

long long double_it(long long n) {
    return n * 2;
}

int greet(const char* name) {
    printf("Hello, %s!\n", name);
    return 1;
}