#!/usr/bin/env python3
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def check_events(output, backend, guest):
    events = [json.loads(line) for line in output.splitlines() if line.startswith('{"source":"aegishv-type1"')]
    assert events, "no kernel events"
    for sequence, event in enumerate(events, 1):
        assert event["sequence"] == sequence, "event sequence gap"
        assert event["backend"] == backend and event["vcpu"] == 0
    if backend == "el2":
        samples = [event for event in events if event["kind"] == "pmu_sample"]
        assert len(samples) == 1 and samples[0]["value"] > 0, samples
    if guest != "builtin":
        main = [event for event in events if event["kind"] != "pmu_sample"]
        assert len(main) == 1, main
        event = main[0]
        if guest == "return":
            assert event["kind"] == "guest_halt" and event["result"] == 55, event
        elif guest == "spin":
            assert event["kind"] == "vcpu_budget" and event["result"] == 20, event
        elif guest == "isolation" and backend == "svm":
            assert event["kind"] == "guest_exception" and event["vector"] == 14 and event["address"] == 0x40000000, event
        elif guest == "syscall":
            assert event["kind"] == "syscall_entry" and event["action"] == "deny", event
            assert event["target"] == (0x2000 if backend == "svm" else 0x40001000), event
        else:
            base = 0x1000 if backend == "svm" else 0x40000000
            address = {"write": base, "execute": base + 0x1008, "isolation": 0x80000000}[guest]
            access = "read" if guest == "isolation" else guest
            assert event["kind"] == "page_access" and event["access"] == access and event["action"] == "deny", event
            if guest == "execute":
                # Instruction fetch may report the start of a cache line in EXITINFO2.
                assert event["gpa"] >> 12 == address >> 12 and event["rip"] == address, event
            else:
                assert event["gpa"] == address, event
        return events
    assert output.count(f"aegishv:{backend}:preempt-exit") == 1, "guest was not preempted"
    assert output.count(f"aegishv:{backend}:guest-run-ok") == 1, "guest did not complete"
    accesses = [event for event in events if event["kind"] == "page_access"]
    expected = [(0x1800, "write"), (0x3000, "execute")] if backend == "svm" else [(0x40000800, "write"), (0x40001000, "execute")]
    assert [(event["gpa"], event["access"]) for event in accesses] == expected, accesses
    assert all(event["action"] == "deny" for event in accesses)
    entries = [event for event in events if event["kind"] == "syscall_entry"]
    assert len(entries) == 1 and entries[0]["action"] == "allow", entries
    assert entries[0]["target"] == (0x1100 if backend == "svm" else 0x40000000)
    return events


def main():
    parser = argparse.ArgumentParser(description="Boot a kernel and check exits produced by its guest.")
    parser.add_argument("backend", choices=["svm", "el2"])
    parser.add_argument("image", type=Path)
    parser.add_argument("--qemu")
    parser.add_argument("--accel", choices=["tcg", "kvm"], default="tcg")
    parser.add_argument("--log", type=Path)
    parser.add_argument("--timeout", type=int, default=30)
    parser.add_argument("--guest", choices=["builtin", "return", "spin", "write", "execute", "isolation", "syscall"], default="builtin")
    args = parser.parse_args()
    if args.backend == "el2" and args.accel != "tcg":
        parser.error("EL2 runner currently requires TCG")
    digest = hashlib.sha256(args.image.read_bytes()).hexdigest()
    command = [args.qemu or ("qemu-system-x86_64" if args.backend == "svm" else "qemu-system-aarch64")]
    if args.backend == "svm":
        command += ["-machine", f"q35,accel={args.accel}", "-cpu", "host" if args.accel == "kvm" else "max", "-cdrom", str(args.image), "-device", "isa-debug-exit,iobase=0xf4,iosize=4"]
    else:
        command += ["-machine", "virt,virtualization=on,gic-version=2", "-cpu", "cortex-a57", "-accel", "tcg", "-kernel", str(args.image), "-semihosting-config", "enable=on,target=native"]
    command += ["-m", "128M", "-smp", "1", "-display", "none", "-serial", "stdio", "-monitor", "none", "-no-reboot"]
    log = args.log or Path("target/type1") / (args.backend + "-serial.log")
    log.parent.mkdir(parents=True, exist_ok=True)
    try:
        run = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=args.timeout)
        output = run.stdout
    except subprocess.TimeoutExpired as error:
        log.write_bytes(error.stdout or b"")
        print((error.stdout or b"").decode("utf-8", errors="replace"), end="")
        raise SystemExit(f"QEMU timed out; serial log: {log}") from error
    log.write_bytes(output)
    text = output.decode("utf-8", errors="strict")
    print(text, end="")
    code = 0 if args.guest in ("builtin", "return") else 2 if args.guest == "spin" else 3
    expected = {0: 33, 2: 35, 3: 37}[code] if args.backend == "svm" else code
    assert run.returncode == expected, f"QEMU exited with {run.returncode}, expected {expected}"
    events = check_events(text, args.backend, args.guest)
    assert hashlib.sha256(args.image.read_bytes()).hexdigest() == digest, "image changed during execution"
    log.with_suffix(".json").write_text(json.dumps({"image_sha256": digest, "command": command, "events": events}, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
