import { describe, expect, it, vi } from "vitest";
import { ApiDriver } from "./api";
import { recallPreset } from "./presetRecall";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const USER_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const FIXTURE_ID = "44444444-4444-4444-8444-444444444444";
const REQUEST_ID = "55555555-5555-4555-8555-555555555555";
const CORRELATION_ID = "66666666-6666-4666-8666-666666666666";

describe("Preset recall acceptance intent", () => {
	it("captures exact scoped authority and performs one production v2 recall", async () => {
		const calls: Array<{ url: string; init?: RequestInit }> = [];
		const fetchMock = vi.fn(
			async (input: RequestInfo | URL, init?: RequestInit) => {
				const url = String(input);
				calls.push({ url, init });
				return responseFor(url, init);
			},
		);
		const outcome = await recallPreset(
			api(),
			{
				surface: "api",
				showId: SHOW_ID,
				preset: { objectId: "2.1", family: "Color", number: 1 },
			},
			{ fetch: fetchMock as typeof fetch, requestId: () => REQUEST_ID },
		);

		expect(outcome).toMatchObject({
			status: "no_change",
			replayed: true,
			preset: { id: "2.1", revision: 4, body: presetBody() },
			appliedFixtures: 1,
		});
		expect(calls).toHaveLength(5);
		expect(calls.filter((call) => call.init?.method === "POST")).toHaveLength(
			1,
		);
		expect(calls.map((call) => call.url)).toEqual(
			expect.arrayContaining([
				`http://desk.local/api/v1/shows/${SHOW_ID}/objects/preset/2.1`,
				`http://desk.local/api/v2/users/${USER_ID}/programmer-values/snapshot`,
				`http://desk.local/api/v2/users/${USER_ID}/programmer-capture-mode/snapshot`,
				`http://desk.local/api/v2/desks/${DESK_ID}/programming-interaction/snapshot`,
				`http://desk.local/api/v2/shows/${SHOW_ID}/presets/recall`,
			]),
		);
		expect(
			calls.some((call) => /bootstrap|playbacks|programmers/.test(call.url)),
		).toBe(false);
		const action = calls.find((call) => call.init?.method === "POST");
		expect(JSON.parse(String(action?.init?.body))).toEqual({
			request_id: REQUEST_ID,
			address: { family: "color", number: 1 },
			expected_preset_revision: 4,
			expected_show_revision: 12,
			expected_programmer_revision: 6,
			expected_capture_mode_revision: 3,
			expected_selection_revision: 8,
		});
	});

	it("rejects a mismatched exact object before the recall mutation", async () => {
		const fetchMock = vi.fn(
			async (input: RequestInfo | URL, init?: RequestInit) => {
				const response = responseFor(String(input), init);
				if (String(input).includes("/objects/preset/"))
					return json(
						{
							...presetObject(),
							body: { ...presetBody(), family: "Beam" },
						},
						200,
						{ "x-light-show-revision": '"12"' },
					);
				return response;
			},
		);
		await expect(
			recallPreset(
				api(),
				{
					surface: "api",
					showId: SHOW_ID,
					preset: { objectId: "2.1", family: "Color", number: 1 },
				},
				{ fetch: fetchMock as typeof fetch, requestId: () => REQUEST_ID },
			),
		).rejects.toThrow(/body family Beam does not match Color/);
		expect(
			fetchMock.mock.calls.filter(([, init]) => init?.method === "POST"),
		).toHaveLength(0);
	});

	it("propagates typed production conflict metadata", async () => {
		const fetchMock = vi.fn(
			async (input: RequestInfo | URL, init?: RequestInit) => {
				if (init?.method === "POST")
					return json(
						{
							kind: "conflict",
							error: "selection revision conflict",
							current_revision: 6,
							current_related_revision: 9,
							retryable: false,
						},
						409,
					);
				return responseFor(String(input), init);
			},
		);
		await expect(
			recallPreset(
				api(),
				{
					surface: "api",
					showId: SHOW_ID,
					preset: { objectId: "2.1", family: "Color", number: 1 },
				},
				{ fetch: fetchMock as typeof fetch, requestId: () => REQUEST_ID },
			),
		).rejects.toMatchObject({
			name: "PresetRecallTransportError",
			kind: "conflict",
			status: 409,
			currentRevision: 6,
			currentRelatedRevision: 9,
			retryable: false,
		});
	});
});

function api() {
	const driver = new ApiDriver("http://desk.local");
	driver.session = {
		session_id: "session",
		client_id: "client",
		token: "token",
		user: { id: USER_ID, name: "Operator" },
		desk: { id: DESK_ID, osc_alias: "main" },
	};
	return driver;
}

function responseFor(url: string, init?: RequestInit) {
	if (url.includes("/objects/preset/"))
		return json(presetObject(), 200, { "x-light-show-revision": '"12"' });
	if (url.includes("programmer-values/snapshot"))
		return json(valuesSnapshot(), 200);
	if (url.includes("programmer-capture-mode/snapshot"))
		return json(captureModeSnapshot(), 200);
	if (url.includes("programming-interaction/snapshot"))
		return json(interactionSnapshot(), 200);
	if (init?.method === "POST") return json(noChangeOutcome(), 200);
	throw new Error(`Unexpected request ${url}`);
}

function presetBody() {
	return {
		name: "Color one",
		family: "Color",
		number: 1,
		values: {
			[FIXTURE_ID]: {
				"color.red": { kind: "normalized", value: 1 },
			},
		},
		group_values: {},
	};
}

function presetObject() {
	return {
		kind: "preset",
		id: "2.1",
		revision: 4,
		updated_at: "2026-07-21T10:00:00Z",
		body: presetBody(),
	};
}

function valuesSnapshot() {
	return {
		cursor: { sequence: 30 },
		projection: {
			user_id: USER_ID,
			revision: 6,
			fixture_values: [],
			group_values: [],
		},
	};
}

function captureModeSnapshot() {
	return {
		cursor: { sequence: 31 },
		projection: {
			user_id: USER_ID,
			revision: 3,
			blind: false,
			preview: false,
			preload_capture_programmer: false,
		},
	};
}

function interactionSnapshot() {
	return {
		cursor: { sequence: 32 },
		projection: {
			desk_id: DESK_ID,
			command_line: {
				text: "",
				target: "FIXTURE",
				pristine: true,
				revision: 2,
				pending_choice: null,
			},
			selection: {
				selected: [FIXTURE_ID],
				expression: null,
				revision: 8,
				gesture_open: false,
			},
		},
	};
}

function noChangeOutcome() {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		replayed: true,
		show_revision: 12,
		programmer_revision: 6,
		capture_mode_revision: 3,
		selection_revision: 8,
		applied_fixtures: 1,
		active_context: "preset:2.1",
		preset: { id: "2.1", revision: 4, body: presetBody() },
		status: "no_change",
	};
}

function json(
	value: unknown,
	status: number,
	headers?: Record<string, string>,
) {
	return new Response(JSON.stringify(value), { status, headers });
}
