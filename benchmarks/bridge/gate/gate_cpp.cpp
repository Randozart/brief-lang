// gate_cpp.cpp — Gate A + B for C++.
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <ctime>
extern "C" {
    typedef long long ll;
    ll feature_hash(ll state, ll count, ll seed);
    ll add(ll a, ll b);
    ll __brief_init_state();
}
static ll native_fh(ll count, ll seed) {
    ll h = seed;
    for (ll i = 0; i < count; i++) h = (h ^ (i * 2654435761LL)) * 1099511628211LL;
    return h;
}
static ll native_add(ll a, ll b) { return a + b; }
static double now_ns(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec * 1e9 + t.tv_nsec; }
int main(int argc, char** argv) {
    ll r = atoll(argv[1]);
    ll st = __brief_init_state();
    const int N = 200000, N2 = 2000000;
    volatile ll sink = 0;
    feature_hash(st, 1000, r);
    double t0 = now_ns();
    for (int i = 0; i < N; i++) sink += feature_hash(st, 1000, r);
    printf("BRIEF_FH %.1f\n", (now_ns() - t0) / N);
    native_fh(1000, r);
    t0 = now_ns();
    for (int i = 0; i < N; i++) sink += native_fh(1000, r + i);
    printf("NATIVE_FH %.1f\n", (now_ns() - t0) / N);
    add(r, 4);
    t0 = now_ns();
    for (int i = 0; i < N2; i++) sink += add(r, 4);
    printf("BRIEF_ADD %.2f\n", (now_ns() - t0) / N2);
    native_add(r, 4);
    t0 = now_ns();
    for (int i = 0; i < N2; i++) sink += native_add(r, i & 7);
    printf("NATIVE_ADD %.2f\n", (now_ns() - t0) / N2);
    (void)sink;
    return 0;
}
