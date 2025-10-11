#pragma once
#include <stdint.h>
int ept_init(void);
int ept_mark_exec_trap(uint64_t gpa);
uint64_t ept_build_2m_4k_mix(void);
int ept_mark_exec(uint64_t gpa, int exec);
