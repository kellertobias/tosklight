import { describe, expect, it } from "vitest";
import { ApiRequestError } from "../../api/ApiRequestError";
import {
	parseScreenAttachment,
	SCREEN_ATTACHMENT_STORAGE_KEY,
} from "../../api/client/screenAttachment";
import {
	attachedDeskSession,
	describeScreenConnectionFailure,
	SCREEN_SESSION_UNAVAILABLE,
	ScreenServerChangedError,
} from "./screenSession";

const session = {
	role: "operator",
	session_id: "session-1",
	client_id: "client-1",
	token: "token-1",
	desk: { id: "desk-1" },
};

function store(values: Record<string, string>) {
	return {
		getItem: (key: string) => values[key] ?? null,
	} as Storage;
}

function attachment(serverUrl = "http://127.0.0.1:5471") {
	return store({
		[SCREEN_ATTACHMENT_STORAGE_KEY]: JSON.stringify({
			server_url: serverUrl,
			session,
			desk_token: "",
		}),
	});
}

describe("screen window desk session", () => {
	it("joins the session handed over by the desk window", () => {
		expect(
			attachedDeskSession("http://127.0.0.1:5471/", store({}), attachment()),
		).toEqual(session);
	});

	it("prefers the handed-over session over one stored for the origin", () => {
		const other = { ...session, session_id: "stale", token: "stale" };
		expect(
			attachedDeskSession(
				"http://127.0.0.1:5471",
				store({ "light.primary-session": JSON.stringify(other) }),
				attachment(),
			),
		).toEqual(session);
	});

	it("follows the desk to a different server instead of joining the wrong one", () => {
		expect(() =>
			attachedDeskSession(
				"http://127.0.0.1:5000",
				store({}),
				attachment("http://desk.local:5000"),
			),
		).toThrow(ScreenServerChangedError);
	});

	it("falls back to the browser desk tab's session and otherwise explains the wait", () => {
		expect(
			attachedDeskSession(
				"http://127.0.0.1:5000",
				store({ "light.primary-session": JSON.stringify(session) }),
				store({}),
			),
		).toEqual(session);
		expect(() =>
			attachedDeskSession("http://127.0.0.1:5000", store({}), store({})),
		).toThrow(SCREEN_SESSION_UNAVAILABLE);
	});

	it("ignores malformed attachments", () => {
		expect(parseScreenAttachment("{")).toBeNull();
		expect(
			parseScreenAttachment(JSON.stringify({ server_url: "http://x" })),
		).toBeNull();
		expect(
			parseScreenAttachment(
				JSON.stringify({ server_url: "http://x/", session, desk_token: "d" }),
			),
		).toEqual({ server_url: "http://x", session, desk_token: "d" });
	});

	it("names the real cause of a failed join", () => {
		const url = "http://127.0.0.1:5471";
		expect(
			describeScreenConnectionFailure(new TypeError("Load failed"), url),
		).toContain(`${url} is not reachable`);
		expect(
			describeScreenConnectionFailure(
				new ApiRequestError("invalid session", 401),
				url,
			),
		).toContain("did not accept the main window's desk session");
		expect(
			describeScreenConnectionFailure(
				new ApiRequestError("desk boundary", 403),
				url,
			),
		).toContain("refused this screen: desk boundary");
		expect(describeScreenConnectionFailure(new Error("other"), url)).toBe(
			"other",
		);
	});
});
