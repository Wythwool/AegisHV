#!/usr/bin/env bash
set -euo pipefail

target="${1:-x86_64-unknown-linux-gnu}"
repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

case "$target" in
    x86_64-unknown-linux-gnu)
        rpm_arch="x86_64"
        ;;
    aarch64-unknown-linux-gnu)
        rpm_arch="aarch64"
        ;;
    *)
        echo "RPM packaging supports GNU Linux targets only: x86_64-unknown-linux-gnu or aarch64-unknown-linux-gnu" >&2
        exit 2
        ;;
esac

if ! command -v rpmbuild >/dev/null 2>&1; then
    echo "rpmbuild is required to build the RPM package" >&2
    exit 127
fi

version="$(grep '^version = ' Cargo.toml | head -1 | cut -d '"' -f2)"
topdir="$repo_root/dist/rpm"
sourcedir="$topdir/SOURCES"
rpmdir="$topdir/RPMS"

rm -rf "$topdir"
mkdir -p "$sourcedir" "$topdir/BUILD" "$topdir/BUILDROOT" "$rpmdir" "$topdir/SPECS" "$topdir/SRPMS"

tar \
    --exclude='./target' \
    --exclude='./.git' \
    --exclude='./dist' \
    --exclude='./.pytest_cache' \
    --exclude='./__pycache__' \
    --exclude='./node_modules' \
    --exclude='./.cache' \
    --exclude='*.zip' \
    --exclude='*.tar' \
    --exclude='*.tar.gz' \
    --exclude='*.tgz' \
    --transform "s,^\.,aegishv-${version}," \
    -czf "$sourcedir/aegishv-${version}.tar.gz" \
    .

rpmbuild -bb packaging/rpm/aegishv.spec --target "$rpm_arch" --define "_topdir $topdir" --define "aegishv_cargo_target $target"

artifact="$(find "$rpmdir" -type f -name 'aegishv-*.rpm' | sort | head -1)"
if [[ -z "$artifact" ]]; then
    echo "rpmbuild did not produce an aegishv RPM" >&2
    exit 3
fi

mkdir -p dist
cp "$artifact" "dist/$(basename "$artifact")"
echo "wrote dist/$(basename "$artifact")"
