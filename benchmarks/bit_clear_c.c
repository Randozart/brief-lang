// bit_clear_c — C reference for bit_clear.bv
// Symmetric loop: clears one bit per iteration via reg & (reg - 1).
// Exactly 63 iterations for INT64_MAX (all 63 lower bits set).
//
// clang -O3 -march=native -ffast-math -o benchmarks/bit_clear_c benchmarks/bit_clear_c.c

#include <stdio.h>
#include <stdint.h>

int main(void) {
    int64_t reg = INT64_MAX;  // 0x7FFFFFFFFFFFFFFF, 63 bits set

    while (reg != 0) {
        reg &= reg - 1;
        if (reg % 1000000 == 0)
            fprintf(stderr, "%ld\n", (long)reg);
    }

    return (int)reg;
}
