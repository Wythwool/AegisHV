# Offline VMI

The VMI library reads supplied memory/register snapshots. It is not connected to
a running guest or the lab kernel's VM-exit frame.

It supports x86_64 four-level/LA57 translation and ARM64 stage-1 translation with
4, 16, and 64 KiB granules. Readers distinguish unmapped, denied, unavailable,
invalid, and partial data. Translation needs exact eight-byte table entries;
partial reads fail rather than padding missing bytes.

```sh
cargo run --locked -- vmi translate \
  --fixture tests/fixtures/vmi/x86_64_basic.vmi \
  --gva 0x0 --mode x86_64-4level --json
```

## Snapshot consistency

Memory, CR3/TTBR, TCR/SCTLR, registers, and profiles must describe the same point
in time. The loader validates structure and paths, not whether the supplied
snapshot is true. Missing values remain missing; an explicit zero is different.

The cache keys include VM, address space, root, mode, page/size, and access.
Lookup rejects addresses outside the key page, insertion rejects mismatched
page sizes, and failed translations are not cached. VMID, root, ASID, and full
invalidation remove matching entries.

## Profiles and fixtures

The registry starts empty. Lookup is exact by OS, architecture, build identity,
and optional variant. Duplicate keys and missing profiles are errors. Shipped
profiles are synthetic test data, not support for a particular OS build.

- [Fixture format](VMI_FIXTURES.md)
- [Linux profiles](VMI_LINUX.md)
- [Windows profiles](VMI_WINDOWS.md)
- [Windows symbol cache](VMI_WINDOWS_SYMBOLS.md)

Fixture references must stay relative and inside the fixture directory.
Absolute paths, drive/UNC paths, traversal, backslashes, and colons are rejected.

Live memory/register collection and protected guest inspection are unavailable.
A future reader would need a consistency protocol for guest execution and page
table changes. Offline tests do not exercise those races.
