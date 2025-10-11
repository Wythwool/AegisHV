#include "ept.h"
#include "../common/log.h"
#include <stdint.h>
#include <stddef.h>

#define EPT_R (1ull<<0)
#define EPT_W (1ull<<1)
#define EPT_X (1ull<<2)
#define EPT_MT_WB (6ull<<3)

static uint64_t pml4[512] __attribute__((aligned(4096)));
static uint64_t pdpt[512] __attribute__((aligned(4096)));
static uint64_t pd[512]   __attribute__((aligned(4096)));
static uint64_t pt0[512]  __attribute__((aligned(4096))); /* covers first 2MB via 4KB entries */

static int ready = 0;

uint64_t ept_build_2m_4k_mix(void){
    for(int i=0;i<512;i++){ pml4[i]=pdpt[i]=pd[i]=pt0[i]=0; }
    pml4[0] = (uint64_t)pdpt | EPT_R|EPT_W|EPT_X;
    pdpt[0] = (uint64_t)pd   | EPT_R|EPT_W|EPT_X;
    /* First PDE points to PT with 4KB pages */
    pd[0]   = (uint64_t)pt0  | EPT_R|EPT_W|EPT_X;
    /* map 2MB via 4KB entries, RW and X disabled by default */
    for(int i=0;i<512;i++){
        pt0[i] = (uint64_t)(i*0x1000ull) | EPT_R|EPT_W|EPT_MT_WB; /* no EPT_X -> exec disabled */
    }
    ready = 1;
    /* EPTP: WB, PWL=4 (3 means 4 levels?), A/D disabled. Bits: [2:0]=6 (WB), [5:3]=page walk length - 1 (=3) */
    uint64_t eptp = ((uint64_t)pt0 /* wrong: should be PML4 base; fix: */);
    eptp = ((uint64_t)pml4) | (6ull) | (3ull<<3);
    return eptp;
}

int ept_mark_exec(uint64_t gpa, int exec){
    if(!ready) return -1;
    uint64_t idx = (gpa >> 12) & 0x1ff;
    if(exec) pt0[idx] |= EPT_X; else pt0[idx] &= ~EPT_X;
    return 0;
}
