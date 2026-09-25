/* The C half of the position-independent test: a PIE or a program linked
 * against libpic_side.so calls into Oli-- code that reads and writes its own
 * statics and calls puts. */
#include <stdio.h>
unsigned long oli_bump(unsigned long);
int oli_hello(void);
int main(void)
{
    unsigned long a, b;
    oli_hello();
    a = oli_bump(0);
    b = oli_bump(1);
    printf("oli_bump(0) = %lu, oli_bump(1) = %lu\n", a, b);
    return 0;
}
