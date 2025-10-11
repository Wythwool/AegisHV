#include "ept.h"
#include "../common/log.h"
#include <stdint.h>

static uint64_t dummy_eptp = 0;

int ept_init(void){
    /* This build only wires minimal EPT structures later when VMX is actually enabled. */
    dummy_eptp = 0;
    return 0;
}

uint64_t ept_build_2m_4k_mix(void){
    /* Placeholder EPTP value; real setup is platform-specific. */
    return dummy_eptp;
}

int ept_mark_exec(uint64_t gpa, int exec){
    (void)gpa; (void)exec;
    return 0;
}

int ept_mark_exec_trap(uint64_t gpa){
    (void)gpa;
    return 0;
}
