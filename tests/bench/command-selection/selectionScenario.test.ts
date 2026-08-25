import { describe, expect, it, vi } from "vitest";
import type { PatchedFixture, StoredGroup } from "../../../apps/light-desktop/src/api/types";
import { ApiDriver } from "../core/api";
import {
	dereferencedGroup,
	fixture,
	group,
	groupRange,
	selectionRange,
} from "./selectionContract";
import { BrowserSelection } from "./selectionScenario";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const SESSION_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const MASTER_1 = "44444444-4444-4444-8444-444444444441";
const HEAD_1 = "44444444-4444-4444-8444-444444444442";
const HEAD_2 = "44444444-4444-4444-8444-444444444443";
const MASTER_2 = "55555555-5555-4555-8555-555555555551";
const REQUEST_ID = "66666666-6666-4666-8666-666666666666";
const CORRELATION_ID = "77777777-7777-4777-8777-777777777777";

describe("typed selection contract", () => {
	it("requires matching typed range endpoints and preserves a head qualifier", () => {
		expect(selectionRange(fixture(101, 2), fixture(105, 2))).toEqual({
			kind: "fixture_range",
			first: 101,
			last: 105,
			head: 2,
		});
		expect(() => selectionRange(fixture(1), group(2))).toThrow(
			/matching typed kinds/,
		);
		expect(() => fixture(1, -1)).toThrow(/non-negative/);
	});
});

describe("BrowserSelection API route", () => {
	it("skips absent Group IDs while preserving live and dereferenced sources", async () => {
		const harness = selectionHarness();
		await harness.selection.targets(groupRange(1, 3), dereferencedGroup(4));

		expect(harness.actions()).toEqual([
			{ action: "replace", fixtures: [], expected_revision: 8 },
			{
				action: "gesture",
				source: { type: "live_group", group_id: "1" },
				remove: false,
			},
			{
				action: "gesture",
				source: { type: "live_group", group_id: "3" },
				remove: false,
			},
			{
				action: "gesture",
				source: { type: "dereferenced_group", group_id: "4" },
				remove: false,
			},
		]);
	});

	it("resolves fixture heads without scenario UUIDs and keeps ordered chunks", async () => {
		const harness = selectionHarness();
		await harness.selection.targets(fixture(101, 2), fixture(102));
		await harness.selection.remove(fixture(101, 1));

		expect(harness.actions().slice(1)).toEqual([
			{
				action: "gesture",
				source: { type: "fixture", fixture_id: HEAD_2 },
				remove: false,
			},
			{
				action: "gesture",
				source: { type: "fixture", fixture_id: MASTER_2 },
				remove: false,
			},
			{
				action: "gesture",
				source: { type: "fixture", fixture_id: HEAD_1 },
				remove: true,
			},
		]);
	});

	it("normalizes the authoritative projection and uses Highlight stepping actions", async () => {
		const harness = selectionHarness({
			selected: [MASTER_1, HEAD_2],
			expression: {
				type: "sources",
				items: [
					{ type: "fixture", fixture_id: MASTER_1 },
					{ type: "fixture", fixture_id: HEAD_2 },
				],
			},
		});
		await expect(harness.selection.observe()).resolves.toMatchObject({
			targets: [fixture(101, 0), fixture(101, 2)],
			revision: 8,
			gestureOpen: true,
		});

		await harness.selection.next();
		await harness.selection.previous();
		await harness.selection.all();
		expect(harness.stepActions()).toEqual(["next", "previous", "all"]);
	});
});

interface HarnessOptions {
	selected?: string[];
	expression?: unknown;
}

function selectionHarness(options: HarnessOptions = {}) {
	const api = new ApiDriver("http://desk.local");
	api.session = {
		session_id: "11111111-1111-4111-8111-111111111111",
		client_id: "client",
		token: "token",
		desk: { id: DESK_ID },
	};
	vi.spyOn(api, "patch").mockResolvedValue({
		revision: 1,
		fixtures: fixtures(),
		routes: [],
	});
	vi.spyOn(api, "showObjects").mockResolvedValue(groups());
	const stepActions: string[] = [];
	vi.spyOn(api, "request").mockImplementation(async (_method, path, body) => {
		if (path !== "/api/v2/output/highlight/actions")
			throw new Error(`Unexpected ApiDriver request ${path}`);
		stepActions.push((body as { action: string }).action);
		return {
			active: false,
			mode: "selection",
			output_enabled: false,
			capture_only: false,
			remembered: [],
			active_index: null,
			active_fixture: null,
			can_previous: true,
			can_next: true,
		};
	});
	const fetchMock = vi.fn(
		async (input: RequestInfo | URL, init?: RequestInit) => {
			const url = String(input);
			if (url.endsWith("/playback-runtime/snapshot"))
				return json(playbackSnapshot());
			if (url.endsWith("/programming-interaction/snapshot"))
				return json(interactionSnapshot(options));
			if (url.endsWith("/programming-selection/actions")) {
				const action = JSON.parse(String(init?.body)).action as string;
				return json(selectionOutcome(action));
			}
			throw new Error(`Unexpected fetch ${url}`);
		},
	);
	return {
		selection: new BrowserSelection(api, {
			fetch: fetchMock as typeof fetch,
			requestId: () => REQUEST_ID,
		}),
		actions: () =>
			fetchMock.mock.calls
				.filter(([input]) =>
					String(input).endsWith("/programming-selection/actions"),
				)
				.map(([, init]) => {
					const { request_id: _, ...action } = JSON.parse(String(init?.body));
					return action;
				}),
		stepActions: () => stepActions,
	};
}

function fixtures(): PatchedFixture[] {
	return [
		{
			fixture_id: MASTER_1,
			fixture_number: 101,
			universe: 1,
			address: 1,
			definition: {} as PatchedFixture["definition"],
			logical_heads: [
				{ fixture_id: HEAD_1, head_index: 1 },
				{ fixture_id: HEAD_2, head_index: 2 },
			],
		},
		{
			fixture_id: MASTER_2,
			fixture_number: 102,
			universe: 1,
			address: 10,
			definition: {} as PatchedFixture["definition"],
			logical_heads: [],
		},
	];
}

function groups(): Array<{ id: string; body: StoredGroup; revision: number }> {
	return [
		{ id: "1", body: { fixtures: [MASTER_1] }, revision: 1 },
		{ id: "3", body: { fixtures: [] }, revision: 1 },
		{ id: "4", body: { fixtures: [HEAD_2] }, revision: 1 },
	];
}

function playbackSnapshot() {
	return {
		cursor: { sequence: 1 },
		desk: {
			scope: { show_id: SHOW_ID, show_revision: 1 },
			desk_id: DESK_ID,
			active_page: 1,
			selected_playback: null,
		},
		projections: [],
	};
}

function interactionSnapshot(options: HarnessOptions) {
	return {
		cursor: { sequence: 2 },
		projection: {
			desk_id: DESK_ID,
			command_line: {
				text: "",
				target: "FIXTURE",
				pristine: true,
				revision: 1,
				pending_choice: null,
			},
			selection: {
				selected: options.selected ?? [],
				expression: options.expression ?? null,
				revision: 8,
				gesture_open: true,
			},
		},
	};
}

function selectionOutcome(action: string) {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		action:
			action === "gesture"
				? "gesture_applied"
				: action === "select_group"
					? "group_selected"
					: "replaced",
		applied: 1,
		selection: interactionSnapshot({}).projection.selection,
		event_sequence: 3,
		replayed: false,
		warning: null,
	};
}

function json(value: unknown) {
	return new Response(JSON.stringify(value), { status: 200 });
}
