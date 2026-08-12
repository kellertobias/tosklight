export function installDeskContextMenuPolicy(target: Document): () => void {
	const suppressUnsupportedMenu = (event: MouseEvent) => {
		if (!event.defaultPrevented) event.preventDefault();
	};
	target.addEventListener("contextmenu", suppressUnsupportedMenu);
	return () =>
		target.removeEventListener("contextmenu", suppressUnsupportedMenu);
}
