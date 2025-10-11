#pragma once
#include <stdint.h>

/* VMCS field encodings (subset) */
#define VMCS_HOST_CR0               0x6c00
#define VMCS_HOST_CR3               0x6c02
#define VMCS_HOST_CR4               0x6c04
#define VMCS_HOST_CS_SELECTOR       0x0c02
#define VMCS_HOST_DS_SELECTOR       0x0c06
#define VMCS_HOST_ES_SELECTOR       0x0c00
#define VMCS_HOST_FS_SELECTOR       0x0c08
#define VMCS_HOST_GS_SELECTOR       0x0c0a
#define VMCS_HOST_SS_SELECTOR       0x0c04
#define VMCS_HOST_TR_SELECTOR       0x0c0c
#define VMCS_HOST_FS_BASE           0x6c06
#define VMCS_HOST_GS_BASE           0x6c08
#define VMCS_HOST_TR_BASE           0x6c0a
#define VMCS_HOST_GDTR_BASE         0x6c0c
#define VMCS_HOST_IDTR_BASE         0x6c0e
#define VMCS_HOST_RSP               0x6c14
#define VMCS_HOST_RIP               0x6c16
#define VMCS_HOST_IA32_EFER         0x6c2a

#define VMCS_GUEST_CR0              0x6800
#define VMCS_GUEST_CR3              0x6802
#define VMCS_GUEST_CR4              0x6804
#define VMCS_GUEST_DR7              0x681a
#define VMCS_GUEST_RSP              0x681c
#define VMCS_GUEST_RIP              0x681e
#define VMCS_GUEST_RFLAGS           0x6820
#define VMCS_GUEST_CS_SELECTOR      0x0800
#define VMCS_GUEST_SS_SELECTOR      0x0804
#define VMCS_GUEST_DS_SELECTOR      0x0802
#define VMCS_GUEST_ES_SELECTOR      0x0800 /* reuse */
#define VMCS_GUEST_FS_SELECTOR      0x0808
#define VMCS_GUEST_GS_SELECTOR      0x080a
#define VMCS_GUEST_TR_SELECTOR      0x080e
#define VMCS_GUEST_CS_BASE          0x6808
#define VMCS_GUEST_SS_BASE          0x680a
#define VMCS_GUEST_DS_BASE          0x680c
#define VMCS_GUEST_ES_BASE          0x6806
#define VMCS_GUEST_FS_BASE          0x680e
#define VMCS_GUEST_GS_BASE          0x6810
#define VMCS_GUEST_GDTR_BASE        0x6816
#define VMCS_GUEST_IDTR_BASE        0x6818
#define VMCS_GUEST_CS_LIMIT         0x4802
#define VMCS_GUEST_SS_LIMIT         0x4808
#define VMCS_GUEST_DS_LIMIT         0x4804
#define VMCS_GUEST_ES_LIMIT         0x4800
#define VMCS_GUEST_FS_LIMIT         0x480a
#define VMCS_GUEST_GS_LIMIT         0x480c
#define VMCS_GUEST_TR_LIMIT         0x4810
#define VMCS_GUEST_CS_AR_BYTES      0x4816
#define VMCS_GUEST_SS_AR_BYTES      0x4818
#define VMCS_GUEST_DS_AR_BYTES      0x481a
#define VMCS_GUEST_ES_AR_BYTES      0x4814
#define VMCS_GUEST_FS_AR_BYTES      0x481c
#define VMCS_GUEST_GS_AR_BYTES      0x481e
#define VMCS_GUEST_TR_AR_BYTES      0x4822
#define VMCS_GUEST_IA32_EFER        0x2806

#define VMCS_PIN_BASED              0x4000
#define VMCS_CPU_BASED              0x4002
#define VMCS_SECONDARY_CTLS         0x401e
#define VMCS_EXCEPTION_BITMAP       0x4004
#define VMCS_VMEXIT_CTLS            0x400c
#define VMCS_VMENTRY_CTLS           0x4012
#define VMCS_EPT_POINTER            0x201a
#define VMCS_EXIT_REASON            0x4402
#define VMCS_EXIT_QUALIFICATION     0x6400

/* MSRs */
#define MSR_IA32_VMX_BASIC          0x480
#define MSR_IA32_FEATURE_CONTROL    0x3a
#define MSR_IA32_VMX_TRUE_PINBASED_CTLS    0x48d
#define MSR_IA32_VMX_TRUE_PROCBASED_CTLS   0x48e
#define MSR_IA32_VMX_PROCBASED_CTLS2       0x48b
#define MSR_IA32_VMX_TRUE_EXIT_CTLS        0x48f
#define MSR_IA32_VMX_TRUE_ENTRY_CTLS       0x490
#define MSR_IA32_VMX_EPT_VPID_CAP          0x48c
#define MSR_IA32_VMX_CR0_FIXED0            0x486
#define MSR_IA32_VMX_CR0_FIXED1            0x487
#define MSR_IA32_VMX_CR4_FIXED0            0x488
#define MSR_IA32_VMX_CR4_FIXED1            0x489
#define MSR_EFER                           0xc0000080

/* VMX helpers */
int vmx_setup_and_launch(uint64_t guest_entry_gpa, uint64_t guest_stack_gpa, uint64_t eptp);
void vmexit_loop(void);

/* Utils */
uint64_t rdmsr_u(uint32_t msr);
void wrmsr_u(uint32_t msr, uint64_t v);
