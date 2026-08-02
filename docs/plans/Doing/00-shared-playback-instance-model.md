# Shared Playback Instance Model

## Status

**Doing.** Claimed on 2026-08-02. The shared-instance identity, Group Master participation, control arbitration,
assignment removal, persistence, and operator feedback decisions are settled below. This contract
must be implemented before work that assigns Groups, Cuelists, target-bound Dynamics, or Timecodes
to Playbacks depends on it.

## Progress

- [x] Claimed from `docs/plans/Next` and began the implementation audit.
- [x] Map current assignment, runtime, persistence, feedback, and pickup ownership.
- [ ] Implement shared runtime identity and assignment lifecycle in coherent compatibility-safe steps.
  Cuelist actions now use one target-keyed domain runtime across physical, Virtual, and direct-Cuelist
  addresses; assignment-local control state, projection/event reconciliation, and topology lifecycle remain.
- [ ] Add focused migration, runtime, cross-surface, and operator acceptance coverage.
- [ ] Run the required major suites and verify the real desktop path.

## Decisions

- The plan contract below is authoritative; implementation will preserve assignment-local presentation,
  temporary gestures, and physical pickup while moving logical runtime ownership to stable target identity.
- Existing valid persisted shows remain supported unless a later recorded decision explicitly declares a
  pre-v1 break and regenerates every repository-owned show.
- Contradictory legacy per-assignment Cuelist runtime rows migrate deterministically by highest activation
  ordinal, then newest activation time, then lowest stable assignment identity. A later persistence chunk
  must surface a visible migration warning when discarded rows disagree; silently retaining two runtimes is
  not permitted.

## Verification

- `cargo check -p light-playback --message-format short` passed after the target-key refactor.
- `cargo test -p light-playback --lib --quiet` passed: 100 tests, including deterministic legacy
  duplicate-runtime precedence.

## Remaining work

- Separate physical fader position/pickup from the shared Cuelist runtime and preserve held controls per
  assignment.
- Compose authoritative projections/events for every assignment and update generated wire/frontend state.
- Reconcile assignment removal/reassignment, migrate persistence and legacy Dynamics, then synchronize
  selection and cross-surface feedback.
- Run full plan closeout verification and record the truthful result.

## Goal

Multiple physical or Virtual Playbacks may be assigned to the same target, but they are controls
for one shared logical playback instance, not independent runs. Runtime identity follows the stable
target object ID. Two Playbacks assigned to the same Group ID control the same Group Master; two
assigned to the same Cuelist ID control the same Cuelist transport; and the same rule applies to a
Timecode ID or an assignable target-bound Dynamic ID.

A state change accepted through one assignment is immediately authoritative and visible through
every other assignment for that target. Assigning the same target twice never clones its runtime.
When an operator needs independent runs, they must create or use distinct target objects.

## Shared runtime by target

The target owns one logical runtime state:

- a Cuelist shares On/Off, current and next Cue, transition progress and direction, pause state,
  and master level;
- a Group ID has at most one Group Master level, regardless of how many Playbacks expose it;
- an assignable target-bound Dynamic shares its running/paused state, phase position, speed, and
  master level; and
- a Timecode shares its running/paused/stopped state, position, rate, and linked-audio state.

Speed Groups and desk-wide Special masters already have one authoritative identity and follow the
same control principle. A client must never infer a second runtime from a second assignment or show
two assignments with contradictory current values, running states, Cue positions, Dynamic states,
or Timecode positions.

### Targetless Dynamics

A targetless Dynamic behaves like a Preset, not like an independently assignable playback target.
It can run only in the Programmer against the Programmer's explicit ordered selection. It can then
be stored as content in a Cue/Cuelist, where the Cue/Cuelist owns the resulting playback runtime.
A targetless Dynamic is not assigned directly to a physical or Virtual Playback and therefore does
not create another shared-playback identity.

## Group Master identity and HTP

A Group becomes a Group Master only while at least one Playback assignment targets that Group ID.
The Group object by itself owns membership and settings but contributes no master value to output.
An unassigned Group containing fixtures neither adds, removes, subtracts, multiplies, nor otherwise
limits their intensity.

All Playback assignments targeting the same Group ID expose one shared Group Master and one shared
level. They are multiple controls for the same thing. Different Group IDs have independent Group
Master levels. When fixtures belong to several different assigned Group Masters, those distinct
Group Master contributions resolve by HTP; the highest eligible master level wins for the shared
fixtures.

For example, a Group containing every lamp has no master effect if it is never assigned to a
Playback. If a second Group containing one lamp is assigned, only that second Group becomes a
Group Master and affects that lamp. Assigning the first Group later creates a second, independent
Group Master, and their overlap resolves by HTP.

## Assignment-local state

Only the target's logical runtime is shared. The following remain local to each Playback
assignment:

- page/Playback placement, name and color presentation;
- button mapping, fader mode, auto-off options, and Swap protection;
- the currently held Flash, Temp, or Swap gesture and its temporary contribution; and
- the sensed physical-fader position and whether that control has satisfied pickup.

An assignment-local action still addresses the shared target. For example, Off or an enabled
auto-off action from one assignment switches the shared target Off and every assignment reflects
that state. Temporary Flash, Temp, and Swap lifetimes remain attributable to the originating
control, while their resolved authoritative effect is visible everywhere.

## Fader arbitration and pickup

All mutations are serialized by the authoritative server. GO followed by OFF means GO is applied
first and OFF second even when the actions arrive from different assignments or transports.

Two absolute-position physical faders may disagree with the shared authoritative level. Each uses
the established soft-takeover behavior from a Playback page change: it cannot change the target
until it reaches or crosses the authoritative level. Once one physical fader takes control and
changes the level, every other disagreeing physical fader receives the new pickup target and must
take over against that value before it can mutate the shared level. Pickup never creates a second
master or runtime.

Software faders do not require pickup. They behave like motorized faders: every authoritative
update moves their displayed position, and a software gesture can immediately change that displayed
shared value. Virtual and other faderless Playbacks have no pickup state.

## Selection and feedback

Selecting any assignment selects the shared target. Every assignment for that target shows the
same selected-target and runtime state. A control may additionally show transient held-action or
pickup feedback, but the last control touched does not become a persistent runtime owner.

Feedback events carry the stable target identity and authoritative target revision/state. They may
also carry the originating page/Playback address when needed for assignment-local presentation or
pickup acknowledgement. Clients reconcile from the authoritative event and never expand one
target mutation into separate local assignment mutations.

## Clearing and reassignment

Clearing or reassigning one of several assignments detaches only that control. It does not switch
Off, reset, or release the shared target while another Playback assignment still references that
same target ID.

Removing the final Playback assignment switches Off and releases an ordinary shared playback
runtime before detaching it. It does not delete the referenced Cuelist, Dynamic, Timecode, or other
pool object. Reassignment applies the same rule to the old target, then attaches the control to the
new target atomically.

Group Masters use their structural rule instead of a hidden Off state. Removing one of several
assignments for a Group leaves the same Group Master active. Removing the final assignment removes
that Group Master contribution entirely; the Group remains in the show as an ordinary Group and no
longer affects output as a master.

## Persistence and restart

The portable show persists target objects and Playback assignment configuration. Shared runtime
state is represented once per stable target identity rather than copied into every assignment.
Legacy shows with per-assignment runtime copies must migrate deterministically without inventing a
second target or silently changing output.

Legacy Playback assignments that reference a targetless Dynamic must preserve their exact stored
target scope and output by migrating that assignment to target-bound content. The original
targetless pool Dynamic remains a Programmer/Cue-content Preset; migration must neither silently
drop the assignment nor turn the original definition into one global target when legacy
assignments used different scopes.

Physical-fader position and pickup satisfaction are desk/session state, not portable show data.
After restart, show load, reconnect, or a page change, each absolute physical fader reacquires the
restored authoritative target level through normal pickup. Software controls immediately display
the restored value and remain directly operable.

Malformed, missing, stale-revision, or scope-mismatched actions fail atomically and visibly.
Loading an old or malformed show follows `docs/acceptance-criteria.md`; it must not leave different
assignments presenting divergent runtime copies.

## Acceptance coverage

- Assigning one Cuelist ID to two physical or Virtual Playbacks produces one runtime and synchronized
  Cue, transition, On/Off, pause, and master feedback.
- Actions arriving from different assignments and transports apply in authoritative server order.
- Two physical faders for one target require pickup independently and never jump or clone the
  shared level; a software fader tracks authoritatively and requires no pickup.
- Clearing one of several assignments leaves the shared target running. Clearing the final
  assignment switches Off/releases the ordinary target without deleting its pool object.
- Two assignments for one Group ID expose one Group Master level. Clearing the final assignment
  removes the master contribution while preserving the Group.
- Different assigned Group IDs have independent levels and overlapping fixture membership resolves
  those Group Master contributions by HTP.
- A Group with no Playback assignment contributes no Group Master effect, including when its
  fixtures also belong to an assigned Group Master.
- Targetless Dynamics run only in the Programmer, can be stored as Cue/Cuelist content, and cannot
  be assigned directly to a Playback.
- Legacy targetless Dynamic Playback assignments retain their exact output through target-bound
  migration without retargeting the original targetless pool definition.
- Assignment-local layout, presentation, behavior, temporary actions, and physical pickup remain
  local while shared target state stays synchronized.
- Selection, feedback, restart, legacy migration, stale revisions, and reconnect behavior remain
  consistent across software, keyboard, OSC, HTTP/WebSocket, Virtual Playback, and attached
  hardware paths wherever those surfaces expose the operation.

## Result

Pending implementation.
