# Packaged Operator and Reference-hardware Verification

## Goal

Close operational evidence that cannot be established by unit tests or a development web server.

Estimated effort: 0.5–1 Codex day plus access to the reference Mac, attached controls, and
Pi-class hardware.

## Required work

1. Stabilize the serial visual-catalog capture harness and finish every catalog case.
2. Run the packaged Tauri desktop and Hardware Controls sibling app through startup, readiness,
   window/display lifecycle, quit/relaunch, OSC attachment, and representative operator flows.
3. Retain strict output benchmark reports on a fixed ordinary show-control Mac and a Pi-class
   low-power host.
4. Add production socket delivery, separate contribution/arbitration/projection timings, and
   practical CPU/allocation observation where the platform permits.
5. Document runner identity, power mode, OS, build artifact checksum, profile, trial count, and raw
   reports.

## Acceptance and verification

- The complete visual catalog finishes with explained skips only.
- Real packaged applications use the just-built/released artifacts and the expected sidecar.
- Hardware/software/OSC behavior remains in parity.
- Hosted-runner degradation remains nonblocking; fixed-hardware results remain strict and
  separately identified.
