import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest
from urllib.request import urlopen

spec = importlib.util.spec_from_file_location("type1_events", Path(__file__).resolve().parents[1] / "tools/type1_events.py")
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)


def event(sequence=1, **fields):
    return dict(source="aegishv-type1", backend="svm", sequence=sequence, vcpu=0,
                kind="page_access", rip=0x1000, gpa=0x1800, access="write", action="deny", **fields)


class SerialEvents(unittest.TestCase):
    def test_cli_rejects_input_as_output_without_changing_it(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "serial.jsonl"
            data = json.dumps(event()) + "\n"
            source.write_text(data, encoding="utf-8")
            alias = source.with_name("alias.jsonl")
            alias.hardlink_to(source)
            for destination in (source, alias):
                for option in ("--jsonl", "--metrics-file"):
                    result = subprocess.run([sys.executable, str(spec.origin), "--input", str(source), option, str(destination)],
                                            capture_output=True, timeout=5)
                    self.assertEqual(result.returncode, 2)
                    self.assertIn(b"must refer to different files", result.stderr)
                    self.assertEqual(source.read_text(encoding="utf-8"), data)

    def test_counts_gaps_and_restarts_without_unbounded_labels(self):
        metrics = bridge.Metrics()
        output = io.StringIO()
        events = [event(1), event(4), event(3), event(1)]
        source = io.BytesIO(b"boot message\n" + b"\n".join(json.dumps(item).encode() for item in events))
        bridge.collect(source, metrics, output)
        text = metrics.render()
        self.assertIn('aegishv_type1_missing_events_total{backend="svm"} 2', text)
        self.assertIn('aegishv_type1_boots_total{backend="svm"} 2', text)
        self.assertIn('aegishv_type1_denied_accesses_total{backend="svm",access="write"} 3', text)
        self.assertIn('aegishv_type1_invalid_events_total 1', text)
        self.assertEqual(len(output.getvalue().splitlines()), 3)
        self.assertNotIn("gpa=", text)

    def test_rejects_bad_values_and_recovers_after_an_oversized_line(self):
        bad = []
        for field, value in [("gpa", -1), ("rip", True), ("sequence", 1 << 64), ("backend", "unknown"), ("access", "x\"}\n")]:
            item = event()
            item[field] = value
            bad.append(json.dumps(item).encode())
        bad += [b'{"source":"x","source":"y"}', b'{"source":"aegishv-type1","kind":[]}']
        metrics, output = bridge.Metrics(), io.StringIO()
        source = io.BytesIO(b"a" * 9000 + b"\n" + b"\n".join(bad) + b"\n" + json.dumps(event()).encode())
        bridge.collect(source, metrics, output)
        self.assertEqual(len(output.getvalue().splitlines()), 1)
        self.assertIn("aegishv_type1_invalid_events_total 8", metrics.render())

    def test_metrics_http_uses_collected_events(self):
        metrics = bridge.Metrics()
        metrics.accept(bridge.decode(json.dumps(event())))
        server = bridge.make_server(("127.0.0.1", 0), metrics)
        worker = threading.Thread(target=server.serve_forever)
        worker.start()
        try:
            with urlopen(f"http://127.0.0.1:{server.server_port}/metrics", timeout=2) as response:
                self.assertEqual(response.status, 200)
                self.assertIn(b'aegishv_type1_events_total{backend="svm",kind="page_access"} 1', response.read())
            with urlopen(f"http://127.0.0.1:{server.server_port}/readyz", timeout=2) as response:
                self.assertEqual(response.status, 200)
        finally:
            server.shutdown()
            server.server_close()
            worker.join()

    def test_smp_keeps_one_sequence_and_distinguishes_allowed_accesses(self):
        events = [event(1), event(2)]
        events[1].update(vcpu=3, access="execute", action="allow")
        common = dict(source="aegishv-type1", backend="svm", vcpu=3)
        events += [dict(common, sequence=3, kind="pmu_status", available=False),
                   dict(common, sequence=4, kind="vector_sample", rip=4096, target=8192,
                        gpa=12288, valid=True, changed=True)]
        metrics = bridge.Metrics()
        for item in events:
            self.assertTrue(metrics.accept(bridge.decode(json.dumps(item))))
        text = metrics.render()
        self.assertIn('aegishv_type1_denied_accesses_total{backend="svm",access="write"} 1', text)
        self.assertNotIn('denied_accesses_total{backend="svm",access="execute"}', text)
        self.assertIn('aegishv_type1_vcpu_events_total{backend="svm",vcpu="3",kind="page_access"} 1', text)
        self.assertIn('aegishv_type1_pmu_available{backend="svm",vcpu="3"} 0', text)
        self.assertIn('aegishv_type1_vector_checks_total{backend="svm",vcpu="3",status="changed"} 1', text)
        self.assertNotIn('pmu_cycles_total', text)
        self.assertNotIn('invalid_events_total', text)
        self.assertTrue(metrics.accept(bridge.decode(json.dumps(event(1)))))
        self.assertNotIn('pmu_available', metrics.render())

    def test_rejects_cpu_label_growth_and_non_boolean_status(self):
        for cpu in (True, -1, 256, 1 << 63):
            item = event()
            item['vcpu'] = cpu
            with self.assertRaises(ValueError):
                bridge.decode(json.dumps(item))
        item = dict(source="aegishv-type1", backend="el2", vcpu=0, sequence=1,
                    kind="pmu_status", available=1)
        with self.assertRaises(ValueError):
            bridge.decode(json.dumps(item))


if __name__ == "__main__":
    unittest.main()
