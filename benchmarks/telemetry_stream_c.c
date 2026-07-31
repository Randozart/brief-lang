// Telemetry Stream — C reference. Same computation as telemetry_stream.bv.
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    long total = 50000000;
    char *env = getenv("BOUND");
    if (env) total = atol(env);
    float ema = 0.0f, m2 = 0.0f;
    const float alpha = 0.05f;
    long count = 0;
    while (count < total) {
        float sample = 100.0f + (float)(count % 97);
        float delta = sample - ema;
        ema = ema + alpha * delta;
        m2 = m2 + delta * delta;
        count++;
        if (count % 1000000 == 0) {
            printf("%.9g\n", (double)(ema + m2));
        }
    }
    return 0;
}
