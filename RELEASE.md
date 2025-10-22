# Release

- Tag `vX.Y.Z` (SemVer), update `CHANGELOG.md`.
- Build and attach binaries for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`.
```bash
cargo build --release
strip target/release/aegishv || true
```
- Publish container image:
```bash
docker build -t ghcr.io/<you>/aegishv:vX.Y.Z .
docker push ghcr.io/<you>/aegishv:vX.Y.Z
```
