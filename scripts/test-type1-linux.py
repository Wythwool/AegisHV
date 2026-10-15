#!/usr/bin/env python3
import argparse
import hashlib
import json
from pathlib import Path
import re
import socket
import struct
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from type1_events import decode, Metrics


def change_vector(port, log):
    def samples():
        return [event for line in log.read_text().splitlines(keepends=True)
                if line.endswith('\n') and (event := decode(line))['kind'] == 'vector_sample'
                and event['vcpu'] == 0 and event['valid']]

    before = samples()[-1]
    assert 0x40000000 <= before['gpa'] <= 0x4ffff800
    # Unused AArch32 SError slot, outside the AArch64 syscall/IRQ paths.
    address = before['gpa'] + 0x10000000 + 2044

    def memory(value=None):
        with socket.create_connection(('127.0.0.1', port), timeout=3) as remote:
            def receive():
                while True:
                    start = remote.recv(1)
                    assert start, 'GDB disconnected'
                    if start == b'$':
                        break
                reply = bytearray()
                while (byte := remote.recv(1)) != b'#':
                    assert byte and len(reply) < 4096, 'bad GDB reply'
                    reply.extend(byte)
                checksum = remote.recv(1) + remote.recv(1)
                assert int(checksum, 16) == sum(reply) % 256
                remote.sendall(b'+')
                return reply.decode()

            def request(command):
                data = command.encode()
                remote.sendall(b'$' + data + f'#{sum(data) % 256:02x}'.encode())
                return receive()

            assert request('?').startswith(('T', 'S'))
            assert request('Qqemu.PhyMemMode:1') == 'OK'
            original = bytes.fromhex(request(f'm{address:x},1'))
            assert len(original) == 1
            replacement = value if value is not None else bytes([original[0] ^ 1])
            assert request(f'M{address:x},1:{replacement.hex()}') == 'OK'
            assert request(f'm{address:x},1') == replacement.hex()
            assert request('D') == 'OK'
            return original, replacement

    def observed(after):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            for event in samples():
                if event['sequence'] > after and event['target'] == before['target'] and event['gpa'] == before['gpa'] and event['changed']:
                    return event['sequence']
            time.sleep(0.02)
        raise AssertionError('EL2 did not report the vector modification')

    original, replacement = memory()
    try:
        changed = observed(before['sequence'])
    finally:
        current, _ = memory(original)
        assert current == replacement
    restored = observed(changed)
    return {'gpa': before['gpa'], 'offset': 2044, 'before': original.hex(),
            'after': replacement.hex(), 'changed_sequence': changed, 'restored_sequence': restored}


def check_events(text, cpus, counters, pmu, probe='none'):
    events = [decode(line) for line in text.splitlines()]
    assert events and all(event is not None for event in events), 'empty or foreign telemetry'
    metrics = Metrics()
    for sequence, event in enumerate(events, 1):
        assert event['sequence'] == sequence, 'telemetry gap'
        assert event['backend'] == 'el2' and event['vcpu'] < cpus
        assert metrics.accept(event)
    metrics.stream_up = 0
    pages = [event for event in events if event['kind'] == 'page_access']
    if probe == 'telemetry':
        denied = [event for event in pages if event['action'] == 'deny']
        assert len(denied) == 1 and denied[0]['gpa'] == 0x09040000 and denied[0]['access'] == 'write', denied
        return metrics.render()
    assert all(event['action'] in ('allow', 'emulate') for event in pages)
    assert sum(event['access'] == 'execute' for event in pages) == counters['execute_faults']
    assert sum(event['access'] == 'write' for event in pages) == counters['write_faults']
    assert any(event['action'] == 'emulate' for event in pages), 'no Linux text patch observed'
    for cpu in range(cpus):
        local = [event for event in events if event['vcpu'] == cpu]
        statuses = [event for event in local if event['kind'] == 'pmu_status']
        assert statuses and all(event['available'] == pmu for event in statuses)
        samples = [event for event in local if event['kind'] == 'pmu_sample']
        if pmu:
            assert samples and sum(event['value'] for event in samples) > 0, cpu
        else:
            assert not samples, 'PMU-off run reported counter values'
        vectors = [event for event in local if event['kind'] == 'vector_sample']
        assert vectors and any(event['valid'] for event in vectors), cpu
    assert 'aegishv_type1_invalid_events_total' not in metrics.render()
    return metrics.render()


def check_output(text, cpus):
    for failure in ('Kernel panic', 'Oops:', 'BUG:', 'aegishv:linux:abort',
                    'aegishv:linux:host-fault', 'aegishv:linux:deadline', 'panicked at'):
        assert failure not in text, failure
    for marker in (f'aegishv-linux: cpus={cpus}', 'aegishv-linux: atomic-workers-ok',
                   'aegishv-linux: userspace-ok'):
        assert text.splitlines().count(marker) == 1, marker
    if cpus > 1:
        assert text.splitlines().count('aegishv-linux: cpu-hotplug-ok') == 1
    for cpu in ([0, cpus - 1] if cpus > 1 else [0]):
        assert text.splitlines().count(f'aegishv-linux: console-ok cpu={cpu}') == 1
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
    parser.add_argument('--pmu', choices=('on', 'off'), default='on')
    parser.add_argument('--probe', choices=('none', 'telemetry'), default='none')
    parser.add_argument('--vector-change', action='store_true')
    args = parser.parse_args()
    if args.vector_change and args.probe != 'none':
        parser.error('vector change and telemetry probe need separate runs')
    files = [args.kernel] + [args.guest / name for name in ('boot.cfg', 'Image', 'guest.dtb', 'initramfs.gz')]
    hashes = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in files}
    config = (args.guest / 'boot.cfg').read_bytes()
    assert config[:8] == b'AEGLNX01' and len(config) == 64
    entry, _, _, cpus, _, _, _ = struct.unpack_from('<7Q', config, 8)
    assert 1 <= cpus <= 4
    command = [args.qemu, '-machine', 'virt,virtualization=on,gic-version=2', '-cpu', f'cortex-a57,pmu={args.pmu}',
               '-accel', 'tcg,thread=multi', '-smp', str(cpus), '-m', '512M', '-kernel', str(args.kernel),
               '-semihosting-config', 'enable=on,target=native', '-display', 'none',
               '-monitor', 'none', '-no-reboot']
    debug_port = None
    modification = None
    if args.vector_change:
        with socket.socket() as reservation:
            reservation.bind(('127.0.0.1', 0))
            debug_port = reservation.getsockname()[1]
        command += ['-gdb', f'tcp:127.0.0.1:{debug_port}']
    for name, address in [('boot.cfg', 0x50000000), ('Image', entry + 0x10000000),
                          ('guest.dtb', 0x50010000), ('initramfs.gz', 0x58000000)]:
        command += ['-device', f'loader,file={args.guest / name},addr={address:#x},force-raw=on']
    log = args.guest / 'serial.log'
    with socket.socket() as server, log.open('wb') as stream, (args.guest / 'qemu.log').open('wb') as errors:
        server.bind(('127.0.0.1', 0))
        server.listen(1)
        server.settimeout(0.1)
        command += ['-serial', f'tcp:127.0.0.1:{server.getsockname()[1]}']
        command += ['-serial', f'file:{args.guest / "edr.jsonl"}']
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=errors, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + args.timeout
            connection = None
            while process.poll() is None and time.monotonic() < deadline:
                try:
                    connection, _ = server.accept()
                    break
                except socket.timeout:
                    pass
            if connection is None:
                raise SystemExit(f'QEMU did not connect its UART; see {args.guest / "qemu.log"}')
            answered = set()
            output = bytearray()
            with connection:
                connection.settimeout(0.1)
                while time.monotonic() < deadline:
                    try:
                        chunk = connection.recv(65536)
                    except ConnectionResetError:
                        break
                    except socket.timeout:
                        if process.poll() is not None:
                            break
                        continue
                    if not chunk:
                        break
                    output.extend(chunk)
                    stream.write(chunk)
                    stream.flush()
                    text = output.decode('utf-8', errors='replace')
                    for cpu in re.findall(r'^aegishv-linux: console-ready cpu=(\d+)\r?$', text, re.M):
                        if cpu not in answered:
                            if args.vector_change and cpu == '0':
                                modification = change_vector(debug_port, args.guest / 'edr.jsonl')
                            connection.sendall(f'aegishv-console-{cpu}\n'.encode())
                            answered.add(cpu)
            process.wait(timeout=2)
        except subprocess.TimeoutExpired as error:
            raise SystemExit(f'Linux boot timed out; see {log}') from error
        finally:
            if process.poll() is None:
                process.kill()
            process.wait()
    text = log.read_text(encoding='utf-8')
    print(text, end='')
    assert process.returncode == (3 if args.probe == 'telemetry' else 0), process.returncode
    assert '"sequence":999999' in text, 'guest console spoof was not attempted'
    if args.probe == 'telemetry':
        assert 'aegishv-linux: telemetry-probe' in text
        assert 'aegishv-linux: telemetry-access-leaked' not in text
        counters = {}
    else:
        counters = check_output(text, cpus)
    telemetry = (args.guest / 'edr.jsonl').read_text(encoding='utf-8')
    (args.guest / 'edr.prom').write_text(check_events(telemetry, cpus, counters, args.pmu == 'on', args.probe), encoding='utf-8')
    assert not args.vector_change or modification is not None
    assert hashes == {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in files}, 'input changed'
    log.with_suffix('.json').write_text(json.dumps({'sha256': hashes, 'command': command,
        'cpus': cpus, 'pmu': args.pmu, 'probe': args.probe, 'counters': counters,
        'vector_modification': modification,
        'telemetry_sha256': hashlib.sha256((args.guest / 'edr.jsonl').read_bytes()).hexdigest()}, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
