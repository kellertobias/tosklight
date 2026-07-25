# Playback Configuration Layout Review

## Status and scope

Perform a layout and reachability review of the completed [Playback Configuration](../Done/10-playback-configuration.DONE.md) feature. This is observation and polish work; it must not reopen the accepted assignment, topology, color, None/Apply clear, or persistence semantics without a separately recorded product decision.

## Review matrix

Inspect Function, Behavior, and Layout tabs for Cuelists, Group Masters, Speed Groups, time masters, Grand Master, None, faderless layouts, and one-, two-, and three-button topologies. Include the edge-to-edge tab strip, two-column scrollable Fixture Library-style function/options lists, compact full-width name/color section, presentation icon/image fields, responsive playback-color dropdown, and explanatory button/fader choice modals. Verify Step Control, Permanent State, Temporary State, Selection, and specialized/fader group headings, plus the title-bar Empty Button action beside Close. Empty Button must not also appear as a Disabled choice in the function grid. The title must remain the only page/playback identity display; no redundant topology summary row belongs beneath it.

At supported software-only and hardware-connected sizes, every field, tab, validation message, title-bar Apply/Close action, and nested choice modal must remain visible or reachable through one obvious scroll region. Switching assignment family may change applicable controls but must not move Apply/Close beyond reach, retain incompatible values invisibly, or cause the modal to jump under the command surface.

## Acceptance criteria

1. Representative assignment/topology combinations have no clipping, overlap, nested-scroll trap, or unreachable title action.
2. Labels, descriptions, palette choices, and selected states remain readable with touch-sized targets.
3. Validation and dirty-state feedback do not resize the dialog beyond the viewport.
4. Keyboard focus order and touch navigation follow the visible tab and field order.
5. Visual regression coverage complements the existing semantic PBK scenarios without duplicating their behavior contract.
