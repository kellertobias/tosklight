> [!CAUTION]
> **BLOCKED PLANNING CAPTURE — NOT YET IMPLEMENTABLE.** This records follow-on product
> scope only. It must not be implemented until its audio model, persistence,
> routing, transport, Cue integration, and acceptance criteria are settled.

# Separate Audio Tracks

This plan follows [Macros](11-macros-and-scheduled-macros.md). It extends the
Timecode MVP; it does not replace the Timecode feature's main audio asset or
its audio-volume lane.

Add separately placeable audio clips to Timecode timelines, with their own
tracks and transport-aligned timing. The eventual feature should also consider
whether Cues can play audio clips, so an operator can program audio as part of
a Cue action as well as on a timeline.

Before implementation, settle the clip data model, overlap and mixing rules,
per-clip volume and fades, audio-output routing, seeking/reconstruction,
storage and show portability, runtime synchronization, and the exact Cue
semantics. In particular, do not assume that one audio file or one audio track
is sufficient once this plan begins.
