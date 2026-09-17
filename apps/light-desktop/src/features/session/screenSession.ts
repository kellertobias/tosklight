import { ApiRequestError } from "../../api/ApiRequestError";
import { readScreenAttachment } from "../../api/client/screenAttachment";
import type { SessionResponse } from "../../api/types";
import { readPrimarySession } from "./ownership";

/**
 * Screen windows never log in. They join the operator session of the desk window that opened
 * them, so a screen can never become a second, isolated desk.
 */
export const SCREEN_SESSION_UNAVAILABLE =
	"This screen joins the desk session of the main ToskLight window, and that session is not available yet. Keep the main window open and connected; the screen retries without starting a second desk.";

export class ScreenServerChangedError extends Error {
	constructor(readonly serverUrl: string) {
		super(
			`The main ToskLight window now uses the server at ${serverUrl}. This screen reloads to follow it.`,
		);
		this.name = "ScreenServerChangedError";
	}
}

/**
 * The desk session this screen window joins: the one handed over by the desk window, or, in a
 * plain browser, the one the desk tab stored for this origin.
 */
export function attachedDeskSession(
	serverUrl: string,
	storage: Pick<Storage, "getItem"> | null = globalThis.localStorage ?? null,
	sessionStore: Storage | null = globalThis.sessionStorage ?? null,
): SessionResponse {
	const attachment = readScreenAttachment(sessionStore);
	if (attachment) {
		if (attachment.server_url !== serverUrl.replace(/\/$/, ""))
			throw new ScreenServerChangedError(attachment.server_url);
		return attachment.session;
	}
	const stored = readPrimarySession(
		storage?.getItem("light.primary-session") ?? null,
	);
	if (stored) return stored;
	throw new Error(SCREEN_SESSION_UNAVAILABLE);
}

/** An operator-facing reason a screen window could not join its desk. */
export function describeScreenConnectionFailure(
	reason: unknown,
	serverUrl: string,
): string {
	if (reason instanceof ApiRequestError && reason.status === 401)
		return `The ToskLight server at ${serverUrl} did not accept the main window's desk session. The main window may be reconnecting; this screen retries with its current session.`;
	if (reason instanceof ApiRequestError && reason.status === 403)
		return `The ToskLight server at ${serverUrl} refused this screen: ${reason.message}`;
	if (reason instanceof TypeError)
		return `The ToskLight server at ${serverUrl} is not reachable. Check that the main ToskLight window is still connected.`;
	return reason instanceof Error ? reason.message : String(reason);
}
