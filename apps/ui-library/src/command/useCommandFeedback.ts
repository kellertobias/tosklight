import { useEffect, useState } from "react";

export type CommandErrorKind = "operator" | "unexpected";
export const COMMAND_ERROR_BLINK_MILLIS = 500;
export const COMMAND_ERROR_TOAST_MILLIS = 2000;

/** Display feedback only; command text and authoritative history stay owned by the desk. */
export function useCommandFeedback(message: string | null, revision = 0) {
	const [blink, setBlink] = useState(false);
	const [toast, setToast] = useState(false);
	useEffect(() => {
		setBlink(Boolean(message));
		setToast(Boolean(message));
		if (!message) return;
		const blinkTimer = globalThis.setTimeout(
			() => setBlink(false),
			COMMAND_ERROR_BLINK_MILLIS,
		);
		const toastTimer = globalThis.setTimeout(
			() => setToast(false),
			COMMAND_ERROR_TOAST_MILLIS,
		);
		return () => {
			globalThis.clearTimeout(blinkTimer);
			globalThis.clearTimeout(toastTimer);
		};
	}, [message, revision]);
	return { blink: Boolean(message) && blink, toast: Boolean(message) && toast };
}
