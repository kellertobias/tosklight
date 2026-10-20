import type { VerticalTouchFaderAction } from "@tosklight/ui/faders";

/** Solo qualifies permanent activation; Flash and Swap retain their bypass semantics. */
export function playbackTileActionLabel(action: VerticalTouchFaderAction | undefined, soloMember: boolean) {
	return soloMember && action && /-(go|toggle|on|temp)$/.test(action.id)
		? `SOLO · ${action.label}`
		: action?.label;
}
