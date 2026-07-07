# Linux VMI profile metadata

The Linux profile path is synthetic/offline and x86_64-only. It stores profile metadata, parses kallsyms/System.map-style symbol maps, and can resolve a KASLR slide from fixed metadata, a known slide, or bounded profile anchors against an offline reader.

No real Linux kernel profile data ships by default. Test profiles are hand-written fixtures for parser, registry, and offline resolver behavior.

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
- `kaslr_anchor=<symbol-name>,<hex-bytes>,<max-slide>,<step>`

Hex and decimal integers are accepted. Duplicate symbol names, duplicate struct field offsets, duplicate syscall numbers, and duplicate syscall names are rejected. Unknown keys are rejected.

`kaslr_anchor` is used only when KASLR is marked `unknown-unsupported` and the caller supplies an offline virtual-memory reader. The resolver checks candidate slides from `0` through `max-slide` using `step`, and accepts only one slide where every configured anchor matches. Zero matches and multiple matches are explicit errors.

## Symbol maps

The kallsyms/System.map loader parses the normal `address type name` fields. kallsyms may also include a `[module]` suffix. Duplicate names are preserved because real kernels can expose aliases, but APIs that require a unique symbol refuse ambiguous names.

## Registry key

The loader preserves `kernel_release` and `kernel_build` separately. For the existing OS profile registry, the Linux profile uses:

```text
kernel_or_build = <kernel_release>#<kernel_build>
```

Lookup is still exact by OS kind, architecture, this combined kernel/build key, and optional variant. There is no nearest-match fallback.

## Limits

This format does not prove that the data matches a real kernel. It does not walk `task_struct`, modules, or syscall tables yet. KASLR resolution is limited to fixed metadata, a known profile slide, or bounded synthetic/offline anchor scans. There is no live Linux guest backend.
