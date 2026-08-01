// sweep_arr_c — C reference for sweep_arr.bv (Float[16] array-state sweep).
// Mirrors sweep_dense's dense cross-indexed update through a single array.

#include <stdlib.h>
#include <stdio.h>

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    float f[16] = {1.0f, 0.5f, 0.25f, 0.125f, 0.0625f, 0.03125f, 0.015625f,
                   0.0078125f, 0.00390625f, 0.001953125f, 0.0009765625f,
                   0.00048828125f, 0.000244140625f, 0.0001220703125f,
                   0.00006103515625f, 0.000030517578125f};
    long i = 0;
    for (; i < N; ) {
        float n0 = f[0]*0.999f + f[1]*0.001f + f[15]*0.000001f;
        float n1 = f[1]*0.999f + f[2]*0.001f + f[0]*0.000001f;
        float n2 = f[2]*0.999f + f[3]*0.001f + f[1]*0.000001f;
        float n3 = f[3]*0.999f + f[4]*0.001f + f[2]*0.000001f;
        float n4 = f[4]*0.999f + f[5]*0.001f + f[3]*0.000001f;
        float n5 = f[5]*0.999f + f[6]*0.001f + f[4]*0.000001f;
        float n6 = f[6]*0.999f + f[7]*0.001f + f[5]*0.000001f;
        float n7 = f[7]*0.999f + f[8]*0.001f + f[6]*0.000001f;
        float n8 = f[8]*0.999f + f[9]*0.001f + f[7]*0.000001f;
        float n9 = f[9]*0.999f + f[10]*0.001f + f[8]*0.000001f;
        float n10 = f[10]*0.999f + f[11]*0.001f + f[9]*0.000001f;
        float n11 = f[11]*0.999f + f[12]*0.001f + f[10]*0.000001f;
        float n12 = f[12]*0.999f + f[13]*0.001f + f[11]*0.000001f;
        float n13 = f[13]*0.999f + f[14]*0.001f + f[12]*0.000001f;
        float n14 = f[14]*0.999f + f[15]*0.001f + f[13]*0.000001f;
        float n15 = f[15]*0.999f + f[0]*0.001f + f[14]*0.000001f;
        f[0]=n0; f[1]=n1; f[2]=n2; f[3]=n3;
        f[4]=n4; f[5]=n5; f[6]=n6; f[7]=n7;
        f[8]=n8; f[9]=n9; f[10]=n10; f[11]=n11;
        f[12]=n12; f[13]=n13; f[14]=n14; f[15]=n15;
        i++;
        if (i % 5000000 == 0)
            fprintf(stdout, "%.9g\n", f[0]+f[1]+f[2]+f[3]+f[4]+f[5]+f[6]+f[7]+f[8]+f[9]+f[10]+f[11]+f[12]+f[13]+f[14]+f[15]);
    }
    return 0;
}
