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

The kernel only runs a fixed Intel guest on one BSP. It has no general loader,
SMP, device/IOMMU backend, full architectural-state context switch, or general
exception delivery. Its final page tables remove aliases and add guards, but
early boot still uses Limine mappings. It is not ready to isolate arbitrary
hostile guests. [Kernel notes](TYPE1_BOOT_BOUNDARY.md) list the exact sequence.

Offline VMI depends on matching memory, registers, and profiles from one
snapshot. Skew or wrong offsets can invalidate findings. Protected guest memory
is outside current support. Release signatures cannot establish the integrity
of a running host or its event stream.
