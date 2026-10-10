/**
 * Quiet operator notices for desk actions that changed nothing while the desk stays healthy.
 *
 * A notice is not an error: it never writes the shared shell error, never marks the command
 * line, and never needs acknowledgement. Recoverable request failures use action feedback;
 * only authoritative loss of a critical desk capability uses "Desk needs attention".
 */
export const DESK_NOTICE_EVENT = "light:desk-notice";

/** How long a notice stays visible before it expires on its own. */
export const DESK_NOTICE_DURATION_MS = 4_000;

export function reportDeskNotice(message: string): void {
	window.dispatchEvent(
		new CustomEvent<string>(DESK_NOTICE_EVENT, { detail: message }),
	);
}
