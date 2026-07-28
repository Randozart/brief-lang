// popcount C reference — standard bit-parallel implementation
#include <stdint.h>
#include <stdio.h>

int main() {
    uint64_t x;
    uint64_t total = 0;
    for (x = 0; x < 50000000; x++) {
        uint64_t v = x;
        v = v - ((v >> 1) & 0x5555555555555555ULL);
        v = (v & 0x3333333333333333ULL) + ((v >> 2) & 0x3333333333333333ULL);
        v = (v + (v >> 4)) & 0x0F0F0F0F0F0F0F0FULL;
        total += (v * 0x0101010101010101ULL) >> 56;
    }
    printf("%lu\n", total);
    return 0;
}
