/* The C half of tests/c: calls into the Oli-- object and is called back. */
#include <stdio.h>

long oli_add(long a, long b);

long c_double(long x)
{
    return x * 2;
}

int main(void)
{
    printf("oli_add(2, 3) via C = %ld\n", oli_add(2, 3));
    return 0;
}
