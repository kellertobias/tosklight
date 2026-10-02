# Semantic performance workloads

Deterministic workload and evidence tooling for the fixture-independent Position, Color and
Focus/Zoom work. TL-548 uses it for scheduler verification. TL-553 uses it for performance
acceptance. The tooling supplies repeatable inputs and honest reports. It does not install a
rig, start Dynamics, measure anything or accept a result. The runtime owner (TL-548) keeps
runner integration, paired baseline/candidate runs, timing diagnosis and acceptance.

The existing baselines and gates are unchanged:
`tools/packaged-stage-profile.mjs`, `tools/stage-dynamics-scene.mjs`, `tools/stage-large-scene.mjs`
and the packaged runners keep their profiles, thresholds and 20-Dynamic capacity rig. Semantic
Dynamics use pool numbers from 9501, clear of the capacity rig's 9001–9020.

## Files

| File | Role |
|---|---|
| `tools/semantic-performance-contract.mjs` | UUIDv5 and seeded PRNG, canonical JSON/digests, strict validator for the generated wire JSON Schemas, and the Rust storage rules the schemas cannot express |
| `tools/semantic-performance-fixtures.mjs` | Reads shipped `.toskfixture` packages (read-only), classifies each profile head, builds a deterministic synthetic rig |
| `tools/semantic-performance-workload.mjs` | Workload builder, manifest, deterministic tracking inputs, CLI |
| `tools/semantic-performance-report.mjs` | Build/binary/host identity, evidence model, claims |
| `tools/semantic-source-manifest.mjs` | Sorted path/content-hash manifest of implementation sources (TL-604) |
| `tests/bench/performance/semanticPerformanceWorkload.ts` | Read-only bridge: builds the workload from the desk's actual `/api/v2/patch` snapshot |
| `tools/semantic-performance-*.test.mjs` | Contract and smoke tests (run by `npm run test:architecture`) |

## What a workload contains

`buildSemanticPerformanceWorkload(patch, { seed, request })` returns:

- `definitions`: targetless `DynamicDefinitionProjection` objects that use programming lanes.
  Each one is validated against `$defs/DynamicDefinitionProjection` in
  `crates/light/contracts/wire/schemas/v2-programming/programming-values-snapshot.schema.json`.
  Each one is also checked against the mirrored rules in `crates/light/domain/dynamics`: the
  representation/component pairing, component domains, keyframe shape and the Angle pair.
- `activations`: each Dynamic, its actual targets, and a start body. The start body has the same
  shape as the one `tests/bench/performance/stageLargeScene.ts` uses.
- `staticValues`: `set_fixture` programmer mutations for `position`, `color`, `focus` and `zoom`.
  These are the static `Current` bases the Dynamics animate around. Each value is validated
  against `$defs/ProgrammingAttributeValue`. Semantic Color bases carry exact virtual-recipe XYZ.
- `positionScenarios`, `dirty`, `tracking`, and the `manifest` described below.

Coverage:

- Angle Dynamics always contain both Pan and Tilt. There are three variants:
  - an authored Pan lane with an automatic Tilt partner;
  - an authored Tilt lane with an automatic Pan partner;
  - authored Pan and authored Tilt.

  An automatic partner is the saved two-point static `Current` lane, identified as
  `uuidv5(definition_id, "tosklight:position-current:<axis>:v1")`. This is the same identity
  that `angle_pair.rs` produces.
- Position Targets use four scenarios:
  - `fixed`: an Origin reference with a world offset.
  - `referenced`: static mounts aim at one shared moving aim Point.
  - `shared-moving-mount`: a group shares one moving mount Point (`position_master`) and one
    moving aim Point.
  - `separate-moving-mount`: each fixture has its own moving mount Point and its own aim Point.

  Angle and Target targets are disjoint, so only one representation owns Position.
- Color intent uses four classes:
  - RGB: recipe red/blue lanes.
  - RGBW: recipe green plus White Blend.
  - CMY: hue/saturation lanes.
  - Wheel-only: held hue keyframes.

  An independent UV Dynamic uses `semantic_color` with `retain` basis and the `uv` component.
  RGBW selection prefers UV-capable heads, so UV composes with Color intent. The manifest
  reports that overlap.
- Focus uses the focus component. Zoom uses the zoom component, and only on heads whose zoom
  function is expressed in degrees ("physical Zoom").
- Dirty scenarios:
  - `static-points`: nothing moves.
  - `small-subset`: `dirtySubsetPoints` Points move.
  - `all-points-move`.

  Each scenario records the moving Point IDs and the dependent target count.
- Tracking inputs come from `trackingFrames(workload, { scenario, rateHz })`. This yields
  deterministic frames at each configured tracking rate: the default is 30, 60 and 120 Hz. Every
  Point is emitted every frame, the way a PSN source sends them. Static Points repeat exactly.
  Output rates are configurable; the default is 44, 60 and 125 Hz. They are recorded as the
  rate matrix.

Targets are real identities. A master-shared head is programmed through its root `fixture_id`.
Every other head is programmed through its `logical_heads[].fixture_id`, matched by
`profile_head_id` with a `head_index` fallback. On a live desk these come from the patch
snapshot. The offline synthetic rig derives them deterministically from the seed and uses the
real shipped profiles and modes.

## Manifest

The `manifest` holds only stable data and contains no timestamps. The same seed and request
produce byte-identical canonical JSON. A different seed changes the IDs, the selection and the
motion. The manifest records:

- Identity:
  - `workloadVersion`, `seed`, `workloadNamespace` and `workloadId`.
  - `identitySource`: `live-patch` or `synthetic-deterministic`.
  - `patchIdentity`: the show, the patch revision and a digest of the fixture IDs.
- Inputs:
  - `contract`: SHA-256 of the generated schemas and `light-wire.ts`.
  - `fixtureLibrary`: package files and their SHA-256 for the synthetic rig. A live patch
    records it as `unavailable`.
- Counts:
  - `counts`: requested and actual root fixtures, Points, Dynamics and targets. Logical heads,
    lanes and static values are actual counts only. Requested counts that a live patch cannot
    know are `{status: "unavailable"}`.
  - `capabilityMix`: requested and realized counts with `covered`, `partial`, `missing` or
    `not-requested`. `capabilityAvailability` shows what the patch could supply. `shortfalls`
    and `limitations` explain every gap.
- Coverage:
  - `laneCoverage`, keyed by representation, component and source kind, with lane and
    lane×target counts.
  - `angleDynamics`: the axes and automatic partners of each Angle Dynamic.
  - `mixedColorWithUv`: the overlap between UV targets and Color intent targets.
- Motion and rates:
  - `dirtyScenarios` and tracking stream digests: frames, samples and SHA-256 per scenario and
    rate.
  - `rates`: `kind: "configured-targets"`. These are never observed rates.
- Support:
  - `productionSupport`: defaults to `unavailable`. Production still accepts contract 0 until
    the TL-548 gates pass.
- `manifestSha256`.

## Reports and claims

`createSemanticPerformanceReport({ manifest, build, host, evidence })` records the following:

- Identity:
  - Build: git HEAD and whether tracked files changed, the binary SHA-256 and size, and the
    build profile.
  - Source: `identity.build.source.sourceSha256`, the digest of a sorted relative-path/SHA-256
    manifest of the implementation sources (root files, `.cargo`, `apps`, `crates`, `tests`,
    `tools`, `assets/fixture-library`). Tracked edits, untracked additions and deletions all
    change it, so two trees with the same HEAD and dirty flag never share an identity. Contents,
    timestamps, generated artifacts, dependencies, Git internals and likely secrets are never
    recorded. A checkout nested in another repository without Git metadata of its own (a
    snapshot under `.artifacts`) is hashed from the filesystem: its `gitHead` and `deletions`
    are `unavailable` instead of borrowing the enclosing HEAD. Unreadable files stay
    `unavailable` entries and mark the manifest `complete: false`. An explicit immutable snapshot
    manifest can be supplied with `collectBuildIdentity({ sourceManifest })`; its digest is
    validated and `origin` records `supplied-snapshot-manifest`.
  - Host: OS, architecture, CPU model, logical CPUs, memory and Node version. The GPU is
    `unavailable` until the native Stage runner supplies it.
  - Workload: the workload's version, ID, seed and manifest digest.
- `rates.configured` and `rates.observed`, kept as separate fields.
- Every required counter in `REQUIRED_EVIDENCE.output`, `.nativeStage` and `.semantic`. Each
  counter is either `measured(value, unit, { source, counter, … })` or
  `unavailable(reason)`. A missing counter becomes `unavailable`, never zero. A bare number is
  rejected.
- `claims.output` and `claims.nativeStage`, evaluated independently:
  - `not-evaluated` for synthetic input.
  - `incomplete` when a counter is missing or semantic production support is not `active`.
  - `evidence-complete` otherwise.

  `acceptance.granted` is always `false`. Thresholds and pass/fail stay with the TL-548/TL-553
  gate.
- There is no browser Stage. Passing `browserStage` evidence, or native Stage evidence with
  `kind: "browser"`, throws.

## Invocation

Offline workload for review or diffing. This writes the manifest, the workload and a
not-evaluated report to `.artifacts/performance/semantic-workloads/<workloadId>/<inputs>/`, where
`<inputs>` is the first 16 hex digits of `manifestSha256`. Two patches that share a seed and
request therefore get separate directories, and a report always sits next to the exact
manifest, workload and tracking inputs it names. Regenerating identical inputs reuses the
directory. Writing different inputs into a directory that already holds a manifest (for example
with `--out`) is refused and leaves the existing evidence untouched. Reports are named
`report-<createdAt>-<reportSha256 prefix>.json` and are never overwritten; the full source manifest
is retained once beside them as `source-manifest-<sourceSha256>.json`:

```sh
node tools/semantic-performance-workload.mjs --seed 548
node tools/semantic-performance-workload.mjs --seed 548 --tracking-hz 30,60,120 --output-hz 44,60,125 \
  --duration 30 --subset-points 2 --emit-tracking
```

TL-553 on a desk whose rig is already installed. Save the patch snapshot, then build against
the actual identities:

```sh
curl -fsS -H "Authorization: …" http://127.0.0.1:5000/api/v2/patch \
  > "$(npm run --silent artifact-path -- tmp)/semantic-patch.json"
node tools/semantic-performance-workload.mjs --seed 553 \
  --patch "$(npm run --silent artifact-path -- tmp)/semantic-patch.json" \
  --binary "$(npm run --silent artifact-path -- cargo)/release/light-headless"
```

TL-548 runner integration from a bench or Node runner:

```ts
import { semanticWorkloadForLivePatch } from "./tests/bench/performance/semanticPerformanceWorkload";
import { trackingFrames } from "./tools/semantic-performance-workload.mjs";
import {
	collectBuildIdentity, collectHostIdentity, createSemanticPerformanceReport,
	measured, unavailable, writeSemanticPerformanceReport,
} from "./tools/semantic-performance-report.mjs";

const workload = await semanticWorkloadForLivePatch(api, { seed: 548, request: { outputRatesHz: [60] } });
// Runner-owned: apply workload.staticValues, create workload.definitions,
// POST each activation.start, then stream trackingFrames(workload, { scenario: "small-subset", rateHz: 120 }).
const report = createSemanticPerformanceReport({
	manifest: workload.manifest,
	build: collectBuildIdentity({ binaryPath, buildProfile: "release" }),
	host: collectHostIdentity(),
	evidence: {
		source: "measured-run",
		productionSemanticSupport: { status: "active", provenance: { source: "desk", counter: "programming.contract" } },
		output: { schedulerP99Ms: measured(0.42, "ms", { source: "tl-548-runner", counter: "scheduler.p99" }) },
		nativeStage: { presentationHz: unavailable("Stage closed for the output-isolation pass") },
	},
});
await writeSemanticPerformanceReport(report); // .artifacts/performance/semantic-workloads/<workloadId>/<inputs>/
```

Tests:

```sh
node --test tools/semantic-performance-workload.test.mjs tools/semantic-performance-report.test.mjs
npm run test:architecture   # includes the same tests
```

## Current library limits

The default synthetic rig covers every requested capability with shipped packages. The manifest
states these limits instead of hiding them:

- No shipped zoom function has a verified opening convention. Only the Cameo Auro Spot Z300
  has a zoom function in degrees, and its quality is `unknown`. Its 12 heads are the only
  physical Zoom targets. The manifest adds a `zoom-physical-model` limitation, and the workload
  assumes the `beam` convention. Heads that have zoom without degrees (Robe, Clay Paky and
  others) are counted as `zoom.unmapped` and are not targeted.
- No package combines a color wheel with UV. UV comes only from `generic--rgbwauv-led` and
  `cameo--root-par-6`, which are both RGBW-class heads.
- Wheel-only heads include the root head of multi-head washes such as the Robin 600X. That root
  head carries only the wheel; its RGBW emitters are separate logical heads. A hybrid head that
  combines a wheel with mixing is counted as `color.hybridWheel`.
- A request larger than the patch can supply realizes what exists. The manifest marks it
  `partial` or `missing` and lists it in `shortfalls`.
