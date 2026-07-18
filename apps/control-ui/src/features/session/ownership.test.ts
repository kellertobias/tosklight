import { expect, it } from "vitest";
import {
	closeOwnedSession,
	mayCloseSession,
	mayCreateSession,
	readPrimarySession,
	requirePrimarySession,
} from "./ownership";

it("allows only the primary surface to create or close the session", () => {
	expect(mayCreateSession("primary")).toBe(true);
	expect(mayCloseSession("primary")).toBe(true);
	expect(mayCreateSession("secondary")).toBe(false);
	expect(mayCloseSession("secondary")).toBe(false);
});

it("disconnects a secondary surface without closing the shared session", () => {
	let closed = 0;
	closeOwnedSession("secondary", () => {
		closed += 1;
	});
	closeOwnedSession("primary", () => {
		closed += 1;
	});
	expect(closed).toBe(1);
});

it("restores a primary session without accepting malformed storage", () => {
	const session = { session_id: "session-1", token: "secret" };
	expect(readPrimarySession(JSON.stringify(session))).toEqual(session);
	expect(readPrimarySession("{")).toBeNull();
	expect(() => requirePrimarySession(null)).toThrow(
		"The primary desk session is not available yet",
	);
});
