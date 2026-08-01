// enemy_swarm_c — C reference for enemy_swarm.bv
#include <stdlib.h>
#include <stdio.h>

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    const int NW = 64;
    float x[64] = {0}, y[64] = {0};
    long hp[64];
    long i, sum = 0;
    for (i = 0; i < 64; i++) hp[i] = 1;
    for (i = 0; i < N; ) {
        x[i % NW] += 0.5f;
        y[i % NW] -= 0.25f;
        if (hp[i % NW] > 0) hp[i % NW] -= 1;
        else hp[i % NW] = 3;
        sum += hp[i % NW];
        i++;
        if (i % 5000000 == 0)
            fprintf(stdout, "%ld\n", sum);
    }
    return 0;
}
