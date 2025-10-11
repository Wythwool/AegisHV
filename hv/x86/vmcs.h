#pragma once
#include <stdint.h>
int vmx_setup_and_launch(uint64_t guest_entry_gpa, uint64_t guest_stack_gpa, uint64_t eptp);
