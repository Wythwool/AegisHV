# Windows VMI fixtures

These files are synthetic inputs for parser and offline inspector tests.

- `../../profiles/windows/synthetic_x86_64.profile` is a hand-written Windows x86_64 profile with exact build/PDB identity, selected symbols, selected structure offsets, syscall records, and protection-limit metadata.
- `synthetic_symbols.cache` is a pre-extracted symbol cache fixture with the same PDB identity.

They do not represent Microsoft binaries, a real memory dump, or a live guest. They are only for deterministic unit tests.
