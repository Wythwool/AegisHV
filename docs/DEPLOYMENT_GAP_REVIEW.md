# Deployment Gap Review

This review is deliberately strict. It lists gaps that block stronger release claims.

## Host Sensor Gaps

- Live tracefs evidence is opt-in and host dependent.
- Package install tests inspect files but do not install packages as root.
- AppArmor, SELinux, and seccomp checks inspect policy material but do not prove enforcement on every distribution.
- Syslog and journald outputs have bounded tests, but fleet-level log pipeline behavior is outside the repository.
- QMP actions are guarded by stable identity rules, but live libvirt lifecycle integration is not implemented.

## VMI Gaps

- Live guest memory reads are not implemented.
- Live guest register reads are not implemented.
- Real Linux and Windows profile extraction is not implemented.
- Offline fixtures do not prove guest OS coverage.
- Confidential guest modes can block inspection.

## Type-1 Gaps Not Closed

- Bootable type-1 image is not present in this repository.
- Bare-metal VMX, SVM, or EL2 runtime backend is not present.
- Direct EPT/NPT/Stage-2 enforcement is not implemented.
- Device isolation is modeled but not programmed into hardware.

## Release Decision

The host-side sensor can keep moving through host-sensor release gates. VMI alpha and type-1 lab milestones must remain separate until their gates have evidence.
