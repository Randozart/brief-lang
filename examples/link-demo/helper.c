// helper.c — C source linked into a Briv program
// Compiled via clang -c -emit-llvm -O2, then LTO'd with Briv IR

#include <stdio.h>

long long double_it(long long n) {
    return n * 2;
}

int greet(const char* name) {
    printf("Hello, %s!\n", name);
    return 1;
}