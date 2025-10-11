#include "../include/common.h"
#include "vmx.h"
#include "ept.h"
#include "../common/log.h"

extern void hv_log(const char* level, const char* fmt, ...);

int kmain(void) {
    hv_log("INFO", "AegisHV starting (bare-metal, multiboot2)");

    if (vmx_init() != 0) { hv_log("ERR", "vmx_init failed"); goto out; }
    if (ept_init() != 0) { hv_log("ERR", "ept_init failed"); goto out_off; }

    /* Demo: mark a GPA as exec-trap (this will be wired after guest launch). */
    ept_mark_exec_trap(0x400000);

    hv_log("INFO", "VMX/EPT init done; entering run loop (stub)");
    vmx_run_loop();

out_off:
    vmx_shutdown();
out:
    for(;;){ __asm__ __volatile__("hlt"); }
    return 0;
}
