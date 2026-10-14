# Intel VMX lab

`aegishv-arch-x86::vmx` contains instruction wrappers, capability/control checks,
VMXON/VMCS region handling, VMCS configuration, EPT, and the fixed guest exit
state machine. Unit tests use mocks. The boot kernel wires the Intel path;
[kernel notes](TYPE1_BOOT_BOUNDARY.md) list its entry sequence and serial markers.

`scripts/vmx-linux-lab-smoke.sh` boots a separately supplied Linux kernel under
QEMU. It needs `AEGISHV_VMX_LAB_KERNEL`; `--print-command` shows the command.
That helper is not the Limine kernel runner. Use `scripts/run-type1-lab.sh` for
the AegisHV image.

Nested VMX requires a KVM host exposing VMX to the nested guest. TCG can exercise
Limine and preflight but cannot complete the VMX guest run. The full exit suite
passes in Bochs 3.1 and in nested KVM on
[Xeon Platinum 8573C](https://github.com/Wythwool/AegisHV/actions/runs/38596593726).
Direct bare-metal boot remains unverified.

The manual `nested-kvm.yml` workflow accepts `backend=vmx` to reject AMD runners
before building. Hosted runner labels do not guarantee a CPU vendor.
