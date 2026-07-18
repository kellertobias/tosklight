# Cuelist and Fade-Time Review

## Status and scope

This is the remaining larger review block formerly recorded as “Check Cuelists + Fade times.” It belongs in Later because it spans the complete Cue/Cuelist timing model rather than one small corrective layout change. It complements, but does not replace, the focused [Cuelist and Cue Settings Layout](../Next/36-cuelist-and-cue-settings-layout.md) and [Chaser Crossfade Percentage](../Next/37-chaser-crossfade-percentage.md) plans.

## Review contract

Audit Cue recording, tracking, playback reconstruction, GO/GO minus, Go To/Load, per-value Fade/Delay, Cue Fade/Delay fallbacks, trigger timing, Programmer Fade, Cue Fade master, Force/Disable Cue Timing, Move in Black, Chaser cadence, manual X-fade, release, and restart. The review must distinguish stored Cue data from playback runtime state and must not generalize playback HTP semantics into programmer LTP behavior.

Build a timing matrix for snap and faded attributes, tracked and explicit values, positive and zero duration, interrupted transitions, speed changes, multiple active Cuelists, Grand Master/Blackout, Preload, and the first frame after restart. Compare UI labels and seconds/percent entry with persisted milliseconds or normalized values and the engine's resolved timestamps.

## Completion criteria

1. Every documented timing control maps to one authoritative persisted or runtime field with units and precedence stated.
2. Paired API/UI scenarios cover normal, interrupted, disabled/forced, Chaser, manual-X-fade, and restart timing.
3. Current/next Cue identity and tracking reconstruction remain correct through renumber, delete, load, release, and page changes.
4. UI, OSC, attached hardware, event feedback, and actual DMX output agree on transition start, progress, completion, and interruption.
5. Any discovered behavior change receives its own scoped plan or implementation task rather than being silently folded into this audit.
