# Windows VMI profile metadata

The Windows profile path is synthetic/offline and x86_64-only. It stores exact build and PDB identity, offline symbol RVAs, selected structure offsets, syscall names, and explicit protection-limit metadata for callers that already have a stable memory/register snapshot.

No real Windows profile data ships by default. Test profiles are synthetic fixtures for parser, registry, and offline walker behavior.

## Format

The first logical line is:

```text
aegishv-windows-profile-v1
```

Blank lines and `#` comments are ignored.

Required identity fields:

- `os=windows`
- `arch=x86_64`
- `build=<windows-build>`
- `pdb_file=<nt-kernel-pdb-name>`
- `pdb_guid=<pdb-guid-without-dashes>`
- `pdb_age=<age>`

Optional identity field:

- `variant=<variant>`

Metadata records:

- `symbol=<name>,<rva>[,<size>]`
- `offset=<struct-name>,<field-name>,<byte-offset>[,<size>]`
- `syscall=<number>,<name>[,<symbol-name>]`
- `limit=<vbs|hvci|confidential_guest>,<not_present|degraded|unsupported>,<detail>`

Hex and decimal integers are accepted. Duplicate symbol names, duplicate structure field offsets, duplicate syscall numbers, duplicate syscall names, and unknown keys are rejected.

## Registry key

The Windows profile registry key is exact:

```text
kernel_or_build = <build>#<pdb_guid>#<pdb_age>
```

Lookup is exact by OS kind, architecture, this combined key, and optional variant. There is no nearest-match fallback.

## Offline Windows inspection

The current Windows inspection code is fixture-driven. It does not read a live guest. Callers provide a profile, an offline virtual-memory reader, register snapshots, and known executable ranges.

Implemented synthetic checks:

- ntoskrnl base resolution from explicit PE-header candidates.
- `EPROCESS` list walking through `PsInitialSystemProcess` and profile-provided offsets.
- current-process attribution through an explicit `aegishv_current_eprocess` pointer symbol or CR3 matching against `EPROCESS.DirectoryTableBase`.
- loaded-module walking through `PsLoadedModuleList` and `KLDR_DATA_TABLE_ENTRY` offsets.
- SSDT handler inspection through `KeServiceDescriptorTable` using Windows x64 signed service-table offsets.
- `MSR_LSTAR` inspection against `KiSystemCall64` and known executable ranges.
- IDT and GDT checks from x86_64 register snapshots.
- process-create callback inventory through `PspCreateProcessNotifyRoutine`.
- kernel and driver text hashing against caller-provided SHA-256 baselines.
- protection-limit reporting for VBS, HVCI, and confidential guest states.
- an off-hot-path Windows detector runner that combines LSTAR, SSDT, descriptors, callbacks, text hashes, and protection-limit checks.

## Limits

This format does not prove that the data matches a real Windows kernel. The walkers and detector runner operate on synthetic/offline readers only. They are x86_64 profile-gated and refuse missing symbols or offsets instead of inventing success.

There is no live Windows guest backend. Real Windows profile extraction is not shipped. Live process/module/syscall/callback reads require a backend that can provide stable guest memory and register snapshots for the same point in time.

VBS, HVCI, confidential guest modes, encrypted memory, and restricted register access are reported as explicit profile limitations when known. AegisHV does not claim a bypass.
