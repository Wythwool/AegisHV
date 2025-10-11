#include "vmcs.h"
#include "../common/log.h"

int vmx_setup_and_launch(uint64_t guest_entry_gpa, uint64_t guest_stack_gpa, uint64_t eptp){
    (void)guest_entry_gpa; (void)guest_stack_gpa; (void)eptp;
    hv_log("INFO", "VMX disabled in this build; run loop ends");
    return -1;
}
