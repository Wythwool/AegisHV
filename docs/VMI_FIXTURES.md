# VMI Fixtures

AegisHV VMI fixtures are tiny offline inputs for tests. They do not read a running VM and they do not describe supported guest OS layouts.

Fixture manifests use `aegishv-vmi-fixture-v1` as the first logical line. Blank lines and comments beginning with `#` are ignored.

Required fields:

- `id=<fixture-id>`
- `name=<short-name>`
- `arch=x86_64` or `arch=arm64`
- `memory=<relative memory map path>`
- `registers=<relative register fixture path>`

Profile metadata is optional. Use `profile=none` for fixtures with no OS profile identity. Otherwise provide:

- `os=linux` or `os=windows`
- `kernel_or_build=<kernel-release-or-build>`
- `variant=<optional-variant>`

Expected translations are data records. They are not executed by the loader:

`translation=name=<case> gva=<u64> gpa=<u64> page_size=<u64> mode=<mode> readable=<bool> writable=<bool> executable=<bool> user=<bool>`

Supported translation mode names:

- `x86_64-4level`
- `x86_64-la57`
- `arm64-stage1-4k`
- `arm64-stage1-16k`
- `arm64-stage1-64k`

The fixture architecture must match the register snapshot architecture and every translation mode. Page sizes are limited by mode:

- x86_64 modes: `0x1000`, `0x200000`, `0x40000000`
- ARM64 4 KiB granule: `0x1000`, `0x200000`, `0x40000000`
- ARM64 16 KiB granule: `0x4000`, `0x2000000`, `0x1000000000`
- ARM64 64 KiB granule: `0x10000`, `0x20000000`

Paths are resolved relative to the fixture manifest directory. Absolute paths, Windows drive paths, UNC-like paths, backslashes, `:`, `.`, and `..` components are rejected.

Memory maps use `aegishv-memory-map-v1`. `map` reads bytes from a relative backing file. `bytes` maps a short inline hex byte string. `deny` and `unavailable` mark fixture ranges that must not read successfully.

The offline translate command executes one translation from a fixture and prints compact JSON:

`cargo run -- vmi translate --fixture tests/fixtures/vmi/x86_64_basic.vmi --gva 0x0 --mode x86_64-4level --json`

The command is fixture tooling. It does not start the sensor runtime, bind metrics, read live tracefs, or call QMP.
