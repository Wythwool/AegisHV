#pragma once
#include <stdint.h>
uint64_t ept_build_2m_4k_mix(void); /* returns EPTP */
int ept_mark_exec(uint64_t gpa, int exec);
