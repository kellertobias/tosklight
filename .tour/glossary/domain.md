## Show

The artistic and technical content of a production: patched fixtures, groups, presets, cues,
cuelists, playback assignments, stage positions, layouts.

A Show is **portable** — carry it to another desk, machine, or venue and it must open unchanged.
Stored as a lossless `.show` document: objects and fields this version does not understand are
preserved verbatim through load, edit, revision, Save As, and export.

Lives in `crates/show/src/portable/`. Contrast with the [Desk](glossary:desk).

## Desk

One ToskLight application instance plus the OSC hardware attached to it. A desk owns the active
show, installation settings (output routes, network, screens, users), and one shared
[command line](glossary:command-line).

Identified by a **desk alias**. Two desks with different aliases are isolated even on the same
machine and show — separate command lines, selections, and page state. A desk plus its attached
hardware controller is *one* desk, not two.

Desk data lives in `<data-dir>/desk.sqlite` and never travels with the show.

## Session

One connected client's authenticated attachment to a desk. Sessions come and go across reconnects;
desk state survives them.

A session carries a **primary or secondary screen role**. Only the primary owns session creation
and destruction, so closing a second monitor cannot tear down the desk.

## User

A named operator, passwordless by default. The desk boundary — not the user — is the security
boundary; `LIGHT_DESK_TOKEN` protects a desk exposed on a network.

[Programmer](glossary:programmer) state is **per user**. Two users on one desk have
independent selections and values; one user on two desks shares them.

## Fixture

A controllable lighting device: a moving head, a par can, a hazer, an LED bar. Fixtures have a
**fixture number** the operator types on the command line and a stable internal ID used by code.

## Profile

A description of a make and model of device — what it can do. Profiles arrive in fixture packages
(`.toskfixture` archives) in the desk-wide **Fixture Library**, which also carries photographs,
icons, and GLB models.

A profile offers one or more [modes](glossary:mode).

## Mode

A specific DMX channel layout a physical device is set to. The same lamp in 8-channel and
32-channel mode is one profile with two modes. Each slot within a mode maps to a semantic
[attribute](glossary:attribute); fine channels give 16-bit or finer values across multiple slots.

## Logical head

One physically separate emitter within a fixture that is addressed independently — a cell of an LED
bar, a pixel of a matrix, one head of a two-head fixture.

Logical heads are selectable and programmable in their own right and keep **stable identity** across
recompilation and migration. Selection paths resolve to logical heads, not to a parent fixture.

## Patch

Assigning a fixture to a DMX universe and start address, plus its mode, stage transform, highlight
overrides, and future external-device binding.

**Split patching** places parts of one fixture at different addresses. **Multipatch** drives several
physical devices from one show fixture.

Patching is a **batch** operation: one command for N fixtures produces one transaction, backup,
persistence [revision](glossary:revision), compile, runtime swap, and event — never a per-fixture
loop.

## Profile snapshot

Patching does not reference the desk library at runtime. On first use it copies the immutable
profile revision into a **show-level snapshot**, keyed by stable revision identity and verified by
content digest. Each patched fixture stores only *snapshot reference + selected mode ID*.

Consequences: fixtures sharing a revision share one snapshot; a later library revision never
silently changes an existing show; and a show is portable without the library.

## Unpatched fixture

A fixture with no output binding. A supported state, not an error.

It remains part of the show — selectable, programmable, groupable, recordable, visible in fixture
and stage views. **Only DMX output is suppressed.** Code that skips, hides, or drops unpatched
fixtures is wrong.

## Programmer

The operator's scratchpad. Everything currently being set lives here, above any
[playback](glossary:playback), until recorded into the show or cleared.

Owns the ordered [selection](glossary:selection), semantic values, timing, modes, and undo/redo
history. Per user, not per desk.

## Selection

The **ordered** set of fixtures and logical heads the next value change applies to.

Order drives value spreading (fan, gradients) and reflects operator intent.
Selection can be built from the command line, the Stage, the Fixture Sheet, a group, or OSC; all
paths converge on one typed selection action. A **live** group selection tracks membership; a
**frozen** one captures it.

## Attribute

A semantic parameter — Intensity, Pan, Tilt, Red, Zoom, Gobo. The Programmer works in semantic
attributes, never raw DMX. Translation to channels happens later, in
[fixture projection](glossary:fixture-projection).

## LTP

Latest Takes Precedence. The most recent assignment wins. Programmer values use LTP.

[Playback](glossary:playback) arbitration uses different rules including [HTP](glossary:htp).
These are distinct and must stay distinct. Generalising playback HTP into the Programmer is a
recurring bug class.

## HTP

Highest Takes Precedence — typically for intensity during playback arbitration, where the highest
contributing value wins rather than the most recent. Distinct from [LTP](glossary:ltp).

## Command line

The typed operator interface: `GROUP 3 AT 50 ENTER`.

One desk has **one shared command line**, visible identically on the app and attached hardware. UI
keypad, physical keys, OSC keys, and the HTTP command API all feed the same parser and state.

Partial unexecuted text is desk-interaction state — shared across the desk's surfaces, but not show
data and never persisted as programming. `docs/help/30-Programmer/01-command-line.md` is the binding
contract.

## Preload

Values staged to apply on the next GO rather than immediately. Preload has its own **capture mode**
and its own pending-values authority, separate from normal Programmer values.

A normal write must not silently cross into an active Preload capture; the code enforces this with
an atomic [revision](glossary:revision) precondition.

## Highlight

A transient overlay lighting the currently selected fixture so the operator can see what they are
working on.

**Highlight is never recorded** — not into Programmer values, not into cues. Highlight leaking into
stored data is a bug.

## Group

A named, ordered set of fixtures and logical heads. A **show** object, not Programmer state.

Two distinctions matter: an intentionally stored **empty group** differs from an
**absent or deleted** group and both must stay representable; and a missing ID inside a range is
**skipped**, not materialised as a stored empty group.

## Preset

A stored set of semantic attribute values, recallable onto any selection. A show object. Not a UI
theme.

## Record

Creating or overwriting a stored object — group, preset, cue — from Programmer content. Passes
through the active-show boundary; the render engine never writes persistence.

## Update

A specific operator action writing current Programmer values back into objects that are already
live. Not "software update".

## Cue

One stored look: semantic attribute values plus timing (fade, delay) and trigger information,
recorded from the [Programmer](glossary:programmer).

## Cuelist

An ordered sequence of [cues](glossary:cue) — a song, a scene, an act. Cues are numbered and can be
renumbered atomically.

**Tracking** means a cue may store only what *changes* from the previous cue, inheriting the rest.
Tracking affects what Record and [Update](glossary:update) write.

## Playback

A fader or button strip that owns a [cuelist](glossary:cuelist) and runs it.

A Playback is a **control-surface slot**, not "media playback". A **physical** playback maps to hardware; a **virtual** one exists only in software.
Playbacks live on [pages](glossary:page).

**Exclusion zones** mean activating one playback can automatically release peers — one atomic engine
transition, with released peers reported as related projections alongside the primary change.

## Page

The current bank of [playback](glossary:playback) assignments, so one physical strip drives
different cuelists depending on the active page.

Two addressing modes exist and must be tested separately: **current-page** ("playback 3", resolved
against the active page) and **explicit-page** ("page 2, playback 3", absolute). Confusing them breaks OSC and hardware addressing.

## Masters

Multiplying levels applied above individual values: the **Grand Master** (global intensity ceiling,
forced to zero by blackout), **group masters**, and **speed groups** (scaling the rate of timed
behaviour).

Group-master movement is high-rate, so the engine updates it with a topology-invariant swap that
shares every unrelated compiled component.

## Chaser

A [playback](glossary:playback) that steps through cues automatically at a rate.

Along with **FOLLOW** (advance when the previous cue completes) and **TIME/timecode**, chasers are
externally observable transitions: they publish the same
[domain event](glossary:domain-event) a manual GO does, and a running chaser must update a
subscribed cue view **without polling**.

## MIB

Move in Black. Moving a fixture's position, colour, or beam to its next cue values **while intensity
is at zero**, so the audience never sees the move. Affects timing and arbitration; not cosmetic.

## Universe

512 DMX channels, each a byte. Finer resolution comes from pairing coarse and fine channels. A
"fully packed universe" — all 512 slots carrying live data — is the benchmark condition.

## DMX

DMX512, the lighting control protocol. Carried over Ethernet by [Art-Net](glossary:art-net) and
[sACN](glossary:sacn).

## Art-Net

A network transport carrying DMX universes over Ethernet. A **route** maps a universe to a
transport, destination, and priority.

## sACN

ANSI E1.31 streaming ACN — the other supported network transport for DMX universes.

## Frame

One complete snapshot of a [universe](glossary:universe)'s 512 values at one instant. The output
loop produces frames on a fixed tick and hands them to delivery adapters.

## Blackout

Global kill of output intensity. Must be immediate and unambiguous. Converges through the typed
output runtime service with one batched persistence and event publication per control action.

## Output health

Whether the system is actually meeting its configured rate. If it cannot, it must surface
**actionable overload diagnostics** rather than silently emitting stale or irregular frames.
Emitting stale frames silently is a bug, not graceful degradation.

## Attribute group

A presentation and control family for semantic attributes, such as Position or Color. It decides
which controls belong together; it is not a DMX channel range.

## Activation group

A fixture-profile rule describing functions that activate or exclude one another. It is distinct
from an [attribute group](glossary:attribute-group).

## Split

A patch assignment that places separately addressable parts of one fixture at different DMX
locations while retaining one show fixture and stable logical-head identities.

## Multipatch

Multiple physical output bindings driven from one semantic show fixture. Programmer and Cues store
one value; fixture projection fans it out to every bound instance.

## Virtual intensity

A synthesized Intensity attribute for a fixture whose mode has emitters but no physical dimmer
channel. The engine scales component output while preserving their ratios.

## Profile revision

An immutable Fixture Library version with stable identity and content digest. Patching copies it
into the show as a [profile snapshot](glossary:profile-snapshot).

## Ordered selection

The [selection](glossary:selection) with stable fixture/logical-head order preserved. Range,
add/subtract/toggle, and overlapping Group operations must never silently sort it.

## Stored empty group

A real Group object whose ordered membership is empty. It differs from an
[absent group](glossary:absent-group).

## Absent group

A Group ID with no stored object. Direct addressing is an error; inside a range it follows
[missing range ID](glossary:missing-range-id) rules.

## Missing range ID

An ID between range endpoints for which no object exists. It is skipped and is not materialized as
a stored empty object.

## Live Group reference

A stable Group identity retained in selection or stored programming. Recall resolves its current
ordered membership.

## DEGRP

The dereference/freeze command. It replaces a live Group expression with its current ordered
fixture/head identities so later membership edits no longer affect it.

## Preset scope and filter

The attribute families and fixtures/heads a Preset owns or applies. They prevent recall or Update
from writing unrelated Programmer attributes.

## Embedded value

A semantic fixture/head/attribute value stored directly in a Preset or Cue. It does not change
when its original Group or Preset later changes.

## Live Preset reference

A stable Preset identity retained by stored programming. Recall resolves the current Preset
content rather than a copied value.

## Undo

A desk-scoped Programmer action reversing Programmer mutations and Record/Update steps. Navigation
does not enter history, and there is no generic show-wide undo.

## Cue tracking

The rule that a Cue may store only changes while inheriting earlier state. Compilation gives direct
GOTO and sequential GO the same reconstructed stage.

## Block assert release and off

Explicit Cue/value semantics controlling tracking and ownership. Block stops inheritance, assert
reclaims a value, release yields ownership, and off removes the active contribution by Playback
rules.

## GOTO reconstruction

Installing the tracked target-Cue state without requiring intervening Cues to have run.

## Automatic transition

A runtime change triggered by FOLLOW, TIME, Chaser, or another scheduler. It publishes the same
semantic Playback event as a manual transition.

## Group Master

An assigned Playback limiting intensity for its Group. Overlapping active Group Masters combine by
HTP before the Grand Master applies once.

## Playback Master

A Playback control governing its own Cuelist runtime. It is not a parent of Group Masters and does
not modify stored Cue values.

## Speed Group

A named runtime timing control. It scales rate and duration, not semantic intensity values.

## Grand Master

The single global intensity ceiling above resolved Group/Playback levels. Blackout forces its
effective result to zero.

## Capture mode

The Programmer/Preload policy selecting which value domains an action captures. Preload capture is
separate authority.

## Per-user versus per-desk state

Programmer values and selection are per user; unfinished command-line, gesture, and page context
is shared by the desk.

## Shared command line

The one authoritative command line shared by the Tauri app and attached OSC hardware for a desk.
Different desk aliases remain isolated.

## Semantic attribute value

A typed Intensity, Pan, Tilt, Color, wheel-slot, or other value addressed by fixture/head and
attribute. It is independent of the fixture's DMX mode until projection.

## Transition

A time-bounded change with delay, fade, MIB, direction, and ownership metadata between source
values and resolved output.

## DMX frame

The complete byte array for one universe at one scheduler tick, produced after arbitration and
fixture projection.

## Output route

A portable show definition binding a rendered universe to an Art-Net, sACN, or other destination
with its enablement and protocol settings.

## Portable show

The lossless `.show` document containing production content. It excludes installation users,
sessions, screens, and credentials.

## Desk database

`desk.sqlite`, owned by the installation. It stores users, desks, screens, settings, show index,
and named recovery checkpoints; Save As never copies it.
