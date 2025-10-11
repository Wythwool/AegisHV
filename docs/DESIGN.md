# DESIGN

AegisHV separates **policy** and **mechanism**. The mechanism (hypervisor / harness) emits facts;
the policy decides what is suspicious.

- Mechanism:
  - For x86: VMX with EPT. For AMD: SVM with NPT. For arm64: EL2 with Stage‑2.
  - W^X policy: stage‑2 mappings default to W^X; *exec trap* opt‑in for sensitive pages.
  - PMU hooks: sampling ISR pushes records into a lock‑free ring buffer.
  - Syscall‑path guard: inspect guest's syscall vector + userspace RIP, resolve path (dev harness simulates with host maps).
  - Events: concise structs → ring buffer → userspace exporter.

- Policy:
  - YAML (`configs/policies.yaml`): paths, hash allow‑lists, thresholds for PMU anomalies, exec‑trap pages.
  - Versioned; hash‑locked by exporter to avoid silent drift.
