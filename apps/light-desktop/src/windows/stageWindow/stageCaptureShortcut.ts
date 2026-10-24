import type { KeyboardEvent } from "react";

/** Local focus owns this shortcut; command-line, editable and modal input stays with its owner. */
export function isStageCaptureShortcut(event: KeyboardEvent<HTMLElement>, modalOwnsKeyboard: boolean) {
	const target = event.target;
	return !modalOwnsKeyboard && !event.defaultPrevented && !event.repeat && !event.nativeEvent.isComposing && event.nativeEvent.keyCode !== 229 &&
		event.key.toLowerCase() === "r" && event.shiftKey && !event.altKey && event.metaKey !== event.ctrlKey &&
		target instanceof HTMLElement && target === document.activeElement && event.currentTarget.contains(target) &&
		!target.closest("input, textarea, select, [contenteditable]:not([contenteditable='false']), [role='textbox'], [role='dialog'], [aria-modal='true']");
}
