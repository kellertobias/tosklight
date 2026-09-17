import type { SessionResponse } from "../types";

/**
 * What an external screen window needs to join the desk that opened it.
 *
 * A screen window is a second webview of the same ToskLight application. It must reach the same
 * server with the same operator session as the main window, never a server or session of its
 * own. Its per-window storage starts empty, so the main window hands this over explicitly: the
 * desktop host writes it into the screen webview's session storage before the page runs and
 * rewrites it whenever the main window reconnects.
 */
export interface ScreenAttachment {
	server_url: string;
	session: SessionResponse;
	desk_token: string | null;
}

export const SCREEN_ATTACHMENT_STORAGE_KEY = "light.screen-attachment";

function windowSessionStorage(): Storage | null {
	const storage = globalThis.sessionStorage;
	return storage && typeof storage.getItem === "function" ? storage : null;
}

function isSession(value: unknown): value is SessionResponse {
	if (!value || typeof value !== "object") return false;
	const session = value as Partial<SessionResponse>;
	return (
		typeof session.session_id === "string" &&
		typeof session.token === "string" &&
		typeof session.client_id === "string" &&
		typeof session.desk?.id === "string"
	);
}

export function parseScreenAttachment(
	value: string | null,
): ScreenAttachment | null {
	if (!value) return null;
	try {
		const parsed = JSON.parse(value) as Partial<ScreenAttachment>;
		if (typeof parsed.server_url !== "string" || !isSession(parsed.session))
			return null;
		return {
			server_url: parsed.server_url.replace(/\/$/, ""),
			session: parsed.session,
			desk_token:
				typeof parsed.desk_token === "string" && parsed.desk_token
					? parsed.desk_token
					: null,
		};
	} catch {
		return null;
	}
}

/** The attachment handed to this window, or null outside an external screen window. */
export function readScreenAttachment(
	storage: Storage | null = windowSessionStorage(),
): ScreenAttachment | null {
	return parseScreenAttachment(
		storage?.getItem(SCREEN_ATTACHMENT_STORAGE_KEY) ?? null,
	);
}
