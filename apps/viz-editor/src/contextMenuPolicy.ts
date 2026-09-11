/**
 * Architect has no browser context menu. Its own right-click actions call `preventDefault`
 * themselves; everything else, including macOS Ctrl-click (a multi-selection here), would otherwise
 * open the WebView's menu with Reload in it.
 */
export function installContextMenuPolicy(target: Document): () => void {
	const suppressBrowserMenu = (event: MouseEvent) => {
		if (!event.defaultPrevented) event.preventDefault();
	};
	target.addEventListener("contextmenu", suppressBrowserMenu);
	return () => target.removeEventListener("contextmenu", suppressBrowserMenu);
}
