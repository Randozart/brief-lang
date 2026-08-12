// bench_shm_client.c — Benchmark SHM protocol bridge (gen_shm)
// 2026-07-25: Measures per-call latency of shared memory IPC.
// Build: gcc -O2 -o bench_shm_client bench_shm_client.c -lrt
// Run: LD_LIBRARY_PATH=out ./bench_shm_client

#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>
#include <time.h>

#define SHM_SIZE 64
#define SHM_NAME "/briev_bridge"

static volatile uint8_t* shm = NULL;

static uint64_t now_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000UL + (uint64_t)ts.tv_nsec;
}

static int64_t shm_call(int64_t a, int64_t b) {
    *(int64_t*)(shm + 8) = a;
    *(int64_t*)(shm + 16) = b;
    shm[2] = 0;
    __sync_synchronize();
    shm[0] = 1;
    while (shm[1] == 0) { }
    int64_t result = *(int64_t*)(shm + 40);
    shm[1] = 0;
    return result;
}

int main(void) {
    // Clean stale SHM
    shm_unlink(SHM_NAME);

    int shm_fd = shm_open(SHM_NAME, O_RDWR | O_CREAT, 0666);
    if (shm_fd < 0) { perror("shm_open"); return 1; }
    ftruncate(shm_fd, SHM_SIZE);
    shm = (volatile uint8_t*)mmap(NULL, SHM_SIZE, PROT_READ | PROT_WRITE, MAP_SHARED, shm_fd, 0);
    if (shm == MAP_FAILED) { perror("mmap"); return 1; }
    close(shm_fd);
    shm[0] = 0; shm[1] = 0;

    pid_t pid = fork();
    if (pid == 0) {
        execl("out/proto_shm_shm", "out/proto_shm_shm", NULL);
        perror("execl"); exit(1);
    }
    if (pid < 0) { perror("fork"); return 1; }

    usleep(200000);  // wait for worker init

    int64_t warm = shm_call(3, 4);
    if (warm != 7) {
        fprintf(stderr, "ERROR: expected 7, got %ld\n", (long)warm);
        shm[0] = 0xFF; waitpid(pid, NULL, 0); return 1;
    }

    const int N = 100000;
    uint64_t t0 = now_ns();
    for (int i = 0; i < N; i++) {
        shm_call(3, 4);
    }
    uint64_t t1 = now_ns();
    uint64_t total = t1 - t0;

    shm[0] = 0xFF;
    waitpid(pid, NULL, 0);
    munmap((void*)shm, SHM_SIZE);
    shm_unlink(SHM_NAME);

    printf("  SHM protocol bridge      median=%luns  result=%ld\n", (long)(total / N), (long)warm);
    printf("  total: %lu ns over %d iterations\n", (unsigned long)total, N);
    return 0;
}
