// popcount C reference — standard bit-parallel implementation
#include <stdio.h>
#include <stdint.h>

int main() {
    uint64_t sum = 0;
    uint64_t N = 50000000;
    for (uint64_t i = 0; i < N; i++) {
        uint64_t v = i;
        v = v - ((v >> 1) & 0x5555555555555555ULL);
        v = (v & 0x3333333333333333ULL) + ((v >> 2) & 0x3333333333333333ULL);
        v = (v + (v >> 4)) & 0x0F0F0F0F0F0F0F0FULL;
        sum += (v * 0x0101010101010101ULL) >> 56;
    }
    printf("%lu\n", sum);
    return 0;
}
