import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ShowObjectKind } from "../showObjects/contracts";
import { ShowObjectsViewProvider } from "../showObjects/ShowObjectsView";
import { ShowObjectsStore } from "../showObjects/store";
import type {
	ShowObjectsEventObserver,
	ShowObjectsEventScope,
	ShowObjectsEventTransport,
} from "../showObjects/transport";
import type { PlaybackTopologyTransport } from "./contracts";
import {
	PlaybackTopologyProvider,
	usePlaybackTopologyActions,
} from "./PlaybackTopologyProvider";
import { usePlaybackTopologyView } from "./PlaybackTopologyView";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

afterEach(cleanup);

class FakeEvents implements ShowObjectsEventTransport {
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

function collection(kind: ShowObjectKind) {
	if (kind === "cue_list") return { objects: [], showRevision: 7 };
	if (kind === "playback")
		return {
			objects: [
				{
					kind,
					id: "7",
					revision: 1,
					updated_at: "",
					body: {
						number: 7,
						name: "Front",
						target: {
							type: "cue_list",
							cue_list_id: "22222222-2222-4222-8222-222222222222",
						},
						buttons: ["blackout", "pause_dynamics", "flash"],
						fader: "master",
						go_activates: true,
						auto_off: true,
						xfade_millis: 0,
					},
				},
			],
			showRevision: 7,
		};
	return {
		objects: [
			{
				kind: "playback_page",
				id: "1",
				revision: 1,
				updated_at: "",
				body: { number: 1, name: "Main", slots: { 1: 7 } },
			},
		],
		showRevision: 7,
	};
}

function Consumer({
	active,
	onRender = () => undefined,
}: {
	active: boolean;
	onRender?: () => void;
}) {
	onRender();
	const view = usePlaybackTopologyView(active);
	return (
		<div>
			<span data-testid="ready">{String(view.ready)}</span>
			<span data-testid="playbacks">{view.playbacks.length}</span>
		</div>
	);
}

function ActionConsumer() {
	const actions = usePlaybackTopologyActions();
	return (
		<button
			type="button"
			onClick={() => void actions?.mapExistingPlayback(1, 1, 7)}
		>
			Map existing
		</button>
	);
}

function harness(active: boolean, onRender?: () => void) {
	const store = new ShowObjectsStore();
	const events = new FakeEvents();
	const loadCollection = vi.fn(async (_show: string, kind: ShowObjectKind) =>
		collection(kind),
	);
	const loadObject = vi.fn(async () => ({ object: null, showRevision: 7 }));
	const actionTransport: PlaybackTopologyTransport = {
		apply: vi.fn(),
	};
	const rendered = render(
		<ShowObjectsViewProvider
			showId={SHOW_ID}
			authorityKey="session-a"
			store={store}
			transport={events}
			loadCollection={loadCollection as never}
			loadObject={loadObject as never}
		>
			<PlaybackTopologyProvider
				showId={SHOW_ID}
				store={store}
				transport={actionTransport}
				loadObject={vi.fn()}
			>
				<Consumer active={active} onRender={onRender} />
			</PlaybackTopologyProvider>
		</ShowObjectsViewProvider>,
	);
	return {
		rendered,
		store,
		events,
		loadCollection,
		loadObject,
		actionTransport,
	};
}

describe("Playback topology scoped composition", () => {
	it("performs no snapshot, socket, or action before the first active view", async () => {
		const onRender = vi.fn();
		const { events, loadCollection, loadObject, actionTransport, store } =
			harness(false, onRender);
		await Promise.resolve();
		expect(screen.getByTestId("ready")).toHaveTextContent("false");
		expect(screen.getByTestId("playbacks")).toHaveTextContent("0");
		expect(loadCollection).not.toHaveBeenCalled();
		expect(loadObject).not.toHaveBeenCalled();
		expect(events.subscriptions).toHaveLength(0);
		expect(actionTransport.apply).not.toHaveBeenCalled();
		const dormantRenderCount = onRender.mock.calls.length;
		act(() => store.setCollection(SHOW_ID, "playback", [], 1, 8));
		expect(onRender).toHaveBeenCalledTimes(dormantRenderCount);
	});

	it("hydrates and subscribes to only the three portable topology kinds", async () => {
		const { events, loadCollection } = harness(true);
		expect(screen.getByTestId("ready")).toHaveTextContent("false");

		await waitFor(() => expect(loadCollection).toHaveBeenCalledTimes(3));
		await waitFor(() =>
			expect(screen.getByTestId("ready")).toHaveTextContent("true"),
		);
		expect(screen.getByTestId("playbacks")).toHaveTextContent("1");
		expect(loadCollection.mock.calls.map((call) => call[1]).sort()).toEqual([
			"cue_list",
			"playback",
			"playback_page",
		]);
		await waitFor(() => expect(events.subscriptions).toHaveLength(1));
		expect(events.subscriptions[0].scope).toEqual({
			kinds: ["cue_list", "playback", "playback_page"],
			objects: [],
		});
	});

	it("marks dormant collections unready before a later remount", async () => {
		const { rendered, store, events } = harness(true);
		await waitFor(() => expect(store.isCollectionReady("playback")).toBe(true));

		rendered.unmount();

		expect(store.isCollectionReady("cue_list")).toBe(false);
		expect(store.isCollectionReady("playback")).toBe(false);
		expect(store.isCollectionReady("playback_page")).toBe(false);
		expect(events.subscriptions[0].close).toHaveBeenCalledOnce();
	});

	it("keeps its action writer live through StrictMode effect replay", async () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		for (const kind of ["cue_list", "playback", "playback_page"] as const)
			store.setCollection(
				SHOW_ID,
				kind,
				collection(kind).objects as never,
				5,
				7,
			);
		const apply = vi.fn();

		render(
			<StrictMode>
				<PlaybackTopologyProvider
					showId={SHOW_ID}
					store={store}
					transport={{ apply }}
					loadObject={vi.fn()}
				>
					<ActionConsumer />
				</PlaybackTopologyProvider>
			</StrictMode>,
		);
		await Promise.resolve();
		fireEvent.click(screen.getByRole("button", { name: "Map existing" }));

		await waitFor(() => expect(apply).toHaveBeenCalledOnce());
	});
});
