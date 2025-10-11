#include "vmx.h"
#include "vmcs.h"
#include "ept.h"
#include "../common/log.h"
#include <stdint.h>

#define CR4_VMXE (1ull<<13)
#define MSR_IA32_FEATURE_CONTROL 0x3a
#define MSR_IA32_VMX_BASIC 0x480

static uint8_t vmxon_region[4096] __attribute__((aligned(4096)));
static uint8_t vmcs_region[4096] __attribute__((aligned(4096)));

static inline uint64_t rdmsr(uint32_t msr){ uint32_t lo,hi; __asm__ volatile("rdmsr":"=a"(lo),"=d"(hi):"c"(msr)); return ((uint64_t)hi<<32)|lo; }

/* Simple guest blob in real mode: jump to 0x3000 to trigger EPT exec violation (since only 0x1000 is executable) */
static uint8_t guest_blob[] = {
    0x31,0xC0,             /* xor ax,ax */
    0x8E,0xD8,             /* mov ds,ax */
    0x8E,0xC0,             /* mov es,ax */
    0x8E,0xD0,             /* mov ss,ax */
    0xEA,0x00,0x30,0x00,0x00 /* jmp far 0x0000:0x3000 */
};

extern int vmx_setup_and_launch(uint64_t guest_entry_gpa, uint64_t guest_stack_gpa, uint64_t eptp);

int vmx_init(void){
#ifdef __x86_64__
    uint64_t cr4;
    __asm__ volatile("mov %%cr4,%0":"=r"(cr4));
    __asm__ volatile("mov %0,%%cr4"::"r"(cr4 | CR4_VMXE));

    uint64_t fc = rdmsr(MSR_IA32_FEATURE_CONTROL);
    if(!(fc & 1) || !(fc & (1<<2))){
        hv_log("ERR","feature control not VMX-enabled");
        return -1;
    }
    uint32_t rev = (uint32_t)(rdmsr(MSR_IA32_VMX_BASIC) & 0xffffffffu);
    ((uint32_t*)vmxon_region)[0] = rev;
    ((uint32_t*)vmcs_region)[0]  = rev;

    int ok;
    __asm__ volatile("vmxon %1; setna %0":"=r"(ok):"m"(vmxon_region):"cc","memory");
    if(ok){ hv_log("ERR","VMXON failed"); return -1; }

    /* Clear and load VMCS */
    __asm__ volatile("vmclear %0"::"m"(vmcs_region):"memory");
    __asm__ volatile("vmptrld %0"::"m"(vmcs_region):"memory");

    hv_log("INFO","VMXON ok");
    return 0;
#else
    hv_log("ERR","not x86_64");
    return -1;
#endif
}

int vmx_shutdown(void){
#ifdef __x86_64__
    __asm__ volatile("vmxoff");
    hv_log("INFO","VMXOFF");
    return 0;
#else
    return -1;
#endif
}

void vmx_run_loop(void){
    /* Build EPT, mark 0x1000 executable, leave 0x3000 non-exec */
    uint64_t eptp = ept_build_2m_4k_mix();
    ept_mark_exec(0x1000, 1);

    /* Place guest blob at 0x1000 and dummy stack at 0x1F00 */
    volatile uint8_t* mem = (volatile uint8_t*)0x1000;
    for (unsigned i=0;i<sizeof(guest_blob);++i) mem[i] = guest_blob[i];

    hv_log("INFO", "Launching guest at 0x1000 (should trap exec at 0x3000)");
    vmx_setup_and_launch(0x1000, 0x1F00, eptp);
}
