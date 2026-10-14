#!/usr/bin/env python3
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import time

MARKERS = ('host-tables-ok', 'backend-vmx', 'vmxon-cycle-ok', 'vmcs-load-ok',
           'host-paging-ok', 'guest-config-ok', 'guest-preempt-exit-ok', 'guest-io-exit-ok',
           'guest-io-b-exit-ok', 'guest-cpuid-exit-ok', 'guest-rdmsr-exit-ok', 'guest-pat-state-ok',
           'guest-nm-x87-exit-ok', 'guest-nm-simd-exit-ok', 'guest-ud-inject-ok',
           'guest-hlt-exit-ok', 'guest-run-ok')


def check_output(text):
    lines = text.splitlines()
    last = -1
    for marker in MARKERS:
        line = 'aegishv:type1:' + marker
        assert lines.count(line) == 1, line
        index = lines.index(line)
        assert index > last, line
        last = index
    assert not re.search(r'aegishv:.*(?:error|panic|failed)', text)
    values = {}
    for name in ('cpu-signature', 'timer-rate', 'timer-reload', 'timer-effective'):
        found = re.findall(rf'^aegishv:type1:vmx-{name}=(0x[0-9a-f]+)\r?$', text, re.M)
        assert len(found) == 1, name
        values[name] = int(found[0], 16)
    assert values['cpu-signature'] != 0 and values['timer-rate'] <= 31
    assert 0 < values['timer-effective'] <= 0x1000000
    assert values['timer-effective'] == values['timer-reload'] << values['timer-rate']
    return values


def main():
    parser = argparse.ArgumentParser(description='Execute VMX in Bochs and check the full exit sequence.')
    parser.add_argument('image', type=Path)
    parser.add_argument('--bochs', type=Path, required=True)
    parser.add_argument('--bios', type=Path, required=True)
    parser.add_argument('--vga-bios', type=Path, required=True)
    parser.add_argument('--out', type=Path, default=Path('target/vmx'))
    parser.add_argument('--timeout', type=int, default=180)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    files = [args.image, args.bochs, args.bios, args.vga_bios]
    hashes = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in files}
    path = lambda value: value.resolve().as_posix()
    serial = args.out / 'serial.log'
    serial.write_bytes(b'')
    config = args.out / 'bochsrc'
    config.write_text(f'''megs: 128
romimage: file="{path(args.bios)}"
vgaromimage: file="{path(args.vga_bios)}"
cpu: model=corei7_haswell_4770, ips=100000000
clock: sync=none
pci: enabled=1, chipset=i440fx
ata0-master: type=cdrom, path="{path(args.image)}", status=inserted
boot: cdrom
com1: enabled=1, mode=file, dev="{path(serial)}"
log: {path(args.out / 'bochs.log')}
display_library: nogui
config_interface: textconfig
mouse: enabled=0
panic: action=fatal
error: action=report
debug: action=ignore
info: action=report
''', encoding='utf-8')
    command = [str(args.bochs.resolve()), '-q', '-f', str(config.resolve())]
    with (args.out / 'console.log').open('wb') as log:
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + args.timeout
            while process.poll() is None and time.monotonic() < deadline:
                text = serial.read_text(encoding='utf-8', errors='replace')
                if 'aegishv:type1:guest-run-ok' in text or 'guest-exit-error' in text:
                    break
                time.sleep(0.2)
        finally:
            if process.poll() is None:
                process.kill()
            process.wait()
    text = serial.read_text(encoding='utf-8')
    print(text, end='')
    values = check_output(text)
    assert hashes == {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in files}, 'input changed'
    serial.with_suffix('.json').write_text(json.dumps({'sha256': hashes, 'command': command,
        'vmx': values, 'emulator': 'Bochs', 'stopped_after_completion': True}, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
