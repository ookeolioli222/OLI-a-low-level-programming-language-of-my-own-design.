/* The C half of tests/c/bits_side.oli: the same bitfields and unions
 * declared in C, written by each side, compared byte for byte. */
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <stdint.h>
struct Pte { uint64_t present:1, writable:1, user:1, pad:9, frame:40, avail:11, nx:1; };
struct Mixed { uint8_t a; uint32_t b:3; int32_t c:5; uint32_t d:30; uint16_t e; int8_t f:4; };
union Word { uint64_t raw; uint32_t half; uint8_t byte0; };
unsigned long oli_pte(struct Pte*, unsigned long); long oli_mixed(struct Mixed*); unsigned long oli_sizes(void);
unsigned long oli_union(unsigned long); unsigned long oli_init(void); void oli_overflow(struct Pte*, unsigned long);
int main(int argc, char **argv){
  struct Pte p; memset(&p,0,sizeof p); p.user=1;
  unsigned long r = oli_pte(&p, 0xABCDE);
  struct Pte q; memset(&q,0,sizeof q); q.present=1;q.writable=1;q.user=1;q.frame=0xABCDE;q.nx=1;
  printf("pte r=%lx same_bits=%d raw=%016lx\n", r, memcmp(&p,&q,8)==0, *(uint64_t*)&p);
  struct Mixed m, n; memset(&m,0x55,sizeof m); memset(&n,0x55,sizeof n);
  long mr = oli_mixed(&m); n.a=0xAA;n.b=5;n.c=-3;n.d=0x2ABCDEF1;n.e=0xBEEF;n.f=-8;
  printf("mixed r=%ld same_bytes=%d C: c=%d f=%d sizeof=%zu\n", mr, memcmp(&m,&n,sizeof m)==0, m.c, m.f, sizeof m);
  printf("sizes oli=%lu C=%zu,%zu,%zu\n", oli_sizes(), sizeof(struct Pte), sizeof(struct Mixed), sizeof(union Word));
  printf("union=%lx want %lx\n", oli_union(0x1122334455667788UL), 0x88UL+0x55667788UL+0xBEEFUL);
  printf("init=%lu want %lu\n", oli_init(), 0x12345UL*10+3);

  return 0; }
