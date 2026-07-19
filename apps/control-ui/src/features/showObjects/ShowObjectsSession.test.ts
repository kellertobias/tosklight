import { describe, expect, it, vi } from "vitest";
import type { ShowObject, ShowObjectsEventMessage } from "./contracts";
import { ShowObjectsSession } from "./session";
import { ShowObjectsStore } from "./store";
import type {
	ShowObjectsEventObserver,
	ShowObjectsEventTransport,
} from "./transport";

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

class FakeTransport implements ShowObjectsEventTransport {
	readonly subscriptions: Array<{
		showId: string;
		afterSequence: number | null;
		observer: ShowObjectsEventObserver;
		close: ReturnType<typeof vi.fn>;
	}> = [];

	subscribe(
		showId: string,
		afterSequence: number | null,
		observer: ShowObjectsEventObserver,
	) {
		const close = vi.fn();
		this.subscriptions.push({ showId, afterSequence, observer, close });
		return { close };
	}

	emit(message: ShowObjectsEventMessage, index = this.subscriptions.length - 1) {
		this.subscriptions[index].observer.message(message);
	}
}

describe("ShowObjectsSession", () => {
	it("queues post-snapshot events while an active view hydrates", async () => {
		let resolveLoad!: (objects: ShowObject[]) => void;
		const loadCollection = vi.fn(
			() => new Promise<ShowObject[]>((resolve) => (resolveLoad = resolve)),
		);
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection,
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
		resolveLoad([group(1, "Snapshot")]);

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
					kind === "group" ? [group(1, "Group")] : [preset(1, "Blue")],
				),
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

	it("does not split a multi-kind transaction around a scoped hydration", async () => {
		let resolveGroups!: (objects: ShowObject[]) => void;
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection: (_showId, kind) =>
				kind === "preset"
					? Promise.resolve([preset(1, "Blue")])
					: new Promise((resolve) => (resolveGroups = resolve)),
		});
		session.activate("preset");
		transport.emit({ type: "ready", cursor: 20 });
		await vi.waitFor(() => expect(store.getSnapshot().presets).toHaveLength(1));
		session.activate("group");
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
		resolveGroups([group(1, "Group")]);
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
				.mockResolvedValueOnce([group(9, "Before restart")])
				.mockResolvedValueOnce([group(1, "After restart")]);
			const onError = vi.fn();
			const session = new ShowObjectsSession({
				showId: SHOW_ID,
				store,
				transport,
				loadCollection,
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
			objects: vi.fn().mockResolvedValue([group(2, "Front")]),
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
		const loadCollection = vi.fn().mockResolvedValue([group(1, "Group")]);
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection,
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
			afterSequence: 12,
		});
		deactivatePreset();
		expect(transport.subscriptions[1].close).toHaveBeenCalledOnce();
	});

	it("discards a hydration invalidated by view deactivation", async () => {
		const groupLoads: Array<(objects: ShowObject[]) => void> = [];
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection: (_showId, kind) =>
				kind === "preset"
					? Promise.resolve([preset(1, "Blue")])
					: new Promise((resolve) => groupLoads.push(resolve)),
		});
		session.activate("preset");
		transport.emit({ type: "ready", cursor: 30 });
		await vi.waitFor(() => expect(store.getSnapshot().presets).toHaveLength(1));

		const deactivate = session.activate("group");
		expect(groupLoads).toHaveLength(1);
		deactivate();
		session.activate("group");
		expect(groupLoads).toHaveLength(2);
		groupLoads[0]([group(1, "Stale hydration")]);
		await Promise.resolve();
		await Promise.resolve();
		expect(store.getSnapshot().groups).toEqual([]);

		groupLoads[1]([group(2, "Current hydration")]);
		await vi.waitFor(() =>
			expect(store.getSnapshot().groups[0]?.body.name).toBe("Current hydration"),
		);
	});

	it("hydrates only Presets when only the Preset pool is mounted", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const loadCollection = vi.fn().mockResolvedValue([preset(2, "Blue")]);
		const session = new ShowObjectsSession({
			showId: SHOW_ID,
			store,
			transport,
			loadCollection,
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
});
