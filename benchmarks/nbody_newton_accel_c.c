// nbody_newton_accel_c.c — C reference for the Briev `accel` benchmark
// (benchmarks/nbody_newton_accel.bv). Central-force N-body, parallel over
// bodies. Same f32 math, same init, same update, same observable (body 0's
// x-position after BOUND steps). Env: BODYCOUNT (default 2048), BOUND
// (default 10).
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    const int MAXB = 4096;
    const char* nb_env = getenv("BODYCOUNT");
    const char* bound_env = getenv("BOUND");
    int nb = nb_env ? atoi(nb_env) : 2048;
    int bound = bound_env ? atoi(bound_env) : 10;
    const float dt = 0.001f;
    const float eps = 0.01f;

    float px[MAXB], py[MAXB], pz[MAXB];
    float vx[MAXB], vy[MAXB], vz[MAXB];

    // init: same seeding as init_bodies.
    for (int i = 0; i < nb && i < MAXB; i++) {
        px[i] = (float)i * 0.1f + 0.5f;
        py[i] = (float)i * 0.05f + 0.3f;
        pz[i] = 0.0f;
        vx[i] = 0.01f;
        vy[i] = -0.02f;
        vz[i] = 0.0f;
    }

    // step: same softened central-force update as step_bodies.
    for (int step = 0; step < bound; step++) {
        for (int i = 0; i < nb && i < MAXB; i++) {
            float dx = -px[i];
            float dy = -py[i];
            float dz = -pz[i];
            float inv = 1.0f / (dx * dx + dy * dy + dz * dz + eps);
            vx[i] = vx[i] + dt * dx * inv;
            vy[i] = vy[i] + dt * dy * inv;
            vz[i] = vz[i] + dt * dz * inv;
            px[i] = px[i] + dt * vx[i];
            py[i] = py[i] + dt * vy[i];
            pz[i] = pz[i] + dt * vz[i];
        }
    }

    printf("%.9g\n", (double)px[0]);
    return 0;
}
