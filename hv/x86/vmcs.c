#include "vmcs.h"
#include "../common/log.h"
#include <stdint.h>

static inline void vmwrite(uint64_t field, uint64_t val){ __asm__ volatile("vmwrite %1,%0"::"r"(field),"r"(val):"cc"); }
static inline uint64_t vmread(uint64_t field){ uint64_t val; __asm__ volatile("vmread %1,%0":"=r"(val):"r"(field):"cc"); return val; }

uint64_t rdmsr_u(uint32_t msr){ uint32_t lo,hi; __asm__ volatile("rdmsr":"=a"(lo),"=d"(hi):"c"(msr)); return ((uint64_t)hi<<32)|lo; }
void wrmsr_u(uint32_t msr, uint64_t v){ uint32_t lo=(uint32_t)v, hi=(uint32_t)(v>>32); __asm__ volatile("wrmsr"::"c"(msr),"a"(lo),"d"(hi)); }

static inline uint64_t adjust_ctrl(uint64_t req, uint32_t msr_true){
    uint64_t msr = rdmsr_u(msr_true);
    uint32_t allow0 = (uint32_t)msr;
    uint32_t allow1 = (uint32_t)(msr>>32);
    req |= allow0;        /* must-be-1 */
    req &= allow1;        /* must-be-0 */
    return req;
}

extern void vmx_run_loop(void);

/* Minimal VMCS init for real-mode guest */
int vmx_setup_and_launch(uint64_t guest_entry_gpa, uint64_t guest_stack_gpa, uint64_t eptp){
    /* Host state snapshot */
    uint16_t cs, ds, es, fs, gs, ss, tr;
    uint64_t cr0, cr3, cr4, rsp, rip, efer;
    struct { uint16_t limit; uint64_t base; } __attribute__((packed)) gdtr, idtr;

    __asm__ volatile("mov %%cs,%0":"=r"(cs));
    __asm__ volatile("mov %%ds,%0":"=r"(ds));
    __asm__ volatile("mov %%es,%0":"=r"(es));
    __asm__ volatile("mov %%fs,%0":"=r"(fs));
    __asm__ volatile("mov %%gs,%0":"=r"(gs));
    __asm__ volatile("mov %%ss,%0":"=r"(ss));
    __asm__ volatile("str %0":"=r"(tr));
    __asm__ volatile("mov %%cr0,%0":"=r"(cr0));
    __asm__ volatile("mov %%cr3,%0":"=r"(cr3));
    __asm__ volatile("mov %%cr4,%0":"=r"(cr4));
    __asm__ volatile("lea (%%rip), %0":"=r"(rip));
    __asm__ volatile("mov %%rsp,%0":"=r"(rsp));
    efer = rdmsr_u(MSR_EFER);
    __asm__ volatile("sgdt %0" : "=m"(gdtr));
    __asm__ volatile("sidt %0" : "=m"(idtr));

    /* Controls */
    uint64_t pin_ctls  = adjust_ctrl(0, MSR_IA32_VMX_TRUE_PINBASED_CTLS);
    uint64_t cpu_ctls  = adjust_ctrl((1ull<<31), MSR_IA32_VMX_TRUE_PROCBASED_CTLS); /* ACTIVATE_SECONDARY=bit31 */
    uint64_t cpu2      = adjust_ctrl((1ull<<1) /* EPT */, MSR_IA32_VMX_PROCBASED_CTLS2);
    uint64_t exit_ctls = adjust_ctrl((1ull<<9) /* host 64-bit */, MSR_IA32_VMX_TRUE_EXIT_CTLS);
    uint64_t entry_ctls= adjust_ctrl(0, MSR_IA32_VMX_TRUE_ENTRY_CTLS);

    vmwrite(VMCS_PIN_BASED, pin_ctls);
    vmwrite(VMCS_CPU_BASED, cpu_ctls);
    vmwrite(VMCS_SECONDARY_CTLS, cpu2);
    vmwrite(VMCS_VMEXIT_CTLS, exit_ctls);
    vmwrite(VMCS_VMENTRY_CTLS, entry_ctls);

    vmwrite(VMCS_EPT_POINTER, eptp);

    /* Guest: real mode (PE=0, PG=0, EFER=0); simple flat segments */
    uint64_t cr0_fixed0 = rdmsr_u(MSR_IA32_VMX_CR0_FIXED0);
    uint64_t cr0_fixed1 = rdmsr_u(MSR_IA32_VMX_CR0_FIXED1);
    uint64_t cr4_fixed0 = rdmsr_u(MSR_IA32_VMX_CR4_FIXED0);
    uint64_t cr4_fixed1 = rdmsr_u(MSR_IA32_VMX_CR4_FIXED1);
    uint64_t gcr0 = (0 /* real mode */) | (cr0_fixed0);
    gcr0 &= cr0_fixed1;
    uint64_t gcr4 = (0) | (cr4_fixed0);
    gcr4 &= cr4_fixed1;

    vmwrite(VMCS_GUEST_CR0, gcr0);
    vmwrite(VMCS_GUEST_CR3, 0);
    vmwrite(VMCS_GUEST_CR4, gcr4);
    vmwrite(VMCS_GUEST_DR7, 0);
    vmwrite(VMCS_GUEST_RSP, guest_stack_gpa);
    vmwrite(VMCS_GUEST_RIP, guest_entry_gpa);
    vmwrite(VMCS_GUEST_RFLAGS, 0x2);
    vmwrite(VMCS_GUEST_IA32_EFER, 0);

    /* Real-mode segment descriptors: base=selector<<4, limit=0xFFFF, AR: present code/data */
    auto set_seg = [](uint64_t sel_field, uint64_t base_field, uint64_t limit_field, uint64_t ar_field, uint16_t sel, uint16_t ar){
        vmwrite(sel_field, sel);
        vmwrite(base_field, (uint64_t)sel<<4);
        vmwrite(limit_field, 0xFFFF);
        vmwrite(ar_field, ar);
    };

    set_seg(VMCS_GUEST_CS_SELECTOR, VMCS_GUEST_CS_BASE, VMCS_GUEST_CS_LIMIT, VMCS_GUEST_CS_AR_BYTES, 0x0000, 0x9b);
    set_seg(VMCS_GUEST_SS_SELECTOR, VMCS_GUEST_SS_BASE, VMCS_GUEST_SS_LIMIT, VMCS_GUEST_SS_AR_BYTES, 0x0000, 0x93);
    set_seg(VMCS_GUEST_DS_SELECTOR, VMCS_GUEST_DS_BASE, VMCS_GUEST_DS_LIMIT, VMCS_GUEST_DS_AR_BYTES, 0x0000, 0x93);
    set_seg(VMCS_GUEST_ES_SELECTOR, VMCS_GUEST_ES_BASE, VMCS_GUEST_ES_LIMIT, VMCS_GUEST_ES_AR_BYTES, 0x0000, 0x93);
    set_seg(VMCS_GUEST_FS_SELECTOR, VMCS_GUEST_FS_BASE, VMCS_GUEST_FS_LIMIT, VMCS_GUEST_FS_AR_BYTES, 0x0000, 0x93);
    set_seg(VMCS_GUEST_GS_SELECTOR, VMCS_GUEST_GS_BASE, VMCS_GUEST_GS_LIMIT, VMCS_GUEST_GS_AR_BYTES, 0x0000, 0x93);

    vmwrite(VMCS_GUEST_TR_SELECTOR, 0);
    vmwrite(VMCS_GUEST_GDTR_BASE, 0);
    vmwrite(VMCS_GUEST_IDTR_BASE, 0);

    /* Host state */
    vmwrite(VMCS_HOST_CR0, cr0);
    vmwrite(VMCS_HOST_CR3, cr3);
    vmwrite(VMCS_HOST_CR4, cr4);
    vmwrite(VMCS_HOST_CS_SELECTOR, cs);
    vmwrite(VMCS_HOST_DS_SELECTOR, ds);
    vmwrite(VMCS_HOST_ES_SELECTOR, es);
    vmwrite(VMCS_HOST_FS_SELECTOR, fs);
    vmwrite(VMCS_HOST_GS_SELECTOR, gs);
    vmwrite(VMCS_HOST_SS_SELECTOR, ss);
    vmwrite(VMCS_HOST_TR_SELECTOR, tr);
    vmwrite(VMCS_HOST_GDTR_BASE, gdtr.base);
    vmwrite(VMCS_HOST_IDTR_BASE, idtr.base);
    vmwrite(VMCS_HOST_RSP, (uint64_t)((uint8_t*)(&guest_entry_gpa) + 0x4000)); /* reuse current stack area */
    extern void vmexit_entry(void);
    vmwrite(VMCS_HOST_RIP, (uint64_t)&vmexit_entry);
    vmwrite(VMCS_HOST_IA32_EFER, efer);

    /* Launch */
    int ok;
    __asm__ volatile("vmlaunch; setna %0":"=r"(ok)::"cc","memory");
    if(ok){
        hv_log("ERR", "VMLAUNCH failed");
        return -1;
    }
    return 0;
}

/* vmexit trampoline */
__attribute__((naked)) void vmexit_entry(void){
    __asm__ volatile(
        "push %rax; push %rbx; push %rcx; push %rdx; push %rsi; push %rdi; push %rbp; push %r8; push %r9; push %r10; push %r11; push %r12; push %r13; push %r14; push %r15;"
        "call vmexit_loop;"
        "pop %r15; pop %r14; pop %r13; pop %r12; pop %r11; pop %r10; pop %r9; pop %r8; pop %rbp; pop %rdi; pop %rsi; pop %rdx; pop %rcx; pop %rbx; pop %rax;"
        "vmresume;"
        "jmp .hang\n.hang: hlt; jmp .hang"
    );
}

void vmexit_loop(void){
    uint64_t reason = vmread(VMCS_EXIT_REASON);
    if ((reason & 0xffff) == 48){ /* EPT violation */
        hv_log("INFO", "EPT execute violation trapped");
    } else {
        hv_log("INFO", "VMEXIT (reason code not 48)");
    }
}
