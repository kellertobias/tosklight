import { render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ShowObjectKind } from "./contracts";
import {
	ShowObjectDetailSubscription,
	ShowObjectsViewProvider,
	useShowObjectView,
} from "./ShowObjectsView";
import { ShowObjectsStore } from "./store";
import type {
	ShowObjectsEventObserver,
	ShowObjectsEventScope,
	ShowObjectsEventTransport,
} from "./transport";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

function collectionSnapshot<T>(objects: T[], showRevision = 1) {
	return { objects, showRevision };
}

function exactSnapshot<T>(object: T | null, showRevision = 1) {
	return { object, showRevision };
}

class FakeTransport implements ShowObjectsEventTransport {
	readonly subscriptions: Array<{
		scope: ShowObjectsEventScope;
		observer: ShowObjectsEventObserver;
		close: ReturnType<typeof vi.fn>;
	}> = [];

	subscribe(
		_showId: string,
		scope: ShowObjectsEventScope,
		_afterSequence: number | null,
		observer: ShowObjectsEventObserver,
	) {
		const close = vi.fn();
		this.subscriptions.push({ scope, observer, close });
		return { close, repair: vi.fn() };
	}
}

function GroupConsumer({ active }: { active: boolean }) {
	useShowObjectView("group", active);
	return null;
}

function PresetConsumer({ active }: { active: boolean }) {
	useShowObjectView("preset", active);
	return null;
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

describe("ShowObjectsViewProvider", () => {
	it("keeps Preset readiness false when an unrelated Group hydration finishes first", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const presets = deferred<ReturnType<typeof collectionSnapshot<never>>>();
		const loadCollection = vi.fn((_showId: string, kind: ShowObjectKind) =>
			kind === "group"
				? Promise.resolve(collectionSnapshot([]))
				: presets.promise,
		);
		render(
			<ShowObjectsViewProvider
				showId={SHOW_ID}
				store={store}
				transport={null}
				loadCollection={loadCollection}
				loadObject={vi.fn()}
			>
				<GroupConsumer active />
				<PresetConsumer active />
			</ShowObjectsViewProvider>,
		);

		await waitFor(() => expect(store.isCollectionReady("group")).toBe(true));
		expect(store.isCollectionReady("preset")).toBe(false);
		presets.resolve(collectionSnapshot([]));
		await waitFor(() => expect(store.isCollectionReady("preset")).toBe(true));
	});

	it("rejects late hydration and events from a replaced same-show authority", async () => {
		const store = new ShowObjectsStore();
		const oldLoad = deferred<{
			objects: Array<{
				kind: "group";
				id: string;
				revision: number;
				updated_at: string;
				body: { name: string; fixtures: string[] };
			}>;
			showRevision: number;
		}>();
		const transport = new FakeTransport();
		const replacement = {
			kind: "group" as const,
			id: "1",
			revision: 9,
			updated_at: "",
			body: { name: "Replacement", fixtures: [] },
		};
		const loadCollection = vi.fn(() =>
			loadCollection.mock.calls.length === 1
				? oldLoad.promise
				: Promise.resolve(collectionSnapshot([replacement], 9)),
		);
		const loadObject = vi.fn();
		const view = (authorityKey: string) => (
			<ShowObjectsViewProvider
				showId={SHOW_ID}
				authorityKey={authorityKey}
				store={store}
				transport={transport}
				loadCollection={loadCollection}
				loadObject={loadObject}
			>
				<GroupConsumer active />
			</ShowObjectsViewProvider>
		);
		const rendered = render(view("session-a"));
		await waitFor(() => expect(loadCollection).toHaveBeenCalledOnce());
		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));

		rendered.rerender(view("session-b"));
		await waitFor(() => expect(loadCollection).toHaveBeenCalledTimes(2));
		await waitFor(() => expect(transport.subscriptions).toHaveLength(2));
		await waitFor(() => expect(store.getSnapshot().groups).toEqual([replacement]));
		oldLoad.resolve(
			collectionSnapshot(
				[
					{
						...replacement,
						revision: 1,
						body: { name: "Late hydration", fixtures: [] },
					},
				],
				1,
			),
		);
		transport.subscriptions[0].observer.message({
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
						body: { name: "Late event", fixtures: [] },
						deleted: false,
					},
				],
			},
		});
		await Promise.resolve();

		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
		expect(store.getSnapshot().groups).toEqual([replacement]);
	});

	it("hydrates an active view before a WebSocket transport is available", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const group = {
			kind: "group" as const,
			id: "1",
			revision: 1,
			updated_at: "",
			body: { name: "Front", fixtures: [] },
		};
		const loadCollection = vi
			.fn()
			.mockResolvedValue(collectionSnapshot([group]));
		const loadObject = vi.fn();
		render(
			<ShowObjectsViewProvider
				showId={SHOW_ID}
				store={store}
				transport={null}
				loadCollection={loadCollection}
				loadObject={loadObject}
			>
				<GroupConsumer active />
			</ShowObjectsViewProvider>,
		);

		await waitFor(() => expect(loadCollection).toHaveBeenCalledOnce());
		expect(loadCollection).toHaveBeenCalledWith(SHOW_ID, "group");
		await waitFor(() => expect(store.getSnapshot().groups).toEqual([group]));
		expect(loadObject).not.toHaveBeenCalled();
	});

	it("opens a stream only while its view is visibly active", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const loadCollection = vi.fn().mockResolvedValue(collectionSnapshot([]));
		const loadObject = vi.fn().mockResolvedValue(exactSnapshot(null));
		const view = (active: boolean) => (
			<ShowObjectsViewProvider
				showId={SHOW_ID}
				store={store}
				transport={transport}
				loadCollection={loadCollection}
				loadObject={loadObject}
			>
				<GroupConsumer active={active} />
			</ShowObjectsViewProvider>
		);
		const rendered = render(view(false));
		expect(transport.subscriptions).toHaveLength(0);

		rendered.rerender(view(true));
		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		rendered.rerender(view(false));
		await waitFor(() =>
			expect(transport.subscriptions[0].close).toHaveBeenCalledOnce(),
		);
	});

	it("hydrates a selected detail through only its exact-object scope", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		const transport = new FakeTransport();
		const loadCollection = vi.fn();
		const loadObject = vi.fn().mockResolvedValue(
			exactSnapshot({
				kind: "group",
				id: "1",
				revision: 1,
				updated_at: "",
				body: { name: "Selected", fixtures: [] },
			}),
		);
		render(
			<ShowObjectsViewProvider
				showId={SHOW_ID}
				store={store}
				transport={transport}
				loadCollection={loadCollection}
				loadObject={loadObject}
			>
				<ShowObjectDetailSubscription kind="group" objectId="1" />
			</ShowObjectsViewProvider>,
		);

		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		expect(transport.subscriptions[0].scope).toEqual({
			kinds: [],
			objects: [{ kind: "group", objectId: "1" }],
		});
		transport.subscriptions[0].observer.message({ type: "ready", cursor: 5 });
		await waitFor(() => expect(loadObject).toHaveBeenCalledOnce());
		expect(loadObject).toHaveBeenCalledWith(SHOW_ID, "group", "1");
		expect(loadCollection).not.toHaveBeenCalled();
	});
});
