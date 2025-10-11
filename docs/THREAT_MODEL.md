# THREAT MODEL

Goal: detect hostile code execution paths and memory policy violations in VMs without installing agents.

- Attacker controls guest ring3/ring0.
- Hypervisor enforces W^X at stage‑2; executes on marked pages generate EPT/NPT violations.
- PMU anomalies (e.g., sudden branch miss spikes) can flag ROP/JOP bursts.
- Syscall‑path policy catches abuse of unexpected binaries in sensitive paths.

Out of scope: physical attackers, SMM/SEV-ES/TSME bypass, microcode backdoors.
