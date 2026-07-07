# VMI Alpha Gate

The VMI alpha gate is for offline fixtures and translation infrastructure. It must not be described as full VMI.

## Required Gates

- Offline x86_64 four-level and LA57 translation fixtures pass.
- Offline ARM64 stage-1 translation fixtures pass.
- Linux synthetic profile fixtures cover task, module, syscall, hook, and BPF metadata.
- Windows synthetic fixtures cover symbol cache, process/module, SSDT, LSTAR, and callback metadata.
- Fixture safety tests reject unsafe paths and malformed inputs.
- Documentation lists exact supported fixture formats and unsupported live behavior.

## Current Scope

The repository has offline translation, profile, and fixture tests. Live guest reads and real OS profile extraction are not implemented.

## Release Notes Shape

Use "offline VMI fixture alpha" wording. Do not imply live inspection of running guests.
