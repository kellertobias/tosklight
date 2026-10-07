import type { PointerEvent } from "react";
import type { DesktopBridge } from "../../platform/desktop";

/** Only the owning built-in's passive title chrome can move the native desk. */
export function startBuiltInWindowDrag(
	event: PointerEvent<HTMLElement>,
	desktop: DesktopBridge,
) {
	if (event.button !== 0 || event.defaultPrevented || !desktop.available) return;
	if (!(event.target instanceof Element)) return;
	const target = event.target;
	const header = target.closest(".ui-window-header");
	if (!header || header !== event.currentTarget.querySelector(".ui-window-header"))
		return;
	if (
		target.closest(
			"button, input, select, textarea, a, [role='button'], [role='tab'], [contenteditable], [tabindex]",
		)
	)
		return;
	// Allow title, status text, spacer and the header background; custom toolbars
	// and action/search groups retain their own pointer interactions.
	if (
		target !== header &&
		!target.closest(".ui-window-title, .ui-window-info, .ui-window-header-spacer")
	)
		return;
	event.preventDefault();
	void desktop
		.currentWindowFullscreen()
		.then((fullscreen) => {
			if (!fullscreen) return desktop.startCurrentWindowDrag();
		})
		.catch((error) => console.error("Could not move the desk window", error));
}
