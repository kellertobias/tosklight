import type { SessionResponse } from "../../api/types";

export type SessionRole = "primary" | "secondary";

export function mayCreateSession(role: SessionRole) {
	return role === "primary";
}

export function mayCloseSession(role: SessionRole) {
	return role === "primary";
}

export function closeOwnedSession(role: SessionRole, closeSession: () => void) {
	if (mayCloseSession(role)) closeSession();
}

export function readPrimarySession(value: string | null) {
	if (!value) return null;
	try {
		return JSON.parse(value) as SessionResponse;
	} catch {
		return null;
	}
}

export function requirePrimarySession(value: string | null) {
	const session = readPrimarySession(value);
	if (session) return session;
	throw new Error("The primary desk session is not available yet");
}
