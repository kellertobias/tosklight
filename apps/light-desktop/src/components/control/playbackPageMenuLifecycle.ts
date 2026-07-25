import { useEffect, useRef } from "react";

/** Closes a Page menu when either writer it opened against is replaced. */
export function useOpenedPageMenuAuthority(
	open: boolean,
	createPage: unknown,
	setActivePage: unknown,
	onClose: () => void,
) {
	const opened = useRef<readonly [unknown, unknown] | null>(null);
	useEffect(() => {
		if (!open) {
			opened.current = null;
			return;
		}
		const current = [createPage, setActivePage] as const;
		if (!opened.current) {
			opened.current = current;
			return;
		}
		if (
			!Object.is(opened.current[0], current[0]) ||
			!Object.is(opened.current[1], current[1])
		)
			onClose();
	}, [createPage, onClose, open, setActivePage]);
}

/** Escape closes only an idle parent Page menu, never a nested keyboard or pending action. */
export function usePlaybackPageMenuEscape(
	open: boolean,
	busy: boolean,
	onClose: () => void,
) {
	useEffect(() => {
		if (!open) return;
		const handleKey = (event: KeyboardEvent) => {
			if (event.key !== "Escape" || busy) return;
			if (document.querySelector(".ui-input-modal-layer")) return;
			event.preventDefault();
			event.stopImmediatePropagation();
			onClose();
		};
		window.addEventListener("keydown", handleKey, true);
		return () => window.removeEventListener("keydown", handleKey, true);
	}, [busy, onClose, open]);
}
