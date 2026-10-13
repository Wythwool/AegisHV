# Threat model

The Linux sensor trusts its host. It can correlate KVM trace events and dispatch
configured QMP actions, but cannot establish trustworthy telemetry after host
kernel compromise. A trace record also does not reveal a guest process or
module without additional, consistent VMI data.

Inputs include trace text, process metadata, optional XML snapshots, config,
QMP replies, and offline VMI fixtures. Malformed input must remain distinguishable
from missing or unsupported input. Queue overflow and output failures can leave
gaps, even when the process stays alive.

Identity mistakes matter most at QMP dispatch: a reused PID or ambiguous name
must not select another VM. Stable mapping, start-time checks, confidence, and
path restrictions reduce that risk. They still depend on trustworthy host data
and socket permissions.

The kernels run one vCPU. SVM and EL2 load bounded static ELF guests; Intel
still uses a fixed payload. Guest mappings exclude host memory, code is not
writable, data is not executable, and a timer bounds IRQ-masked guest loops.
Final host page tables remove aliases and add stack guards; x86 early boot
still uses Limine mappings. SVM/EL2 save GPR and FPU/SIMD state, but general OS
state, SMP, device/IOMMU isolation and general exception delivery are unfinished.
These kernels are not ready to isolate arbitrary hostile guests.
[Kernel notes](TYPE1_BOOT_BOUNDARY.md) list the supported paths and checks.

Offline VMI depends on matching memory, registers, and profiles from one
snapshot. Skew or wrong offsets can invalidate findings. Protected guest memory
is outside current support. Release signatures cannot establish the integrity
of a running host or its event stream.
