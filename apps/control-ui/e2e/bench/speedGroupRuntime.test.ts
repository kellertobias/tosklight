import { describe, expect, it, vi } from "vitest";
import type {
	SpeedGroupAction,
	SpeedGroupId,
} from "../../src/features/speedGroupRuntime/contracts";
import type { SpeedGroupTransportError } from "../../src/features/speedGroupRuntime/transport";
import { ApiDriver, type Session } from "./api";
import { applySpeedGroupRuntimeAction } from "./speedGroupRuntime";

const USER_ID = "11111111-1111-4111-8111-111111111111";
const DESK_ID = "22222222-2222-4222-8222-222222222222";
const OTHER_DESK_ID = "33333333-3333-4333-8333-333333333333";
const AUTHORITY_ID = "44444444-4444-4444-8444-444444444444";
const OTHER_AUTHORITY_ID = "55555555-5555-4555-8555-555555555555";
const REQUEST_ID = "speed-command-request";
const CORRELATION_ID = "66666666-6666-4666-8666-666666666666";

const actionCases = [
	{
		name: "absolute BPM",
		action: { type: "set_bpm", group: "A", bpm: 128.5 },
		wire: { type: "set_bpm", group: "A", bpm: 128.5 },
	},
	{
		name: "relative BPM",
		action: { type: "adjust_bpm", group: "B", deltaBpm: -5 },
		wire: { type: "adjust_bpm", group: "B", delta_bpm: -5 },
	},
	{
		name: "synchronization",
		action: { type: "synchronize", source: "A", target: "C" },
		wire: { type: "synchronize", source: "A", target: "C" },
	},
] as const satisfies readonly {
	name: string;
	action: SpeedGroupAction;
	wire: Record<string, unknown>;
}[];

describe("Speed Group runtime acceptance intent", () => {
	it.each(
		actionCases,
	)("sends one strict $name action against fresh authority", async ({
		action,
		wire,
	}) => {
		const fetchMock = speedGroupFetch({
			actionResponses: [json(changedOutcome(action))],
		});
		const outcome = await applySpeedGroupRuntimeAction(
			api(),
			{ surface: "api", action },
			dependencies(fetchMock),
		);

		expect(outcome).toMatchObject({
			status: "changed",
			requestId: REQUEST_ID,
			authorityId: AUTHORITY_ID,
			revision: 5,
			eventSequence: 19,
		});
		assertNarrowCalls(fetchMock, wire);
	});

	it("accepts replayed no-change and retries once with the identical request", async () => {
		const noChangeAction = {
			type: "set_bpm",
			group: "A",
			bpm: 120,
		} as const;
		const replayFetch = speedGroupFetch({
			actionResponses: [json(noChangeOutcome(noChangeAction))],
		});
		await expect(
			applySpeedGroupRuntimeAction(
				api(),
				{ surface: "api", action: noChangeAction },
				dependencies(replayFetch),
			),
		).resolves.toMatchObject({
			status: "no_change",
			replayed: true,
			eventSequence: null,
			revision: 4,
		});
		expect(actionCalls(replayFetch)).toHaveLength(1);

		const retryFetch = speedGroupFetch({
			actionResponses: [
				json(
					{
						kind: "unavailable",
						error: "connection interrupted",
						current_revision: 4,
						retryable: true,
					},
					503,
				),
				json(changedOutcome(actionCases[0].action)),
			],
		});
		await expect(
			applySpeedGroupRuntimeAction(
				api(),
				{ surface: "api", action: actionCases[0].action },
				dependencies(retryFetch),
			),
		).resolves.toMatchObject({ status: "changed", requestId: REQUEST_ID });
		const bodies = actionCalls(retryFetch).map(([, init]) => init?.body);
		expect(bodies).toHaveLength(2);
		expect(bodies[1]).toBe(bodies[0]);
	});

	it("surfaces a typed revision conflict without retrying", async () => {
		const fetchMock = speedGroupFetch({
			actionResponses: [
				json(
					{
						kind: "conflict",
						error: "Speed Group revision conflict",
						current_revision: 6,
						retryable: false,
					},
					409,
				),
			],
		});

		await expect(
			applySpeedGroupRuntimeAction(
				api(),
				{ surface: "api", action: actionCases[0].action },
				dependencies(fetchMock),
			),
		).rejects.toEqual(
			expect.objectContaining<Partial<SpeedGroupTransportError>>({
				name: "SpeedGroupTransportError",
				kind: "conflict",
				status: 409,
				currentRevision: 6,
				retryable: false,
			}),
		);
		expect(actionCalls(fetchMock)).toHaveLength(1);
	});

	it("rejects malformed snapshots and foreign action authority", async () => {
		const malformedFetch = speedGroupFetch({
			snapshot: { ...speedGroupSnapshot(), legacy_projection: {} },
		});
		await expect(
			applySpeedGroupRuntimeAction(
				api(),
				{ surface: "api", action: actionCases[0].action },
				dependencies(malformedFetch),
			),
		).rejects.toThrow(/legacy_projection.*declared wire field/);
		expect(actionCalls(malformedFetch)).toHaveLength(0);

		const foreignFetch = speedGroupFetch({
			actionResponses: [
				json({
					...changedOutcome(actionCases[0].action),
					authority_id: OTHER_AUTHORITY_ID,
				}),
			],
		});
		await expect(
			applySpeedGroupRuntimeAction(
				api(),
				{ surface: "api", action: actionCases[0].action },
				dependencies(foreignFetch),
			),
		).rejects.toThrow(/authority_id/);
		expect(actionCalls(foreignFetch)).toHaveLength(1);
	});

	it("rejects scope replacement before POST and after a late response", async () => {
		const beforeDriver = api();
		const beforeFetch = speedGroupFetch({
			onSnapshot: () => replaceSession(beforeDriver, OTHER_DESK_ID),
		});
		await expect(
			applySpeedGroupRuntimeAction(
				beforeDriver,
				{ surface: "api", action: actionCases[0].action },
				dependencies(beforeFetch),
			),
		).rejects.toThrow(/session changed/);
		expect(actionCalls(beforeFetch)).toHaveLength(0);

		const lateDriver = api();
		let resolveAction!: (response: Response) => void;
		const lateFetch = vi.fn(
			async (_input: RequestInfo | URL, init?: RequestInit) => {
				if (init?.method !== "POST") return json(speedGroupSnapshot());
				return new Promise<Response>((resolve) => {
					resolveAction = resolve;
				});
			},
		);
		const pending = applySpeedGroupRuntimeAction(
			lateDriver,
			{ surface: "api", action: actionCases[0].action },
			dependencies(lateFetch),
		);
		await vi.waitFor(() => expect(actionCalls(lateFetch)).toHaveLength(1));
		replaceSession(lateDriver, OTHER_DESK_ID);
		resolveAction(json(changedOutcome(actionCases[0].action)));
		await expect(pending).rejects.toThrow(/session changed/);
	});
});

interface SpeedGroupFetchOptions {
	snapshot?: unknown;
	actionResponses?: Response[];
	onSnapshot?: () => void;
}

function speedGroupFetch(options: SpeedGroupFetchOptions = {}) {
	let actionIndex = 0;
	return vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => {
		if (init?.method === "POST") {
			const response = options.actionResponses?.[actionIndex];
			actionIndex += 1;
			if (!response) return json(changedOutcome(actionCases[0].action));
			return response;
		}
		options.onSnapshot?.();
		return json(options.snapshot ?? speedGroupSnapshot());
	});
}

function assertNarrowCalls(
	fetchMock: ReturnType<typeof speedGroupFetch>,
	expectedAction: Record<string, unknown>,
) {
	const urls = fetchMock.mock.calls.map(([input]) => String(input));
	const expected = `http://desk.local/api/v2/desks/${DESK_ID}/speed-groups`;
	expect(urls).toEqual([expected, expected]);
	expect(
		urls.some((url) =>
			/\/api\/v1|bootstrap|configuration|playbacks|sound/u.test(url),
		),
	).toBe(false);
	for (const [, init] of fetchMock.mock.calls)
		expect((init?.headers as Headers).get("authorization")).toBe(
			"Bearer token",
		);
	const [, action] = fetchMock.mock.calls;
	expect(action?.[1]?.method).toBe("POST");
	expect(JSON.parse(String(action?.[1]?.body))).toEqual({
		request_id: REQUEST_ID,
		expected_authority_id: AUTHORITY_ID,
		expected_revision: 4,
		action: expectedAction,
	});
	expect(actionCalls(fetchMock)).toHaveLength(1);
}

function actionCalls(fetchMock: ReturnType<typeof vi.fn>) {
	return fetchMock.mock.calls.filter(([, init]) => init?.method === "POST");
}

function dependencies(fetchMock: ReturnType<typeof vi.fn>) {
	return {
		fetch: fetchMock as typeof globalThis.fetch,
		requestId: () => REQUEST_ID,
	};
}

function api() {
	const driver = new ApiDriver("http://desk.local");
	driver.session = session();
	return driver;
}

function session(deskId = DESK_ID): Session {
	return {
		session_id: "session",
		client_id: "client",
		token: "token",
		user: { id: USER_ID, name: "Operator" },
		desk: { id: deskId, osc_alias: "main" },
	};
}

function replaceSession(driver: ApiDriver, deskId = DESK_ID) {
	driver.session = { ...session(deskId), token: "replacement-token" };
}

function speedGroupSnapshot() {
	return {
		cursor: { sequence: 18 },
		projection: {
			authority_id: AUTHORITY_ID,
			revision: 4,
			groups: (["A", "B", "C", "D", "E"] as const).map((group) =>
				wireGroup(group),
			),
		},
	};
}

function changedOutcome(action: SpeedGroupAction) {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		authority_id: AUTHORITY_ID,
		revision: 5,
		applied_at_millis: 200,
		groups: outcomeGroups(action),
		status: "changed",
		event_sequence: 19,
		replayed: false,
		durability: "durable",
	};
}

function noChangeOutcome(action: SpeedGroupAction) {
	return {
		...changedOutcome(action),
		revision: 4,
		status: "no_change",
		event_sequence: undefined,
		replayed: true,
	};
}

function outcomeGroups(action: SpeedGroupAction) {
	if (action.type === "set_bpm")
		return [wireGroup(action.group, { manual_bpm: action.bpm })];
	if (action.type === "adjust_bpm")
		return [wireGroup(action.group, { manual_bpm: 120 + action.deltaBpm })];
	return [
		wireGroup(action.source, { synchronized_with: action.target }),
		wireGroup(action.target, { synchronized_with: action.source }),
	].sort((left, right) => left.group.localeCompare(right.group));
}

function wireGroup(
	group: SpeedGroupId,
	overrides: Record<string, unknown> = {},
) {
	return {
		group,
		manual_bpm: 120,
		paused: false,
		speed_master_scale: 1,
		synchronized_with: null,
		phase_origin_millis: 100,
		...overrides,
	};
}

function json(value: unknown, status = 200) {
	return new Response(JSON.stringify(value), { status });
}
