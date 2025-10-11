// Extremely small KVM demo: 16-bit real-mode guest that does CPUID and HLT.
// Emits VMEXIT reasons to feed the pipeline.
use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_LOG_DIRTY_PAGES, KVM_EXIT_HLT, KVM_EXIT_IO, KVM_EXIT_FAIL_ENTRY, KVM_EXIT_INTERNAL_ERROR, KVM_EXIT_CPUID};
use kvm_ioctls::{Kvm, VmFd, VcpuFd};
use std::io::Write;
use std::ptr::null_mut;
use std::slice;
use std::fs::File;

const MEM_SIZE: usize = 0x20000; // 128 KiB
const GUEST_ADDR: u64 = 0x0000;

pub fn run_hlt_cpuid_demo() -> anyhow::Result<Vec<(String,u64)>> {
    let kvm = Kvm::new()?;
    let vm = kvm.create_vm()?;

    // allocate guest memory
    let mem = unsafe {
        let mem_ptr = libc::mmap(null_mut(), MEM_SIZE, libc::PROT_READ | libc::PROT_WRITE,
                                 libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE, -1, 0);
        if mem_ptr == libc::MAP_FAILED { anyhow::bail!("mmap failed"); }
        slice::from_raw_parts_mut(mem_ptr as *mut u8, MEM_SIZE)
    };

    let region = kvm_userspace_memory_region {
        slot: 0,
        guest_phys_addr: GUEST_ADDR,
        memory_size: MEM_SIZE as u64,
        userspace_addr: mem.as_ptr() as u64,
        flags: 0,
    };
    unsafe { vm.set_user_memory_region(region)?; }

    // 16-bit code: CPUID; HLT; HLT;
    let code: [u8; 9] = [
        0x31,0xC0,          // xor ax,ax
        0x0F,0xA2,          // cpuid
        0xF4,               // hlt
        0xF4,               // hlt
        0xEB,0xFE           // jmp $
    ];
    mem[0x1000..0x1000+code.len()].copy_from_slice(&code);

    let vcpu_fd = vm.create_vcpu(0)?;

    // Real mode: set CS:IP to 0x0000:0x1000
    let mut sregs = vcpu_fd.get_sregs()?;
    sregs.cs.base = 0;
    sregs.cs.selector = 0;
    vcpu_fd.set_sregs(&sregs)?;
    let mut regs = vcpu_fd.get_regs()?;
    regs.rip = 0x1000;
    regs.rax = 0x0;
    regs.rbx = 0x0;
    vcpu_fd.set_regs(&regs)?;

    let mut events = Vec::new();
    loop {
        match vcpu_fd.run()? {
            kvm_ioctls::VcpuExit::Hlt => {
                let rip = vcpu_fd.get_regs()?.rip;
                events.push(("hlt".to_string(), rip));
                break;
            }
            kvm_ioctls::VcpuExit::IoIn(_port, _data) |
            kvm_ioctls::VcpuExit::IoOut(_port, _data) => {
                let rip = vcpu_fd.get_regs()?.rip;
                events.push(("io".to_string(), rip));
            }
            kvm_ioctls::VcpuExit::Cpuid => {
                let rip = vcpu_fd.get_regs()?.rip;
                events.push(("cpuid".to_string(), rip));
            }
            other => {
                let rip = vcpu_fd.get_regs()?.rip;
                events.push((format!("{:?}", other), rip));
                break;
            }
        }
    }
    Ok(events)
}
