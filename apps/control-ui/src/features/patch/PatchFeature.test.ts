import { describe, expect, it, vi } from "vitest";
import type {
	PatchChange,
	PatchFixtureProjection,
	PatchMutation,
	PatchMutationOutcome,
	PatchProfileRevision,
	PatchSnapshot,
} from "./contracts";
import { HttpPatchTransport } from "../../api/PatchTransport";
import type { FixtureDefinition } from "../../api/types";
import {
	decodePatchEventServerMessage,
	decodePatchFixturesOutcome,
	decodePatchSnapshot,
} from "../../api/patchWire";
import {
	blankFixtureProfile,
	fixtureDefinitionsFromProfiles,
} from "../../components/setup/fixtureProfileModel";
import {
	createPatchDefinitionResolver,
	newPatchFixtureCandidate,
} from "./model";
import { patchedFixtureResults } from "./PatchContext";
import { PatchSession } from "./session";
import { PatchStore } from "./store";
import {
	type PatchEventObserver,
	type PatchEventStream,
	type PatchTransport,
	PatchTransportError,
} from "./transport";

const SHOW_ID = "10000000-0000-0000-0000-000000000001";
const PROFILE_ID = "20000000-0000-0000-0000-000000000001";
const MODE_ID = "30000000-0000-0000-0000-000000000001";
const FIXTURE_ID = "40000000-0000-0000-0000-000000000001";
const CORRELATION_ID = "50000000-0000-0000-0000-000000000001";

function definition(): FixtureDefinition {
	const profile = blankFixtureProfile();
	profile.id = PROFILE_ID;
	profile.revision = 3;
	profile.manufacturer = "Acme";
	profile.name = "Atomic Wash";
	profile.short_name = "Atomic Wash";
	profile.fixture_type = "wash";
	profile.modes[0].id = MODE_ID;
	profile.modes[0].name = "16 channel";
	profile.modes[0].splits = [{ number: 1, footprint: 16 }];
	return fixtureDefinitionsFromProfiles([profile])[0];
}

function profileProjection(): PatchProfileRevision {
	return {
		profileId: PROFILE_ID,
		profileRevision: 3,
		contentDigest: "digest",
		manufacturer: "Acme",
		name: "Atomic Wash",
		fixtureType: "wash",
		patchPolicy: "dmx",
		referencedModes: [
			{
				modeId: MODE_ID,
				name: "16 channel",
				splits: [{ split: 1, footprint: 16 }],
			},
		],
	};
}

function fixtureProjection(
	fixtureId = FIXTURE_ID,
	fixtureNumber = 1,
): PatchFixtureProjection {
	return {
		fixtureId,
		fixtureRevision: 1,
		fixtureNumber,
		virtualFixtureNumber: null,
		name: "Atomic Wash " + fixtureNumber,
		profileId: PROFILE_ID,
		profileRevision: 3,
		modeId: MODE_ID,
		splitPatches: [{ split: 1, universe: 1, address: fixtureNumber }],
		layerId: "default",
		directControl: null,
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logicalHeads: [
			{
				profileHeadId: "60000000-0000-0000-0000-000000000001",
				headIndex: 0,
				fixtureId: "70000000-0000-0000-0000-000000000001",
			},
		],
		multipatch: [],
		moveInBlackEnabled: true,
		moveInBlackDelayMillis: 0,
		highlightOverrides: [],
	};
}

function snapshot(
	showRevision = 1,
	patchRevision = 1,
	cursor = 10,
	fixtures: PatchFixtureProjection[] = [],
): PatchSnapshot {
	return {
		showId: SHOW_ID,
		showRevision,
		patchRevision,
		cursor,
		fixtures,
		profileRevisions: fixtures.length ? [profileProjection()] : [],
	};
}

function delta(
	showRevision: number,
	patchRevision: number,
	sequence: number,
	fixtures: PatchFixtureProjection[] = [],
): PatchChange {
	return {
		showId: SHOW_ID,
		showRevision,
		patchRevision,
		eventSequence: sequence,
		fixtures,
		removedFixtureIds: [],
		profileRevisions: fixtures.length ? [profileProjection()] : [],
	};
}

function outcome(
	requestId: string,
	showRevision = 2,
	patchRevision = 2,
	fixtures: PatchFixtureProjection[] = [],
): PatchMutationOutcome {
	return {
		requestId,
		replayed: false,
		changed: true,
		...delta(showRevision, patchRevision, 11, fixtures),
	};
}

function wireProfileProjection() {
	const profile = profileProjection();
	return {
		profile_id: profile.profileId,
		profile_revision: profile.profileRevision,
		content_digest: profile.contentDigest,
		manufacturer: profile.manufacturer,
		name: profile.name,
		fixture_type: profile.fixtureType,
		patch_policy: profile.patchPolicy,
		referenced_modes: profile.referencedModes.map((mode) => ({
			mode_id: mode.modeId,
			name: mode.name,
			splits: mode.splits,
		})),
	};
}

function wireFixtureProjection(fixtureId = FIXTURE_ID, fixtureNumber = 1) {
	const fixture = fixtureProjection(fixtureId, fixtureNumber);
	return {
		fixture_id: fixture.fixtureId,
		fixture_revision: fixture.fixtureRevision,
		fixture_number: fixture.fixtureNumber,
		virtual_fixture_number: fixture.virtualFixtureNumber,
		name: fixture.name,
		profile_id: fixture.profileId,
		profile_revision: fixture.profileRevision,
		mode_id: fixture.modeId,
		split_patches: fixture.splitPatches,
		layer_id: fixture.layerId,
		direct_control: null,
		location: fixture.location,
		rotation: fixture.rotation,
		logical_heads: fixture.logicalHeads.map((head) => ({
			profile_head_id: head.profileHeadId,
			head_index: head.headIndex,
			fixture_id: head.fixtureId,
		})),
		multipatch: [],
		move_in_black_enabled: fixture.moveInBlackEnabled,
		move_in_black_delay_millis: fixture.moveInBlackDelayMillis,
		highlight_overrides: [],
	};
}

function wireDelta(
	showRevision: number,
	patchRevision: number,
	sequence: number,
	fixtures: ReturnType<typeof wireFixtureProjection>[] = [],
) {
	return {
		show_id: SHOW_ID,
		show_revision: showRevision,
		patch_revision: patchRevision,
		event_sequence: sequence,
		fixtures,
		removed_fixture_ids: [],
		profile_revisions: fixtures.length ? [wireProfileProjection()] : [],
	};
}

function wireSnapshot(
	showRevision = 1,
	patchRevision = 1,
	cursor = 10,
	fixtures: ReturnType<typeof wireFixtureProjection>[] = [],
) {
	return {
		show_id: SHOW_ID,
		show_revision: showRevision,
		patch_revision: patchRevision,
		cursor: { sequence: cursor },
		fixtures,
		profile_revisions: fixtures.length ? [wireProfileProjection()] : [],
	};
}

function wireOutcome(
	requestId: string,
	showRevision = 2,
	patchRevision = 2,
	fixtures: ReturnType<typeof wireFixtureProjection>[] = [],
) {
	return {
		request_id: requestId,
		replayed: false,
		changed: true,
		...wireDelta(showRevision, patchRevision, 11, fixtures),
	};
}

function candidate(fixtureId = FIXTURE_ID, fixtureNumber = 1) {
	const created = newPatchFixtureCandidate({
		name: "Atomic Wash " + fixtureNumber,
		fixture_number: fixtureNumber,
		definition: definition(),
		universe: 1,
		address: fixtureNumber,
	});
	created.input.fixtureId = fixtureId;
	created.fixture.fixture_id = fixtureId;
	return created;
}

describe("Patch v2 wire boundary", () => {
	it("derives selection targets from authoritative logical heads", () => {
		expect(patchedFixtureResults([candidate()], [fixtureProjection()])).toEqual(
			[
				{
					fixtureId: FIXTURE_ID,
					selectionFixtureIds: ["70000000-0000-0000-0000-000000000001"],
				},
			],
		);
	});

	it("accepts a complete targeted snapshot and rejects unsafe revisions", () => {
		const value = wireSnapshot(7, 4, 22, [wireFixtureProjection()]);
		expect(decodePatchSnapshot(value)).toMatchObject({
			showId: SHOW_ID,
			showRevision: 7,
			patchRevision: 4,
			cursor: 22,
		});
		expect(() =>
			decodePatchSnapshot({
				...value,
				show_revision: Number.MAX_SAFE_INTEGER + 1,
			}),
		).toThrowError("$.show_revision");
	});

	it("requires a Patch event delta to carry its enclosing sequence", () => {
		const message = {
			type: "event",
			event: {
				sequence: 11,
				occurred_at: "2026-07-19T12:00:00Z",
				desk_id: null,
				class: "projection",
				object: { capability: "show", id: "patch:" + SHOW_ID },
				source: { kind: "action", source: "http" },
				correlation_id: CORRELATION_ID,
				delivery: "lossless",
				payload: {
					type: "show_patch_changed",
					delta: wireDelta(2, 2, 11, [wireFixtureProjection()]),
				},
			},
		};
		expect(decodePatchEventServerMessage(message)).toMatchObject({
			type: "event",
			sequence: 11,
			change: { showId: SHOW_ID, patchRevision: 2 },
		});
		expect(() =>
			decodePatchEventServerMessage({
				...message,
				event: {
					...message.event,
					payload: {
						type: "show_patch_changed",
						delta: wireDelta(2, 2, 12),
					},
				},
			}),
		).toThrowError("$.event.payload.delta.event_sequence");
	});

	it("rejects a changed outcome without a semantic event sequence", () => {
		const value = wireOutcome("request-1");
		expect(() =>
			decodePatchFixturesOutcome({ ...value, event_sequence: null }),
		).toThrowError("$.event_sequence");
	});
});

describe("Patch v2 network boundary", () => {
	it("calls the browser fetch implementation with its required global receiver", async () => {
		const originalFetch = globalThis.fetch;
		let receiver: unknown;
		globalThis.fetch = async function (this: unknown) {
			receiver = this;
			return new Response(JSON.stringify(wireSnapshot()), {
				status: 200,
				headers: { "content-type": "application/json" },
			});
		} as typeof fetch;

		try {
			const transport = new HttpPatchTransport({
				baseUrl: "http://desk.local",
				sessionToken: "session-token",
			});
			await transport.snapshot(SHOW_ID);
			expect(receiver).toBe(globalThis);
		} finally {
			globalThis.fetch = originalFetch;
		}
	});

	it.each([
		1, 4,
	])("sends one atomic request for a batch of %i fixture(s) and no unrelated reads", async (count) => {
		const fetchMock = vi.fn(
			async (_input: RequestInfo | URL, init?: RequestInit) => {
				const body = JSON.parse(String(init?.body)) as {
					request_id: string;
					fixtures: unknown[];
				};
				return new Response(JSON.stringify(wireOutcome(body.request_id)), {
					status: 200,
					headers: { "content-type": "application/json" },
				});
			},
		);
		const transport = new HttpPatchTransport({
			baseUrl: "http://desk.local",
			sessionToken: "session-token",
			fetch: fetchMock as typeof fetch,
		});
		const inputs = Array.from({ length: count }, (_, index) => {
			const item = candidate(
				"40000000-0000-0000-0000-" + String(index + 1).padStart(12, "0"),
				index + 1,
			);
			return item.input;
		});

		await transport.patchFixtures(SHOW_ID, 7, {
			requestId: "request-" + count,
			fixtures: inputs,
			removeFixtureIds: [],
		});

		expect(fetchMock).toHaveBeenCalledOnce();
		const [url, init] = fetchMock.mock.calls[0];
		expect(String(url)).toBe(
			"http://desk.local/api/v2/shows/" + SHOW_ID + "/patch/fixtures",
		);
		expect(JSON.parse(String(init?.body)).fixtures).toHaveLength(count);
		expect((init?.headers as Headers).get("if-match")).toBe("7");
		expect(String(url)).not.toMatch(
			/\/bootstrap|\/playbacks|\/shows$|\/configuration|\/media-servers|\/fixture-library|\/fixture-profiles/,
		);
	});

	it("subscribes only to the active show's Patch projection and closes with the view", () => {
		const sockets: FakeWebSocket[] = [];
		class CapturingWebSocket extends FakeWebSocket {
			constructor(url: string | URL, protocols?: string | string[]) {
				super(url, protocols);
				sockets.push(this);
			}
		}
		const transport = new HttpPatchTransport({
			baseUrl: "http://desk.local",
			sessionToken: "session-token",
			webSocket: CapturingWebSocket as unknown as typeof WebSocket,
		});
		const observer: PatchEventObserver = {
			message: vi.fn(),
			error: vi.fn(),
			closed: vi.fn(),
		};

		const stream = transport.subscribe(SHOW_ID, 19, observer);
		sockets[0].emit("open", new Event("open"));

		expect(JSON.parse(sockets[0].sent[0])).toEqual({
			type: "subscribe",
			filter: {
				capabilities: ["show"],
				classes: ["projection"],
				objects: [{ capability: "show", id: "patch:" + SHOW_ID }],
			},
			after_sequence: 19,
			capacity: 128,
			rate_limits: [],
		});
		stream.close();
		expect(sockets[0].close).toHaveBeenCalledOnce();
	});
});

describe("Patch optimistic store", () => {
	const resolve = createPatchDefinitionResolver([definition()]);

	it("shows the complete pending batch immediately, then reconciles authoritatively", () => {
		const store = new PatchStore(SHOW_ID, resolve);
		store.applySnapshot(snapshot());
		const candidates = [
			candidate(FIXTURE_ID, 1),
			candidate("40000000-0000-0000-0000-000000000002", 2),
		];
		store.begin("request-batch", candidates, []);

		expect(store.getSnapshot().fixtures).toHaveLength(2);
		expect(store.getSnapshot().pendingFixtureIds).toEqual(
			new Set(candidates.map((item) => item.fixture.fixture_id)),
		);

		store.applyOutcome(
			"request-batch",
			outcome("request-batch", 2, 2, [
				fixtureProjection(FIXTURE_ID, 1),
				fixtureProjection("40000000-0000-0000-0000-000000000002", 2),
			]),
		);
		expect(store.getSnapshot()).toMatchObject({
			showRevision: 2,
			patchRevision: 2,
			cursor: 11,
		});
		expect(store.getSnapshot().pendingFixtureIds.size).toBe(0);
		expect(store.getSnapshot().fixtures[0].logical_heads[0]).toEqual({
			profile_head_id: "60000000-0000-0000-0000-000000000001",
			head_index: 0,
			fixture_id: "70000000-0000-0000-0000-000000000001",
		});
	});

	it("reconciles each queued write with its own optimistic definition", () => {
		const store = new PatchStore(SHOW_ID, () => null);
		store.applySnapshot(snapshot());
		const first = candidate(FIXTURE_ID, 1);
		first.fixture.definition = {
			...first.fixture.definition,
			name: "First request definition",
		};
		const second = candidate(FIXTURE_ID, 2);
		second.fixture.definition = {
			...second.fixture.definition,
			name: "Second request definition",
		};
		store.begin("request-first", [first], []);
		store.begin("request-second", [second], []);

		store.applyOutcome(
			"request-first",
			outcome("request-first", 2, 2, [fixtureProjection(FIXTURE_ID, 1)]),
		);
		store.rollback("request-second", new Error("second request failed"));

		expect(store.getSnapshot().fixtures[0].definition.name).toBe(
			"First request definition",
		);
	});

	it("rolls back only the failed overlay and preserves a newer external delta", () => {
		const store = new PatchStore(SHOW_ID, resolve);
		store.applySnapshot(snapshot());
		store.begin("request-local", [candidate(FIXTURE_ID, 1)], []);
		expect(
			store.applyDelta(
				delta(2, 2, 11, [
					fixtureProjection("40000000-0000-0000-0000-000000000009", 9),
				]),
			),
		).toBe("applied");

		store.rollback("request-local", new Error("network failed"));

		expect(
			store.getSnapshot().fixtures.map((fixture) => fixture.fixture_id),
		).toEqual(["40000000-0000-0000-0000-000000000009"]);
		expect(store.getSnapshot().error).toBe("network failed");
	});

	it("requests repair when a Patch revision is skipped", () => {
		const store = new PatchStore(SHOW_ID, resolve);
		store.applySnapshot(snapshot());
		expect(store.applyDelta(delta(4, 3, 14))).toBe("repair");
		expect(store.getSnapshot().patchRevision).toBe(1);
	});

	it("requests repair when an advanced Patch revision has an old event cursor", () => {
		const store = new PatchStore(SHOW_ID, resolve);
		store.applySnapshot(snapshot(1, 1, 20));
		expect(store.applyDelta(delta(2, 2, 19))).toBe("repair");
		expect(store.getSnapshot().patchRevision).toBe(1);
	});

	it("leaves the authoritative projection unchanged when reconciliation fails", () => {
		const store = new PatchStore(SHOW_ID, resolve);
		store.applySnapshot(snapshot(1, 1, 20, [fixtureProjection()]));
		const before = store.getSnapshot();
		const malformed = delta(2, 2, 21, [
			{
				...fixtureProjection(),
				profileId: "80000000-0000-0000-0000-000000000001",
			},
		]);

		expect(() => store.applyDelta(malformed)).toThrowError(
			"references missing profile",
		);
		expect(store.getSnapshot()).toEqual(before);
	});
});

describe("Patch session repair lifecycle", () => {
	it("does no snapshot or socket work before the first Patch view mounts", async () => {
		const transport = new FakePatchTransport([snapshot()]);
		const session = patchSession(transport);

		expect(transport.snapshot).not.toHaveBeenCalled();
		expect(transport.subscribe).not.toHaveBeenCalled();

		const release = session.activate();
		await vi.waitFor(() => expect(transport.snapshot).toHaveBeenCalledOnce());
		await vi.waitFor(() =>
			expect(session.store.getSnapshot().status).toBe("ready"),
		);
		expect(transport.subscribe).toHaveBeenCalledOnce();
		release();
		await Promise.resolve();
	});

	it("hides retained authority and refuses writes across a view restart", async () => {
		const transport = new FakePatchTransport([
			snapshot(1, 1, 10, [fixtureProjection()]),
			snapshot(2, 1, 12, [fixtureProjection()]),
		]);
		const session = patchSession(transport);
		const releaseFirst = session.activate();
		await vi.waitFor(() =>
			expect(session.store.getSnapshot().status).toBe("ready"),
		);

		releaseFirst();
		await Promise.resolve();
		expect(session.store.getSnapshot()).toMatchObject({
			status: "loading",
			fixtures: [],
		});

		const releaseSecond = session.activate();
		expect(session.store.getSnapshot()).toMatchObject({
			status: "loading",
			fixtures: [],
		});
		await expect(
			session.updateFixture(FIXTURE_ID, { name: "Stale edit" }),
		).rejects.toThrow(
			"Patch authority changed before the mutation completed",
		);
		await vi.waitFor(() =>
			expect(session.store.getSnapshot().status).toBe("ready"),
		);
		expect(transport.snapshot).toHaveBeenCalledTimes(2);
		releaseSecond();
		await Promise.resolve();
	});

	it("drops a late mutation outcome after the final Patch view releases", async () => {
		const transport = new FakePatchTransport([
			snapshot(1, 1, 10, [fixtureProjection()]),
		]);
		let complete!: (outcome: PatchMutationOutcome) => void;
		transport.patchFixtures.mockImplementation(
			() =>
				new Promise((resolve) => {
					complete = resolve;
				}),
		);
		const session = patchSession(transport);
		const release = session.activate();
		await vi.waitFor(() =>
			expect(session.store.getSnapshot().status).toBe("ready"),
		);

		const write = session.updateFixture(FIXTURE_ID, { name: "Late name" });
		await vi.waitFor(() =>
			expect(transport.patchFixtures).toHaveBeenCalledOnce(),
		);
		const request = transport.patchFixtures.mock.calls[0][2];
		release();
		await Promise.resolve();
		complete(outcome(request.requestId, 2, 2, [fixtureProjection()]));

		await expect(write).rejects.toThrow(
			"Patch authority changed before the mutation completed",
		);
		expect(session.store.getSnapshot()).toMatchObject({
			status: "loading",
			fixtures: [],
			pendingFixtureIds: new Set(),
		});
	});

	it("restarts with a fresh snapshot and subscription after cleanup", async () => {
		const transport = new FakePatchTransport([snapshot(), snapshot(2, 1, 12)]);
		const session = new PatchSession({
			showId: SHOW_ID,
			transport,
			resolveDefinition: createPatchDefinitionResolver([definition()]),
		});

		await session.start();
		session.stop();
		await session.start();

		expect(transport.snapshot).toHaveBeenCalledTimes(2);
		expect(transport.subscribe).toHaveBeenCalledTimes(2);
		session.stop();
	});

	it("repairs a reported stream gap with one targeted snapshot GET", async () => {
		const transport = new FakePatchTransport([
			snapshot(1, 1, 10),
			snapshot(3, 2, 24),
		]);
		const session = new PatchSession({
			showId: SHOW_ID,
			transport,
			resolveDefinition: createPatchDefinitionResolver([definition()]),
		});
		await session.start();

		transport.observer?.message({
			type: "gap",
			afterSequence: 10,
			oldestAvailable: 20,
			latestSequence: 24,
		});

		await vi.waitFor(() => expect(transport.snapshot).toHaveBeenCalledTimes(2));
		expect(session.store.getSnapshot()).toMatchObject({
			showRevision: 3,
			patchRevision: 2,
			cursor: 24,
		});
		expect(transport.repair).toHaveBeenCalledWith(24);
		session.stop();
	});

	it("retries an ambiguous response loss with the same request identity", async () => {
		const recovered = {
			...fixtureProjection(),
			name: "Recovered name",
		};
		const transport = new FakePatchTransport([
			snapshot(7, 3, 10, [fixtureProjection()]),
			snapshot(8, 4, 11, [recovered]),
		]);
		transport.patchFixtures.mockImplementation(
			async (_showId, _revision, mutation) => {
				if (transport.patchFixtures.mock.calls.length === 1)
					throw new TypeError("response connection closed");
				return outcome(mutation.requestId, 8, 4, [recovered]);
			},
		);
		const session = patchSession(transport);
		await session.start();

		await session.updateFixture(FIXTURE_ID, { name: "Recovered name" });

		expect(transport.patchFixtures).toHaveBeenCalledTimes(2);
		const attempts = transport.patchFixtures.mock.calls;
		expect(attempts[0][1]).toBe(3);
		expect(attempts[1][1]).toBe(3);
		expect(attempts[0][2]).toEqual(attempts[1][2]);
		expect(session.store.getSnapshot().fixtures[0].name).toBe("Recovered name");
		expect(session.store.getSnapshot().pendingFixtureIds.size).toBe(0);
		session.stop();
	});

	it("settles optimistic state from authoritative no-change and replay outcomes", async () => {
		const transport = new FakePatchTransport([
			snapshot(1, 1, 10, [fixtureProjection()]),
		]);
		transport.patchFixtures.mockImplementation(
			async (_showId, _revision, mutation) => ({
				requestId: mutation.requestId,
				replayed: true,
				changed: false,
				showId: SHOW_ID,
				showRevision: 1,
				patchRevision: 1,
				eventSequence: null,
				fixtures: [fixtureProjection()],
				removedFixtureIds: [],
				profileRevisions: [profileProjection()],
			}),
		);
		const session = patchSession(transport);
		await session.start();

		const result = await session.updateFixture(FIXTURE_ID, {
			name: "No-op optimistic name",
		});

		expect(result).toMatchObject({ replayed: true, changed: false });
		expect(session.store.getSnapshot().fixtures[0].name).toBe("Atomic Wash 1");
		expect(session.store.getSnapshot().pendingFixtureIds.size).toBe(0);
		session.stop();
	});

	it("repairs a revision conflict and rebases one typed Patch action", async () => {
		const repaired = {
			...fixtureProjection(),
			name: "External name",
		};
		const accepted = {
			...repaired,
			location: { x: 10, y: 0, z: 0 },
		};
		const transport = new FakePatchTransport([
			snapshot(1, 1, 10, [fixtureProjection()]),
			snapshot(2, 2, 11, [repaired]),
		]);
		transport.patchFixtures.mockImplementation(
			async (_showId, _revision, mutation) => {
				if (transport.patchFixtures.mock.calls.length === 1)
					throw new PatchTransportError("revision conflict", 409, 2, false);
				return {
					...outcome(mutation.requestId, 3, 3, [accepted]),
					eventSequence: 12,
				};
			},
		);
		const session = patchSession(transport);
		await session.start();

		await session.updateFixture(FIXTURE_ID, {
			location: { x: 10, y: 0, z: 0 },
		});

		expect(transport.patchFixtures).toHaveBeenCalledTimes(2);
		expect(transport.patchFixtures.mock.calls.map((call) => call[1])).toEqual([
			1, 2,
		]);
		expect(transport.patchFixtures.mock.calls[1][2].fixtures[0]).toMatchObject({
			name: "External name",
			location: { x: 10, y: 0, z: 0 },
		});
		expect(session.store.getSnapshot().fixtures[0].name).toBe("External name");
		session.stop();
	});

	it("rebases a later partial write after an earlier write rolls back", async () => {
		const layered = {
			...fixtureProjection(),
			layerId: "layer-2",
		};
		const transport = new FakePatchTransport([
			snapshot(1, 1, 10, [fixtureProjection()]),
		]);
		transport.patchFixtures.mockImplementation(
			async (_showId, _revision, mutation) => {
				if (transport.patchFixtures.mock.calls.length === 1)
					throw new PatchTransportError("rejected", 400, null, false);
				return outcome(mutation.requestId, 2, 2, [layered]);
			},
		);
		const session = patchSession(transport);
		await session.start();

		const rejected = session.updateFixture(FIXTURE_ID, {
			name: "Rejected name",
		});
		const accepted = session.updateFixture(FIXTURE_ID, {
			layer_id: "layer-2",
		});
		await expect(rejected).rejects.toThrow("rejected");
		await accepted;

		const second = transport.patchFixtures.mock.calls[1][2];
		expect(second.fixtures[0]).toMatchObject({
			name: "Atomic Wash 1",
			layerId: "layer-2",
		});
		expect(transport.snapshot).toHaveBeenCalledOnce();
		session.stop();
	});
});

function patchSession(transport: PatchTransport) {
	return new PatchSession({
		showId: SHOW_ID,
		transport,
		resolveDefinition: createPatchDefinitionResolver([definition()]),
	});
}

class FakeWebSocket {
	static readonly OPEN = 1;
	readonly sent: string[] = [];
	readyState = FakeWebSocket.OPEN;
	close = vi.fn();
	private listeners = new Map<string, Array<(event: Event) => void>>();

	constructor(
		readonly url: string | URL,
		readonly protocols?: string | string[],
	) {}

	addEventListener(type: string, listener: EventListenerOrEventListenerObject) {
		const callback =
			typeof listener === "function"
				? listener
				: (event: Event) => listener.handleEvent(event);
		this.listeners.set(type, [...(this.listeners.get(type) ?? []), callback]);
	}

	send(value: string) {
		this.sent.push(value);
	}

	emit(type: string, event: Event) {
		for (const listener of this.listeners.get(type) ?? []) listener(event);
	}
}

class FakePatchTransport implements PatchTransport {
	observer: PatchEventObserver | null = null;
	repair = vi.fn();
	snapshot: ReturnType<typeof vi.fn<() => Promise<PatchSnapshot>>>;

	constructor(snapshots: PatchSnapshot[]) {
		this.snapshot = vi.fn(async () => {
			const next = snapshots.shift();
			if (!next) throw new Error("No fake Patch snapshot remains");
			return next;
		});
	}

	patchFixtures = vi.fn(
		(
			_showId: string,
			_expectedRevision: number,
			_mutation: PatchMutation,
		): Promise<PatchMutationOutcome> => Promise.reject(new Error("Not used")),
	);

	subscribe = vi.fn(
		(
			_showId: string,
			_afterSequence: number,
			observer: PatchEventObserver,
		): PatchEventStream => {
			this.observer = observer;
			return { repair: this.repair, close: vi.fn() };
		},
	);
}
