import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaybackDefinition } from "../../api/types";
import type { ShowObject, ShowObjectKind } from "../../features/showObjects/contracts";
import { ShowObjectsStore } from "../../features/showObjects/store";
import type {
	ShowObjectsEventObserver,
	ShowObjectsEventScope,
	ShowObjectsEventTransport,
} from "../../features/showObjects/transport";
import { ShowObjectsViewProvider } from "../../features/showObjects/ShowObjectsView";
import { PaneSettingsModal } from "./PaneSettingsModal";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const REPLACEMENT_SHOW_ID = "22222222-2222-4222-8222-222222222222";

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	state: {
		activeDeskId: "desk-1",
		paneSettingsId: null as string | null,
		presetFamily: "Mixed" as const,
		desks: [
			{
				id: "desk-1",
				name: "Desk 1",
				panes: [
					{
						id: "cues-1",
						kind: "cues" as const,
						title: "Cues",
						x: 1,
						y: 1,
						width: 6,
						height: 6,
					},
					{
						id: "stage-1",
						kind: "stage" as const,
						title: "Stage",
						x: 1,
						y: 1,
						width: 6,
						height: 6,
					},
				],
			},
		],
	},
}));

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));

function playbackObject(
	number: number,
	name: string,
	target: PlaybackDefinition["target"],
): ShowObject<"playback"> {
	return {
		kind: "playback",
		id: String(number),
		revision: 1,
		updated_at: "",
		body: {
			number,
			name,
			target,
			buttons: ["go", "go_minus", "flash"],
			fader: "master",
			go_activates: true,
			auto_off: true,
			xfade_millis: 0,
		},
	};
}

/** Deliberately unsorted so the picker's own Playback-number ordering is proven. */
const PLAYBACKS: ShowObject<"playback">[] = [
	playbackObject(9, "Rear wash", { type: "cue_list", cue_list_id: "rear" }),
	playbackObject(2, "Grand", { type: "grand_master" }),
	playbackObject(3, "Front wash", { type: "cue_list", cue_list_id: "front" }),
	playbackObject(5, "Movers", { type: "group", group_id: "movers" }),
];

class FakeTransport implements ShowObjectsEventTransport {
	readonly subscriptions: ShowObjectsEventScope[] = [];
	readonly closes: ReturnType<typeof vi.fn>[] = [];

	subscribe(
		_showId: string,
		scope: ShowObjectsEventScope,
		_afterSequence: number | null,
		_observer: ShowObjectsEventObserver,
	) {
		this.subscriptions.push(scope);
		const close = vi.fn();
		this.closes.push(close);
		return { close, repair: vi.fn() };
	}
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

function harness() {
	const transport = new FakeTransport();
	const store = new ShowObjectsStore();
	const pending: Array<{
		kind: ShowObjectKind;
		resolve: (value: { objects: ShowObject[]; showRevision: number }) => void;
	}> = [];
	const loadCollection = vi.fn((_showId: string, kind: ShowObjectKind) => {
		const gate = deferred<{ objects: ShowObject[]; showRevision: number }>();
		pending.push({ kind, resolve: gate.resolve });
		return gate.promise;
	});
	const view = render(
		<ShowObjectsViewProvider
			showId={SHOW_ID}
			store={store}
			transport={transport}
			loadCollection={loadCollection}
			loadObject={vi.fn()}
		>
			<PaneSettingsModal />
		</ShowObjectsViewProvider>,
	);
	const rerender = (showId: string) =>
		view.rerender(
			<ShowObjectsViewProvider
				showId={showId}
				store={store}
				transport={transport}
				loadCollection={loadCollection}
				loadObject={vi.fn()}
			>
				<PaneSettingsModal />
			</ShowObjectsViewProvider>,
		);
	const settle = async (objects: ShowObject[] = PLAYBACKS) => {
		await waitFor(() => expect(pending.length).toBeGreaterThan(0));
		for (const run of pending.splice(0))
			run.resolve({ objects: run.kind === "playback" ? objects : [], showRevision: 1 });
	};
	return { loadCollection, pending, rerender, settle, transport, view };
}

function openCuesTab() {
	fireEvent.click(screen.getByRole("tab", { name: "Cues" }));
}

function cuelistOptionLabels() {
	const label = [...document.querySelectorAll("label")].find(
		(candidate) => candidate.textContent === "Cuelist",
	);
	const field = label?.closest(".ui-form-field") as HTMLElement;
	fireEvent.click(within(field).getByRole("button"));
	const listbox = screen.getByRole("listbox", { name: "Cuelist" });
	return within(listbox)
		.getAllByRole("option")
		.map((option) => option.textContent);
}

beforeEach(() => {
	mocks.dispatch.mockReset();
	mocks.state.paneSettingsId = null;
});

afterEach(cleanup);

describe("Cues Pane Settings Cuelist authority", () => {
	it("requests no Playback snapshot or socket before Pane Settings mounts", async () => {
		const { loadCollection, transport } = harness();

		await Promise.resolve();

		expect(loadCollection).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);
	});

	it("stays dormant while Pane Settings is open for another pane kind", async () => {
		mocks.state.paneSettingsId = "stage-1";
		const { loadCollection, transport } = harness();

		expect(await screen.findByRole("tab", { name: "Stage" })).toBeInTheDocument();
		expect(loadCollection).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);
	});

	it("hydrates only Playback definitions while the modal is mounted for a Cues pane", async () => {
		mocks.state.paneSettingsId = "cues-1";
		const { loadCollection, settle, transport } = harness();

		await settle();

		expect(loadCollection.mock.calls.map(([, kind]) => kind)).toEqual([
			"playback",
		]);
		expect(transport.subscriptions).toEqual([
			{ kinds: ["playback"], objects: [] },
		]);
	});

	it("offers only Cuelist Playbacks, ordered by Playback number, with exact labels", async () => {
		mocks.state.paneSettingsId = "cues-1";
		const { settle } = harness();
		await settle();

		openCuesTab();

		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "3 · Front wash" }),
			).toBeInTheDocument(),
		);
		expect(cuelistOptionLabels()).toEqual(["3 · Front wash", "9 · Rear wash"]);
	});

	it("selects a Cuelist Playback by number without a broad Playback read", async () => {
		mocks.state.paneSettingsId = "cues-1";
		const { loadCollection, settle } = harness();
		await settle();
		openCuesTab();
		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "3 · Front wash" }),
			).toBeInTheDocument(),
		);

		cuelistOptionLabels();
		fireEvent.click(screen.getByRole("option", { name: "9 · Rear wash" }));

		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_CUELIST",
			id: "cues-1",
			number: 9,
		});
		expect(loadCollection.mock.calls.map(([, kind]) => kind)).toEqual([
			"playback",
		]);
	});

	it("does not expose cached Playback definitions while a replacement show loads", async () => {
		mocks.state.paneSettingsId = "cues-1";
		const { rerender, settle } = harness();
		await settle();
		openCuesTab();
		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "3 · Front wash" }),
			).toBeInTheDocument(),
		);

		rerender(REPLACEMENT_SHOW_ID);

		await waitFor(() =>
			expect(
				screen.queryByRole("button", { name: "3 · Front wash" }),
			).not.toBeInTheDocument(),
		);
		expect(
			screen.queryByRole("button", { name: "9 · Rear wash" }),
		).not.toBeInTheDocument();
		expect(mocks.dispatch).not.toHaveBeenCalled();

		await settle([
			playbackObject(4, "House", { type: "cue_list", cue_list_id: "house" }),
		]);

		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "4 · House" }),
			).toBeInTheDocument(),
		);
	});

	it("closes its Playback subscription when Pane Settings closes", async () => {
		mocks.state.paneSettingsId = "cues-1";
		const { settle, transport, view } = harness();
		await settle();
		expect(transport.subscriptions).toHaveLength(1);

		view.unmount();

		expect(transport.closes[0]).toHaveBeenCalled();
	});
});
