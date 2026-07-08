# Type-1 Lab Milestone Gate

The type-1 lab milestone is blocked until `docs/TYPE1_READINESS_GATE.md` passes. The current repository has model code and opt-in scripts, not a bootable hypervisor.

## Candidate Evidence

- boot image path and checksum;
- boot boundary manifest from `scripts/build-type1-skeleton.sh`;
- kernel ELF build manifest from `scripts/build-type1-kernel.sh`;
- kernel ELF inspection manifest from `scripts/inspect-type1-kernel.sh`;
- ISO-root staging manifest from `scripts/stage-type1-limine-iso.sh`;
- Limine ISO build manifest from `scripts/build-type1-limine-iso.sh`, when external tools are reviewed and available;
- image input manifest from `scripts/plan-type1-image.sh`;
- QEMU command line;
- serial log containing the expected marker;
- VM-exit trace;
- shutdown or crash record;
- host CPU, firmware, and QEMU versions;
- negative test showing unsupported hosts fail clearly.

## Release Notes Shape

Use "lab milestone" wording only after a bootable image and log evidence exist. Do not describe the current host-side binary as a type-1 hypervisor.
