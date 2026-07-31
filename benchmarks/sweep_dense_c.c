// Sweep Dense — C reference. Same computation as sweep_dense.bv.
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    long total = 50000000;
    char *env = getenv("BOUND");
    if (env) total = atol(env);
    float f[16] = {1.0f,0.5f,0.25f,0.125f,0.0625f,0.03125f,0.015625f,0.0078125f,
                   0.00390625f,0.001953125f,0.0009765625f,0.00048828125f,
                   0.000244140625f,0.0001220703125f,0.00006103515625f,0.000030517578125f};
    long count = 0;
    while (count < total) {
        float n[16];
        for (int i = 0; i < 16; i++) {
            n[i] = f[i] * 0.999f + f[(i + 1) % 16] * 0.001f + f[(i + 15) % 16] * 0.000001f;
        }
        for (int i = 0; i < 16; i++) f[i] = n[i];
        count++;
        if (count % 5000000 == 0) {
            float s = 0;
            for (int i = 0; i < 16; i++) s += f[i];
            printf("%.9g\n", (double)s);
        }
    }
    return 0;
}
