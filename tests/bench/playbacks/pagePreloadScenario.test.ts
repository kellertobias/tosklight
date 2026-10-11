import { afterEach, describe, expect, it, vi } from "vitest";
import { HttpPlaybackTopologyTransport } from "../../../apps/light-desktop/src/api/PlaybackTopologyTransport";
import type { ApiDriver } from "../core/api";
import {
	BrowserPages,
	BrowserPreload,
	pendingPlaybackTuples,
} from "./pagePreloadScenario";
const SHOW = "11111111-1111-4111-8111-111111111111";
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); });
describe("canonical page and Preload scenarios", () => {
	it("selects the authoritative surface page live without an intent ID", async () => {
		let page = 2;
		const request = vi.fn(
			async (method: string, path: string, body?: unknown) => {
				if (method === "GET")
					return {
						command_page: page, selected_surface_id: "screen:test",
					};
				page = (body as { page: number }).page;
				return {};
			},
		);
		const api = {
			request,
			session: { desk: { id: "desk" } },
		} as unknown as ApiDriver;
		const pages = new BrowserPages(api, {} as never, {} as never, () => SHOW);
		await pages.via.api.select(99);
		expect(
			request.mock.calls.find(([method]) => method === "POST")?.slice(0, 3),
		).toEqual([
			"POST",
			"/api/v2/playback-surfaces/screen%3Atest/page",
			{ page: 99 },
		]);
	});
	it("retains exact Temp press/release address, ordering and control source", () => {
		const source = { surface_id: "screen:left", control_id: 7 };
		const entries = [
			{
				address: { page: 1, position: 1000 },
				action: "temp-on",
				control_source: source,
			},
			{
				address: { page: 1, position: 1000 },
				action: "temp-off",
				control_source: source,
			},
			{
				address: { page: 2, position: 1000 },
				action: "go",
				control_source: { surface_id: "main", control_id: 2 },
			},
		];
		const pending = pendingPlaybackTuples(entries);
		entries[0].address.page = 99;
		source.control_id = 9;
		expect(pending).toEqual([
			[
				{ page: 1, position: 1000 },
				"temp-on",
				{ surface_id: "screen:left", control_id: 7 },
			],
			[
				{ page: 1, position: 1000 },
				"temp-off",
				{ surface_id: "screen:left", control_id: 7 },
			],
			[
				{ page: 2, position: 1000 },
				"go",
				{ surface_id: "main", control_id: 2 },
			],
		]);
	});
	it.each([
		null,
		[2, 99],
	])("writes only declared configuration and guarded all-page/restricted ranges %j", async (pages) => {
		const scope = { position_ranges: [{ start: 401, end: 430 }], pages };
		const request = vi.fn().mockResolvedValue({});
		const fetch = vi.fn(
			async (_input: RequestInfo | URL, init?: RequestInit) => {
				const body = init?.body ? JSON.parse(String(init.body)) : null;
				return new Response(
					JSON.stringify(
						body
							? {
									show_id: SHOW,
									revision: 5,
									scope,
									request_id: body.request_id,
									replayed: false,
									changed: true,
								}
							: {
									show_id: SHOW,
									revision: 4,
									scope: { position_ranges: [], pages: null },
								},
					),
				);
			},
		);
		vi.stubGlobal("fetch", fetch);
		const api = {
			request,
			baseUrl: "http://desk.local",
			session: { token: "token", desk: { id: "desk" } },
		} as unknown as ApiDriver;
		const preload = new BrowserPreload(
			api,
			{} as never,
			{} as never,
			() => SHOW,
			{} as never,
		);
		await preload.configure({
			programmer: true,
			playbackScope: scope,
			programmerFade: 10,
			cueFade: 20,
		});
		expect(request.mock.calls[0]?.slice(0, 2)).toEqual([
			"POST",
			"/api/v2/configuration/update",
		]);
		expect(request.mock.calls[0]?.[2].patch).toEqual({
			programmer_fade_millis: 10,
			sequence_master_fade_millis: 20,
			preload_programmer_changes: true,
		});
		expect(JSON.parse(String(fetch.mock.calls[1]?.[1]?.body))).toEqual({
			request_id: expect.any(String),
			expected_revision: 4,
			scope,
		});
	});
	it.each([
		0, 100, 1.5,
	])("rejects page %s before a live write", async (page) => {
		const request = vi.fn();
		const pages = new BrowserPages(
			{ request } as unknown as ApiDriver,
			{} as never,
			{} as never,
			() => SHOW,
		);
		await expect(pages.via.api.select(page)).rejects.toThrow("1–99");
		expect(request).not.toHaveBeenCalled();
	});
});

it("creates and renames sparse Pages using canonical typed topology operands", async () => {
	const apply = vi.spyOn(HttpPlaybackTopologyTransport.prototype, "apply").mockResolvedValue({} as never);
	const api = {
		baseUrl: "http://test",
		session: { token: "test", desk: { id: "desk" } },
		request: vi.fn(async () => ({ desk: { scope: { show_revision: 5 } } })),
		showObject: vi.fn(async () => ({ id: "2", revision: 3, body: { number: 2, name: "Second", assignments: {} } })),
	} as unknown as ApiDriver;
	const pages = new BrowserPages(api, {} as never, {} as never, () => SHOW);
	await pages.create(2);
	await pages.rename(2, "Second");
	expect(apply.mock.calls.map((call) => call[2].action)).toEqual([
		{ type: "create_playback_page", number: 2, name: "Page 2", expectedPageRevision: 0, expectedPageObjectId: null },
		{ type: "rename_playback_page", number: 2, name: "Second", expectedPageRevision: 3, expectedPageObjectId: "2" },
	]);
});
