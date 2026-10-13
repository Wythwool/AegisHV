import importlib.util
import io
import json
from pathlib import Path
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


if __name__ == "__main__":
    unittest.main()
