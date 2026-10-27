# Guided fixture commissioning and calibration

Status: design proposal for [TL-672](https://plans.tokenet.de/TL/672). No workflow,
solver, persistence extension or live calibration feature is implemented by this document.
This proposal supplies the written-design part of MAP-06; it does not turn the other
MAP-01–06 execution results or unavailable physical measurements into passes.

## Existing controls and boundaries

Reuse the existing editors and their validation; add guided capture and review around them.
Do not create a second calibration registry or silently publish changes to the Fixture Library.

| Existing surface | Current responsibility | Proposed reuse |
| --- | --- | --- |
| Fixture Library → Mapping → Details | Physical endpoints, monotonic samples, quality/source/revision, Zoom Beam/Field convention | Review source mapping before capture; promote reusable measurements only through an explicit new profile revision. |
| Fixture Library → Geometry / Emitters & Motion | Physical coordinate frame, pivots, axes and native function bindings | Establish the kinematic model and identify unsupported or estimated geometry. Do not fit visual mesh scale as physical travel. |
| Show Patch → Pan / Tilt → Position calibration | Physical root/copy offsets and identity-bound individual zero/direction overrides | Keep manual editing and Clear actions; present guided results in these terms without applying family and individual corrections twice. |
| Show Patch → Light source → Color calibration | Unit-specific emitter gains and complete optical-path observations, exact source identity | Reuse evidence, recipe and validation controls; add measurement guidance and a clearly separate live-preview integration. |
| Programmer Focus / Zoom, Iris and Position controls | Semantic requests, independent ownership and supported hardware/OSC gestures | Reuse their interaction components against an isolated commissioning output owner, never by overwriting Normal or Preload. |

Source baseline: [fixture/profile help](../help/10-ToskLight-Architect/02-Fixtures-and-Patch/11-physical-channel-mapping.md),
[installed Position](../help/10-ToskLight-Architect/02-Fixtures-and-Patch/13-installed-calibration.md),
[installed Color](../help/10-ToskLight-Architect/02-Fixtures-and-Patch/13-installed-calibration.md#installed-color-calibration),
[Focus and Zoom](../help/20-ToskLight-Control/20-Programmer-and-Cues/06-focus-and-zoom.md),
[replacement](../help/10-ToskLight-Architect/02-Fixtures-and-Patch/05-replace-a-fixture.md),
and [MAP-01–06](../testing/42-release-acceptance.md).
`PositionCalibration.tsx` and `ColorCalibration.tsx` under
`apps/light-desktop/src/components/setup/fixturePatch/` already use exact root/copy
identity, local drafts, disabled Save on invalid/stale data and scoped patch intents.

Current help distinguishes live Position correction from save-only installed Color observations
and save/editor-preview physical mapping data. Preserve those distinctions. A future live adapter
must prove which resolver consumes each new calibration; merely saving a measured label must not
claim changed light or Stage output. Existing semantic Zoom output behavior does not establish
that every newly edited sampled mapping is consumed live.

Two concerns stay separate:

- **Fixture light production:** reusable profile knowledge, plus observations of a particular
  physical unit and optical path. A unit measurement is not automatically true of its family.
- **Installed Position:** this physical unit's pose and axis references in this show's stage
  coordinates. Rough hanging XYZ is an initial estimate, not a prerequisite for exact aiming.

DMX addresses and transport routes do not define calibration identity. An unpatched fixture
remains programmable and may retain a draft, but cannot claim a physically verified observation.

## Operator flow

### 1. Select the installation and establish the reference frame

Enter from the selected Show Patch physical row using a proposed **Guided calibration…** action
alongside the existing editors. Show fixture number/name, root or copy, profile/revision, mode,
physical geometry and optical head. Selection is explicit and stays fixed through capture.
A shared-axis fixture lists affected heads; do not imply one head's axis can move independently
when the personality couples them. Commission one independently controllable physical instance
at a time; batch review may reuse profile evidence without copying installed measurements.

Choose Position, Color or Optics. Show previous evidence, validity and what actually affects live
output. Record operator, date, unit identifier if known, measurement source/instrument and quality.
Use Unknown/Estimated/Manufacturer/Measured consistently; imported GDTF is not a measurement.

For Position, choose the existing show coordinate reference and units. Display conversion between
stage metres and stored/local millimetres, axes and handedness; use the model's existing transforms,
including local Y-up and neutral −Y, rather than introducing a second convention. Register known
stage points by stable identity, XYZ, uncertainty and survey method. Record a rough hanging XYZ,
rough mounting orientation and their confidence. A known surveyed mount may be fixed; otherwise
allow the fit to refine it. Explain any locked parameter and why it is needed for identifiability.
No silent rescaling or rotation of the stage reference is allowed.

### 2. Prepare a narrow visible beam

This is a fixed, visible step before aiming, not an optional hidden preset. Present **Preview output**
and **Stop preview**, the affected fixture/head and current output ownership. Opening the wizard
sends nothing. Preview requires the existing programming capability and an explicit operator action;
a read-only/Guest session may inspect saved evidence but cannot arm output or commit.

Select the smallest practical **physical opening** available from supported Zoom mapping and its
Beam/Field convention. Raw minimum is not assumed to be narrow: decreasing mappings are valid.
Choose a supported Iris aperture that narrows the visible beam without closing it; Closed is not
a calibration beam. If aperture is only a named/native function, offer the declared usable choices
and an operator-confirmed visible setting. Do not invent degrees or diameter for unknown Iris.
Fixtures without Zoom or Iris show **Not available** and use their actual fixed beam. Unknown Zoom
mapping offers explicit native adjustment labeled uncalibrated, not a fabricated angular minimum.

Keep Zoom and Iris adjustment reachable throughout capture. Focus remains independent travel percent;
the operator may sharpen the spot without an inferred focus distance or automatic Focus change.
Confirm a stationary visible beam, safe shutter/open function and the operator's chosen safe intensity.
Do not send reset, macro, rotation-speed or strobe commands. Never bypass Blackout, route disable,
hazardous-fixture suppression or emergency controls. If output is blocked, say which condition
prevents observation instead of capturing a fictitious measured point.

Preview is a temporary, scoped output overlay. Snapshot the fields it owns and underlying owner
versions, but do not freeze the running show. On stop/cancel/finish it releases its ownership and
reveals the **current** underlying output; it never replays an obsolete raw snapshot over newer Cue,
Programmer or hardware changes. A newer competing edit wins and suspends capture with a local reason.
No preview edit creates Presets, Cues, Normal values, Preload values or Undo entries for programming.

### 3. Capture known stage points

Select a known XYZ point, aim with normal Pan/Tilt controls routed to the commissioning owner,
and confirm the physical spot intersects the mark. Capture simultaneously: point coordinates and
uncertainty, axis requests, exact native coarse/fine bytes and functions, unwrapped angles and
branch/multi-turn references, current physical model identity, mount estimate, preview optics,
controller sample/time and whether achieved output was independently observed. Distinguish requested,
resolved and measured values. A controller request alone is not proof that the lamp arrived.

Allow re-aim, replace sample, exclude with reason and delete. Show a small table of observations and
stage distribution. Recommend at least six spread observations plus a separate verification point
as collection guidance, **not** a promise that six uniquely solve the chosen model. Seek different
azimuths, elevations and useful distances where physically accessible. Warn about coincident points,
collinear rays, narrow angular coverage, nearly symmetric arrangements and uncertain stage surveys.
Planar floor points can be useful but must pass the actual model's conditioning and branch tests;
they are not automatically sufficient or automatically forbidden. Additional points should target
an identified weak direction, not merely increase the count.

## Position fit, solvability and verification

Use the profile's forward kinematics to predict a beam ray for each captured native axis state.
Fit the installed translation XYZ, mounting rotation and applicable zero/reference corrections
jointly, with rough estimates as declared priors. Axis direction is a discrete hypothesis; enumerate
only physically admissible alternatives. Profile travel, axes, pivots and units remain fixed unless
an explicit, separately reviewed profile correction is made. Speed-only axes cannot yield an absolute
position fit. Unknown geometry may permit an estimated effective aim model, never a measured pivot.

Some parameter combinations are mathematically indistinguishable: mounting yaw and Pan zero can share
a gauge, and signed multi-turn branches can produce the same ray. Do not report independently measured
values for both coupled parameters from beam observations alone. Fix a documented reference from an
independent survey/axis mark, or fit an effective transform and expose the unresolved decomposition.
Keep chosen gauge/reference and alternative branches with the result. Do not silently wrap to ±180°.
The operator captures reference turns/limits where required; if observations cannot determine the turn,
retain ambiguity and refuse exact automatic multi-turn acceptance.

For point p and predicted origin o/direction d, compute the perpendicular miss
`||(I − d dᵀ)(p − o)||`, require forward intersection `(p − o)·d > 0`, and report angular error
as well as stage-distance miss. Use measured uncertainty weights and robust fitting to flag possible
outliers; never silently delete them. Show each excluded sample, alternative solution and change made.
Finite coordinates, legal ranges, monotonic mappings and native function applicability are validated
before fitting. A low residual alone is not evidence of an identifiable solution.

Evaluate numerical rank of the weighted Jacobian after gauge constraints, conditioning/singular values,
spatial coverage, discrete sign/branch alternatives and sensitivity to uncertain point coordinates.
Report identifiable parameters, constrained/prior-dependent parameters and remaining ambiguity. Before
implementation, a solver validation fixture set must fix units/scaling, numerical rank tolerance and
conditioning policy; they must not be tuned to make one demonstration pass. Expose prediction uncertainty
at intended stage points using identified-subspace covariance/sensitivity and repeated observations;
do not show spurious precision or confidence intervals in rank-deficient directions. Report RMS, maximum
miss and per-sample residuals, measurement uncertainty and model limitations separately.

**Review fit** explains failures with remedies: survey another elevation, spread points farther,
confirm axis direction/turn, improve target XYZ, lock a surveyed mount reference, or correct a suspect
sample. Preserve the draft on failure. Show old versus proposed pose/offsets, predicted native DMX,
Stage rays and capability clipping. Screen render agreement is independent software evidence only.

Require an additional known point **not used in the fit**. Before aiming, freeze its prediction and
acceptance tolerance derived from the operator's required stage accuracy and instrument/survey limits.
Physically aim via the candidate model, measure the miss and record it. If the error exceeds tolerance,
return to capture/review; do not fit that same point and still call it held-out verification. A separate
new verification point is then needed. Exact/Measured acceptance is disabled for unresolved rank,
branch or verification failures; an explicit estimated save must retain its warning and cannot masquerade
as successful measured calibration. Calibration does not promise accuracy outside verified coverage.

## Color and optics procedures

| Concern | Procedure and required evidence | Ownership / limits |
| --- | --- | --- |
| Additive emitter response | Warm/stabilize the lamp, record ambient/background, instrument calibration and measurement geometry; isolate declared emitters at repeatable native levels, measure spectral/XYZ output and intensity response without clipping. Repeat to estimate variation. | Spectrometer for spectral claims; suitable calibrated colorimeter for supported XYZ claims. Unit gains scale existing XYZ/spectrum, not hue. Changed chromaticity needs an actual measured optical-path representation. |
| Wheel, CMY and compound paths | Capture the complete native recipe, parked wheels, dimmer, shutter and optical path; measure representative combinations, neutral/open reference and level dependence. | Reuse whole-path observations. A sparse measurement set cannot guarantee interpolation, gamut or all wheel combinations; declare unsupported fits and approximation. |
| Reusable profile knowledge | Review source quality, instruments/units, temperature/operating conditions and multiple-unit evidence where family claims are proposed. | Explicit publication as a new immutable profile revision; never promote a selected unit's measurement by default or replace embedded show profiles implicitly. |
| Zoom | Measure full opening at several declared native samples using known screen distance and width or calibrated angular measurement; record Beam versus Field intensity criterion, repeatability and uncertainty. | For suitable geometry use full angle `2 atan(width / (2 distance))`; background/threshold and measuring plane must be recorded. Do not convert Beam↔Field or infer either from a nominal range. Reuse directed monotonic endpoint/sample validation. |
| Focus | Record independent near-to-far travel percent and repeatable test target/contrast observations at stated distance. | No universal metres conversion. Changing Zoom does not author Focus. Any optional coupling table is separate future work. |
| Iris | Capture named/native aperture functions and, only if actually measured, geometric opening/beam effect at a stated optical setup. | Keep independent from Zoom and Focus; distinguish Open, Closed, animation and unsupported physical aperture. Do not assume raw0 or raw255 means open. |

Compare predicted recipes and actual received Art-Net/sACN bytes before/after, then independently measure
light. DMX is not a substitute for the instrument, and a plausible renderer image is not photometry.
Expose source, unit, date, instrument identifier, relevant settings, uncertainty and repeat samples.
Uninstrumented visual matching may be saved as Estimated with an explicit limitation. Color fitting/live
Stage consumption is a proposed implementation milestone, not a claim about the current save-only form.

## Save, ownership, replacement and interruption

Store raw observations and accepted installation results with the portable show, keyed by physical
root/copy identity and exact profile/revision/mode/geometry/optical-path identity. Include model/solver
version, coordinate reference, constrained parameters, applicability and evidence references. Instrument
configuration belongs to the desk; captured measurement provenance belongs to the show. Referenced evidence
must be included as a portable asset or clearly marked unavailable after transfer, never a presumed local
absolute path. Avoid recording session tokens or machine-specific credentials in evidence.

Moving a DMX address or temporarily unpatching does not erase the installation. Moving the physical unit,
changing mount/axis geometry, source profile/mode/path or replacing hardware requires applicability review.
A hardware swap with unchanged profile still invalidates unit-specific measurements. New copies start
without installed measurements. Do not clone a root's measured pose into another physical lamp.

Preview requires no persistence. **Commit calibration** revalidates show/root/copy, author revision,
capabilities, source identity and preview ownership, then saves the accepted pose/mount/calibration/evidence
as one atomic show change with one Undo boundary. If publication, validation or persistence fails, leave
all authored data unchanged, release preview and keep recoverable draft/evidence. Do not partially save a
pose before its corrections. Recheck permissions and concurrent changes; never silently rebase consent.
A stale review says what changed and offers **Review current fixture** rather than discarding measurements.
No commit may rewrite authored Target, Angles, Color, Zoom, Preset/Cue references or their timing to native
values. Runtime resolutions may change through the calibrated installation; the intended programming does not.

Cancel/Escape/title Close stop preview and discard only the unsaved draft after a clear keep/discard choice
when observations exist. Atomic Commit disables duplicate activation and dismissal until a truthful result.
Window loss, disconnect, fixture deletion, show switch or operator interruption release output through the
server-owned commissioning lease; lease expiry must release even if the UI disappears. Recovery offers saved
draft observations for explicit review, never automatically resumes light or silently commits them. A selected
fixture's disappearance is an actionable refusal; other running fixtures/Cues continue. Commit resolves to a
known success/failure before retry; retry cannot create a second calibration revision for one accepted request.

Replacement preserves logical fixture/head correspondence and authored intentions. Retain original observations
inactive with their original source identity; never relabel them as measured for the replacement. Guide fresh
unit/installation calibration and a held-out point on the new lamp. Keep feasible and unsupported results explicit:
a fixture lacking Position or Zoom retains dormant intent, and a limited color gamut is reported rather than
rewriting Presets. Recalibration restores resolution within capability, not identical physical output by assertion.
Undo/reopen and replacement back to an earlier profile require explicit validity review, not automatic trust in old
hardware measurements merely because a UUID matches again.

## Reviewable implementation proposal

This document authorizes no implementation. Sequence later claimed work in independently reviewable chunks:

1. **Model and solver contract:** audit current installed/profile storage, frame transforms and resolver order;
   specify additive observation/result representation and numerical identifiability policy. Reuse current fields
   where semantics match; do not force a joint pose fit into offsets alone. Keep missing legacy fields equivalent
   to today's behavior. Follow [persistence compatibility](../acceptance-criteria.md), including representative
   legacy readers, startup/recovery and new result roundtrip. No schema break is declared here.
2. **Pure solver and measurement validation:** typed units, forward ray prediction, uncertainty/rank/gauge and
   discrete branch search, reproducible diagnostics and synthetic fixtures with independent truth. No device I/O,
   UI state, database or fitting thresholds hidden in rendering code. Bound work and expose cancellation/progress.
3. **Scoped preview/output owner:** selected physical-instance lease, narrow-beam semantic preparation, readback
   capture, competing-edit precedence and interruption cleanup. Resource preparation/fitting must not hold the
   ordered show-output gate; final validation/install is short and atomic. Prove running Cue output stays live.
4. **Guided UI using existing editors:** capture table, narrow-beam controls, stage distribution, review/error and
   held-out verification; same authoritative transactions for touch, keyboard, OSC and attached desk controls.
   Reuse standard modal title actions, accessible labels and touch sizing. Background fitting shows honest phase
   and elapsed progress, no fabricated percentage; cancellation ignores late results by generation.
5. **Persistence and resolver integration:** accepted installation/model/evidence transaction, stale identity rules,
   undo/recovery, explicit profile publication and portable assets. Position and Color adapters report whether data
   is active/live or save-only. Any future HTTP/WS seam follows [API rules](../engineering/api-rules.md): typed scope,
   authorized operator capability, validation before effects and current-authority revalidation at commit. No
   endpoint or schema is introduced by this plan.
6. **Native and measured commissioning:** staged release behind proven lifecycle/compatibility tests; update numbered
   help only when actual behavior ships. Record candidate/build, profiles and instrument/setup with the evidence.

## Acceptance-test plan

| ID | Test / independent oracle | Required result |
| --- | --- | --- |
| GC-01 | Open existing manual forms and new guide on root/copy; old supported show with no new data, new saved result, malformed stored draft, real startup/recovery. | Existing values and no-calibration behavior preserved; actionable recovery retains originals; no duplicate registry. |
| GC-02 | Reversed Zoom endpoint mapping, named Iris Closed/Open/animation, no Zoom/Iris, unknown convention, supported native controls on touch/keyboard/OSC/attached desk. | Fixed preparation step chooses a visible semantic beam, never raw-minimum/Closed; Focus and unselected fixtures untouched. |
| GC-03 | Independent synthetic known pose/axis model with rough XYZ/orientation error, noisy surveyed points, sign alternatives and unwrapped turns. | Fit recovers identifiable truth within predeclared uncertainty; no double zero/inversion; physically equivalent branches handled explicitly. |
| GC-04 | Coincident/collinear/narrow-coverage/ambiguous planar/symmetric points, coupled yaw/zero gauge, contradictory turns, speed-only axis and uncertain pivot. | Rank/conditioning/gauge/unsupported diagnostics block false Measured claims; actionable additional survey/reference guidance. |
| GC-05 | Deliberately bad sample, explicit exclusion, held-out target frozen before measurement, failed then new verification point. | No silent outlier deletion or training-point verification; residual and uncertainty reports reproduce; no precision beyond evidence. |
| GC-06 | Save joint pose/mount/axis result; inject revision/identity conflict and validation/persistence/publication failure. | Atomic all-or-none revision/Undo, retained reviewable draft, no silent rebase; Target/Color/Zoom/Cue bodies remain byte-equivalent. |
| GC-07 | Running unrelated Cue/fade, concurrent operator edit, Cancel/Escape/window loss/disconnect/lease expiry/show switch; blocked Commit then retry. | No running-output stall; selected preview releases to latest underlying output, no stale restore, no late commit or duplicated revision. |
| GC-08 | Two physical copies, unpatch/repatch, address move, profile/mode/geometry/path change, hardware swap under same profile, explicit replacement and Undo/reopen. | Independent data; valid nonidentity edits retain observations; incompatible data retained inactive; intentions preserved and limitations reported. |
| GC-09 | Known instrument data and synthetic color truth for emitter gain versus hue shift, whole-path recipes, missing units/source, repeated/contradictory measurements. | Correct reusable-versus-unit ownership, truthful provenance/quality, no fabricated spectrum or gamut; live/save-only distinction verified. |
| GC-10 | Nonlinear/reversed Zoom samples with Beam/Field criterion, endpoint/resolution conflict, independent Focus and Iris captures. | Directed mapping validation and uncertainty; no Beam/Field conversion or hidden Focus/Iris authoring; portable asset roundtrip. |
| GC-11 | Receive actual coarse/fine Art-Net and sACN bytes against frozen source/kinematic oracle; inspect Stage separately; physically survey an additional point and measure light. | Software output, model visualization and physical measurement each have separate results; no measured PASS from imported data or received packets alone. |
| GC-12 | Save/reopen portable show and copy, reopen with missing evidence asset, interruption during draft and calibration commit; Guest/foreign desk controls. | Scope/capability enforced, no automatic preview on recovery, missing evidence explicit, old authored programming/settings unchanged. |

A physical rig and suitable instruments are required for GC-11's measured portion. If unavailable, report that
portion untested/blocked and keep nominal/synthetic/native-wire results separate. These are proposed acceptance
tests, not executed results. Existing MAP-01–06 campaign captures can seed regression assets without being
relabeled as guided-flow evidence.

## Explicitly deferred: adapting a show to another venue

[TL-674](https://plans.tokenet.de/TL/674) is separate. Calibration establishes where the actual installation is
and how the physical lamps respond in the current venue. Adapting composition to a smaller/different stage
requires an explicit spatial-intent mapping policy, operator review and its own acceptance criteria. This flow
never silently scales target coordinates, stage points, mounting positions or authored intentions.

## TL672 criteria review

| Criterion | Design coverage |
| --- | --- |
| 1 — existing forms / Color versus installed Position | Existing controls and boundaries; Color and optics procedures; explicit save-only/live baseline. |
| 2 — rough estimate, fixed adjustable narrow-beam step, multiple known XYZ captures | Operator flow steps1–3; semantic narrow beam and complete axis/branch capture. |
| 3 — parameters, sufficiency, ambiguity, uncertainty, correction, held-out verification | Position fit section; gauges/rank, point geometry, uncertainty and separate verification policy; GC-03–05. |
| 4 — accepted targeting and replacement preserve intent | Save/ownership/replacement section; GC-06/08/11; no native-value rewrite or physical-capability guarantee. |
| 5 — all physical families, provenance, instruments, copies, portability | Existing controls, capture schema and Color/optics table; GC-08–12. |
| 6 — preview/commit/cancel/restoration/invalid/stale safety | Scoped preview ownership and atomic lifecycle; GC-06–08/12. |
| 7 — implementation proposal/tests, no product changes, separate venue adaptation | Sequenced proposal and GC-01–12 above; explicit TL674 deferral. |
