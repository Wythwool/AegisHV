#pragma once
#include <stdint.h>
#include "../common/log.h"
#include "ept.h"

int vmx_init(void);     // enable VMX, prepare VMXON region
int vmx_shutdown(void); // VMXOFF

int ept_init(void);     // setup basic EPT with W^X defaults
void vmx_run_loop(void);// minimal VMEXIT loop

// exec-trap control
int ept_mark_exec_trap(uint64_t gpa);
