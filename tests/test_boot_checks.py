import importlib.util
import json
from pathlib import Path
import unittest


def script(name):
    path = Path(__file__).resolve().parents[1] / 'scripts' / f'test-type1-{name}.py'
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class BootChecks(unittest.TestCase):
    def test_linux_telemetry_rejects_missing_cpus_spoofing_and_false_pmu_values(self):
        check = script('linux').check_events
        fields = [dict(kind='pmu_status', available=False),
                  dict(kind='page_access', rip=4096, gpa=4096, access='execute', action='allow'),
                  dict(kind='page_access', rip=4100, gpa=4104, access='write', action='emulate'),
                  dict(kind='vector_sample', rip=4096, target=8192, gpa=12288, valid=True, changed=False)]
        events = [dict(source='aegishv-type1', backend='el2', sequence=index, vcpu=0, **item)
                  for index, item in enumerate(fields, 1)]
        counters = dict(execute_faults=1, write_faults=1)
        good = '\n'.join(map(json.dumps, events))
        self.assertIn('pmu_available{backend="el2",vcpu="0"} 0', check(good, 1, counters, False))
        for bad in (good.replace('"sequence": 2', '"sequence": 999999'),
                    good.replace('"available": false', '"available": true'),
                    good.replace('"valid": true', '"valid": false')):
            with self.assertRaises(AssertionError):
                check(bad, 1, counters, False)
        with self.assertRaises(AssertionError):
            check(good, 2, counters, False)

    def test_linux_rejects_poweroff_before_userspace_and_missing_hotplug(self):
        check = script('linux').check_output
        good = '\n'.join(['aegishv:linux:cpu-online 0', 'aegishv:linux:cpu-online 1',
            'aegishv:linux:cpu-online 1', 'aegishv-linux: cpus=2', 'aegishv-linux: atomic-workers-ok',
            'aegishv-linux: cpu-hotplug-ok', 'aegishv-linux: console-ok cpu=0',
            'aegishv-linux: console-ok cpu=1', 'aegishv-linux: userspace-ok',
            'aegishv:linux:poweroff ticks=200 exec=1000 write=50']) + '\n'
        self.assertEqual(check(good, 2)['ticks'], 200)
        for bad in (good.replace('aegishv-linux: userspace-ok', ''),
                    good.replace('aegishv-linux: cpu-hotplug-ok', ''),
                    good.replace('aegishv-linux: console-ok cpu=1', ''),
                    'Kernel panic\n' + good, good + 'aegishv-linux: userspace-ok\n'):
            with self.assertRaises(AssertionError):
                check(bad, 2)

    def test_vmx_requires_exit_order_and_consistent_timer(self):
        vmx = script('vmx')
        good = '\n'.join('aegishv:type1:' + name for name in vmx.MARKERS) + '\n'
        good += '\n'.join('aegishv:type1:vmx-' + name for name in (
            'cpu-signature=0x306c3', 'timer-rate=0x1', 'timer-reload=0x800000', 'timer-effective=0x1000000')) + '\n'
        self.assertEqual(vmx.check_output(good)['timer-rate'], 1)
        for bad in (good.replace('guest-pat-state-ok', 'missing'),
                    good.replace('timer-reload=0x800000', 'timer-reload=0x1'),
                    good.replace('guest-hlt-exit-ok', 'guest-run-ok'),
                    good + 'aegishv:type1:guest-exit-error\n'):
            with self.assertRaises(AssertionError):
                vmx.check_output(bad)

    def test_dma_requires_permission_faults_and_a_guest_after_quarantine(self):
        check = script('iommu').check_output
        good = '\n'.join('aegishv:iommu:' + name for name in (
            'quarantine-on', 'blocked-before-map', 'page-roundtrip-ok', 'adjacent-page-blocked',
            'read-only-ok', 'revoked-ok', 'dma-test-ok')) + '\n'
        good += '\n'.join(f'aegishv:iommu:fault source=0x18 address={address} reason={reason} read={read}'
            for address, reason, read in [('0x800000', 1, 'false'), ('0x2000', 5, 'false'),
                ('0x1000', 5, 'false'), ('0x1000', 5, 'false'), ('0x1000', 6, 'true')]) + '\n'
        good += 'aegishv:svm:guest-run-ok\n'
        self.assertEqual(len(check(good)), 5)
        for bad in (good.replace('reason=6', 'reason=5'), good.replace('address=0x2000', 'address=0x3000'),
                    good.replace('aegishv:svm:guest-run-ok', ''), good.replace('read-only-ok', 'missing')):
            with self.assertRaises(AssertionError):
                check(bad)


if __name__ == '__main__':
    unittest.main()
