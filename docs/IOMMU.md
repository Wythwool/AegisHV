# DMA remapping

The x86 kernel reads ACPI DMAR before dropping the bootloader mappings. With a
supported VT-d unit, it installs a zeroed root table and enables DMA translation
before entering the guest. Every requester starts without a context. There are
no implicit identity mappings of host RAM.

The driver uses legacy root/context tables, register-based context and IOTLB
invalidation, and bounded completion waits. Table writes are flushed before
publication. Register mappings use UC, are non-executable, and are absent from
the guest's second-level page tables. Unsupported units or failed invalidations
stop boot. A machine without DMAR reports `aegishv:iommu:unavailable`.

## Test

```sh
AEGISHV_IOMMU_TEST=1 bash scripts/build-type1-limine-iso.sh --build-kernel
python3 scripts/test-type1-iommu.py target/type1/aegishv-type1.iso
```

This boots with QEMU's `intel-iommu` and PCI EDU device. EDU issues DMA reads and
writes through the IOMMU. The test checks a whole page of data, rather than only
the device's completion flag:

- A write with no context leaves the destination unchanged and records a fault.
- One mapped page survives a DMA round trip with 512 distinct u64 values.
- DMA to the adjacent IOVA fails.
- Read-only permission rejects a write but permits a read.
- Removing the mapping and invalidating caches blocks both directions.

Fault records must contain the expected source ID, page address, access type
and reason. The device is quiesced and the context removed before completion.
The Linux `OS boot` workflow retains serial output, image hashes and the ISO.

## Scope

Currently supported: one legacy VT-d unit in PCI segment 0, 39-bit second-level
translation, an include-all scope or direct endpoint scopes on bus 0. RMRR,
bridge paths and additional units require further device ownership handling
and are rejected. The test's temporary domain maps one 4 KiB page; normal boot
keeps all DMA blocked. There is no guest PCI assignment API yet.

This has run with QEMU VT-d emulation. Physical VT-d, AMD-Vi and Arm SMMU have
not been exercised. Interrupt remapping, ATS/PASID and PCI hotplug are not
implemented. DMA quarantine alone is not sufficient for general passthrough.

References: [Intel VT-d](https://www.intel.com/content/www/us/en/content-details/868911/intel-virtualization-technology-for-directed-i-o-architecture-specification.html)
and [QEMU EDU](https://www.qemu.org/docs/master/specs/edu.html).
