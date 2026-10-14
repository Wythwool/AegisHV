#!/usr/bin/env python3
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess


def check_output(text, cpus):
    for failure in ('Kernel panic', 'Oops:', 'BUG:', 'aegishv:linux:abort',
                    'aegishv:linux:host-fault', 'aegishv:linux:deadline', 'panicked at'):
        assert failure not in text, failure
    for marker in (f'aegishv-linux: cpus={cpus}', 'aegishv-linux: atomic-workers-ok',
                   'aegishv-linux: userspace-ok'):
        assert text.splitlines().count(marker) == 1, marker
    if cpus > 1:
        assert text.splitlines().count('aegishv-linux: cpu-hotplug-ok') == 1
    online = [int(cpu) for cpu in re.findall(r'^aegishv:linux:cpu-online (\d+)\r?$', text, re.M)]
    assert sorted(online) == sorted(list(range(cpus)) + ([1] if cpus > 1 else [])), online
    poweroff = re.findall(r'^aegishv:linux:poweroff ticks=(\d+) exec=(\d+) write=(\d+)\r?$', text, re.M)
    assert len(poweroff) == 1 and all(int(value) > 0 for value in poweroff[0]), poweroff
    assert text.index('aegishv-linux: userspace-ok') < text.index('aegishv:linux:poweroff')
    return dict(zip(('ticks', 'execute_faults', 'write_faults'), map(int, poweroff[0])))


def main():
    parser = argparse.ArgumentParser(description='Boot an unmodified ARM64 Linux Image through EL2.')
    parser.add_argument('kernel', type=Path)
    parser.add_argument('--guest', type=Path, default=Path('target/linux-guest'))
    parser.add_argument('--qemu', default='qemu-system-aarch64')
    parser.add_argument('--timeout', type=int, default=180)
    args = parser.parse_args()
    files = [args.kernel] + [args.guest / name for name in ('boot.cfg', 'Image', 'guest.dtb', 'initramfs.gz')]
    hashes = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in files}
    config = (args.guest / 'boot.cfg').read_bytes()
    assert config[:8] == b'AEGLNX01' and len(config) == 64
    entry, _, _, cpus, _, _, _ = struct.unpack_from('<7Q', config, 8)
    assert 1 <= cpus <= 4
    command = [args.qemu, '-machine', 'virt,virtualization=on,gic-version=2', '-cpu', 'cortex-a57',
               '-accel', 'tcg,thread=multi', '-smp', str(cpus), '-m', '512M', '-kernel', str(args.kernel),
               '-semihosting-config', 'enable=on,target=native', '-display', 'none', '-serial', 'stdio',
               '-monitor', 'none', '-no-reboot']
    for name, address in [('boot.cfg', 0x50000000), ('Image', entry + 0x10000000),
                          ('guest.dtb', 0x50010000), ('initramfs.gz', 0x58000000)]:
        command += ['-device', f'loader,file={args.guest / name},addr={address:#x},force-raw=on']
    log = args.guest / 'serial.log'
    with log.open('wb') as stream:
        try:
            result = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, timeout=args.timeout)
        except subprocess.TimeoutExpired as error:
            raise SystemExit(f'Linux boot timed out; see {log}') from error
    text = log.read_text(encoding='utf-8')
    print(text, end='')
    assert result.returncode == 0, result.returncode
    counters = check_output(text, cpus)
    assert hashes == {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in files}, 'input changed'
    log.with_suffix('.json').write_text(json.dumps({'sha256': hashes, 'command': command,
        'cpus': cpus, 'counters': counters}, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
