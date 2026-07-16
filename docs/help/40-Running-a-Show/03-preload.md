# Preload and Preload GO

Preload prepares a separate scene or queued playback actions without immediately disturbing normal live output.

## Capture domains

Desk Setup provides independent switches for programmer changes, physical playback actions, and virtual playback actions. All eight on/off combinations are meaningful. Disabled domains act live; enabled domains are captured in order for the pending Preload operation.

![Independent Preload capture-domain switches](../assets/screenshots/workflows/desk-setup-inputs.png)

## Operator flow

1. Press Preload to enter the pending workflow.
2. Make programmer changes and/or execute configured playback actions.
3. Inspect Preload-aware Stage/Fixture views while separately verifying that live DMX has not changed unexpectedly.
4. Press **Preload GO** to apply the captured work atomically.
5. Hold Preload for release when only the Preload programmer scene must be cleared.

The Preload programmer, physical action queue, and virtual action queue are distinct. A combined GO uses one commit point so the prepared change does not tear across frames.

Use a dedicated Stage pane with **Follow Preload** for preview and another live-output pane for comparison.
