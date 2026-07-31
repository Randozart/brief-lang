// Sweep Mid — C reference. Same computation as sweep_mid.bv.
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    long total = 50000000;
    char *env = getenv("BOUND");
    if (env) total = atol(env);
    float f0=1.0f,f1=0.5f,f2=0.25f,f3=0.125f,f4=0.0625f,f5=0.03125f,f6=0.015625f,f7=0.0078125f;
    long count = 0;
    while (count < total) {
        f0 = f0 * 0.999f + f1 * 0.001f;
        f1 = f1 * 0.999f + f2 * 0.001f;
        f2 = f2 * 0.999f + f3 * 0.001f;
        f3 = f3 * 0.999f + f4 * 0.001f;
        f4 = f4 * 0.999f + f5 * 0.001f;
        f5 = f5 * 0.999f + f6 * 0.001f;
        f6 = f6 * 0.999f + f7 * 0.001f;
        f7 = f7 * 0.999f + f0 * 0.001f;
        count++;
        if (count % 1000000 == 0) {
            printf("%.9g\n", (double)(f0+f1+f2+f3+f4+f5+f6+f7));
        }
    }
    return 0;
}
