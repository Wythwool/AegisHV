#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-x86_64-unknown-linux-gnu}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

case "$TARGET" in
  x86_64-unknown-linux-gnu)
    ARCH="amd64"
    ;;
  aarch64-unknown-linux-gnu)
    ARCH="arm64"
    ;;
  *)
    echo "Debian packaging supports GNU Linux targets only: x86_64-unknown-linux-gnu or aarch64-unknown-linux-gnu" >&2
    exit 2
    ;;
esac

if ! command -v dpkg-deb >/dev/null 2>&1; then
  echo "dpkg-deb is required to build the Debian package" >&2
  exit 127
fi

VERSION="$(grep '^version = ' Cargo.toml | head -1 | cut -d '"' -f2)"
BIN="${AEGISHV_BINARY:-target/${TARGET}/release/aegishv}"
if [[ ! -x "$BIN" ]]; then
  BIN="${AEGISHV_BINARY:-target/release/aegishv}"
fi
if [[ ! -x "$BIN" ]]; then
  echo "release binary is missing or not executable; build with cargo or cross before packaging" >&2
  exit 2
fi

PKGROOT="dist/debian/aegishv_${VERSION}_${ARCH}"
OUT="dist/aegishv_${VERSION}_${ARCH}.deb"
rm -rf "$PKGROOT"
mkdir -p "$PKGROOT/DEBIAN"

install -D -m 0755 "$BIN" "$PKGROOT/usr/bin/aegishv"
install -D -m 0644 config.example.toml "$PKGROOT/etc/aegishv/config.toml"
install -D -m 0644 schema/event.schema.json "$PKGROOT/usr/share/aegishv/schema/event.schema.json"
install -D -m 0644 schema/snapshot.schema.json "$PKGROOT/usr/share/aegishv/schema/snapshot.schema.json"
install -D -m 0644 packaging/seccomp/aegishv-seccomp.json "$PKGROOT/usr/share/aegishv/seccomp/aegishv-seccomp.json"
install -D -m 0644 packaging/apparmor/usr.bin.aegishv "$PKGROOT/usr/share/aegishv/apparmor/usr.bin.aegishv"
install -D -m 0644 packaging/selinux/aegishv.te "$PKGROOT/usr/share/aegishv/selinux/aegishv.te"
install -D -m 0644 packaging/selinux/aegishv.fc "$PKGROOT/usr/share/aegishv/selinux/aegishv.fc"
install -D -m 0644 packaging/selinux/aegishv.if "$PKGROOT/usr/share/aegishv/selinux/aegishv.if"
install -D -m 0644 packaging/selinux/README.md "$PKGROOT/usr/share/aegishv/selinux/README.md"
install -D -m 0644 packaging/debian/aegishv.service "$PKGROOT/usr/lib/systemd/system/aegishv.service"
install -D -m 0644 packaging/debian/aegishv.tmpfiles "$PKGROOT/usr/lib/tmpfiles.d/aegishv.conf"
install -D -m 0755 scripts/live-tracefs-smoke.sh "$PKGROOT/usr/share/aegishv/scripts/live-tracefs-smoke.sh"
install -D -m 0755 scripts/smoke-replay.sh "$PKGROOT/usr/share/aegishv/scripts/smoke-replay.sh"
install -D -m 0755 scripts/validate-jsonl-schema.py "$PKGROOT/usr/share/aegishv/scripts/validate-jsonl-schema.py"
install -D -m 0644 README.md "$PKGROOT/usr/share/doc/aegishv/README.md"
install -D -m 0644 CHANGELOG.md "$PKGROOT/usr/share/doc/aegishv/CHANGELOG.md"
install -D -m 0644 RELEASE.md "$PKGROOT/usr/share/doc/aegishv/RELEASE.md"
install -D -m 0644 packaging/debian/copyright "$PKGROOT/usr/share/doc/aegishv/copyright"
install -D -m 0644 packaging/debian/README.md "$PKGROOT/usr/share/doc/aegishv/DEBIAN_PACKAGING.md"
for doc in docs/*.md; do
  install -D -m 0644 "$doc" "$PKGROOT/usr/share/doc/aegishv/$(basename "$doc")"
done

install -m 0755 packaging/debian/postinst "$PKGROOT/DEBIAN/postinst"
install -m 0755 packaging/debian/postrm "$PKGROOT/DEBIAN/postrm"
install -m 0644 packaging/debian/conffiles "$PKGROOT/DEBIAN/conffiles"

cat > "$PKGROOT/DEBIAN/control" <<EOF
Package: aegishv
Version: ${VERSION}
Section: admin
Priority: optional
Architecture: ${ARCH}
Maintainer: Nullbit1 <noreply@github.com>
Depends: adduser, systemd-tmpfiles | systemd
Homepage: https://github.com/Nullbit1/AegisHV
Description: host-side KVM telemetry sensor
 AegisHV reads Linux KVM tracefs text, emits JSONL telemetry, exposes
 Prometheus-style metrics, correlates W^X patterns, and can call configured
 QMP actions. This package installs the current host-side sensor and its
 operator files. It does not provide type-1 hypervisor support, full VMI,
 EPT/NPT enforcement, syscall-path integrity, or hardware PMU sampling.
EOF

mkdir -p "$PKGROOT/var/lib/aegishv/dumps"
mkdir -p "$PKGROOT/var/lib/aegishv/spool"
mkdir -p "$PKGROOT/var/log/aegishv"

chmod 0750 "$PKGROOT/var/lib/aegishv" "$PKGROOT/var/lib/aegishv/dumps" "$PKGROOT/var/lib/aegishv/spool" "$PKGROOT/var/log/aegishv"

dpkg-deb --build --root-owner-group "$PKGROOT" "$OUT"
echo "wrote $OUT"
