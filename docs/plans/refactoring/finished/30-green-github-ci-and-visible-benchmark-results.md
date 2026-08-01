# Green GitHub CI and Visible Benchmark Results

## Status

**Finished refactoring queue item 30.**
Start this plan after
[Attribute Registry, Activation Groups, and Indexed Presets](../finished/29-attribute-registry-and-activation-groups.md)
and before
[Supported Scale, Output Isolation, and Warm Operator UI](../pending/31-supported-scale-output-isolation-and-warm-operator-ui.md).
Move this file to `doing/` before changing workflows, tests, benchmark tooling, or the generated
static website.

The benchmark result is temporarily informational. A benchmark gate may report a measured
failure or degraded result without making GitHub CI red, but missing, invalid, or undisclosed
benchmark evidence is not an acceptable substitute.

## Goal

Make the GitHub workflow for the exact pushed commit complete successfully apart from the
benchmark's own measured performance classification, and publish the benchmark's current
numbers on the generated static website even when a performance gate is missed.

Everything else in GitHub CI must be green. This includes quality checks, tests, documentation,
screenshots and visual assets, platform builds, release assembly where applicable, static-site
assembly, and GitHub Pages deployment. A skipped downstream job caused by an upstream failure is
not green.

## Mandatory first check

Before changing implementation:

1. identify the exact local commit, GitHub `main` commit, and applicable GitHub workflow run;
2. confirm that the GitHub CLI can read the repository, run, job, and complete failure logs;
3. if authentication is unavailable, pause and let the operator complete `gh auth login` before
   diagnosing from partial evidence;
4. record every failed, cancelled, or unexpectedly skipped job from the complete run; and
5. inspect the full failing job logs before deciding whether the defect belongs to a test,
   product code, build/release setup, benchmark reporting, or Pages assembly.

Do not infer the current GitHub result from local tests, a Forgejo run, an older commit, one
attached log excerpt, or a branch status badge.

## CI success contract

For the exact pushed commit:

- every non-benchmark job and required matrix entry completes successfully;
- expected release artifacts are built and validated on every supported platform;
- generated and tracked documentation checks agree;
- deterministic screenshot, visual-asset, and static-site inputs are present;
- the Pages artifact assembles and deploys;
- no required check is hidden by `continue-on-error`, an unconditional success fallback, or an
  inappropriate skip condition; and
- the workflow retains useful logs and artifacts when a step fails.

Fix genuine product or test regressions rather than weakening assertions. Update a test only
when the current operator contract or an intentional implementation change proves that its old
expectation is obsolete. Keep unrelated flaky infrastructure distinct from deterministic
repository failures and add bounded retry or diagnostics only where the failure is genuinely
external.

## Informational benchmark contract

The released benchmark executable still runs against the published artifact. Its execution and
report publication are required even when one of its performance gates fails.

A valid measured benchmark failure:

- produces machine-readable raw evidence and a normalized status document;
- retains the benchmark process exit code and the failed gate identities;
- classifies the public result as degraded rather than converting it to healthy;
- does not fail or block the otherwise-green workflow during this temporary policy;
- remains distinguishable from missing evidence, invalid JSON, a crashed executable, a missing
  release artifact, or CI infrastructure failure; and
- allows Pages assembly to consume the evidence regardless of the measured classification.

Only a genuine, parsed benchmark report is allowed to use the degraded path. Benchmark
infrastructure failures remain explicit `unknown` evidence and must not be presented as a
measured performance result.

## Static benchmark page

The generated static performance page must show current observed numbers, not only a
healthy/degraded label, target budgets, or a link to a downloadable archive.

At minimum it shows, when present in the benchmark report:

- release version, tested commit, generation time, runner or hardware label, and benchmark
  workload;
- fixture count, universe count, requested output rate, achieved output cadence or equivalent
  frame/timing measurements, and deadline misses;
- required-floor result and each separately measured mutation gate;
- Patch transaction p50 and p95 values for the measured fixture batches and their budgets;
- doubled-density probe numbers when attempted, or an explicit reason it was not attempted; and
- every failed or unavailable metric with an honest degraded or unknown state.

The page and its copied `performance/status.json` must come from the same normalized evidence.
HTML generation must escape report-controlled text, tolerate schema-compatible absent optional
metrics, and never replace real zero values with an em dash or truthiness fallback.

Add focused tests with healthy, degraded, unknown, zero-valued, and partially available reports.
The degraded fixture must prove that a failed benchmark still displays its observed numbers in
the assembled static output.

## Repair and verification loop

Work from the first actionable failure in the current GitHub run:

1. reproduce it with the smallest relevant local check where practical;
2. make one coherent repair while preserving unrelated work;
3. run the focused check and proportionate broader checks;
4. commit the coherent change with a semantic commit message;
5. push the intended commit to the GitHub-tested branch;
6. wait for the new GitHub workflow run and inspect every job for that exact commit; and
7. repeat until the success contract above is met.

Do not call the plan complete because local checks pass, because an earlier run was green, or
because the benchmark failure was ignored. The final GitHub run, Pages artifact, and deployed
static performance page must all refer to the intended commit and release evidence.

## Acceptance

- GitHub CLI authentication and repository/run/log visibility are proven.
- The final GitHub workflow run is green for the exact intended commit, with no unexpected
  failed, cancelled, or skipped non-benchmark work.
- A deliberately degraded benchmark fixture exits through the informational policy while
  retaining its failure classification and observed metrics.
- Missing or invalid benchmark evidence remains visibly unknown and cannot masquerade as a
  degraded measured run.
- The generated static page visibly contains the normalized current benchmark numbers for both
  passing and failing measured gates.
- The deployed GitHub Pages performance page and downloadable raw report match the tested
  release commit.
- Focused benchmark-report, landing-page, workflow-contract, and affected regression tests pass.
- `## Result` records the final commit, GitHub run URL, job outcomes, benchmark classification
  and numbers, Pages URL, verification commands, and any explicitly accepted temporary
  benchmark failure.

## Result

Completed on 2026-07-31.

The final implementation commit is
[`572bbc05aa0f3d30dc8c3c52d4afe0d9381e2206`](https://github.com/kellertobias/tosklight/commit/572bbc05aa0f3d30dc8c3c52d4afe0d9381e2206),
with the preceding coherent CI repairs in `a8cb775e` and `b45e0dbd`. The exact-commit
[GitHub workflow run 30598292317](https://github.com/kellertobias/tosklight/actions/runs/30598292317)
completed successfully. Linting, unit and architecture tests, the single reusable Playwright
application build, API E2E, all four UI E2E shards, Linux x86_64, Linux ARM64, Windows and macOS
release archives, reviewed documentation screenshots, the product demo, operator manual,
release publication, benchmark measurement, static-site assembly and GitHub Pages deployment
all completed successfully. No required downstream work was skipped.

The repair:

- restored truthful Storybook fixtures and consolidated help and marketing screenshot generation
  behind one Storybook build;
- removed the repeated `ts-rs` Serde warning without weakening `deny_unknown_fields`;
- made Playwright shards consume one exact-commit Linux application build and kept the long
  frontend warmup cases behind the explicit performance suite;
- repaired deterministic Text Editor and OSC acceptance paths;
- removed the debugging-only Hardware Controls application from release CI and public downloads
  while retaining its local development path;
- restored the accidentally disconnected File Manager/Text Editor stylesheet cascade, added an
  architecture guard for its ordered entrypoint and refreshed the reviewed File Manager image;
  and
- retained failure diagnostics and kept benchmark infrastructure failures distinct from valid
  measured degradation.

The published benchmark is a valid measured **degraded** result. It tested release `0.1.0` at
commit `572bbc05aa0f3d30dc8c3c52d4afe0d9381e2206` on GitHub Actions
`ubuntu-22.04` (`AMD EPYC 9V74`, four logical CPUs):

- required floor: 1,024 fixtures, 32 fully packed universes, 32 fixtures per universe, requested
  100 Hz;
- achieved cadence: 63.454267 Hz, with a minimum completed one-second cadence of 63 Hz,
  191 deadline misses, 109 dropped ticks and 180 deferred ticks;
- large-show mutation gate: pass; 120 fixtures measured 1,727.438 µs p50 and 1,774.978 µs p95,
  while 1,200 fixtures measured 1,734.078 µs p50 and 1,790.59 µs p95;
- persisted Patch gate: pass; one fixture measured 114,121.322 µs p50 and 244,117.258 µs p95
  against a 250,000 µs budget, while 100 fixtures measured 165,273.323 µs p50 and
  306,632.752 µs p95 against a 500,000 µs budget; and
- doubled density (2,048 fixtures) was not attempted because the required baseline floor failed.

The required-floor failure is explicitly accepted under this plan's temporary informational
benchmark policy. Its process exit code remains `1`, `required_floor` remains the failed gate,
and the public classification remains degraded rather than healthy. Patch UI action-to-visible
latency remains explicitly informational browser evidence rather than a release gate.

The deployed [performance page](https://kellertobias.github.io/tosklight/performance/) visibly
contains those numbers. Its
[`performance/status.json`](https://kellertobias.github.io/tosklight/performance/status.json)
is byte-identical (SHA-256
`becd3189468bbaf4e2b422081c3c85eac5bf44681a45e5b287f4198d0643498b`) to the
[release status asset](https://github.com/kellertobias/tosklight/releases/download/v0.1.0/tosklight-performance-status-0.1.0.json).
The [detailed raw report](https://github.com/kellertobias/tosklight/releases/download/v0.1.0/tosklight-performance-report-0.1.0.zip)
contains the measured `hard-floor.json`, normalized status, stderr and summary for the same
release commit.

Verification included:

- `npm run test:architecture`
- `bash tools/test-artifact-paths.sh`
- `bash tools/test.sh documentation-screenshots` in both generation and no-update review modes
- `bash tools/test.sh e2e-build`
- `LIGHT_REUSE_E2E_BUILD=1 npm run test:e2e -- tests/50-semantic-files-and-text-editor.spec.ts --grep TEXT-015 --workers=1`
- focused OSC and Text Editor acceptance runs, frontend builds, Storybook build, generated wire
  contract checks and command-boundary tests
- exact-commit GitHub run inspection through every matrix and downstream job
- deployed Pages and release-asset retrieval, normalized evidence inspection and status-document
  digest comparison

The product-demo job is green but remains the dominant CI duration at roughly sixteen minutes;
shortening that real recording is follow-up optimization, not an unverified or skipped Plan 30
gate.

### 2026-07-31 Storybook and capacity-policy follow-up

The public-site pipeline now preserves the exact static Storybook build used for reviewed Help
and marketing screenshots, transfers it as the `storybook-static` CI artifact, publishes it at
`/storybook/`, and links it from the Pages header, developer cards, and footer. Pages assembly
requires `storybook/index.html`; local assembly builds Storybook only when that reviewed artifact
is not already available.

The release-performance indicator now applies the revised capacity policy without changing the
separate packaged UI contracts:

- the canonical 301-physical-instance demo remains a 100 Hz packaged release gate;
- the released 1,024-fixture engine/output workload is green at a minimum one-second cadence of
  at least 60 Hz, yellow from 40 Hz through 59.999 Hz, and red below 40 Hz;
- mutation and persisted Patch gate failures remain red regardless of cadence; and
- the 2,048-fixture run is always attempted as a non-blocking diagnostic, even when the
  1,024-fixture workload misses its configured 100 Hz reference.

The normalized report retains the per-phase p50/p95/p99 evidence and identifies the largest
measured p95 contribution among engine render/fixture projection, protocol encoding, loopback
delivery, and benchmark validation. The performance page displays that limiting phase for both
1,024 and 2,048 fixtures and links the raw report for the complete measurements. It explicitly
states that this Linux release probe has no UI: the exact 1,000-instance Stage plus Fixture Sheet
usability proof remains the packaged/UI acceptance recorded in Plans 20 and 21.

The already-required product-demo run now also emits current-release performance evidence for
the exact canonical show: 262 controllable fixtures, 33 visual-only Venue records, 295 total
patch records, 343 physical Stage instances, and the 3D Stage visible. Release Performance reports
its Stage presentation cadence, source-to-canvas
p95, render-duration p95, maximum draw calls, and maximum triangles. This reuses the existing
recording window instead of adding another long CI run. The page labels the measurement as
Chromium evidence and retains the packaged Tauri run as the authoritative 100 Hz output and
WebView acceptance.

Focused verification passed with `node --test tools/run-release-performance.test.mjs
tools/performance-publication.test.mjs`, `npm run storybook:build`, `npm run test:architecture`,
the complete `npm run test:unit`, and a full `npm run pages:generate` using the same reused manual
artifact path as CI. A local release-binary policy run measured 1,024 fixtures at 100 Hz minimum
and 2,048 fixtures at 84 Hz minimum; the limiting p95 phase was engine render and fixture
projection in both runs. These local numbers validate the report shape and are not substituted
for the next GitHub runner measurement.

### 2026-08-01 product-demo and operator-workflow follow-up

The canonical product demo now starts from an empty show and carries a JSON-shaped, 25 fps edit
contract at the top of its scenario. Every chapter has an exact frame budget, the crossfades and
narration holds are configurable, repetitive work is accelerated visibly, and the first decoded
frame is already the complete ToskLight Product Demo card. The final 12:05.15 edit covers Show
Setup, Output Configuration, Defining Groups, Assigning Group Masters, Preset Setup, Cuelists,
Dynamics, Virtual Playbacks, and Busking and Preload. It ends after a 16-bar, 120 BPM busking
sequence in which bar 8 prepares a preload look and bar 16 commits it with a two-second Programmer
Fade.

The production workflows exercised by the recording were tightened alongside the scenario:

- Fixture Library search accepts human-readable digit and word variants such as `4Point Truss`,
  and the titlebar search occupies the full modal-header height.
- Fixture Sheet location edits apply a single value to every selected fixture, while THRU spreads
  retain ordered selection semantics.
- output configuration accepts paired logical and destination universe ranges such as
  `1 THRU 8`; the server creates the full range atomically and replay-safely, with no partial
  mutation on invalid input.
- Desktop configuration keeps Delete beside Close and presents Clone Current Desktop as the main
  body action.
- Group tiles support the mode-aware SET touch paths used to name Groups in Programmer mode and
  assign Group Masters in Playback mode.
- the generated starter show now retains the revised patch, eight Cuelists, fourteen physical
  Playbacks, thirty Dynamics, and the canonical 10 by 5 Virtual Playback layout.

Verification passed with 60 focused desktop tests, four focused Rust output-range tests, the
programmer action timing gate, the planned Playback topology test, the production desktop build,
the generated-show acceptance test, and a complete non-recording product-demo rehearsal. The
single final recording run passed in 16.6 minutes and produced 18,140 canonical frames at 25 fps.
Both delivery files are 1920 by 1080 and 725.6 seconds long; the reviewed WebM uses VP9 and the
reviewed MP4 uses H.265.
