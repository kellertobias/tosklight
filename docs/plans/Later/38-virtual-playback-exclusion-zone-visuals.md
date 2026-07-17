# Virtual Playback Exclusion Zone Visual Review

## Status and scope

This is a verification and visual-polish pass over the completed [Virtual Playback Exclusion Zones](../Done/17-virtual-playback-exclusion-zones.DONE.md) feature. The exclusion behavior, persistence, and functional acceptance coverage already exist; this plan closes the remaining visual-confidence gap without redesigning the feature.

## Review contract

Verify that configuration selection and saved zone membership are two distinct visual states. A cell selected while Shift is held must be unmistakable before zone creation. A cell already in one or more saved zones must remain identifiable after selection clears. Overlapping zones must expose all applicable names through reachable detail or settings, not only an opaque color.

Zone styling must coexist with configured playback color, active/running state, Record or Update targeting, empty cells, keyboard focus, and disabled/error states. Exclusion decoration cannot hide the playback label, Cue/action feedback, or the difference between active and inactive members. Software keyboard Shift and attached-hardware Shift must produce the same selection visuals.

## Acceptance criteria

1. Screenshot and computed-style checks distinguish temporary selection from saved membership.
2. Overlapping-zone cells expose every zone name and remain editable through settings.
3. Light and dark playback colors remain legible in active and inactive zone members.
4. Creating, editing, deleting, reconnecting, and restarting restore visuals from authoritative pane state.
5. Visual inspection covers representative software-only and hardware-connected sizes without changing completed exclusion semantics.
