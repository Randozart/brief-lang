// Matrix Pipeline — C reference. Same computation as matrix_pipeline.bv.
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    long total = 50000000;
    char *env = getenv("BOUND");
    if (env) total = atol(env);
    float m00=1,m01=0,m02=0,m03=0,m10=0,m11=1,m12=0,m13=0,m20=0,m21=0,m22=1,m23=0,m30=0,m31=0,m32=0,m33=1;
    const float a00=0.999f,a01=0.001f,a02=0.0001f,a03=0.00001f,
        a10=0.001f,a11=0.999f,a12=0.001f,a13=0.0001f,
        a20=0.0001f,a21=0.001f,a22=0.999f,a23=0.001f,
        a30=0.00001f,a31=0.0001f,a32=0.001f,a33=0.999f;
    long count = 0;
    while (count < total) {
        float n00 = a00*m00+a01*m10+a02*m20+a03*m30;
        float n01 = a00*m01+a01*m11+a02*m21+a03*m31;
        float n02 = a00*m02+a01*m12+a02*m22+a03*m32;
        float n03 = a00*m03+a01*m13+a02*m23+a03*m33;
        float n10 = a10*m00+a11*m10+a12*m20+a13*m30;
        float n11 = a10*m01+a11*m11+a12*m21+a13*m31;
        float n12 = a10*m02+a11*m12+a12*m22+a13*m32;
        float n13 = a10*m03+a11*m13+a12*m23+a13*m33;
        float n20 = a20*m00+a21*m10+a22*m20+a23*m30;
        float n21 = a20*m01+a21*m11+a22*m21+a23*m31;
        float n22 = a20*m02+a21*m12+a22*m22+a23*m32;
        float n23 = a20*m03+a21*m13+a22*m23+a23*m33;
        float n30 = a30*m00+a31*m10+a32*m20+a33*m30;
        float n31 = a30*m01+a31*m11+a32*m21+a33*m31;
        float n32 = a30*m02+a31*m12+a32*m22+a33*m32;
        float n33 = a30*m03+a31*m13+a32*m23+a33*m33;
        m00=n00;m01=n01;m02=n02;m03=n03;
        m10=n10;m11=n11;m12=n12;m13=n13;
        m20=n20;m21=n21;m22=n22;m23=n23;
        m30=n30;m31=n31;m32=n32;m33=n33;
        count++;
        if (count % 5000000 == 0) {
            printf("%.9g\n", (double)(m00 + m11 + m22 + m33));
        }
    }
    return 0;
}
