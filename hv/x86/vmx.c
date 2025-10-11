#include "vmx.h"
#include "../common/log.h"
#include <stdint.h>

static inline void cpuid(uint32_t leaf, uint32_t *a, uint32_t *b, uint32_t *c, uint32_t *d){
    __asm__ volatile("cpuid":"=a"(*a),"=b"(*b),"=c"(*c),"=d"(*d):"a"(leaf),"c"(0));
}

static int has_vmx(void){
    uint32_t a,b,c,d;
    cpuid(1,&a,&b,&c,&d);
    return (c & (1u<<5)) != 0; /* VMX bit */
}

int vmx_init(void){
    if(!has_vmx()){
        hv_log("WARN", "VMX not supported by CPU; skipping VMXON");
        return -1;
    }
    hv_log("INFO", "VMX supported; VMXON deferred in this build");
    return 0;
}

int vmx_shutdown(void){
    hv_log("INFO", "VMX shutdown");
    return 0;
}

void vmx_run_loop(void){
    hv_log("INFO", "Run loop start");
    for(;;){
        __asm__ volatile("hlt");
    }
}
