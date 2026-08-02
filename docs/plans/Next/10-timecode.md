> [!CAUTION]
> **IMPLEMENTATION PREREQUISITE — STOP.** The Timecode MVP contract is settled,
> but it depends on [Cuelist Transition Timing](09-cuelist-transition-timing.md).
> Timecode implementation must not begin until that prerequisite is specified
> and complete; this plan then provides the implementation-ready Timecode scope.

# Timecode

This is the eleventh item in the current [Next plan order](README.md), after all other currently queued
work and directly before Macros.

## Delivery decision

When this plan becomes implementable, it must deliver the first real Timecode
MVP: a production-ready, maintainable feature within the scope settled here,
not a documentation-only exercise or a throwaway prototype. Its state model,
persistence, transport, execution behavior, and operator workflows must be
deliberate extension points for later Timecode capability; future work may add
capabilities, but must not require replacing an intentionally temporary core.

The MVP must run reliably from ToskLight's internal clock. External-timecode
synchronization is not a delivery gate: include it only when it can be added
cleanly without compromising or delaying the internal-clock implementation.
Desk settings select one explicit Timecode source rather than an automatic
source-priority list. The desk's internal generator is that explicit source
when no external source is to be used.
When the operator changes that selected source while a Timecode is running,
switch and seek immediately whenever the new source can be used.

Timecode frame rate is one desk setting, not a per-Timecode setting. It defaults
to the desk's configured DMX frame rate, and the operator may select a different
Timecode frame rate there when needed.
If an external source declares a different frame rate, convert it to the desk
rate when possible and show a warning. If the source rate is unknown or cannot
be converted, reject it with a clear error.

When LTC support is added, it must decode from the operator-selected microphone
or line-level audio input rather than requiring dedicated timecode hardware.
LTC generation must run in parallel with program audio through its own
independently selected output; LTC is never mixed into the music/program-audio
output.
Art-Net ArtTimeCode is required as a supported network Timecode source in the
final extensible product, but is deferred beyond the first MVP. MIDI Timecode
may arrive through the native-adapter `timecode_source` interface; OSC timed
messages and sACN synchronization are not standard Timecode sources.
When an external source disappears, the desk settings configure whether a
running Timecode continues on its internal clock, pauses, or stops.
When that external source returns after fallback, the running Timecode
immediately re-locks to it.
The MVP supports looping the internally generated Timecode from its end to zero,
reconstructing the same deterministic state on each cycle. An externally synced
Timecode follows its incoming source and does not loop independently. The
initial MVP loops only the complete Timecode; a configurable loop range with a
start and end is required as a later extension. At every loop point, all
Timecode-controlled state, including main audio, jumps to the loop start; a
later range loop therefore jumps to its configured start rather than zero.

When a Timecode has an audio asset, the server owns its playback and keeps
timeline execution synchronized with it. Each control desk configures its audio
output destination in desk settings; that configuration determines where the
server's audio playback is heard. A Timecode with no audio asset remains fully
usable as a timeline-only transport.

The MVP must support PCM WAV audio. MP3 support is also desired, subject to
confirming that its selected decoder/library and distribution terms are
license-compatible.

The MVP has a main audio asset and an audio lane with volume keyframes for
controlling that asset's level and fades during the Timecode. Audio-volume
fades support linear, ease-in, ease-out, and ease-in-out curves in the MVP.
Separately placeable audio clips and additional audio tracks are
deferred to [Separate Audio Tracks](12-separate-audio-tracks.blocked.md), after Macros.

A Timecode's editor timeline is always authored from `00:00:00:00`. It must
support a configurable transport offset, so that this zero-based timeline can
align to an internal or external timecode position such as `01:00:00:00` for
the first song and `02:00:00:00` for the second. The audio asset and the editor
continue to begin at zero; the offset only maps that local timeline to the
transport position used for synchronization.

Each Timecode owns a per-Timecode auto-start setting. A Timecode is **armed**
when that setting is enabled: it is ready to begin automatically when its
configured external-Timecode trigger is reached. When auto-start is disabled,
it is not armed and will not begin automatically. Arming and auto-start do not
apply to the internal generator: internally timed Timecodes start manually
unless a future desk-wide background-Timecode concept is introduced. Operators
configure this directly on the Timecode, without first creating a Scheduler
entry. Scheduler integration may later offer another way to control Timecodes,
but must not be the sole configuration surface.

If an armed Timecode first sees an external source already past its configured
offset, it starts immediately at the corresponding local timeline position and
reconstructs that position's state. It does not wait for a later rollover or
require the external source to cross the offset again. While externally synced,
an incoming Timecode jump likewise seeks the local timeline to the corresponding
position and reconstructs its authoritative state.
After a server or desk restart, internally timed Timecodes remain stopped.
Armed externally synced Timecodes reconstruct from their current external
position.

Add a Timecode feature for programming and running a show against a fixed timeline. An operator can create a Timecode with either an imported song/audio file or a duration-only timeline when no media file is required. With audio loaded, the editor should display the song and its waveform; without audio, it should use the saved duration as the timeline boundary.

## Existing product-design evidence

The repository already contains a Storybook-only product-design prototype at
`TimecodeWindow.stories.tsx`. It models a frame-addressed editor with local,
deterministic state: transport, audio presentation, lanes, Cuelist instances and
cues, Group Master points, and Speed Group points. It is useful interaction and
visual evidence for this plan, but it is not implementation or runtime proof:
it has no authoritative Timecode state, persistence, backend execution, or real
audio transport. Decisions made here must turn that prototype into an explicit
operator and runtime contract rather than inheriting its demo behavior by
accident. It is a starting point, not a prescribed final UI: implementation may
retain, revise, or replace its visual layout and interactions when the settled
operator contract calls for it.

## Timecode editor

Timecode should have its own editor that behaves like a simple nonlinear video editor. The operator can scrub and zoom the timeline, play or pause it, jump to a precise position, and place lighting **keyframes** at exact times. A keyframe is one scheduled action on the Timecode timeline. At minimum, keyframes must be able to execute:

- a specific Cue in a Cuelist; and
- a Sequence or other repeatable sequence-style object once that object model is finalized.

Keyframes should be visible and movable on the timeline, with clear labels for their target and trigger time. Planning must define snapping, time display and precision, overlapping keyframes, keyframe duration where applicable, timeline tracks or lanes, copying and deleting keyframes, and what happens when the linked Cue, Cuelist, Sequence, or audio file is renamed, moved, or removed.

Operators can place non-executing **markers** on a Timecode. Markers are visual
timeline aids only; keyframes can snap to them, but they never trigger output or
other behavior. Operators can import markers from CSV, with a required Timecode
position and optional name and color. At import time, the operator chooses
whether to append imported markers or replace the existing marker set.

Keyframes are frame-accurate: they always land on the desk's configured
Timecode frames, never between frames. Snapping to a marker resolves to the
corresponding Timecode frame.

Timecode editing participates in the standard undo and redo history, including
keyframe creation, movement, deletion, and property changes.

The MVP must execute Cuelists and set Speed Groups. Cuelists are the core
timecoded content; Speed Group control is required so a repeated Timecode run
does not inherit an unintended speed state from an earlier run. Direct Group
Master control is required in the eventual extensible product but may be
deferred beyond the first MVP.
Each Speed Group keyframe can set the group's BPM and an explicit phase value;
it is not limited to merely restarting the phase.

If a referenced Cuelist is missing, its Cuelist clip is retained and clearly
marked as an error. The Timecode remains runnable and skips only that missing
clip; it does not delete the clip automatically or block the rest of the
Timecode. The operator repairs or removes it deliberately.

Cuelist clips remain live references to their Cuelist. Adding a Cue to that
Cuelist adds it into every applicable Timecode clip. A clip inherits transition
timing from its Cuelist unless its relevant Timecode keyframe explicitly
overrides that timing. Existing Timecode overrides are never silently replaced
by a Cuelist edit; the operator must be able to arrange the affected interval
in the Timecode deliberately. If a specific referenced Cue is deleted, retain
its Timecode keyframe as a visible missing-Cue error but ignore it during
playback, allowing the rest of the Timecode to continue.
Renaming or renumbering a referenced Cuelist or Cue preserves the reference and
updates the affected clip or keyframe's displayed label automatically.

A Cuelist lane is bound to one specific Cuelist. It may contain repeated
**Cuelist clips**, each a bounded run of that Cuelist with its own selectable
start Cue and end Cue; a clip whose range contains one Cue runs that single
Cue. At each clip's start, the operator must choose whether to reconstruct the
Cuelist's tracked state as
though earlier Cues had already run, then enter that state using the start
Cue's fade (**State Start**), or to apply only the start Cue's own changes
(**Cue Start**). State Start is the default because it produces the state an
operator expects at that point in a tracked Cuelist; Cue Start is the explicit
alternative. At each clip's end keyframe, the final state is released by
default. The operator may instead
choose to hold it, for example where the duration is uncertain or long, or for
rapid adjacent clips where repeatedly adjusting the endpoint is impractical.

Preload is the safe Timecode editing context: editing or scrubbing a Timecode
while in Preload must not reach live output. This provides the required
distinction between editing and live execution, so selecting or moving a
keyframe cannot accidentally fire it. It should also define whether playback
can begin from the middle of the timeline, whether earlier keyframes are
reconstructed or skipped, and how seeking backward affects already-running
Cuelists and Sequences.

In Preload, **Play** is a preloaded action: it does not start the Timecode
immediately. The Timecode begins only when the operator presses **Preload GO**.

Timecode playback must be repeatable from any position. Starting or seeking to a
position reconstructs the authoritative state as though the Timecode had run
continuously from zero, including the effect of earlier Cuelist clips and Speed
Group keyframes; it must not simply skip earlier work.

## Timecode Pool and assignment

Timecodes are first-class, numbered show objects in a dedicated **Timecode Pool**. A pool tile opens the corresponding Timecode editor and provides the normal create, name, copy, move, delete, and assignment workflows used by other pool objects.

A Timecode can run standalone from its pool or editor through the Programmer,
without a playback assignment. Assignment to a physical or Virtual Playback is
an optional live control surface, not a prerequisite for its transport; an
unassigned armed Timecode may therefore start from external Timecode.

A Timecode can be assigned to a physical playback or a Virtual Playback. Playback controls should be able to start, pause, resume, stop, and restart the assigned Timecode as appropriate for the configured control layout. The current position, running state, duration, and linked audio state must remain authoritative and synchronized across physical controls, Virtual Playbacks, the pool, and the editor. Multiple assignments control the one shared logical Timecode instance, as defined by [Shared Playback Instance Model](00-shared-playback-instance-model.md); they do not create independent Timecode runs.

The primary manual trigger is an assigned physical or Virtual Playback: pressing
**GO** on that playback starts its Timecode from its beginning and, when
present, its synchronized audio playback. It does so even when the Timecode is
paused mid-timeline. This is available whether or not the Timecode is armed;
arming is the separate automatic-start behavior. **Pause** is the transport
toggle: its first press pauses and its second press resumes from the paused
position. In the Timecode transport UI, **Stop** returns to zero and remains
stopped; **Rewind** returns to zero and begins playback, matching GO's result.

Starting a Timecode from a Cuelist is a possible additional trigger and remains
to be designed.

### Cuelist chaining requirement

Timecodes must be able to participate in internally sequenced Cuelists, so a
hard-coded section can start the next Timecode without an external source. This
requires Cuelist trigger modes that apply beyond Timecode: **GO**, **Follow**,
and **Link**. GO is the manual trigger. Follow begins its trigger time after
the previous Cue has completed. Link begins its trigger time when the previous
Cue starts, allowing their work to overlap. Every mode has a trigger time:
zero triggers immediately at its reference point, while a positive value delays
the actual trigger before its fade begins. This applies to manual GO too, so a
GO with a two-second trigger time waits two seconds after the operator presses
GO before executing the Cue. Completion-relative offsets may be negative,
allowing overlap—for example, triggering the next Cue ten seconds before a
Timecode finishes. Negative trigger time is not supported for GO or Link. A
Cue action that starts a Timecode instead has an explicit Timecode start
position, so an operator can intentionally start it five seconds into its
timeline without overloading trigger-time semantics.

Cue completion is the latest completion of all work started by that Cue. When a
Cue starts a Timecode and has lighting fades, its following Cue's Follow delay
begins only after both the Timecode and those fades have finished—whichever ends
last. The complete Cuelist-transition contract, including independent Intensity
out-delay and out-fade behavior, is owned by the prerequisite
[Cuelist Transition Timing](09-cuelist-transition-timing.md) plan. Links
between Cuelists and Timecodes are validated as an execution graph and rejected
when they create direct or indirect recursion. Shared assignment behavior is
owned by [Shared Playback Instance Model](00-shared-playback-instance-model.md).

## Audio, persistence, and runtime behavior

The show must persist the Timecode object, its duration, its ordered timeline keyframes, and its playback assignments. Planning must define whether imported audio is embedded in the show, copied into managed show storage, or referenced externally, including how missing files, large files, portability, backups, and Save As are handled. Duration-only Timecodes must remain fully usable without an audio asset.

Imported Timecode audio is copied into managed show storage. Backups and Save
As must therefore include it and remain portable; the MVP does not depend on an
external original-file path at runtime.

Managed audio assets are immutable within ToskLight: operators can select a
different asset for a Timecode, but do not edit the audio itself. Copying a
Timecode duplicates all of its editable timeline state and settings
independently, while referencing the same managed audio asset rather than
storing a duplicate. Editing the copy never changes the original.
Deleting a Timecode does not automatically delete its managed audio asset,
even when it has no remaining Timecode references; operators may use that asset
again later.

Runtime timing must use one authoritative clock and remain stable across UI reconnects. Before implementation, specify audio-output routing, latency compensation, pause/resume and restart behavior, end-of-timeline behavior, loop support, recovery after server or desk restart, external timecode synchronization if it is ever added, and deterministic behavior when the system cannot keep up with closely spaced keyframes.

For the MVP, a configured duration takes precedence as the Timecode's end,
whether or not it has audio. When no duration is configured, a Timecode with
audio finishes when its audio ends; duration-only Timecodes require a configured
duration. More configurable terminal policies, such as ending after the last
Cuelist clip, are deliberate extension points and are not required for the
first MVP. The relation between a configured Timecode end and the audio endpoint
is configurable. With no configured Timecode end, the timeline ends with its
audio. When an end is configured, an optional signed audio-end fade determines
what happens there: zero stops audio at the Timecode end; a positive value fades
audio out after that end; a negative value fades it out before that end; and an
unset value leaves the audio playing to its natural end.
