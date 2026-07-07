# Host Sensor Release Plan

This plan is for the host-side KVM telemetry sensor only. It is not a type-1, full VMI, or hardware PMU sampling release plan.

## Required Gates

- Locked build, clippy, tests, replay smoke, and documentation link check pass.
- Event schema compatibility examples pass.
- Package metadata and systemd unit tests pass.
- Release checksum, SBOM, and signing scripts are reviewed.
- `docs/STATUS.md` and README describe the same runtime scope.
- `docs/HARDWARE_TEST_MATRIX.md` has no checked row without evidence.

## Release Notes Shape

The release notes should list:

- tracefs replay and live smoke boundaries;
- JSONL schema and spool behavior;
- policy/action dry-run and stable identity safety gates;
- packaging and service defaults;
- unsupported backend paths.

## Not Included

This plan does not include live guest memory inspection, type-1 boot, direct Stage-2 enforcement, or real hardware PMU sampling.
