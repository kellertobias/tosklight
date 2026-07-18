# Preload GO Secondary Text Review

## Status and scope

Review whether the completed [Finalize Preload](../Done/03-finalize-preload.DONE.md) implementation's **PRELOAD GO** secondary text gives the operator enough useful information without overloading the control. Current software controls already summarize pending programmer values and ordered playback actions; the final copy and hardware-connected presentation need explicit review.

## Review questions and desired outcome

Compare the empty, pending, applied, and release-available states. The primary label remains **PRELOAD** before capture and **PRELOAD GO** while pending capture can be committed. Secondary text should communicate the most useful pending summary that fits, with the complete detail available through an accessible label or non-hover-only expansion.

Decide from visual/operator review whether attached-hardware mode should show a compact secondary summary, rely on the command display, or intentionally keep only **PRELOAD GO**. Do not add text that shrinks the primary label below desk readability. Overflow must truncate predictably without hiding the full accessible value.

## Acceptance criteria

1. No-pending, programmer-only, playback-only, mixed, committed, and released states have unambiguous labels.
2. Multiple ordered playback actions remain understandable and do not imply predicted end states.
3. Full pending detail is keyboard/touch accessible and does not depend on hover.
4. Text fits or truncates safely at supported software-only and hardware-connected sizes.
5. GO, long-hold Release, cancellation, reconnect, and restart clear or restore the summary consistently with authoritative Preload state.
