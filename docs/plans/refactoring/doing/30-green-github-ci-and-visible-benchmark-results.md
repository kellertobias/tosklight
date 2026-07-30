# Green GitHub CI and Visible Benchmark Results

## Status

**Doing refactoring queue item 30 — verify the current GitHub commit before implementing.**
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
