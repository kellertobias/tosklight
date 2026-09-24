/**
 * The CAD's platform modifiers. A drag of the gizmo with the duplicate modifier held places a copy
 * where it is let go and leaves the original where it was: Option on macOS, as a Mac drawing
 * program duplicates, and Ctrl elsewhere, as Windows and Linux ones do.
 */

/** Whether this is a Mac, whose duplicate modifier is Option rather than Ctrl. */
export function isMac(): boolean {
	const platform =
		(navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ??
		navigator.platform ??
		"";
	return /mac/iu.test(platform);
}

/** Whether an event holds the platform's duplicate modifier. */
export function holdsDuplicateModifier(event: { altKey: boolean; ctrlKey: boolean }): boolean {
	return isMac() ? event.altKey : event.ctrlKey;
}

/** Whether a key is the platform's duplicate modifier itself. */
export function isDuplicateModifierKey(key: string): boolean {
	return key === (isMac() ? "Alt" : "Control");
}
