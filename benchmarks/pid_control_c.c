// PID Control Loop — C reference. Same computation as pid_control.bv.
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    long total = 50000000;
    char *env = getenv("BOUND");
    if (env) total = atol(env);
    const float sp = 100.0f;
    float pv = 0.0f, err_prev = 0.0f, integral = 0.0f, output = 0.0f;
    const float Kp = 0.1f, Ki = 0.01f, Kd = 0.5f;
    long count = 0;
    while (count < total) {
        float err = sp - pv;
        integral = integral + err;
        float deriv = err - err_prev;
        output = Kp * err + Ki * integral + Kd * deriv;
        pv = pv + output * 0.001f;
        err_prev = err;
        count++;
        if (count % 1000000 == 0) {
            printf("%.9g\n", (double)(output + pv));
        }
    }
    return 0;
}
