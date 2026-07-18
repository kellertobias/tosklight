import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createHighlightActions(
	model: ServerController,
): Pick<ServerContextValue, "highlightAction" | "setPatchPreviewHighlight"> {
	const {
		client,
		setError,
		highlight,
		setHighlight,
		setHighlightError,
		highlightEpoch,
		highlightWrite,
		patchPreviewWrite,
		highlightErrorSticky,
	} = model;
	return {
		highlightAction: async (action) => {
			const request = ++highlightEpoch.current;
			highlightErrorSticky.current = false;
			setHighlightError(null);
			try {
				const write = client.highlightAction(action);
				highlightWrite.current = write.catch(() => undefined);
				const next = await write;
				if (request === highlightEpoch.current) {
					setHighlight(next);
					highlightErrorSticky.current = false;
					setHighlightError(null);
				}
				return true;
			} catch (reason) {
				const raw = reason instanceof Error ? reason.message : String(reason);
				let message = raw;
				try {
					const parsed = JSON.parse(raw) as {
						error?: string;
						message?: string;
					};
					message = parsed.error ?? parsed.message ?? raw;
				} catch {
					// The server may already have returned a plain operator-facing message.
				}
				if (/409|ownership|owned by|another (?:user|operator)/i.test(message)) {
					const owner = highlight?.owner_user_name?.trim();
					message = `Highlight is controlled by ${owner || "another operator"}. ${message}`;
				}
				highlightErrorSticky.current = true;
				setHighlightError(message);
				return false;
			}
		},
		setPatchPreviewHighlight: async (active, fixtureIds = []) => {
			try {
				const write = patchPreviewWrite.current.then(() =>
					client.setPatchPreviewHighlight(active, fixtureIds),
				);
				patchPreviewWrite.current = write.catch(() => undefined);
				const result = await write;
				setError(null);
				return result.active;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
	};
}
