import { render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
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

describe("ShowObjectsViewProvider", () => {
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
		const loadCollection = vi.fn().mockResolvedValue([group]);
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
		const loadCollection = vi.fn().mockResolvedValue([]);
		const loadObject = vi.fn().mockResolvedValue(null);
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
		const loadObject = vi.fn().mockResolvedValue({
			kind: "group",
			id: "1",
			revision: 1,
			updated_at: "",
			body: { name: "Selected", fixtures: [] },
		});
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
