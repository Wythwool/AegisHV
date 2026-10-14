#!/usr/bin/env python3
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess


def check_output(text):
    markers = ('quarantine-on', 'blocked-before-map', 'page-roundtrip-ok',
               'adjacent-page-blocked', 'read-only-ok', 'revoked-ok', 'dma-test-ok')
    last = -1
    lines = text.splitlines()
    for marker in markers:
        line = 'aegishv:iommu:' + marker
        assert lines.count(line) == 1, line
        index = lines.index(line)
        assert index > last, line
        last = index
    assert 'aegishv:iommu:error' not in text and 'panicked at' not in text
    faults = re.findall(r'^aegishv:iommu:fault source=(0x[0-9a-f]+) address=(0x[0-9a-f]+) reason=(\d+) read=(true|false)\r?$', text, re.M)
    assert len(faults) == 5 and len({fault[0] for fault in faults}) == 1, faults
    assert [(int(reason), read) for _, _, reason, read in faults] == [(1, 'false'), (5, 'false'), (5, 'false'), (5, 'false'), (6, 'true')]
    assert [int(address, 16) for _, address, _, _ in faults[1:]] == [0x2000, 0x1000, 0x1000, 0x1000]
    return faults


def main():
    parser = argparse.ArgumentParser(description='Check VT-d with real EDU DMA transactions in QEMU.')
    parser.add_argument('image', type=Path)
    parser.add_argument('--qemu', default='qemu-system-x86_64')
    parser.add_argument('--out', type=Path, default=Path('target/iommu'))
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    digest = hashlib.sha256(args.image.read_bytes()).hexdigest()
    command = [args.qemu, '-machine', 'q35,accel=tcg', '-cpu', 'max', '-m', '128M', '-smp', '1',
               '-device', 'intel-iommu,caching-mode=on', '-device', 'edu', '-cdrom', str(args.image),
               '-device', 'isa-debug-exit,iobase=0xf4,iosize=4', '-display', 'none', '-serial', 'stdio',
               '-monitor', 'none', '-no-reboot']
    log = args.out / 'serial.log'
    with log.open('wb') as stream:
        try:
            result = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, timeout=30)
        except subprocess.TimeoutExpired as error:
            raise SystemExit(f'VT-d test timed out; see {log}') from error
    text = log.read_text(encoding='utf-8')
    print(text, end='')
    assert result.returncode == 33, result.returncode
    faults = check_output(text)
    assert hashlib.sha256(args.image.read_bytes()).hexdigest() == digest
    log.with_suffix('.json').write_text(json.dumps({'image_sha256': digest, 'command': command,
        'faults': faults}, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
