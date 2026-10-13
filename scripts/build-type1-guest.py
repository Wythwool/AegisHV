#!/usr/bin/env python3
import argparse
import os
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description="Build a freestanding guest ELF.")
    parser.add_argument("arch", choices=["x86_64", "arm64"])
    parser.add_argument("case", choices=["return", "spin", "write", "execute", "isolation"])
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    output = args.output or root / "target/type1" / f"guest-{args.arch}-{args.case}.elf"
    output.parent.mkdir(parents=True, exist_ok=True)
    target = "x86_64-unknown-none" if args.arch == "x86_64" else "aarch64-unknown-none"
    base = "0x1000" if args.arch == "x86_64" else "0x40000000"
    env = dict(os.environ, AEGISHV_GUEST_CASE=str(["return", "spin", "write", "execute", "isolation"].index(args.case)))
    subprocess.run(["rustc", "--edition=2021", str(root / "examples/guests/guest.rs"), "--target", target,
                    "-C", "panic=abort", "-C", "relocation-model=static", "-C", "opt-level=2",
                    "-C", f"link-arg=--defsym=GUEST_BASE={base}",
                    "-C", f"link-arg=-T{root / 'examples/guests/guest.ld'}", "-o", str(output)], env=env, check=True)
    print(output)


if __name__ == "__main__":
    main()
