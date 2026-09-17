/**
 * Quiet operator notices for desk actions that changed nothing while the desk stays healthy.
 *
 * A notice is not an error: it never writes the shared shell error, never marks the command
 * line, and never needs acknowledgement. Genuine failures keep using the blocking
 * "Desk needs attention" lane.
 */
export const DESK_NOTICE_EVENT = "light:desk-notice";

/** How long a notice stays visible before it expires on its own. */
export const DESK_NOTICE_DURATION_MS = 4_000;

export function reportDeskNotice(message: string): void {
	window.dispatchEvent(
		new CustomEvent<string>(DESK_NOTICE_EVENT, { detail: message }),
	);
}
