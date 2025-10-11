#include "../include/common.h"
#include "vmx.h"
#include "ept.h"
#include "../common/log.h"

int kmain(void) {
    hv_log("INFO", "AegisHV boot");

    if (vmx_init() != 0) { hv_log("ERR", "vmx_init failed"); goto out; }
    if (ept_init() != 0) { hv_log("ERR", "ept_init failed"); goto out; }

    hv_log("INFO", "Init complete; entering loop");
    vmx_run_loop();

out:
    for(;;){ __asm__ __volatile__("hlt"); }
    return 0;
}
