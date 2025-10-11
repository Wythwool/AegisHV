#include "../common/log.h"
#include <stdint.h>

int el2_init(void) {
#ifdef __aarch64__
    // Set HCR_EL2 flags, set up stage‑2 tables, trap sensitive sysregs
    // This is a bring-up stub, intentionally minimal.
    hv_log("INFO", "EL2 init stub");
    return 0;
#else
    hv_log("ERR", "el2_init called on non‑arm64");
    return -1;
#endif
}
