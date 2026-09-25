/* The C half of the float test: calls into Oli-- with doubles, floats and
 * narrow integers, and is called back through c_lerp and c_minus_five. */
#include <stdio.h>
double oli_norm(double, double);
double oli_mid(double, double);
float oli_scale32(float, int);
long oli_narrow(short);
double c_lerp(double a, double b, float t, unsigned long n) { return a + (b - a) * t + (double)n; }
signed char c_minus_five(void) { return -5; }
int main(void)
{
    printf("%.17g %.17g %.9g %ld\n", oli_norm(3.0, 4.0), oli_mid(10.0, 20.0), oli_scale32(1.25f, -3), oli_narrow(-7));
    return 0;
}
