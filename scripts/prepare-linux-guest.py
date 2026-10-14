#!/usr/bin/env python3
import argparse
import gzip
import hashlib
import io
from pathlib import Path
import stat
import struct
import subprocess
import tarfile
from urllib.request import urlopen

BASE = 'https://dl-cdn.alpinelinux.org/alpine/v3.22/releases/aarch64/'
FILES = [
    ('netboot-3.22.0/vmlinuz-virt', 'vmlinuz-virt', '00ad04e557735cc39cd3ba0464b454e7a73a6cc398fe745171b2dafb6a109798'),
    ('alpine-minirootfs-3.22.0-aarch64.tar.gz', 'rootfs.tar.gz', '74eaa1f80aceb35a69f26a7772cd22848a51921323a32b2ae057b2a4c8188fd5'),
]


def cells(*values):
    return struct.pack('>' + 'I' * len(values), *values)


def device_tree(cpus, initrd_end):
    strings, body = bytearray(), bytearray()
    def node(name, props, children=()):
        body.extend(cells(1) + name.encode() + b'\0')
        body.extend(b'\0' * (-len(body) % 4))
        for key, value in props.items():
            offset = len(strings)
            strings.extend(key.encode() + b'\0')
            value = value.encode() + b'\0' if isinstance(value, str) else value
            body.extend(cells(3, len(value), offset) + value)
            body.extend(b'\0' * (-len(body) % 4))
        for child in children:
            node(*child)
        body.extend(cells(2))
    node('', {'compatible': 'aegishv,virt', '#address-cells': cells(2), '#size-cells': cells(2), 'interrupt-parent': cells(1)}, [
        ('chosen', {'bootargs': 'console=ttyAMA0 earlycon=pl011,0x9000000 rdinit=/init loglevel=7 panic=-1',
                    'linux,initrd-start': cells(0, 0x48000000), 'linux,initrd-end': cells(0, initrd_end)}),
        ('memory@40000000', {'device_type': 'memory', 'reg': cells(0, 0x40000000, 0, 0x10000000)}),
        ('cpus', {'#address-cells': cells(2), '#size-cells': cells(0)}, [
            (f'cpu@{i}', {'device_type': 'cpu', 'compatible': 'arm,cortex-a57', 'reg': cells(0, i), 'enable-method': 'psci'}) for i in range(cpus)]),
        ('psci', {'compatible': 'arm,psci-0.2', 'method': 'hvc'}),
        ('intc@8000000', {'compatible': 'arm,cortex-a15-gic', '#interrupt-cells': cells(3), 'interrupt-controller': b'',
                         'phandle': cells(1), 'reg': cells(0, 0x08000000, 0, 0x1000, 0, 0x08010000, 0, 0x2000)}),
        ('timer', {'compatible': 'arm,armv8-timer', 'always-on': b'',
                   'interrupts': cells(1, 13, 0xf08, 1, 14, 0xf08, 1, 11, 0xf08, 1, 10, 0xf08)}),
        ('clock', {'compatible': 'fixed-clock', '#clock-cells': cells(0), 'clock-frequency': cells(24000000), 'phandle': cells(2)}),
        ('pl011@9000000', {'compatible': b'arm,pl011\0arm,primecell\0', 'reg': cells(0, 0x09000000, 0, 0x1000),
                          'clocks': cells(2, 2), 'clock-names': b'uartclk\0apb_pclk\0', 'interrupts': cells(0, 1, 4)})])
    body.extend(cells(9))
    return cells(0xd00dfeed, 56 + len(body) + len(strings), 56, 56 + len(body), 40, 17, 16, 0, len(strings), len(body)) + bytes(16) + body + strings


def initramfs(archive, cpus, worker):
    output = bytearray()
    ino = 1
    def add(name, mode, data=b'', major=0, minor=0):
        nonlocal ino
        name = name.encode() + b'\0'
        fields = (ino, mode, 0, 0, 1, 0, len(data), 0, 0, major, minor, len(name), 0)
        output.extend(b'070701' + ''.join(f'{value:08x}' for value in fields).encode() + name)
        output.extend(bytes(-len(output) % 4))
        output.extend(data)
        output.extend(bytes(-len(output) % 4))
        ino += 1
    with tarfile.open(fileobj=io.BytesIO(archive), mode='r:gz') as source:
        for item in source:
            name = item.name.removeprefix('./').rstrip('/')
            if not name or name.startswith('/') or '..' in Path(name).parts or name in ('dev/console', 'dev/null'):
                continue
            if item.isfile():
                add(name, stat.S_IFREG | item.mode, source.extractfile(item).read())
            elif item.isdir():
                add(name, stat.S_IFDIR | item.mode)
            elif item.issym():
                add(name, stat.S_IFLNK | item.mode, item.linkname.encode())
    add('dev/console', stat.S_IFCHR | 0o600, major=5, minor=1)
    add('dev/null', stat.S_IFCHR | 0o666, major=1, minor=3)
    init = f'''#!/bin/sh
set -eu
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev
uname -a
cpus=$(grep -c '^processor' /proc/cpuinfo)
echo "aegishv-linux: cpus=$cpus"
test "$cpus" -eq {cpus}
printf 'guest filesystem round trip\\n' > /tmp/check
test "$(cat /tmp/check)" = 'guest filesystem round trip'
/bin/smp-test "$cpus"
if [ "$cpus" -gt 1 ]; then
    echo 0 > /sys/devices/system/cpu/cpu1/online
    test "$(cat /sys/devices/system/cpu/cpu1/online)" = 0
    echo 1 > /sys/devices/system/cpu/cpu1/online
    test "$(cat /sys/devices/system/cpu/cpu1/online)" = 1
    echo 'aegishv-linux: cpu-hotplug-ok'
fi
sleep 1
echo 'aegishv-linux: userspace-ok' 
poweroff -f
'''
    add('init', stat.S_IFREG | 0o755, init.encode())
    add('bin/smp-test', stat.S_IFREG | 0o755, worker)
    add('TRAILER!!!', 0)
    return gzip.compress(output, mtime=0)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--out', type=Path, default=Path('target/linux-guest'))
    parser.add_argument('--cpus', type=int, choices=range(1, 5), default=1)
    parser.add_argument('--budget-seconds', type=int, default=120)
    args = parser.parse_args()
    if not 0 <= args.budget_seconds <= 3600:
        parser.error('budget must be between 0 (disabled) and 3600 seconds')
    args.out.mkdir(parents=True, exist_ok=True)
    for suffix, name, expected in FILES:
        path = args.out / name
        if not path.exists():
            with urlopen(BASE + suffix, timeout=60) as response:
                path.write_bytes(response.read())
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise SystemExit(f'bad SHA256: {path}')
    image = (args.out / 'vmlinuz-virt').read_bytes()
    if image[4:8] == b'zimg':
        start, length = struct.unpack_from('<II', image, 8)
        assert image[24:28] == b'gzip' and start + length <= len(image)
        image = image[start:start + length]
    if image.startswith(b'\x1f\x8b'):
        image = gzip.decompress(image)
    assert image[56:60] == b'ARM\x64', 'not an ARM64 Image'
    offset, size = struct.unpack_from('<QQ', image, 8)
    assert offset in (0, 0x80000) and 0 < size <= 0x7d80000
    (args.out / 'Image').write_bytes(image)
    worker = args.out / 'smp-test'
    subprocess.run(['rustc', '--edition=2021', '--target=aarch64-unknown-none', '-C', 'panic=abort', '-C', 'relocation-model=static', '-C', 'link-arg=--entry=_start', str(Path(__file__).resolve().parents[1] / 'examples/linux/smp.rs'), '-o', str(worker)], check=True)
    ramdisk = initramfs((args.out / 'rootfs.tar.gz').read_bytes(), args.cpus, worker.read_bytes())
    (args.out / 'initramfs.gz').write_bytes(ramdisk)
    (args.out / 'guest.dtb').write_bytes(device_tree(args.cpus, 0x48000000 + len(ramdisk)))
    (args.out / 'boot.cfg').write_bytes(b'AEGLNX01' + struct.pack('<7Q', 0x40200000 + offset, 0x40010000, 0x10000000, args.cpus, len(ramdisk), size, args.budget_seconds * 100))
    print(f'Linux Image: {len(image)} bytes; initramfs: {len(ramdisk)} bytes; CPUs: {args.cpus}')


if __name__ == '__main__':
    main()
