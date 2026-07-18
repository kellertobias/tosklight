# Playback Configuration Layout Review

## Status and scope

Perform a layout and reachability review of the completed [Playback Configuration](../Done/10-playback-configuration.DONE.md) feature. This is observation and polish work; it must not reopen the accepted assignment, topology, color, Clear Playback, or persistence semantics without a separately recorded product decision.

## Review matrix

Inspect Assignment and Playback Layout tabs for Cuelists, Group Masters, Speed Groups, time masters, Grand Master, faderless layouts, and one-, two-, and three-button topologies. Include presentation icon/image fields and the full playback-color palette.

At supported software-only and hardware-connected sizes, every field, tab, validation message, and footer action must remain visible or reachable through one obvious scroll region. Switching assignment family may change applicable controls but must not move Save/Cancel beyond reach, retain incompatible values invisibly, or cause the modal to jump under the command surface.

## Acceptance criteria

1. Representative assignment/topology combinations have no clipping, overlap, nested-scroll trap, or unreachable footer.
2. Labels, descriptions, palette choices, and selected states remain readable with touch-sized targets.
3. Validation and dirty-state feedback do not resize the dialog beyond the viewport.
4. Keyboard focus order and touch navigation follow the visible tab and field order.
5. Visual regression coverage complements the existing semantic PBK scenarios without duplicating their behavior contract.
