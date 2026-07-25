# 10b3 — Playback, OSC, and demo representative migration

## Outcome

Migrate an existing two-Cue Playback/Page workflow, an OSC parity workflow, and
the product-demo/free-run path to the semantic bench.

## Done gate

- Page addressing, Cue/Playback ownership, and OSC feedback remain explicit.
- Demo free run coexists with ordinary deterministic-clock scenarios.
- Maintained video/show/screenshot artifacts keep their existing workflow.
- Focused UI/OSC/demo, architecture, inventory, parallel stress, and full E2E
  pass.

## Result

Completed on 2026-07-25.

- Promoted the existing two-Cue runtime, current/explicit Page addressing, and
  visible keypad/API/OSC selection-parity scenarios into an enforced root
  semantic-world spec.
- Extracted the product-demo choreography behind the public `t.demo.run()` helper.
  The root spec now states only the demo intent while the helper preserves the
  recording timeout, free-run behavior, fixed WebM/PNG paths, maintained
  `assets/demo.show` update path, and existing `npm run test:demo` entrypoint.
- The generated inventory now records 308 active root cases across 40 files and
  identifies the three playback/OSC scenarios plus the demo as
  `migrated-semantic-world`.

Verification:

- Control UI typecheck, architecture checks, inventory generation, and diff check:
  passed;
- playback/OSC cases plus five-bench isolation stress: 5 passed using four workers;
- focused product demo: 1 passed in 57 seconds;
- full browser E2E exercised all 336 cases: the migrated paths passed, with three
  unrelated timing failures; their exact focused reruns passed 2 of 3 together,
  followed by the remaining Stage shift-click case passing alone.
