/* The C half of tests/c/global_side.oli: globals of C read and written by
 * Oli--, and globals of Oli-- read by C. */
#include <stdio.h>
unsigned long c_counter = 37;
unsigned c_table[4] = { 1, 2, 3, 4 };
struct { int x, y; } c_pt = { 100, 0 };
extern unsigned long oli_seen;
extern unsigned oli_limit;
unsigned long oli_globals(void);
int main(void)
{
    unsigned long r = oli_globals();
    fflush(stdout);
    printf("r=%lu counter=%lu table2=%u pt.y=%d seen=%lu limit=%u\n", r, c_counter, c_table[2], c_pt.y, oli_seen, oli_limit);
    return 0;
}
