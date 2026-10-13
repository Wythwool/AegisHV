# Network actions and models

The sensor's `quarantine_nic` action uses QMP link control for the configured VM.
Stable identity and the selected monitor socket determine which VM receives it.

`aegishv-devices` also has a virtio-net decision model. Link-down drops all
packets. A normal link allows data and management traffic. Quarantine drops
guest data and optionally permits packets the caller classifies as management.
The model does not inspect protocols, configure taps, or install firewall rules.

Bridge/tap policy and passthrough isolation need a live backend. The DMA model
requires device ownership, translation, interrupt isolation, and fault reporting
before assignment. It does not program VT-d, AMD-Vi, or SMMU hardware.
