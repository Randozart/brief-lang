// nbody_newton_sym_c — Symmetric C reference for nbody_newton_sym.bv.
// Mirrors nbody_newton_c.c with periodic energy print every 5M iterations.
//
// clang -O3 -march=native -ffast-math -o benchmarks/nbody_newton_sym_c benchmarks/nbody_newton_sym_c.c

#include <stdio.h>
#include <stdlib.h>

int main(void) {
    const char* env = getenv("BOUND");
    long total = env ? atol(env) : 50000000L;

    const float pi = 3.141592653589793f;
    const float solar_mass = 4.0f * pi * pi;
    const float days_per_year = 365.24f;
    const float dt = 0.01f;

    float bx[5], by[5], bz[5];
    float vx[5], vy[5], vz[5];
    float m[5];

    bx[0] = 0.0f; by[0] = 0.0f; bz[0] = 0.0f;
    vx[0] = 0.0f; vy[0] = 0.0f; vz[0] = 0.0f;
    m[0] = solar_mass;

    bx[1] = 4.84143144246472090f;
    by[1] = -1.16032004402742839f;
    bz[1] = -1.03622044471123109e-01f;
    vx[1] = (float)(1.66007664274403694e-03 * days_per_year);
    vy[1] = (float)(7.69901118419740425e-03 * days_per_year);
    vz[1] = (float)(-6.90460016972063023e-05 * days_per_year);
    m[1] = (float)(9.54791938424326609e-04 * solar_mass);

    bx[2] = 8.34336671824457987f;
    by[2] = 4.12479856412430479f;
    bz[2] = -4.03523417114321381e-01f;
    vx[2] = (float)(-2.76742510726862411e-03 * days_per_year);
    vy[2] = (float)(4.99852801234917238e-03 * days_per_year);
    vz[2] = (float)(2.30417297573763929e-05 * days_per_year);
    m[2] = (float)(2.85885980666130812e-04 * solar_mass);

    bx[3] = 1.28943695621309110e+01f;
    by[3] = -1.51111514016986312e+01f;
    bz[3] = -2.23307578892655734e-01f;
    vx[3] = (float)(2.96460137564761618e-03 * days_per_year);
    vy[3] = (float)(2.37847173959480950e-03 * days_per_year);
    vz[3] = (float)(-2.96589568540237556e-05 * days_per_year);
    m[3] = (float)(4.36624404335156298e-05 * solar_mass);

    bx[4] = 1.53796971148509165e+01f;
    by[4] = -2.59193146099879641e+01f;
    bz[4] = 1.79258772950371181e-01f;
    vx[4] = (float)(2.68067772490389322e-03 * days_per_year);
    vy[4] = (float)(1.62824170038242295e-03 * days_per_year);
    vz[4] = (float)(-9.51592254519715870e-05 * days_per_year);
    m[4] = (float)(5.15138902046611451e-05 * solar_mass);

    #define NEWTON5(dsq, dist) do { \
        float g = (dsq) * 0.5f; \
        float h = 0.5f * (g + (dsq) / g); \
        float i = 0.5f * (h + (dsq) / h); \
        float j = 0.5f * (i + (dsq) / i); \
        float k = 0.5f * (j + (dsq) / j); \
        (dist) = 0.5f * (k + (dsq) / k); \
    } while(0)

    long count = 0;
    while (count < total) {
        float dx, dy, dz, dsq, dist, mag;

        #define PAIR(ia, ib) \
            dx = bx[ia] - bx[ib]; dy = by[ia] - by[ib]; dz = bz[ia] - bz[ib]; \
            dsq = dx*dx + dy*dy + dz*dz; \
            NEWTON5(dsq, dist); \
            mag = dt / (dsq * dist); \
            vx[ia] -= dx * m[ib] * mag; vy[ia] -= dy * m[ib] * mag; vz[ia] -= dz * m[ib] * mag; \
            vx[ib] += dx * m[ia] * mag; vy[ib] += dy * m[ia] * mag; vz[ib] += dz * m[ia] * mag;

        PAIR(0,1) PAIR(0,2) PAIR(0,3) PAIR(0,4)
        PAIR(1,2) PAIR(1,3) PAIR(1,4)
        PAIR(2,3) PAIR(2,4)
        PAIR(3,4)

        #undef PAIR

        // Position update uses UPDATED velocities (matches Briev's
        // store-and-forward semantics within a txn body).
        for (int i = 0; i < 5; i++) {
            bx[i] += dt * vx[i];
            by[i] += dt * vy[i];
            bz[i] += dt * vz[i];
        }

        // Energy periodic print
        {
            float energy = 0.0f;
            #define EPAIR(ia, ib) { \
                float _dx = bx[ia] - bx[ib]; \
                float _dy = by[ia] - by[ib]; \
                float _dz = bz[ia] - bz[ib]; \
                float _dsq = _dx*_dx + _dy*_dy + _dz*_dz; \
                float _dist; \
                NEWTON5(_dsq, _dist); \
                energy -= m[ia] * m[ib] / _dist; \
            }

            EPAIR(0,1) EPAIR(0,2) EPAIR(0,3) EPAIR(0,4)
            EPAIR(1,2) EPAIR(1,3) EPAIR(1,4)
            EPAIR(2,3) EPAIR(2,4)
            EPAIR(3,4)
            #undef EPAIR

            for (int i = 0; i < 5; i++) {
                energy += 0.5f * m[i] * (vx[i]*vx[i] + vy[i]*vy[i] + vz[i]*vz[i]);
            }

            if (count % 5000000 == 0)
                fprintf(stdout, "%.9f\n", energy);
        }

        count++;
    }

    // Final energy print
    {
        float energy = 0.0f;
        #define EPAIR(ia, ib) { \
            float _dx = bx[ia] - bx[ib]; \
            float _dy = by[ia] - by[ib]; \
            float _dz = bz[ia] - bz[ib]; \
            float _dsq = _dx*_dx + _dy*_dy + _dz*_dz; \
            float _dist; \
            NEWTON5(_dsq, _dist); \
            energy -= m[ia] * m[ib] / _dist; \
        }

        EPAIR(0,1) EPAIR(0,2) EPAIR(0,3) EPAIR(0,4)
        EPAIR(1,2) EPAIR(1,3) EPAIR(1,4)
        EPAIR(2,3) EPAIR(2,4)
        EPAIR(3,4)
        #undef EPAIR

        for (int i = 0; i < 5; i++) {
            energy += 0.5f * m[i] * (vx[i]*vx[i] + vy[i]*vy[i] + vz[i]*vz[i]);
        }
        fprintf(stdout, "%.9f\n", energy);
    }
}
