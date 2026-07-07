# Linux VMI profile metadata

P078 adds a synthetic/offline Linux x86_64 profile metadata format. It does not add Linux guest inspection, KASLR resolution, kallsyms parsing, System.map parsing, task walking, syscall table resolution, or syscall integrity checks.

No real Linux kernel profile data ships by default. Test profiles are hand-written fixtures for parser and registry behavior.

## Format

The first logical line is:

```text
aegishv-linux-profile-v1
```

Blank lines and `#` comments are ignored.

Required identity fields:

- `os=linux`
- `arch=x86_64`
- `kernel_release=<release>`
- `kernel_build=<build-or-build-id>`
- `kaslr=<fixed|none|slide-known|unknown-unsupported>`

Optional identity field:

- `variant=<variant>`

`kaslr=slide-known` requires `kaslr_slide=<u64>`. `kaslr=fixed`, `kaslr=none`, and `kaslr=unknown-unsupported` must not set a slide. P079 handles KASLR base resolution later.

Metadata records:

- `symbol=<name>,<virtual-address>[,<size>]`
- `offset=<struct-name>,<field-name>,<byte-offset>[,<size>]`
- `syscall=<number>,<name>[,<symbol-name>]`

Hex and decimal integers are accepted. Duplicate symbol names, duplicate struct field offsets, duplicate syscall numbers, and duplicate syscall names are rejected. Unknown keys are rejected.

## Registry key

The loader preserves `kernel_release` and `kernel_build` separately. For the existing OS profile registry, the Linux profile uses:

```text
kernel_or_build = <kernel_release>#<kernel_build>
```

Lookup is still exact by OS kind, architecture, this combined kernel/build key, and optional variant. There is no nearest-match fallback.

## Limits

This format is metadata storage only. It does not prove that the data matches a real kernel. It does not load real kernel symbols. It does not resolve KASLR. It does not walk `task_struct`, modules, or syscall tables. Later work must add those pieces with real inputs and tests before any stronger Linux VMI claim.
