import { describe, expect, it, vi } from "vitest";
import type {
	ShowObject,
	ShowObjectKind,
	ShowObjectsEventMessage,
} from "./contracts";
import { ShowObjectsSession } from "./session";
import { ShowObjectsStore } from "./store";
import type {
	ShowObjectsEventObserver,
	ShowObjectsEventScope,
	ShowObjectsEventTransport,
} from "./transport";
import { ShowObjectsProtocolError } from "./transport";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

function group(revision: number, name: string): ShowObject<"group"> {
	return {
		kind: "group",
		id: "1",
		revision,
		updated_at: "",
		body: { name, fixtures: ["fixture-1"] },
	};
}

function preset(revision: number, name: string): ShowObject<"preset"> {
	return {
		kind: "preset",
		id: "2.1",
		revision,
		updated_at: "",
		body: { name, number: 1, family: "Color", values: {} },
	};
}

function loadExactObject(
	_showId: string,
	kind: ShowObjectKind,
): Promise<{ object: ShowObject; showRevision: number }> {
	return Promise.resolve(
		exactSnapshot(kind === "group" ? group(1, "Group") : preset(1, "Blue")),
	);
}

function collectionSnapshot<T extends ShowObject[]>(objects: T, showRevision = 1) {
	return { objects, showRevision };
}

function exactSnapshot<T extends ShowObject | null>(object: T, showRevision = 1) {
	return { object, showRevision };
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

class FakeTransport implements ShowObjectsEventTransport {
	readonly subscriptions: Array<{
		showId: string;
		scope: ShowObjectsEventScope;
		afterSequence: number | null;
		observer: ShowObjectsEventObserver;
		close: ReturnType<typeof vi.fn>;
		repair: ReturnType<typeof vi.fn>;
	}> = [];

	subscribe(
		showId: string,
		scope: ShowObjectsEventScope,
		afterSequence: number | null,
		observer: ShowObjectsEventObserver,
	) {
		const close = vi.fn();
		const repair = vi.fn();
		this.subscriptions.push({ showId, scope, afterSequence, observer, close, repair });
		return { close, repair };
	}

	emit(message: ShowObjectsEventMessage, index = this.subscriptions.length - 1) {
		this.subscriptions[index].observer.message(message);
	}
}

describe("ShowObjectsSession", () => {
	it("queues post-snapshot events while an active view hydrates", async () => {
		let resolveLoad!: (snapshot: ReturnType<typeof collectionSnapshot>) => void;
		const loadCollection = vi.fn(
			() =>
				new Promise<ReturnType<typeof collectionSnapshot>>(
					(resolve) => (resolveLoad = resolve),
				),
		);
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection,
			loadObject: loadExactObject,
		});
		session.activate("group");
		transport.emit({ type: "ready", cursor: 10 });
		transport.emit({
			type: "event",
			change: {
				showId: SHOW_ID,
				showRevision: 4,
				eventSequence: 11,
				changes: [
					{
						kind: "group",
						objectId: "1",
						objectRevision: 2,
						body: group(2, "After snapshot").body,
						deleted: false,
					},
				],
			},
		});
		resolveLoad(collectionSnapshot([group(1, "Snapshot")], 4));

		await vi.waitFor(() =>
			expect(store.getSnapshot().groups[0]?.body.name).toBe("After snapshot"),
		);
		expect(store.getSnapshot().eventSequence).toBe(11);
	});

	it("applies one active Group/Preset transaction as one store publication", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection: (_showId, kind) =>
				Promise.resolve(
					collectionSnapshot(
						kind === "group" ? [group(1, "Group")] : [preset(1, "Blue")],
					),
				),
			loadObject: loadExactObject,
		});
		session.activate("group");
		session.activate("preset");
		transport.emit({ type: "ready", cursor: 20 });
		await vi.waitFor(() => expect(store.getSnapshot().presets).toHaveLength(1));
		const publications: Array<{ group: string; preset: string }> = [];
		const unsubscribe = store.subscribe(() => {
			const snapshot = store.getSnapshot();
			publications.push({
				group: snapshot.groups[0]?.body.name ?? "",
				preset: snapshot.presets[0]?.body.name ?? "",
			});
		});

		transport.emit({
			type: "event",
			change: {
				showId: SHOW_ID,
				showRevision: 8,
				eventSequence: 21,
				changes: [
					{
						kind: "group",
						objectId: "1",
						objectRevision: 2,
						body: group(2, "Group 2").body,
						deleted: false,
					},
					{
						kind: "preset",
						objectId: "2.1",
						objectRevision: 2,
						body: preset(2, "Cyan").body,
						deleted: false,
					},
				],
			},
		});
		unsubscribe();

		expect(publications).toEqual([{ group: "Group 2", preset: "Cyan" }]);
	});

	it("opens one complete stream for a multi-kind Cue view", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection: () => Promise.resolve(collectionSnapshot([])),
			loadObject: loadExactObject,
		});

		session.activateKinds(["cue_list", "playback", "playback_page"]);

		expect(transport.subscriptions).toHaveLength(1);
		expect(transport.subscriptions[0].scope).toEqual({
			kinds: ["cue_list", "playback", "playback_page"],
			objects: [],
		});
	});

	it("does not split a multi-kind transaction around a scoped hydration", async () => {
		let resolveGroups!: (snapshot: ReturnType<typeof collectionSnapshot>) => void;
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection: (_showId, kind) =>
				kind === "preset"
					? Promise.resolve(collectionSnapshot([preset(1, "Blue")]))
					: new Promise<ReturnType<typeof collectionSnapshot>>(
							(resolve) => (resolveGroups = resolve),
						),
			loadObject: loadExactObject,
		});
		session.activate("preset");
		transport.emit({ type: "ready", cursor: 20 });
		await vi.waitFor(() => expect(store.getSnapshot().presets).toHaveLength(1));
		session.activate("group");
		await Promise.resolve();
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
		transport.emit({ type: "ready", cursor: 20 });
		const publications: Array<{ group: string; preset: string }> = [];
		store.subscribe(() => {
			const snapshot = store.getSnapshot();
			publications.push({
				group: snapshot.groups[0]?.body.name ?? "",
				preset: snapshot.presets[0]?.body.name ?? "",
			});
		});

		transport.emit({
			type: "event",
			change: {
				showId: SHOW_ID,
				showRevision: 8,
				eventSequence: 21,
				changes: [
					{
						kind: "group",
						objectId: "1",
						objectRevision: 2,
						body: group(2, "Group 2").body,
						deleted: false,
					},
					{
						kind: "preset",
						objectId: "2.1",
						objectRevision: 2,
						body: preset(2, "Cyan").body,
						deleted: false,
					},
				],
			},
		});
		expect(store.getSnapshot().presets[0]?.body.name).toBe("Blue");
		resolveGroups(collectionSnapshot([group(1, "Group")]));
		await vi.waitFor(() =>
			expect(store.getSnapshot().presets[0]?.body.name).toBe("Cyan"),
		);

		expect(publications).not.toContainEqual({ group: "Group", preset: "Cyan" });
		expect(publications.at(-1)).toEqual({ group: "Group 2", preset: "Cyan" });
	});

	it("drops a stale cursor and fully hydrates after an event-bus restart", async () => {
		vi.useFakeTimers();
		try {
			const store = new ShowObjectsStore();
			store.reset(SHOW_ID);
			const transport = new FakeTransport();
			const loadCollection = vi
				.fn()
				.mockResolvedValueOnce(
					collectionSnapshot([group(9, "Before restart")], 9),
				)
				.mockResolvedValue(collectionSnapshot([group(1, "After restart")], 1));
			const onError = vi.fn();
			const session = new ShowObjectsSession({
				showId: SHOW_ID,
				store,
				transport,
				loadCollection,
				loadObject: loadExactObject,
				onError,
			});
			session.activate("group");
			transport.emit({ type: "ready", cursor: 100 });
			await vi.advanceTimersByTimeAsync(0);
			expect(store.getSnapshot().groups[0]?.revision).toBe(9);

			transport.emit({
				type: "error",
				error: "event cursor is newer than the event stream",
			});
			expect(onError).toHaveBeenLastCalledWith(expect.any(Error));
			await vi.advanceTimersByTimeAsync(750);
			expect(transport.subscriptions[1]?.afterSequence).toBeNull();
			transport.emit({ type: "ready", cursor: 1 });
			await vi.advanceTimersByTimeAsync(0);
			expect(onError).toHaveBeenLastCalledWith(null);
			expect(store.getSnapshot().groups[0]).toMatchObject({
				revision: 1,
				body: { name: "After restart" },
			});
			transport.emit({
				type: "event",
				change: {
					showId: SHOW_ID,
					showRevision: 2,
					eventSequence: 2,
					changes: [
						{
							kind: "group",
							objectId: "1",
							objectRevision: 2,
							body: group(2, "New epoch event").body,
							deleted: false,
						},
					],
				},
			});
			expect(store.getSnapshot().groups[0]?.body.name).toBe("New epoch event");
		} finally {
			vi.useRealTimers();
		}
	});

	it("hydrates and reconciles only the mounted Group view", async () => {
		const network = {
			objects: vi
				.fn()
				.mockResolvedValue(collectionSnapshot([group(2, "Front")], 9)),
			bootstrap: vi.fn(),
			shows: vi.fn(),
			configuration: vi.fn(),
			mediaServers: vi.fn(),
			fixtureLibrary: vi.fn(),
			fixtureProfiles: vi.fn(),
			patch: vi.fn(),
		};
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection: (showId, kind) => network.objects(showId, kind),
			loadObject: loadExactObject,
		});

		const deactivate = session.activate("group");
		expect(transport.subscriptions).toHaveLength(1);
		transport.emit({ type: "ready", cursor: 30 });
		await vi.waitFor(() => expect(store.getSnapshot().groups).toHaveLength(1));

		transport.emit({
			type: "event",
			change: {
				showId: SHOW_ID,
				showRevision: 9,
				eventSequence: 31,
				changes: [
					{
						kind: "group",
						objectId: "1",
						objectRevision: 3,
						body: { name: "Front Wash", fixtures: ["fixture-2"] },
						deleted: false,
					},
					{
						kind: "preset",
						objectId: "2.1",
						objectRevision: 1,
						body: { name: "Blue", number: 1, values: {} },
						deleted: false,
					},
				],
			},
		});

		expect(store.getSnapshot().groups[0]).toMatchObject({
			revision: 3,
			body: { name: "Front Wash", fixtures: ["fixture-2"] },
		});
		expect(store.getSnapshot().presets).toEqual([]);
		expect(network.objects).toHaveBeenCalledOnce();
		expect(network.objects).toHaveBeenCalledWith(SHOW_ID, "group");
		for (const request of [
			network.bootstrap,
			network.shows,
			network.configuration,
			network.mediaServers,
			network.fixtureLibrary,
			network.fixtureProfiles,
			network.patch,
		])
			expect(request).not.toHaveBeenCalled();

		deactivate();
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
	});

	it("reference-counts views and unsubscribes when the final view deactivates", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const loadCollection = vi
			.fn()
			.mockResolvedValue(collectionSnapshot([group(1, "Group")]));
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection,
			loadObject: loadExactObject,
		});

		const deactivateFirst = session.activate("group");
		const deactivateSecond = session.activate("group");
		transport.emit({ type: "ready", cursor: 12 });
		await vi.waitFor(() => expect(loadCollection).toHaveBeenCalledOnce());
		deactivateFirst();
		expect(transport.subscriptions[0].close).not.toHaveBeenCalled();
		deactivateSecond();
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();

		const deactivatePreset = session.activate("preset");
		expect(transport.subscriptions).toHaveLength(2);
		expect(transport.subscriptions[1]).toMatchObject({
			showId: SHOW_ID,
			afterSequence: 0,
		});
		deactivatePreset();
		expect(transport.subscriptions[1].close).toHaveBeenCalledOnce();
	});

	it("resubscribes to exact identities without reloading unchanged kind scopes", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const loadCollection = vi
			.fn()
			.mockResolvedValue(collectionSnapshot([group(1, "Group")]));
		const loadObject = vi
			.fn()
			.mockResolvedValue(exactSnapshot(preset(1, "Blue")));
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection,
			loadObject,
		});

		const deactivateGroup = session.activate("group");
		expect(transport.subscriptions[0].scope).toEqual({
			kinds: ["group"],
			objects: [],
		});
		transport.emit({ type: "ready", cursor: 20 });
		await vi.waitFor(() => expect(loadCollection).toHaveBeenCalledOnce());

		const deactivatePreset = session.activate("preset", "2.1");
		await Promise.resolve();
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
		expect(transport.subscriptions[1].scope).toEqual({
			kinds: ["group"],
			objects: [{ kind: "preset", objectId: "2.1" }],
		});
		transport.emit({ type: "ready", cursor: 20 });
		await vi.waitFor(() => expect(loadObject).toHaveBeenCalledOnce());
		expect(loadObject).toHaveBeenCalledWith(SHOW_ID, "preset", "2.1");
		expect(loadCollection).toHaveBeenCalledOnce();

		deactivateGroup();
		await Promise.resolve();
		expect(transport.subscriptions[1].close).toHaveBeenCalledOnce();
		expect(transport.subscriptions[2]).toMatchObject({
			afterSequence: 0,
			scope: {
				kinds: [],
				objects: [{ kind: "preset", objectId: "2.1" }],
			},
		});
		transport.emit({ type: "ready", cursor: 20 });
		await Promise.resolve();
		expect(loadObject).toHaveBeenCalledOnce();
		expect(loadCollection).toHaveBeenCalledOnce();

		deactivatePreset();
		expect(transport.subscriptions[2].close).toHaveBeenCalledOnce();
	});

	it("rehydrates an exact object after its inactive scope falls behind", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const loadCollection = vi
			.fn()
			.mockResolvedValue(collectionSnapshot([preset(1, "Blue")]));
		const loadObject = vi
			.fn()
			.mockResolvedValueOnce(exactSnapshot(group(1, "Before deactivation")))
			.mockResolvedValueOnce(exactSnapshot(group(2, "While inactive"), 2));
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection,
			loadObject,
		});
		session.activate("preset");
		const deactivateGroup = session.activate("group", "1");
		transport.emit({ type: "ready", cursor: 10 });
		await vi.waitFor(() => expect(loadObject).toHaveBeenCalledOnce());

		deactivateGroup();
		await Promise.resolve();
		transport.emit({ type: "ready", cursor: 10 });
		transport.emit({
			type: "event",
			change: {
				showId: SHOW_ID,
				showRevision: 2,
				eventSequence: 20,
				changes: [
					{
						kind: "preset",
						objectId: "2.1",
						objectRevision: 2,
						body: preset(2, "Cyan").body,
						deleted: false,
					},
				],
			},
		});

		session.activate("group", "1");
		await Promise.resolve();
		expect(transport.subscriptions.at(-1)?.afterSequence).toBe(20);
		transport.emit({ type: "ready", cursor: 20 });
		await vi.waitFor(() => expect(loadObject).toHaveBeenCalledTimes(2));
		await vi.waitFor(() =>
			expect(store.getSnapshot().groups[0]?.body.name).toBe("While inactive"),
		);
	});

	it("installs a missing exact-object snapshot as authoritative absence", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(SHOW_ID, "group", [group(1, "Deleted elsewhere")]);
		const transport = new FakeTransport();
		const loadCollection = vi.fn();
		const loadObject = vi.fn().mockResolvedValue(exactSnapshot(null));
		const onError = vi.fn();
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection,
			loadObject,
			onError,
		});

		session.activate("group", "1");
		transport.emit({ type: "ready", cursor: 30 });
		await vi.waitFor(() => expect(loadObject).toHaveBeenCalledOnce());
		await vi.waitFor(() => expect(store.getSnapshot().groups).toEqual([]));

		expect(loadCollection).not.toHaveBeenCalled();
		expect(onError).toHaveBeenLastCalledWith(null);
		expect(transport.subscriptions).toHaveLength(1);
	});

	it("rejects a mismatched exact-object snapshot and retries the requested identity", async () => {
		vi.useFakeTimers();
		try {
			const store = new ShowObjectsStore();
			store.reset(SHOW_ID);
			const transport = new FakeTransport();
			const loadObject = vi
				.fn()
				.mockResolvedValueOnce(exactSnapshot(preset(1, "Wrong object")))
				.mockResolvedValueOnce(exactSnapshot(group(1, "Requested object")));
			const onError = vi.fn();
			const session = new ShowObjectsSession({
				showId: SHOW_ID,
				store,
				transport,
				loadCollection: vi.fn(),
				loadObject,
				onError,
			});
			session.activate("group", "1");
			transport.emit({ type: "ready", cursor: 10 });
			await vi.advanceTimersByTimeAsync(0);

			expect(onError).toHaveBeenCalledWith(
				expect.objectContaining({
					message: "Expected group 1, received preset 2.1",
				}),
			);
			expect(store.getSnapshot().groups).toEqual([]);
			await vi.advanceTimersByTimeAsync(750);
			transport.emit({ type: "ready", cursor: 10 });
			await vi.advanceTimersByTimeAsync(0);

			expect(loadObject).toHaveBeenCalledTimes(2);
			expect(store.getSnapshot().groups[0]?.body.name).toBe("Requested object");
		} finally {
			vi.useRealTimers();
		}
	});

	it("repairs a gap with the same narrow scope and an authoritative reload", async () => {
		vi.useFakeTimers();
		try {
			const store = new ShowObjectsStore();
			store.reset(SHOW_ID);
			const transport = new FakeTransport();
			const loadCollection = vi
				.fn()
				.mockResolvedValueOnce(collectionSnapshot([group(1, "Before gap")], 1))
				.mockResolvedValueOnce(collectionSnapshot([group(2, "After gap")], 2));
			const session = new ShowObjectsSession({
				showId: SHOW_ID,
				store,
				transport,
				loadCollection,
				loadObject: loadExactObject,
			});
			session.activate("group");
			transport.emit({ type: "ready", cursor: 10 });
			await vi.advanceTimersByTimeAsync(0);

			transport.emit({
				type: "gap",
				afterSequence: 10,
				oldestAvailable: 15,
				latestSequence: 20,
			});
			await vi.advanceTimersByTimeAsync(0);
			expect(transport.subscriptions).toHaveLength(1);
			expect(transport.subscriptions[0].repair).toHaveBeenCalledWith(20);
			expect(loadCollection).toHaveBeenCalledTimes(2);
			expect(store.getSnapshot().groups[0]?.body.name).toBe("After gap");
		} finally {
			vi.useRealTimers();
		}
	});

	it("restores the Show revision when an exact-only scope repairs a gap", async () => {
		vi.useFakeTimers();
		try {
			const store = new ShowObjectsStore();
			store.reset(SHOW_ID);
			const transport = new FakeTransport();
			const loadObject = vi
				.fn()
				.mockResolvedValueOnce(exactSnapshot(group(1, "Before gap"), 3))
				.mockResolvedValueOnce(exactSnapshot(group(2, "After gap"), 9));
			const session = new ShowObjectsSession({
				showId: SHOW_ID,
				store,
				transport,
				loadCollection: vi.fn(),
				loadObject,
			});
			session.activate("group", "1");
			transport.emit({ type: "ready", cursor: 10 });
			await vi.advanceTimersByTimeAsync(0);
			expect(store.getSnapshot().showRevision).toBe(3);

			transport.emit({
				type: "gap",
				afterSequence: 10,
				oldestAvailable: 15,
				latestSequence: 20,
			});
			expect(store.getSnapshot().showRevision).toBeNull();
			await vi.advanceTimersByTimeAsync(0);

			expect(store.getSnapshot().showRevision).toBe(9);
			expect(transport.subscriptions[0].repair).toHaveBeenCalledWith(20);
		} finally {
			vi.useRealTimers();
		}
	});

	it("discards a pre-gap collection revision before the repair hydration settles", async () => {
		const beforeGap = deferred<ReturnType<typeof collectionSnapshot>>();
		const afterGap = deferred<ReturnType<typeof collectionSnapshot>>();
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const loadCollection = vi
			.fn()
			.mockReturnValueOnce(beforeGap.promise)
			.mockReturnValueOnce(afterGap.promise);
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection,
			loadObject: loadExactObject,
		});
		session.activate("group");
		transport.emit({
			type: "gap",
			afterSequence: 0,
			oldestAvailable: 10,
			latestSequence: 20,
		});
		expect(loadCollection).toHaveBeenCalledTimes(2);

		beforeGap.resolve(collectionSnapshot([group(99, "Stale")], 99));
		await Promise.resolve();
		await Promise.resolve();
		expect(store.getSnapshot().showRevision).toBeNull();
		expect(store.getSnapshot().groups).toEqual([]);

		afterGap.resolve(collectionSnapshot([group(2, "Repaired")], 2));
		await vi.waitFor(() => expect(store.getSnapshot().showRevision).toBe(2));
		expect(store.getSnapshot().groups[0]?.body.name).toBe("Repaired");
	});

	it("resets instead of advancing authority for a foreign-Show event", async () => {
		vi.useFakeTimers();
		try {
			const store = new ShowObjectsStore();
			store.reset(SHOW_ID);
			const transport = new FakeTransport();
			const onError = vi.fn();
			const session = new ShowObjectsSession({
				showId: SHOW_ID,
				store,
				transport,
				loadCollection: () =>
					Promise.resolve(collectionSnapshot([group(1, "Current")], 1)),
				loadObject: loadExactObject,
				onError,
			});
			session.activate("group");
			await vi.advanceTimersByTimeAsync(0);

			transport.emit({
				type: "event",
				change: {
					showId: "22222222-2222-4222-8222-222222222222",
					showRevision: 50,
					eventSequence: 50,
					changes: [
						{
							kind: "group",
							objectId: "1",
							objectRevision: 50,
							body: group(50, "Foreign").body,
							deleted: false,
						},
					],
				},
			});
			expect(onError).toHaveBeenLastCalledWith(expect.any(ShowObjectsProtocolError));
			expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
			await vi.advanceTimersByTimeAsync(0);
			expect(transport.subscriptions[1].afterSequence).toBeNull();
			expect(store.getSnapshot().groups[0]?.body.name).not.toBe("Foreign");
		} finally {
			vi.useRealTimers();
		}
	});

	it("discards a hydration invalidated by view deactivation", async () => {
		const groupLoads: Array<
			(snapshot: ReturnType<typeof collectionSnapshot>) => void
		> = [];
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection: (_showId, kind) =>
				kind === "preset"
					? Promise.resolve(collectionSnapshot([preset(1, "Blue")]))
					: new Promise<ReturnType<typeof collectionSnapshot>>((resolve) =>
							groupLoads.push(resolve),
						),
			loadObject: loadExactObject,
		});
		session.activate("preset");
		transport.emit({ type: "ready", cursor: 30 });
		await vi.waitFor(() => expect(store.getSnapshot().presets).toHaveLength(1));

		const deactivate = session.activate("group");
		transport.emit({ type: "ready", cursor: 30 });
		expect(groupLoads).toHaveLength(1);
		deactivate();
		session.activate("group");
		transport.emit({ type: "ready", cursor: 30 });
		expect(groupLoads).toHaveLength(2);
		groupLoads[0](collectionSnapshot([group(1, "Stale hydration")]));
		await Promise.resolve();
		await Promise.resolve();
		expect(store.getSnapshot().groups).toEqual([]);

		groupLoads[1](collectionSnapshot([group(2, "Current hydration")], 2));
		await vi.waitFor(() =>
			expect(store.getSnapshot().groups[0]?.body.name).toBe("Current hydration"),
		);
	});

	it("hydrates only Presets when only the Preset pool is mounted", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const loadCollection = vi
			.fn()
			.mockResolvedValue(collectionSnapshot([preset(2, "Blue")]));
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection,
			loadObject: loadExactObject,
		});

		const deactivate = session.activate("preset");
		transport.emit({ type: "ready", cursor: 18 });
		await vi.waitFor(() => expect(store.getSnapshot().presets).toHaveLength(1));
		expect(loadCollection).toHaveBeenCalledOnce();
		expect(loadCollection).toHaveBeenCalledWith(SHOW_ID, "preset");
		expect(store.getSnapshot().groups).toEqual([]);

		deactivate();
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
	});

	it("resumes from the last installed delta rather than the ready replay boundary", async () => {
		vi.useFakeTimers();
		try {
			const store = new ShowObjectsStore();
			store.reset(SHOW_ID);
			const transport = new FakeTransport();
			const session = new ShowObjectsSession({
				showId: SHOW_ID,
				store,
				transport,
				loadCollection: (_showId, kind) =>
					Promise.resolve(
						collectionSnapshot(kind === "group" ? [group(1, "Initial")] : []),
					),
				loadObject: loadExactObject,
			});
			session.activate("group");
			await vi.advanceTimersByTimeAsync(0);
			transport.emit(groupChange(5, "Installed"));

			transport.subscriptions[0].observer.closed();
			await vi.advanceTimersByTimeAsync(750);
			expect(transport.subscriptions[1].afterSequence).toBe(5);
			transport.emit({ type: "ready", cursor: 10 }, 1);
			session.activate("preset");
			await vi.advanceTimersByTimeAsync(0);

			expect(transport.subscriptions[2].afterSequence).toBe(5);
			transport.emit(groupChange(6, "Replayed after Ready"), 2);
			expect(store.getSnapshot().groups[0]?.body.name).toBe(
				"Replayed after Ready",
			);
		} finally {
			vi.useRealTimers();
		}
	});

	it("keeps queued deltas across a scope change and resumes after installation", async () => {
		let resolvePreset!: (snapshot: ReturnType<typeof exactSnapshot>) => void;
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection: () =>
				Promise.resolve(collectionSnapshot([group(1, "Initial")])),
			loadObject: (_showId, kind) =>
				kind === "preset"
					? new Promise<ReturnType<typeof exactSnapshot>>(
							(resolve) => (resolvePreset = resolve),
						)
					: Promise.resolve(exactSnapshot(group(1, "Initial"))),
		});
		session.activate("group");
		await vi.waitFor(() => expect(store.getSnapshot().groups).toHaveLength(1));
		transport.emit(groupChange(10, "Baseline"));
		const deactivatePreset = session.activate("preset", "2.1");
		await Promise.resolve();
		transport.emit(presetChange(11, "Queued Preset"));
		transport.emit(groupChange(12, "Queued Group"));

		deactivatePreset();
		await Promise.resolve();
		expect(store.getSnapshot().groups[0]?.body.name).toBe("Queued Group");
		expect(transport.subscriptions.at(-1)?.afterSequence).toBe(12);
		resolvePreset(exactSnapshot(preset(2, "Stale"), 2));
		await Promise.resolve();
		expect(store.getSnapshot().presets).toEqual([]);
	});

	it("escapes a malformed replay with an authoritative latest-boundary reset", async () => {
		vi.useFakeTimers();
		try {
			const store = new ShowObjectsStore();
			store.reset(SHOW_ID);
			const transport = new FakeTransport();
			const loadCollection = vi
				.fn()
				.mockResolvedValueOnce(
					collectionSnapshot([group(1, "Before poison")], 1),
				)
				.mockResolvedValue(
					collectionSnapshot([group(2, "Recovered snapshot")], 2),
				);
			const session = new ShowObjectsSession({
				showId: SHOW_ID,
				store,
				transport,
				loadCollection,
				loadObject: loadExactObject,
			});
			session.activate("group");
			await vi.advanceTimersByTimeAsync(0);
			transport.emit(groupChange(5, "Before poison event"));

			transport.subscriptions[0].observer.error(
				new ShowObjectsProtocolError("malformed event", 6),
			);
			await vi.advanceTimersByTimeAsync(0);
			expect(transport.subscriptions[1].afterSequence).toBeNull();
			transport.emit({ type: "ready", cursor: 6 }, 1);
			await vi.advanceTimersByTimeAsync(0);
			await vi.waitFor(() =>
				expect(store.getSnapshot().groups[0]?.body.name).toBe(
					"Recovered snapshot",
				),
			);
			transport.emit(groupChange(7, "After recovery"), 1);
			expect(store.getSnapshot().groups[0]?.body.name).toBe("After recovery");
		} finally {
			vi.useRealTimers();
		}
	});

	it("hydrates and subscribes to the transitive dependencies of an exact live Group", async () => {
		const source = group(1, "Source");
		source.body.fixtures = ["a", "b", "c", "d"];
		const derived: ShowObject<"group"> = {
			...group(1, "Derived"),
			id: "2",
			body: {
				name: "Derived",
				fixtures: ["a", "c"],
				derived_from: {
					source_group_id: "1",
					rule: { type: "odd" },
				},
			},
		};
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const loadObject = vi.fn((_showId, _kind, id: string) =>
			Promise.resolve(exactSnapshot(id === "2" ? derived : source)),
		);
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection: vi.fn(),
			loadObject,
		});
		session.activate("group", "2");
		await vi.waitFor(() => expect(loadObject).toHaveBeenCalledTimes(2));
		await vi.waitFor(() =>
			expect(store.getSnapshot().groups.find((item) => item.id === "2")?.body.fixtures).toEqual([
				"a",
				"c",
			]),
		);
		await Promise.resolve();
		expect(transport.subscriptions.at(-1)?.scope).toEqual({
			kinds: [],
			objects: [
				{ kind: "group", objectId: "1" },
				{ kind: "group", objectId: "2" },
			],
		});

		source.revision = 2;
		source.body.fixtures = ["b", "c", "d"];
		transport.emit({
			type: "event",
			change: {
				showId: SHOW_ID,
				showRevision: 2,
				eventSequence: 1,
				changes: [
					{
						kind: "group",
						objectId: "1",
						objectRevision: 2,
						body: source.body,
						deleted: false,
					},
				],
			},
		});
		await vi.waitFor(() =>
			expect(store.getSnapshot().groups.find((item) => item.id === "2")?.body.fixtures).toEqual([
				"b",
				"d",
			]),
		);
	});

	it("batches an exact identity replacement into one WebSocket reconfiguration", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection: () =>
				Promise.resolve(collectionSnapshot([preset(1, "Blue")])),
			loadObject: (_showId, kind, id) =>
				Promise.resolve(
					exactSnapshot(
						kind === "group"
							? { ...group(1, `Group ${id}`), id }
							: preset(1, "Blue"),
					),
				),
		});
		session.activate("preset");
		const deactivateFirst = session.activate("group", "1");
		await Promise.resolve();
		const beforeReplacement = transport.subscriptions.length;

		deactivateFirst();
		session.activate("group", "2");
		await Promise.resolve();

		expect(transport.subscriptions).toHaveLength(beforeReplacement + 1);
		expect(transport.subscriptions.at(-1)?.scope.objects).toContainEqual({
			kind: "group",
			objectId: "2",
		});
	});
});

function groupChange(sequence: number, name: string): ShowObjectsEventMessage {
	return {
		type: "event",
		change: {
			showId: SHOW_ID,
			showRevision: sequence,
			eventSequence: sequence,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: sequence,
					body: group(sequence, name).body,
					deleted: false,
				},
			],
		},
	};
}

function presetChange(sequence: number, name: string): ShowObjectsEventMessage {
	return {
		type: "event",
		change: {
			showId: SHOW_ID,
			showRevision: sequence,
			eventSequence: sequence,
			changes: [
				{
					kind: "preset",
					objectId: "2.1",
					objectRevision: sequence,
					body: preset(sequence, name).body,
					deleted: false,
				},
			],
		},
	};
}
