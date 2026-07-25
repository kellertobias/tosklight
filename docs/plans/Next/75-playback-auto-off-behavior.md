# Playback Auto-Off Behavior

## Status

**Specification only.** This plan records future playback behavior configuration for Cuelist playbacks, with a later extension to Dynamic playbacks. It does not implement playback state changes, persistence, UI, API behavior, OSC behavior, hardware behavior, help changes, or executable tests.

## Goal

Add explicit behavior settings that can automatically turn a playback Off when the operator's own control gesture has ended, independent of the existing **Turn off when other playbacks take full control** rule.

This plan is for Cuelist playbacks first. Once Dynamic playback assignment exists, the same policy family should apply to Dynamic playbacks where the matching fader and flash concepts exist.

## Relationship to Existing Behavior

The completed Playback Configuration contract already defines:

- **Turn off when other playbacks take full control**, where another normal non-temporary playback may switch this playback Off only after every active attribute address it contributes has been overwritten; and
- Flash/Temp behavior, where temporary priority-stack entries are removed when released without counting as full-control overwrite.

This new feature is different. It turns the configured playback Off because its own fader or flash interaction has ended:

- when its fader reaches zero; or
- when its Flash is released.

It must not depend on another playback taking control, and it must not reinterpret temporary Flash/Temp arbitration as full-control overwrite.

## Configuration

Cuelist playback settings gain a persisted **Auto-off** behavior group with independent options:

- **When fader reaches zero**;
- **When Flash is released**; and
- future options only when their trigger and playback type are explicitly specified.

The options are disabled by default for existing shows unless a later migration decision deliberately maps an existing setting to one of them. Enabling one option must not silently enable the other.

The settings belong to the playback assignment/configuration, not to the Cuelist object itself. The same Cuelist assigned to two playbacks may therefore have different auto-off behavior.

## Fader Reaches Zero

When **When fader reaches zero** is enabled, moving the playback's configured fader to its bottom/zero level turns that playback Off after the zero level is accepted by the authoritative playback service.

This applies only to fader modes where zero means this playback's own active output level has reached zero. For Cuelist playbacks this initially means the **Master** fader. It must not apply to **X-fade** merely because the physical fader reaches one end of travel; X-fade endpoints complete cue transitions rather than meaning "this playback level is zero."

The transition to Off must be one authoritative playback mutation with normal feedback, events, undo/audit behavior where applicable, hardware LED updates, software state updates, OSC/API notifications, and persisted active-state behavior. It must not leave a hidden running Cuelist with level zero unless the option is disabled.

If the operator raises the fader from zero after auto-off, implementation must define whether that restarts the playback, leaves it Off until GO/On, or follows the existing playback take-over behavior. This decision must be explicit before implementation.

## Flash Released

When **When Flash is released** is enabled, releasing Flash turns the playback Off after the Flash contribution is removed.

This is separate from the existing **When Flash or Swap is released** mode:

- **Release all** describes which flashed attributes are removed from the temporary Flash contribution.
- **Intensity only** describes whether non-intensity state is retained at zero intensity.
- **Auto-off when Flash is released** describes whether the underlying playback assignment itself turns Off after the Flash gesture ends.

If both release-mode and auto-off settings are present, their order must be deterministic. The Flash contribution releases first, then the underlying playback Off mutation applies if the auto-off option is enabled. The operator must not see a stale playback remain On because the release-mode setting retained non-intensity state.

Temp buttons and Temp faders do not use this option unless a future plan explicitly adds matching Temp auto-off behavior.

## Dynamic Playbacks

Once [Dynamics Playback Assignment](../Later/dynamics/04-playback-assignment.md) is implemented, Dynamic playbacks should use the same Auto-off behavior vocabulary where applicable.

For Dynamic playbacks, **When fader reaches zero** must define whether zero stops, pauses, releases, or hides the Dynamic runtime instance. **When Flash is released** must define whether releasing Flash stops only a temporary contribution or turns off the assigned Dynamic playback source. Those runtime semantics belong to the Dynamics playback assignment chunk before Dynamic support can ship.

## Surface Requirements

The configuration and behavior must be consistent across:

- Playback Configuration UI;
- software playback surfaces;
- virtual playbacks;
- attached hardware playbacks;
- keyboard and command-line playback operations where exposed;
- OSC and HTTP/API playback operations where exposed;
- playback feedback, running-source feedback, and LEDs; and
- operator help and acceptance scenarios.

No surface may locally decide to hide a zero-level playback as "effectively off" without the server state actually switching Off.

## Acceptance Coverage

1. Existing shows load with auto-off options disabled unless a deliberate migration says otherwise.
2. A Cuelist playback can enable **When fader reaches zero** without enabling **When Flash is released**.
3. A Cuelist playback can enable **When Flash is released** without enabling **When fader reaches zero**.
4. With fader auto-off enabled, moving the Cuelist Master fader to zero turns the playback Off through the authoritative playback service.
5. With fader auto-off disabled, moving the Cuelist Master fader to zero leaves the playback On at zero level.
6. X-fade endpoints do not trigger fader auto-off.
7. With Flash auto-off enabled, releasing Flash removes the Flash contribution and then turns the underlying playback Off.
8. With Flash auto-off disabled, Flash release follows the existing release-mode setting and leaves the underlying playback state according to that setting.
9. Flash release mode and Flash auto-off order is deterministic for both **Release all** and **Intensity only**.
10. Temp button and Temp fader gestures do not trigger these Cuelist auto-off options.
11. Software UI, virtual playback, hardware, OSC/API where exposed, playback feedback, and LEDs agree on the resulting Off state.
12. The same Cuelist assigned to two playbacks can use different auto-off settings.
13. Dynamic playback auto-off remains unavailable or clearly not implemented until the Dynamics playback assignment semantics define its runtime behavior.
