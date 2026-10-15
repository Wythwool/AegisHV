#!/usr/bin/env python3
import argparse
from collections import Counter
from contextlib import ExitStack
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import sys
import threading

BACKENDS = {"vmx", "svm", "el2"}
FIELDS = {
    "page_access": {"rip", "gpa", "access", "action"},
    "syscall_entry": {"rip", "register", "target", "action"},
    "pmu_sample": {"counter", "value"},
    "pmu_status": {"available"},
    "vector_sample": {"rip", "target", "gpa", "valid", "changed"},
    "guest_halt": {"rip", "result"},
    "vcpu_budget": {"rip", "result"},
    "guest_exception": {"rip", "vector", "address"},
}
COMMON = {"source", "backend", "sequence", "vcpu", "kind"}
NUMBERS = {"sequence", "vcpu", "rip", "gpa", "register", "target", "value", "result", "vector", "address"}


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate field")
        result[key] = value
    return result


def decode(line):
    event = json.loads(line, object_pairs_hook=unique_object)
    if not isinstance(event, dict) or event.get("source") != "aegishv-type1":
        return None
    kind = event.get("kind")
    if not isinstance(kind, str) or kind not in FIELDS or event.keys() != COMMON | FIELDS[kind]:
        raise ValueError("unknown event layout")
    if event["backend"] not in BACKENDS or not 0 <= event["vcpu"] < 256 or event["sequence"] == 0:
        raise ValueError("invalid event origin")
    for key in NUMBERS & event.keys():
        if type(event[key]) is not int or not 0 <= event[key] < 1 << 64:
            raise ValueError("invalid integer")
    for key in {"available", "valid", "changed"} & event.keys():
        if type(event[key]) is not bool:
            raise ValueError("invalid boolean")
    if kind == "page_access" and (event["access"] not in ("read", "write", "execute") or event["action"] not in ("allow", "deny", "emulate")):
        raise ValueError("invalid access event")
    if kind == "syscall_entry" and event["action"] not in ("allow", "deny"):
        raise ValueError("invalid syscall action")
    if kind == "pmu_sample" and event["counter"] != "cycles":
        raise ValueError("unknown PMU counter")
    return event


class Metrics:
    def __init__(self):
        self.lock = threading.Lock()
        self.counters = Counter()
        self.last = {}
        self.pmu = {}
        self.stream_up = 1
        self.error = False

    def reject(self):
        with self.lock:
            self.counters[("invalid_events_total", ())] += 1

    def accept(self, event):
        backend, sequence, kind = event["backend"], event["sequence"], event["kind"]
        labels = (("backend", backend),)
        with self.lock:
            previous = self.last.get(backend, 0)
            if sequence == 1:
                self.counters[("boots_total", labels)] += 1
                self.pmu = {key: value for key, value in self.pmu.items() if key[0] != backend}
                previous = 0
            elif sequence <= previous:
                self.counters[("invalid_events_total", ())] += 1
                return False
            self.counters[("missing_events_total", labels)] += sequence - previous - 1
            self.last[backend] = sequence
            self.counters[("events_total", labels + (("kind", kind),))] += 1
            cpu_labels = labels + (("vcpu", str(event["vcpu"])),)
            self.counters[("vcpu_events_total", cpu_labels + (("kind", kind),))] += 1
            if kind == "page_access":
                self.counters[("page_accesses_total", labels + (("access", event["access"]), ("action", event["action"])))] += 1
                if event["action"] == "deny":
                    self.counters[("denied_accesses_total", labels + (("access", event["access"]),))] += 1
            elif kind == "syscall_entry":
                self.counters[("syscall_checks_total", labels + (("action", event["action"]),))] += 1
            elif kind == "pmu_sample":
                self.counters[("pmu_cycles_total", labels)] += event["value"]
                self.counters[("vcpu_pmu_cycles_total", cpu_labels)] += event["value"]
            elif kind == "vector_sample":
                status = "invalid" if not event["valid"] else "changed" if event["changed"] else "valid"
                self.counters[("vector_checks_total", cpu_labels + (("status", status),))] += 1
            elif kind == "pmu_status":
                self.pmu[(backend, event["vcpu"])] = int(event["available"])
            return True

    def render(self):
        with self.lock:
            counters, sequences = dict(self.counters), dict(self.last)
            stream_up = self.stream_up
            pmu = dict(self.pmu)
        lines = []
        typed = set()
        for (name, labels), value in sorted(counters.items()):
            name = "aegishv_type1_" + name
            if name not in typed:
                lines.append(f"# TYPE {name} counter")
                typed.add(name)
            suffix = "{" + ",".join(f'{key}="{value}"' for key, value in labels) + "}" if labels else ""
            lines.append(f"{name}{suffix} {value}")
        lines.append("# TYPE aegishv_type1_last_sequence gauge")
        for backend, sequence in sorted(sequences.items()):
            lines.append(f'aegishv_type1_last_sequence{{backend="{backend}"}} {sequence}')
        if pmu:
            lines.append("# TYPE aegishv_type1_pmu_available gauge")
            for (backend, cpu), available in sorted(pmu.items()):
                lines.append(f'aegishv_type1_pmu_available{{backend="{backend}",vcpu="{cpu}"}} {available}')
        lines += ["# TYPE aegishv_type1_stream_up gauge", f"aegishv_type1_stream_up {stream_up}"]
        return "\n".join(lines) + "\n"


def collect(source, metrics, output=None):
    try:
        while line := source.readline(8193):
            if len(line) > 8192:
                while line and not line.endswith(b"\n"):
                    line = source.readline(8193)
                metrics.reject()
                continue
            if not line.lstrip().startswith(b"{"):
                continue
            try:
                event = decode(line)
            except (ValueError, TypeError, KeyError, UnicodeError, RecursionError):
                metrics.reject()
                continue
            if event is not None and metrics.accept(event) and output:
                output.write(json.dumps(event, separators=(",", ":")) + "\n")
                output.flush()
    except OSError as error:
        with metrics.lock:
            metrics.error = True
        print(f"serial input: {error}", file=sys.stderr)
    finally:
        with metrics.lock:
            metrics.stream_up = 0


def make_server(address, metrics):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            content_type = "text/plain; charset=utf-8"
            if self.path == "/metrics":
                code, body = 200, metrics.render()
                content_type = "text/plain; version=0.0.4; charset=utf-8"
            elif self.path in ("/healthz", "/readyz"):
                with metrics.lock:
                    healthy = not metrics.error and (self.path == "/healthz" or bool(metrics.last))
                code, body = (200, "ok\n") if healthy else (503, "unavailable\n")
            else:
                code, body = 404, "not found\n"
            data = body.encode("utf-8")
            self.send_response(code)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def log_message(self, *_):
            pass

    return ThreadingHTTPServer(address, Handler)


def main():
    parser = argparse.ArgumentParser(description="Collect kernel serial JSON and export Prometheus metrics.")
    parser.add_argument("--input", default="-", help="serial log or - for stdin")
    parser.add_argument("--jsonl", type=Path, help="append validated events to this file")
    parser.add_argument("--metrics-file", type=Path, help="write a metrics snapshot at EOF")
    parser.add_argument("--listen", help="HTTP bind address, for example 127.0.0.1:9465")
    args = parser.parse_args()
    if not (args.jsonl or args.metrics_file or args.listen):
        parser.error("choose --jsonl, --metrics-file or --listen")
    paths = [path for path in [Path(args.input) if args.input != "-" else None, args.jsonl, args.metrics_file] if path]
    for index, path in enumerate(paths):
        for previous in paths[:index]:
            if path.resolve() == previous.resolve() or (path.exists() and previous.exists() and path.samefile(previous)):
                parser.error("input, JSONL and metrics paths must refer to different files")
    metrics = Metrics()
    server = None
    if args.listen:
        host, separator, port = args.listen.rpartition(":")
        if not separator or not host or not port.isdecimal() or not 0 < int(port) < 65536:
            parser.error("--listen requires HOST:PORT")
        server = make_server((host, int(port)), metrics)

    def run():
        try:
            with ExitStack() as files:
                source = sys.stdin.buffer if args.input == "-" else files.enter_context(open(args.input, "rb"))
                output = files.enter_context(args.jsonl.open("a", encoding="utf-8")) if args.jsonl else None
                collect(source, metrics, output)
                if args.metrics_file:
                    args.metrics_file.write_text(metrics.render(), encoding="utf-8")
        except OSError as error:
            with metrics.lock:
                metrics.error, metrics.stream_up = True, 0
            print(f"serial bridge: {error}", file=sys.stderr)

    if server:
        threading.Thread(target=run, daemon=True).start()
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass
        finally:
            server.server_close()
    else:
        run()
        if metrics.error:
            raise SystemExit(1)


if __name__ == "__main__":
    main()
