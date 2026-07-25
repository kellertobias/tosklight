import {
	act,
	cleanup,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ShowObject } from "../../../features/showObjects/contracts";
import { ShowObjectsViewProvider } from "../../../features/showObjects/ShowObjectsView";
import { ShowObjectsStore } from "../../../features/showObjects/store";
import type {
	ShowObjectsEventObserver,
	ShowObjectsEventScope,
	ShowObjectsEventTransport,
} from "../../../features/showObjects/transport";
import {
	selectedGroupSupportedAttributes,
	useSelectedPortableGroup,
} from "./useSelectedPortableGroup";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

function group(
	id: string,
	revision: number,
	programming: Record<string, unknown>,
): ShowObject<"group"> {
	return {
		kind: "group",
		id,
		revision,
		updated_at: "",
		body: { name: `Group ${id}`, fixtures: [], programming },
	};
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
		scope: ShowObjectsEventScope;
		observer: ShowObjectsEventObserver;
	}> = [];

	subscribe(
		_showId: string,
		scope: ShowObjectsEventScope,
		_afterSequence: number | null,
		observer: ShowObjectsEventObserver,
	) {
		this.subscriptions.push({ scope, observer });
		return { close: vi.fn(), repair: vi.fn() };
	}
}

function Consumer({
	active,
	groupId,
	onRender,
}: {
	active: boolean;
	groupId: string | null;
	onRender: () => void;
}) {
	onRender();
	const selected = useSelectedPortableGroup(groupId, active);
	const attributes = selectedGroupSupportedAttributes(groupId, selected);
	return (
		<div>
		<span data-testid="authority">
			{selected === undefined
				? "loading"
				: selected === null
					? "missing"
					: `${selected.id}:${selected.revision}`}
		</span>
		<span data-testid="attributes">{attributes.join(",")}</span>
		</div>
	);
}

function harness({
	active = true,
	groupId = "1",
	store = new ShowObjectsStore(),
	transport = new FakeTransport(),
	loadCollection = vi.fn(async () => ({
		objects: [] as ShowObject[],
		showRevision: 1,
	})),
	onRender = vi.fn(),
} = {}) {
	const body = (nextActive = active, nextGroupId: string | null = groupId) => (
		<ShowObjectsViewProvider
			showId={SHOW_ID}
			store={store}
			transport={transport}
			loadCollection={loadCollection}
			loadObject={vi.fn()}
		>
			<Consumer
				active={nextActive}
				groupId={nextGroupId}
				onRender={onRender}
			/>
		</ShowObjectsViewProvider>
	);
	return { body, loadCollection, onRender, store, transport };
}

afterEach(cleanup);

describe("selected portable Group parameter authority", () => {
	it("opens no Group snapshot or socket while inactive or without a Group target", async () => {
		const view = harness({ active: false });
		const rendered = render(view.body());

		await act(async () => undefined);
		expect(view.loadCollection).not.toHaveBeenCalled();
		expect(view.transport.subscriptions).toHaveLength(0);

		rendered.rerender(view.body(true, null));
		await act(async () => undefined);
		expect(view.loadCollection).not.toHaveBeenCalled();
		expect(view.transport.subscriptions).toHaveLength(0);
		expect(screen.getByTestId("attributes")).toHaveTextContent("");
	});

	it("hides retained Group programming until portable authority loads", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(SHOW_ID, "group", [group("1", 1, { color: {} })]);
		store.markCollectionDormant("group");
		const loaded = deferred<{
			objects: ShowObject<"group">[];
			showRevision: number;
		}>();
		const view = harness({
			store,
			loadCollection: vi.fn(() => loaded.promise),
		});

		render(view.body());

		expect(screen.getByTestId("authority")).toHaveTextContent("loading");
		expect(screen.getByTestId("attributes")).toHaveTextContent("intensity");
		expect(screen.getByTestId("attributes")).not.toHaveTextContent("color");
		await waitFor(() => expect(view.loadCollection).toHaveBeenCalledOnce());
		loaded.resolve({
			objects: [group("1", 2, { pan: {}, intensity: {} })],
			showRevision: 2,
		});

		await waitFor(() =>
			expect(screen.getByTestId("authority")).toHaveTextContent("1:2"),
		);
		expect(screen.getByTestId("attributes")).toHaveTextContent(
			"intensity,pan",
		);
		await waitFor(() => expect(view.transport.subscriptions).toHaveLength(1));
		expect(view.transport.subscriptions[0].scope).toEqual({
			kinds: ["group"],
			objects: [],
		});
	});

	it("rerenders for the selected Group but not unrelated authoritative objects", async () => {
		const selected = group("1", 1, { color: {} });
		const other = group("2", 1, { pan: {} });
		const view = harness({
			loadCollection: vi.fn(async () => ({
				objects: [selected, other],
				showRevision: 1,
			})),
		});
		render(view.body());
		await waitFor(() =>
			expect(screen.getByTestId("authority")).toHaveTextContent("1:1"),
		);
		const renders = view.onRender.mock.calls.length;

		act(() => {
			view.store.applyChange({
				showId: SHOW_ID,
				showRevision: 2,
				eventSequence: 2,
				changes: [
					{
						kind: "group",
						objectId: "2",
						objectRevision: 2,
						body: { ...other.body, programming: { tilt: {} } },
						deleted: false,
					},
				],
			});
		});
		expect(view.onRender).toHaveBeenCalledTimes(renders);

		act(() => {
			view.store.applyChange({
				showId: SHOW_ID,
				showRevision: 3,
				eventSequence: 3,
				changes: [
					{
						kind: "group",
						objectId: "1",
						objectRevision: 2,
						body: { ...selected.body, programming: { zoom: {} } },
						deleted: false,
					},
				],
			});
		});

		expect(screen.getByTestId("authority")).toHaveTextContent("1:2");
		expect(screen.getByTestId("attributes")).toHaveTextContent(
			"intensity,zoom",
		);
		expect(view.onRender).toHaveBeenCalledTimes(renders + 1);
	});
});
