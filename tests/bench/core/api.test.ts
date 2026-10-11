import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiDriver, type CommandLineState, type Session } from "./api";

describe("ApiDriver command-line replacement", () => {
	afterEach(() => vi.unstubAllGlobals());

	it("rebases one command-line revision race against fresh authority", async () => {
		const fetch = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValueOnce(commandLineResponse(1, "FIXTURE"))
			.mockResolvedValueOnce(
				new Response(
					"command-line revision conflict: expected revision 1, actual revision 2",
					{ status: 409 },
				),
			)
			.mockResolvedValueOnce(commandLineResponse(2, "GROUP"))
			.mockResolvedValueOnce(commandLineResponse(3, "RECORD CUE 2"));
		vi.stubGlobal("fetch", fetch);
		const driver = new ApiDriver("http://desk.local");
		driver.session = session();

		await expect(
			driver.setCommandLineText("RECORD CUE 2"),
		).resolves.toMatchObject({
			commandLine: { revision: 3, text: "RECORD CUE 2" },
		});
		expect(fetch).toHaveBeenCalledTimes(4);
		const firstPut = fetch.mock.calls[1]?.[1];
		const secondPut = fetch.mock.calls[3]?.[1];
		expect(new Headers(firstPut?.headers).get("if-match")).toBe("1");
		expect(new Headers(secondPut?.headers).get("if-match")).toBe("2");
		expect(JSON.parse(String(secondPut?.body))).toEqual({
			text: "RECORD CUE 2",
		});
	});
});

describe("ApiDriver canonical Playback actions", () => {
	afterEach(() => vi.unstubAllGlobals());

	it("sends exact addresses without replay IDs and retains independent control owners", async () => {
		const fetch = playbackFetch();
		vi.stubGlobal("fetch", fetch);
		const first = new ApiDriver("http://desk.local");
		const second = new ApiDriver("http://desk.local");
		first.session = session();
		second.session = session();
		await first.playbackAction({ page: 4, position: 305 }, "flash", {
			pressed: true,
		});
		await first.playbackAction({ page: 4, position: 305 }, "flash", {
			pressed: false,
		});
		await second.playbackAction({ page: 4, position: 305 }, "flash", {
			pressed: true,
		});
		const calls = fetch.mock.calls.filter(([url]) =>
			String(url).endsWith("/playback-actions"),
		);
		const bodies = calls.map(([, init]) => JSON.parse(String(init?.body)));
		expect(bodies[0]).toEqual({
			address: { kind: "playback", page: 4, position: 305 },
			action: { type: "flash", pressed: true },
			control_source: {
				surface_id: expect.stringMatching(/^bench:/),
				control_id: 305,
			},
		});
		expect(bodies[1].control_source).toEqual(bodies[0].control_source);
		expect(bodies[1].action.pressed).toBe(false);
		expect(bodies[2].control_source.surface_id).not.toBe(
			bodies[0].control_source.surface_id,
		);
		expect(new Headers(calls[0]?.[1]?.headers).get("x-tosk-desk")).toBe(
			session().desk.id,
		);
	});

	it("preserves fractional textual Cue numbers and rejects scalar cue input", async () => {
		const fetch = playbackFetch();
		vi.stubGlobal("fetch", fetch);
		const driver = new ApiDriver("http://desk.local");
		driver.session = session();
		await driver.playbackAction({ page: 4, position: 305 }, "go-to", {
			cue_number: "1.5",
		});
		const call = fetch.mock.calls.find(([url]) =>
			String(url).endsWith("/playback-actions"),
		);
		expect(JSON.parse(String(call?.[1]?.body)).action).toEqual({
			type: "go_to",
			cue_number: "1.5",
		});
		await expect(
			driver.playbackAction({ page: 4, position: 305 }, "load", {
				cue_number: 1.5,
			}),
		).rejects.toThrow("cue number is required");
		expect(
			fetch.mock.calls.filter(([url]) =>
				String(url).endsWith("/playback-actions"),
			),
		).toHaveLength(1);
	});

	it("rejects malformed runtime outcomes without retrying a live action", async () => {
		const fetch = playbackFetch({ malformed: true });
		vi.stubGlobal("fetch", fetch);
		const driver = new ApiDriver("http://desk.local");
		driver.session = session();
		await expect(
			driver.playbackAction({ page: 4, position: 305 }, "go"),
		).rejects.toThrow();
		expect(
			fetch.mock.calls.filter(([url]) =>
				String(url).endsWith("/playback-actions"),
			),
		).toHaveLength(1);
	});

	it("does not send a Playback action after its captured session is replaced", async () => {
		const driver = new ApiDriver("http://desk.local");
		driver.session = session();
		const fetch = vi.fn<typeof globalThis.fetch>(async () => {
			driver.session = { ...session(), token: "replacement" };
			return new Response(
				JSON.stringify({
					active_show: { id: "33333333-3333-4333-8333-333333333333" },
				}),
			);
		});
		vi.stubGlobal("fetch", fetch);
		await expect(
			driver.playbackAction({ page: 4, position: 305 }, "go"),
		).rejects.toThrow("session changed before");
		expect(fetch).toHaveBeenCalledTimes(1);
	});

	it("reads the authoritative visible-surface command page rather than assuming Main", async () => {
		const fetch = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValue(new Response(JSON.stringify({ command_page: 7 })));
		vi.stubGlobal("fetch", fetch);
		const driver = new ApiDriver("http://desk.local");
		driver.session = session();
		await expect(driver.playbackCommandPage()).resolves.toBe(7);
		expect(String(fetch.mock.calls[0]?.[0])).toBe(
			"http://desk.local/api/v2/playback-surfaces",
		);
	});
});

function playbackFetch(options: { malformed?: boolean } = {}) {
	return vi.fn<typeof globalThis.fetch>(async (url, _init) => {
		const showId = "33333333-3333-4333-8333-333333333333";
		if (String(url).endsWith("/bootstrap"))
			return new Response(JSON.stringify({ active_show: { id: showId } }));
		const identity = { kind: "playback", page: 4, position: 305 };
		return new Response(
			JSON.stringify(
				options.malformed
					? {}
					: {
							correlation_id: "44444444-4444-4444-8444-444444444444",
							requested: identity,
							resolved: identity,
							outcome: { status: "applied" },
							durability: "durable",
							projection: {
								scope: { show_id: showId, show_revision: 1 },
								identity,
								target: "cue_list",
								cue_list_id: "55555555-5555-4555-8555-555555555555",
								runtime: null,
							},
							related: [],
							desk: null,
							event_sequence: 1,
							desk_event_sequence: null,
						},
			),
		);
	});
}

function commandLineResponse(revision: number, text: string) {
	const commandLine: CommandLineState = {
		text,
		target: "FIXTURE",
		pristine: text.length === 0,
		revision,
		pending_choice: null,
	};
	return new Response(JSON.stringify(commandLine), {
		status: 200,
		headers: {
			"content-type": "application/json",
			etag: `"${revision}"`,
		},
	});
}

function session(): Session {
	return {
		session_id: "11111111-1111-4111-8111-111111111111",
		client_id: "client",
		token: "token",
		desk: {
			id: "22222222-2222-4222-8222-222222222222",
			name: "Desk",
		},
	};
}

describe("ApiDriver canonical portable object metadata", () => {
	it("retains kind, revision and timestamp in list and exact-object reads", async () => {
		const object = {
			kind: "group",
			id: "310",
			revision: 7,
			updated_at: "2026-10-11T06:52:00Z",
			body: { name: "Ordered" },
		};
		const driver = new ApiDriver("http://desk.local");
		const request = vi.spyOn(driver, "request");
		request
			.mockResolvedValueOnce({ objects: [object] })
			.mockResolvedValueOnce({ object });
		expect(await driver.showObjects("show", "group")).toEqual([object]);
		expect(await driver.showObject("show", "group", "310")).toEqual(object);
		expect(request.mock.calls.map((call) => [call[1], call[5]])).toEqual([
			["/api/v2/objects/group", { showId: "show" }],
			["/api/v2/objects/group/310", { showId: "show" }],
		]);
	});
});
