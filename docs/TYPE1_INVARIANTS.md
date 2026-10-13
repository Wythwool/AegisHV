# Kernel invariants

- The BSP owns VMX state. VMXON, VMCS, guest/EPT, and bitmap pages come from one
  allocation ledger. Kernel pages and the inherited CR3 root are reserved first.
- Limine-reclaimable memory is excluded. Physical references must be aligned,
  distinct where required, and inside the checked address range.
- The final Intel path finishes HHDM access before switching CR3. Its four-level
  hierarchy maps the linked kernel window as RX, R/NX, and RW/NX; null, identity,
  HHDM, and the five lower stack guards are absent. Early boot still uses Limine
  mappings, so those guards are not active yet.
- Host PAT entry zero must be write-back. Captured host state and guest PAT are
  read back after each exit. The host text must contain no FPU/SIMD instructions
  because extended state is not saved.
- VMCS control bits, bitmap addresses, guest state, and host state are checked
  before entry. I/O requests are contained; only guest `RDMSR IA32_PAT` is allowed
  through the MSR bitmap. Other requests must match the fixed exit sequence.
- The timer starts with a zero sentinel, then a reload of at least two with
  `reload << rate <= 0x01000000` TSC ticks. Unexpected exits, late timers, or
  delivery faults stop the run.
- The fixed #UD injection writes its valid bit last. Success requires the final
  HLT, expected cookie/RIP/stack, and cleared entry/vectoring valid bits.

The core ring model records overflow. VM and vCPU models reject invalid state
transitions. These library checks do not supply a live SMP scheduler, interrupt
controller, dynamic mapping policy, or device-isolation backend.

[Boot and evidence format](TYPE1_BOOT_BOUNDARY.md).
