# Network Isolation

AegisHV currently has a virtio-net quarantine model in `aegishv-devices`. It is a library model, not a live virtual switch, not a tap manager, and not SR-IOV enforcement.

## Quarantine State

The model distinguishes:

- link down: all packet classes are dropped;
- normal link: guest data and management traffic are allowed;
- quarantined link: guest data is dropped;
- quarantined link with management allowance: bounded management traffic can remain allowed.

The model is intentionally small. It does not classify protocols, inspect payloads, or push firewall rules. A live backend must provide the packet class before using the decision.

## Bridge And Tap Limits

Linux bridge and tap setups are host policy, not hypervisor isolation by themselves. A bridge can leak traffic if the host firewall, tap ownership, MAC filters, or namespace boundary is wrong. Treat bridge/tap isolation as degraded unless the operator can prove:

- the tap belongs to the intended VM;
- the tap is not shared with another VM;
- host firewall and forwarding state match the quarantine decision;
- lifecycle teardown removes stale tap state;
- metrics and audit events record quarantine transitions.

## SR-IOV And Passthrough Limits

SR-IOV virtual functions and passthrough devices require IOMMU isolation. A unique function number is not enough. Before assignment, the backend must prove requester ID or Stream ID ownership, DMA translation, interrupt isolation, and fault reporting.

If the proof is missing, assignment must fail closed. The current repository models that rule in the DMA domain code. It does not program VT-d, AMD-Vi, or SMMU hardware.

## Virtual Switch Boundary

A virtual switch belongs outside the smallest hypervisor runtime unless a later implementation proves that keeping it inside the runtime is smaller and safer. The current design expects quarantine policy to be decided in the control plane or service VM and enforced by a live network backend that can report success or refusal.

Do not describe the current tree as having live network isolation. It has a tested quarantine decision model and documents the checks required before a live backend can claim enforcement.
