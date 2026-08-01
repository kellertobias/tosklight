# Separate Demo and Benchmark Shows

## Status

**Completed 2026-07-29.** The finished refactoring record
[`20-three-tier-demo-and-benchmark-shows.md`](../refactoring/finished/20-three-tier-demo-and-benchmark-shows.md)
documents the generated 262-fixture demo, 1,000-instance interactive tier, 2,000- and
4,000-fixture headless tiers, maintained demo recording, focused acceptance, reports, and semantic
commit. The royalty-free Theater source and script-specific Theater Cues remain intentionally
deferred as allowed by this plan.

## Goal

Give the demo and benchmark different jobs:

- the **benchmark show** deliberately stresses the engine and output pipeline with a large,
  fully packed workload;
- the **demo show** presents a credible, attractive venue that is easy to understand, navigate,
  program, and demonstrate.

The demo must not inherit fixture counts, universe packing, active effects, or other artificial
load whose only purpose is to maximize benchmark pressure. The benchmark remains free to stress
the system without dictating the shipped or recorded demo experience.

## Three canonical performance shows

This plan owns three distinct generated show tiers. They share fixture-library and deterministic
generation infrastructure, but their inventories, runtime surfaces, acceptance roles, and reports
must remain explicit.

| Tier | Show and runtime | Performance contract |
|---|---|---|
| Realistic demo | The exact generated demo inventory below: **262 controllable fixtures and 301 physical Stage instances**, including multi-patches. Run in the packaged desktop with both a 3D Stage view and Fixture Sheet/list visible. | Release-blocking CI acceptance using the programmed benchmark look below. It must remain responsive and satisfy the established interactive Stage, control, output-isolation, and end-to-end gates. A visibly stalled or unusably laggy demo is a failed build and blocks release. |
| Interactive large show | Exactly 1,000 fixture instances, with the packaged desktop, 3D Stage, and Fixture Sheet/list open. | Report the achieved engine/output rate and operator-visible Stage metrics. The target is **100 Hz** for the measured engine/output workload while the UI remains responsive; retain the stricter safety and output-isolation gates from the Stage plan. |
| Headless stress show | Begin with 2,000 fixtures and scale deterministically to 4,000 when the same generator and hardware can sustain it. Run with every Stage surface and other visualization UI disabled. | Report the achieved rate, deadline misses, work-time percentiles, and resource use in the build. The target is **60 Hz**. Missing 60 Hz is informative and non-blocking, but missing data or silently opening a Stage/UI surface invalidates the report. |

The realistic benchmark uses the actual demo show; do not add filler fixtures merely to round it
to 200 or 300. The rates above describe the measured engine/output workload. They do not require the Stage
visualization transport or canvas to publish at 100 Hz; those surfaces retain their own latency
and cadence budgets. Build reports must state the fixture count, profile/mode mix, Dynamic count,
active UI surfaces, run duration, hardware identity, achieved rate, latency/work percentiles,
deadline misses, and whether each applicable blocking gate passed.

The complete three-tier sweep runs in the final renderer/benchmark queue phase. Earlier refactor
plans still run focused tests and their required end-to-end acceptance, but do not rerun this full
benchmark suite after every change.

### Dynamic workload

Use approximately 20 production Dynamic instances across the interactive large and headless
stress shows. Prefer representative higher-channel fixtures and substantially occupied universes
over a patch dominated by tiny RGB fixtures; universes do not have to end on channel 512.

- Include at least 40 Showtec Sunstrip LED RGB fixtures in the 30-channel mode, with Dynamic-driven
  intensity and color values.
- Give every applicable intensity, color, pan, and tilt value on beam, wash, and other moving
  fixtures a Dynamic-driven workload.
- Partition driven fixtures across the Dynamic instances without accidental overlap.
- Retain a distinct set of conventional dimmers with fixed intensity and no intensity Dynamic as
  the static control population.
- In the headless tier, all compatible fixture values participate in Dynamics; only genuinely
  fixed or unsupported attributes are exempt.

## Identity and ownership

Create two explicit, independently generated show definitions.

1. Rename benchmark-facing `demo show` terminology to **benchmark show**, **sustained show**, or
   another unambiguous benchmark-only name. This includes the current `--demo-show` option,
   benchmark source/module names, report labels, inventory labels, help text, and focused tests.
   Retain a compatibility alias only if an external caller still relies on the old command-line
   spelling; do not continue presenting that alias as the canonical name.
2. Give the operator demo its own deterministic generator and artifact. It must use shipped
   fixture packages and valid modes rather than borrowing the benchmark's universe-filling
   inventory.
3. Identify and document which current consumer receives the new demo: the built-in/default show,
   the maintained product-demo recording, a reusable generated `.show`, or more than one of those
   surfaces. Reuse one canonical demo generator or portable show artifact rather than maintaining
   divergent fixture inventories.
4. Keep benchmark-only Groups, Cues, Playbacks, phasers, sampled contributions, and full-universe
   filler out of the demo. Later demo programming may add intentional operator examples, but only
   after the patch and Groups in this plan are stable.
5. Preserve existing show-file compatibility and regeneration behavior required by
   `docs/acceptance-criteria.md`. A built-in demo replacement must not silently mutate an existing
   user's active show.

## Demo venue model

Lay out a stage with a **Front Truss**, **Mid Truss**, and **Back Truss**, an audience area in
front of the stage, floor-mounted stage fixtures, and an auxiliary fixture area. Add four vertical
pipes at the Back Truss for the RGB Sunstrips.

The Stage view must clearly communicate the requested lighting roles:

- front and side light aims at the performance area;
- stage movers hang in alternating Profile/Wash lines;
- audience movers and LED PARs point down into the audience;
- the two follow spots originate at the back of the audience and aim at the stage;
- blinders face the audience;
- floor-mounted stage LED PARs point upward;
- ACL sets visibly form their requested fans; and
- auxiliary fixtures form a neat, deliberately composed grid rather than an arbitrary storage
  pile.

Use stable names, fixture numbers, layers, mounts, positions, and rotations. Allocate
non-overlapping DMX addresses across as many universes as the real modes require; do not distort
the rig merely to minimize or fill universe count.

## Front, side, house, and atmospheric fixtures

### Front and side light

Patch and position:

- **4 Fresnels** on the Front Truss, distributed across the width and aimed approximately straight
  down as the primary front wash;
- **2 static Profile lamps** on the Mid Truss, one left and one right, both aimed at the centre
  position;
- **1 static Profile lamp** at the front, aimed directly at the centre position;
- **2 pairs of static Profile lamps**, one pair covering the stage-left position and one pair
  covering the stage-right position; and
- **2 side Fresnels**, one at stage left and one at stage right, cross-lighting the stage.

These static Profile and Fresnel fixtures are front-lighting tools; do not silently include them
in the moving-Profile family Groups defined below.

### House lights

Represent the house lights as one controllable strip-light fixture with multi-patches for a
**4-row by 3-column** physical layout: 12 visible house-light instances in total. All instances
share the intended control values while retaining their own Stage positions.

### Blinders and hazers

Patch:

- **2 four-light blinders** on the Front Truss, facing the audience; and
- **2 hazers**, placed symmetrically so the Stage view and demo programming can address both sides
  of the venue.

Use the fixture package's logical heads for each four-light blinder where available. Do not model
the blinders as unrelated single lamps unless that is the shipped fixture's actual control model.

## Stage fixtures

### Stage moving lights

Patch:

- **16 Profile moving lights** on the Back Truss;
- **12 Profile moving lights** on the Mid Truss;
- **15 Wash moving lights** on the Back Truss, one between each adjacent pair of its 16 Profiles;
- **11 Wash moving lights** on the Mid Truss, one between each adjacent pair of its 12 Profiles.

The resulting stage-family inventory is 28 Profiles and 26 Washes. Truss fixtures point down or
toward useful stage positions.

### Stage LED PARs

Patch **16 regular LED PARs** as four compact floor groups of four. Point them upward and lay out
the four clusters symmetrically across the stage.

### Back-truss RGB Sunstrips

Patch **8 RGB Sunstrips** on four vertical pipes at the Back Truss. Arrange them as two rows by four
columns, with two Sunstrips on each pipe, matching the recognizable composition from the earlier
demo show.

## Audience and follow-spot fixtures

Build the overhead audience rig as an interleaved grid:

- **16 Wash moving lights** in four rows of four;
- **20 Profile moving lights** in five rows of four, with the Profile rows placed between and
  around the Wash rows;
- **100 regular LED PARs** in a 10-by-10 downward-facing grid over the audience.

Add **2 Profile moving lights** at the back of the audience for use as follow spots. Name and aim
them so their follow-spot role is obvious. Because the requested family scheme is based on Stage,
Audience, and Auxiliary placement, include these two fixtures in **Profile Audience** and also in
the dedicated overlapping **Follow Spots** role Group.

## ACL sets

Create **4 independently controlled ACL sets**, each consisting of one primary fixture and seven
multi-patch instances for eight physical ACL lamps in total. Multi-patch is required: do not patch
eight separately controlled fixture objects per set.

1. **Back Centre ACL** — mounted in the middle of the Back Truss and aimed as a centred fan out.
2. **Back Split ACL** — mounted as four lamps on the left and four on the right of the Back Truss,
   aimed inward as a fan in.
3. **Mid Split ACL** — mounted on the Mid Truss as four lamps centred within the left section and
   four centred within the right section, aimed as a fan out.
4. **Front Split ACL** — mounted as four lamps on the left and four on the right of the Front
   Truss, aimed inward as a fan in.

Give every physical instance an individual Stage position and aim while preserving one shared
control fixture per set.

## Auxiliary fixtures

Patch an auxiliary inventory of:

- **4 Profile moving lights**;
- **4 Wash moving lights**;
- **16 regular LED PARs**.

Name this location/role **Auxiliary** in operator-facing descriptions and use the compact suffix
**Aux** in Group names. Arrange the fixtures in a balanced grid that clearly separates the three
fixture blocks and looks intentional in the Stage view.

## Canonical moving-light and LED inventory

After removing every Beam fixture, the requested family totals are:

| Family | Stage | Audience | Aux | All |
|---|---:|---:|---:|---:|
| Profile moving lights | 28 | 22, including 2 follow spots | 4 | 54 |
| Wash moving lights | 26 | 16 | 4 | 46 |
| Regular LED PARs | 16 | 100 | 16 | 132 |

These totals exclude static Profile front lights, Fresnels, RGB Sunstrips, ACLs, blinders, house
lights, and hazers. Fixture numbering must make family and placement membership easy to inspect,
but Groups—not number-range accidents—remain the authoritative membership definition.

## Exact demo fixture count

The generated demo contains **262 independently controllable fixture objects**:

| Inventory | Controllable fixtures |
|---|---:|
| Profile moving lights | 54 |
| Wash moving lights | 46 |
| Regular LED PARs | 132 |
| Static Profile lamps | 7 |
| Fresnels, including the two side Fresnels | 6 |
| RGB Sunstrips | 8 |
| ACL control fixtures | 4 |
| Four-light blinders | 2 |
| Hazers | 2 |
| House-light control fixture | 1 |
| **Total** | **262** |

The four ACL controls each render as eight physical lamps, adding 28 Stage instances beyond their
four primaries. The house-light control renders as 12 physical instances, adding 11 beyond its
primary. Therefore the Stage and realistic benchmark contain exactly **301 physical fixture
instances**. Logical heads inside the two four-light blinders remain heads of those two fixtures;
they do not increase the fixture-instance count.

## Initial Groups

After the complete patch and Stage layout exist, create the following exact first-level Groups:

| Family | All | Stage | Audience | Auxiliary |
|---|---|---|---|---|
| Profile | `Profile All` | `Profile Stage` | `Profile Audience` | `Profile Aux` |
| Wash | `Wash All` | `Wash Stage` | `Wash Audience` | `Wash Aux` |
| LED | `LED All` | `LED Stage` | `LED Audience` | `LED Aux` |

Requirements:

- `All` is the ordered union of that family's Stage, Audience, and Aux membership.
- Stage, Audience, and Aux Groups preserve a stable spatial order suitable for later value
  spreading.
- No fixture is duplicated within a Group.
- The four location Groups for a family reconcile exactly with its documented inventory.
- Static front-light Profiles do not enter `Profile All`; the family is specifically Profile
  moving lights.
- RGB Sunstrips do not enter `LED All`; `LED` in this group set means regular LED PARs.
- Group creation happens only after patch creation so the stored IDs resolve to the final,
  authoritative fixtures.

Beam moving lights are intentionally absent from the revised demo. References below to the
**Beam** preset pool mean optical Beam attributes such as Gobo and Prism, not a Beam fixture
family.

## Show and Aux Show Groups

Build the performance-facing Groups by recalling the first-level Groups and recording the result
as another Group. Do not rebuild the memberships from fixture-number ranges.

For each of Profile, Wash, and LED:

1. Recall its `Stage` Group and add its `Audience` Group.
2. Record that ordered union as `Show Profile`, `Show Wash`, or `Show LED`.
3. Recall its `Aux` Group.
4. Record that membership as `Aux Show Profile`, `Aux Show Wash`, or `Aux Show LED`.

Then create:

- `Show` from the ordered union of `Show Profile`, `Show Wash`, and `Show LED`; and
- `Aux Show` from the ordered union of `Aux Show Profile`, `Aux Show Wash`, and `Aux Show LED`.

The `Show` Groups deliberately exclude Auxiliary fixtures. `Aux Show` contains only Auxiliary
fixtures. The family-specific `All` Groups remain available when an operation genuinely needs
both inventories.

Create odd and even derivatives for each family-specific **Show** Group:

- `Show Profile Odd` and `Show Profile Even`;
- `Show Wash Odd` and `Show Wash Even`; and
- `Show LED Odd` and `Show LED Even`.

Odd/even membership follows the stored spatial order of its source Show Group. Do not create
odd/even Aux Show Groups.

## Role and utility Groups

Create these additional Groups:

- one Group for each eight-lamp ACL control set: `ACL 1`, `ACL 2`, `ACL 3`, and `ACL 4`;
- `All ACLs`, recorded by selecting the four ACL set Groups rather than reconstructing their
  fixture membership;
- `Blinders`, containing both four-light blinders;
- `Front Lights`, containing all static front and side Fresnels and static Profile lamps, but not
  the audience-facing blinders;
- `Front Center`, containing the two centre-aimed Mid Truss Profiles and the directly front-mounted
  centre Profile;
- `Follow Spots`, containing the two rear-audience Profile moving lights;
- `Sunstrips`, containing all eight RGB Sunstrips;
- `House Lights`, containing the house-light control fixture whose multi-patches produce the
  4-by-3 physical grid; and
- `Hazers`, containing both hazers.

Groups address controllable fixtures or logical heads according to the fixture definition.
Multi-patch instances remain physical representations of their primary control fixture and are
not duplicated as independently controllable Group members.

## Group-master Playbacks

Assign Group objects to Playback faders so they operate as Group Masters. Reserve stable Playback
slots and document the final page/slot mapping.

Create Group Masters for:

- `Show Profile Odd`;
- `Show Profile Even`;
- `Show LED`;
- `Show Wash`;
- `All ACLs`; and
- `Blinders`.

Do not assign the four individual ACL Groups as Group Masters; only `All ACLs` receives that
master. Do not create Beam Group Masters, because the revised demo has no Beam fixtures.

The implementation must use the desk's real Group-to-Playback assignment behavior. A Cuelist that
contains a Group selection is not a substitute for a Group Master.

## Initial Cuelists and Playback assignments

All fixture programming in this section starts by recalling Groups. Do not select fixtures by
ad-hoc number ranges merely because the generated numbering makes that convenient.

### Start

Create a one-cue Cuelist named `Start` and assign it to a Playback. Its first Cue recalls
`Profile All`, `Wash All`, and `LED All`, then stores:

- all supported intensity values on;
- `Blinders` at full;
- an intentional initial white Color preset; and
- an intentional initial Position preset for every selected fixture that has Pan/Tilt.

The Start Cue is a useful, composed initial look—not a request to activate ACLs, house lights,
hazers, or every patched control channel indiscriminately.

### Individual ACL Playbacks

Create four one-Cue Cuelists, one per ACL set. Each Cuelist turns on exactly one of `ACL 1` through
`ACL 4` by recalling that Group, and each Cuelist receives its own Playback assignment. The four
individual ACL Playbacks provide look selection while the separate `All ACLs` Group Master scales
the family as a whole.

### Hazer Playback

Create a one-Cue Hazer Cuelist by recalling `Hazers` and setting the hazer output to **20%**. Assign
it to a Playback whose fader directly controls the running Cuelist level. Verify intermediate
fader positions scale the Hazer Cue instead of behaving like a button-only trigger.

## Preset programming

Program every preset through Groups and merge the result across all compatible fixtures. A fixture
that does not expose the relevant attribute is skipped without preventing compatible fixtures
from contributing.

### Color presets

Create these exact Color presets:

1. `Red`
2. `Orange`
3. `Yellow`
4. `Lime`
5. `Green`
6. `Teal`
7. `Cyan`
8. `Light Blue`
9. `Dark Blue`
10. `Purple`
11. `Magenta`
12. `White`
13. `Tungsten White`

`Tungsten White` is the warm/CTO white look. Its implementation must account for the actual Color
systems available across the selected fixtures rather than assuming every fixture exposes a
literal CTO channel.

### Beam presets

For compatible Profile fixtures, program:

- Gobo: `Open`, `Dot`, `Circle`, `Line`, and `Jungle`;
- Gobo rotation: `Gobo Rotation` and `No Gobo Rotation`;
- Prism: `Prism` and `No Prism`; and
- Prism rotation: `Prism Rotation`.

Keep Gobo selection, Gobo rotation, Prism insertion, and Prism rotation as distinct attribute
operations even if the fixture mode combines some of them onto shared DMX channels.

### Position presets

For every fixture with Pan/Tilt, program these exact Position presets:

- `Down`
- `Up`
- `Center`
- `Fan Out`
- `Blind`
- `Cross 1`
- `Cross 2`

The preset data must produce meaningful venue-relative compositions for both Stage and Audience
fixtures. `Blind` must aim the selected fixtures at a documented safe/non-performance direction;
it must not be an ambiguous intensity blackout.

## Demo Dynamics library

Create the demo's Dynamics only after the patch, Stage positions, Groups, and presets above are
stable. Every Dynamic is programmed by recalling its named Group; generated fixture-number ranges
are not a substitute.

### Family intensity Dynamics

For each of these six Groups:

- `Show Profile`;
- `Aux Show Profile`;
- `Show Wash`;
- `Aux Show Wash`;
- `Show LED`; and
- `Aux Show LED`;

create three independently assignable Intensity Dynamics:

1. **PWM** — a phase-spread pulse/chaser across the stored Group order;
2. **Random** — deterministic per-target Random intensity pulses; and
3. **Sinus** — continuous Sinus intensity modulation.

This produces 18 family intensity Dynamics. Assign the pure intensity Dynamics to:

- **Speed Group A** for `Show Profile` and `Aux Show Profile`;
- **Speed Group B** for `Show Wash` and `Aux Show Wash`; and
- **Speed Group C** for `Show LED` and `Aux Show LED`.

### Moving-light Dynamics

For each of `Show Profile`, `Aux Show Profile`, `Show Wash`, and `Aux Show Wash`, create:

- a **Circle** Dynamic with Pan on Sinus and Tilt on Cosinus; and
- a **Waterfall** Dynamic using absolute keyframes for the useful top and bottom Position values,
  with an Intensity keyframe envelope that rises as the fixture travels down and returns to zero
  after reaching the bottom.

Assign Circle and Waterfall Dynamics to **Speed Group E**. A Dynamic has one authoritative speed
source for all of its lanes, so the Waterfall's Intensity lane follows Speed Group E as part of
that movement composition; the separate pure Intensity Dynamics remain on family Speed Groups
A or B.

Create one additional **Wash Row Waterfall** over `Wash All`. Use 2D **Grid linear** Phase Spread
from the Stage positions, oriented top-to-bottom at 90°, so spatial rows descend coherently rather
than following fixture-number order. Use absolute Position and Intensity keyframes with the same
downward-rise-and-release composition and assign it to Speed Group E.

### Sunstrip and LED Dynamics

Create:

- **Sunstrip Random Color** on `Sunstrips`, using one correlated local Random group for all
  applicable color-component lanes so each emitted color is coherent;
- **Sunstrip Rain** on `Sunstrips`, using keyframes and top-to-bottom Grid-linear Phase Spread.
  A drop starts dark, becomes blue, reaches white at the bottom, and then fades to zero intensity;
  its color and intensity lanes remain synchronized;
- **Show LED Random Strobe** on `Show LED`, applying Random pulses to the Strobe attribute on every
  compatible LED PAR.

Assign the Sunstrip and LED Dynamics to **Speed Group C**.

The deterministic generated library therefore contains **30 Dynamics**: 18 family intensity
Dynamics, eight Group-specific Circle/Waterfall Dynamics, one Wash Row Waterfall, two Sunstrip
Dynamics, and one LED strobe Dynamic. Assign every generated Dynamic to a stable, documented
Virtual Playback so the Busking desktop can start, stop, scale, and combine them without rebuilding
the programmer selection. The canonical benchmark uses those normal Playback assignments.

## ACL chaser

Create one four-Cue Cuelist named `ACL Chase`:

1. only `ACL 1` on;
2. only `ACL 2` on;
3. only `ACL 3` on; and
4. only `ACL 4` on.

Store explicit off values or tracked state as required so each Cue leaves exactly one ACL Group
on. Configure the Cuelist in **Chaser** mode with wrap enabled, assign it to **Speed Group D**, and
place it on a stable Playback. This replaces the earlier idea of an ACL PWM Dynamic; the four
individual ACL Playbacks remain available for manual operation.

## Canonical realistic benchmark look

The release-blocking realistic benchmark uses this exact generated demo and starts these
assignments together:

- `ACL Chase`;
- `Show Wash Waterfall`, providing the active Show-wash movement and intensity composition;
- `Show Profile Circle`;
- `Show Profile PWM`;
- `Show LED Random`;
- `Show LED Random Strobe`;
- `Sunstrip Rain`;
- `Aux Show Profile Circle` and `Aux Show Profile PWM`;
- `Aux Show Wash Waterfall` and `Aux Show Wash Random`; and
- `Aux Show LED Sinus`.

Start Speed-Group-linked assignments with their synchronized activation policy and record the
Speed Groups' BPM, multipliers, and phase origins in the benchmark manifest. The benchmark must
prove that all listed Cuelist and Dynamic instances are active before timed measurement begins.
It runs in the packaged desktop with the 3D Stage and Fixture Sheet/list visible; it does not
substitute the artificial 1,000-fixture or headless stress inventory for this demo workload.

## Desktops

Create and persist three named desktops with literal pane content and useful initial sizing.

### Busking

The `Busking` desktop has:

- Groups, Color presets, Position presets, and Beam presets on one side; and
- a Virtual Playback Grid on the other side.

The visible pools must expose the Groups and presets created by this plan, and the Virtual Playback
Grid must expose the Start, ACL, Hazer, and Group-Master assignments needed for live operation.

### Programming

The `Programming` desktop has:

- Fixture Sheet on one side;
- Stage on the other side; and
- DMX Output directly below the Stage.

Fixture selection and programmer changes must remain visible in both Fixture Sheet and Stage while
DMX Output shows the resulting channel data.

### Theater

The `Theater` desktop has:

- a Cuelist pane on one side; and
- a Text Editor on the other side.

The royalty-free Theater script will be supplied separately. Once supplied, store or reference it
through the show's portable text-document behavior, display it in the Text Editor, and program
several interesting Cues whose names and looks follow the script. Do not invent final script text
or claim the Theater Cues are complete before that source is provided.

## Implementation sequence

1. Inventory every current use of demo/default/benchmark show terminology and identify the
   canonical consumers of each generated show.
2. Establish the benchmark-only name and preserve the existing sustained workload and performance
   acceptance behavior.
3. Implement the approved Front Split ACL as a four-left/four-right Front Truss fan in.
4. Select real shipped fixture packages and modes for every role. Add or correct a fixture profile
   only through the fixture-package contract if a required representative fixture is missing.
5. Build the deterministic demo patch with stable fixture numbers, layers, mounts, non-overlapping
   addresses, multi-patches, and logical heads.
6. Build the complete 3D Stage layout and visually review aims, interleaving, symmetry, spacing,
   and audience/stage separation.
7. Create and verify the 12 first-level Groups, Show/Aux Show hierarchy, odd/even Show derivatives,
    and role/utility Groups.
8. Assign the Group Masters and create the Start, four ACL, and Hazer Cuelists and Playbacks.
9. Program the Color, Beam, and Position preset libraries through the Group hierarchy.
10. Create the 30 demo Dynamics, ACL Chase, their Playback assignments, and Speed Group A–E
    mappings.
11. Create and verify the Busking, Programming, and Theater desktops.
12. After the royalty-free script is supplied, add it to the Theater desktop and program the
    script-specific Theater Cues.
13. Connect the canonical generated demo to the agreed built-in/default and product-demo consumers
    without duplicating the inventory.
14. Update focused documentation and executable coverage for the renamed benchmark interface and
    the new demo-show contract.
15. Run the canonical realistic benchmark look as a release-blocking packaged UI acceptance, then
    run and report the
    1,000-fixture interactive target and 2,000–4,000-fixture headless stress target.

## Acceptance checks

Implementation is complete only when:

1. Benchmark commands, reports, source names, and help no longer call the 32-universe stress
   workload the canonical demo show.
2. The sustained benchmark retains its fully packed workload, timed contributions, performance
   measurements, and hard-floor behavior after the rename.
3. The operator demo can be generated independently without invoking benchmark code or adding
   universe-filling fixtures.
4. Every fixture quantity and placement in this plan is asserted, including 12 house-light
   instances and four eight-lamp ACL multi-patches.
5. Patch validation proves real modes fit in non-overlapping DMX addresses.
6. Every requested fixture is visible in the Stage data with a stable layer, position, and
   orientation; visual review confirms the rig reads as the described venue.
7. The alternating Stage Profile/Wash lines contain 16/15 fixtures on the Back Truss and 12/11 on
   the Mid Truss.
8. The audience contains the 10-by-10 LED grid, interleaved five Profile and four Wash rows, two
   rear follow spots, and no Beam fixtures.
9. The auxiliary area contains exactly 4 Profiles, 4 Washes, and 16 LED PARs in an intentional
   grid, with no Beam fixtures.
10. The four ACL control fixtures each expose seven multi-patches. They render as the requested
    Back Centre fan out, Back Split fan in, Mid Split fan out, and Front Split fan in, with the
    Front Split lamps arranged four left and four right on the Front Truss.
11. All 12 named first-level Groups exist, preserve their intended order, and reconcile with the
    canonical family totals.
12. `Show` is composed from the three family Show Groups, `Aux Show` is composed from the three
    family Aux Show Groups, and neither leaks membership into the other.
13. Odd/even Show Groups are derived from stored Show Group order, cover their source without
    duplication, and no Aux Show odd/even Groups exist.
14. The individual ACL, `All ACLs`, Blinders, Front Lights, Front Center, Follow Spots, Sunstrips,
    House Lights, and Hazers Groups have their literal requested memberships.
15. Group Master assignments exist for Show Profile Odd/Even, Show LED, Show Wash, All ACLs, and
    Blinders. The four individual ACL Groups remain Cuelist targets rather than separate Group
    Masters.
16. `Start` uses Groups to store all Profile, Wash, and LED fixtures on, Blinders at full, and an
    intentional initial white and Position in the same Cue.
17. Four individual ACL Cuelists and Playbacks each operate exactly one ACL Group while the All
    ACLs Group Master scales all four.
18. The Hazer Playback recalls the Hazers Group at 20% and responds proportionally to its fader.
19. All 13 named Color presets, the requested Gobo/rotation/Prism Beam presets, and all seven named
    Position presets exist and produce valid values only on compatible fixtures.
20. Busking, Programming, and Theater desktops persist with their literal pane composition, names,
    and useful geometry.
21. The Theater script and script-specific Cues remain an explicit pending dependency until the
    user supplies the royalty-free source; after it is supplied, the text and programmed looks are
    verified together on the Theater desktop.
22. Existing user shows are neither overwritten nor migrated merely because the generated demo
    definition changes.
23. The maintained product-demo path, if connected to this generator, still performs visible
    state-changing work through normal operator interactions after its labelled generation
    boundary.
24. The exact 262-control-fixture, 301-physical-instance demo passes its packaged desktop
    end-to-end and responsiveness gates with 3D Stage and Fixture Sheet/list visible; failure
    blocks release.
25. The 1,000-fixture packaged interactive report records the 100 Hz engine/output target and
    operator-visible Stage metrics with both required UI surfaces open.
26. The headless report records either 2,000 or 4,000 fixtures, proves visualization UI is
    disabled, records the 60 Hz target and required metrics, and remains informational rather
    than release-blocking when only that target is missed.
27. The large and headless workloads use approximately 20 Dynamics, include at least 40
    30-channel RGB Sunstrips, dynamically drive every applicable intensity/color/pan/tilt value,
    and retain fixed dimmers as a no-intensity-Dynamic control population.
28. The demo generator creates all 30 named Dynamics through the six Show/Aux Show family Groups,
    the four moving-family Groups, `Wash All`, `Sunstrips`, and `Show LED`, with the literal lane
    functions, keyframes, Random correlation, and Grid-linear Phase Spread defined above.
    Every Dynamic has a stable Virtual Playback assignment visible from the Busking desktop.
29. Speed Groups A, B, and C own the pure Profile, Wash, and LED/Sunstrip effects respectively;
    Speed Group D owns `ACL Chase`; and Speed Group E owns Circle and Waterfall movement
    compositions.
30. `ACL Chase` is a wrapping four-step Chaser whose Cues leave exactly ACL 1, 2, 3, then 4 on.
31. The realistic benchmark starts every literal assignment in the canonical benchmark look and
    proves them active before timing begins.

## Open decisions

The remaining input is which royalty-free Theater script should be stored, and which scenes or
beats should become the first programmed Theater Cues.

The fourth ACL decision is approved: **Front Split ACL**, with four lamps on the left and four on
the right of the Front Truss, fanning inward. The patch, Groups, presets, Playbacks, and desktops
may be completed before the script arrives, but Theater Cue programming remains incomplete until
the remaining input is provided.
