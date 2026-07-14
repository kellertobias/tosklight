# Finalize Preload

Extend preload so it can contain playback actions as well as programmer changes. The configuration surface for the preload scene and Preload GO is still to be decided, but it must let the operator choose whether a preload contains programmer changes, playback actions, or both.

For playbacks, the preload payload must identify the target playback and contain only explicit button actions:

- Toggle
- Go
- Go minus
- Off
- On

Flash actions, fader changes, and an On state caused by moving a fader must never be captured. Planning must also define how multiple actions for the same playback are represented and ordered, what happens if a playback changes before Preload GO, and how the active preload scene displays and clears its captured playback actions.
