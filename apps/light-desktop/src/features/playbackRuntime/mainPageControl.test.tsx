import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { useSyncExternalStore } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PlaybackPageControl } from "../../components/control/PlaybackPageControl";
import { PlaybackPageMenu } from "../../components/control/PlaybackPageDialogs";
import { PlaybackRuntimeActionWriter } from "./actionWriter";
import { PlaybackRuntimeStore } from "./store";
import {
	DESK_ID,
	deskProjection,
	playbackSnapshot,
	SHOW_ID,
} from "./testFixtures";

let store: PlaybackRuntimeStore;
let writer: PlaybackRuntimeActionWriter;
const pages = [1, 2, 3, 4].map((number) => ({
	kind: "playback_page",
	id: String(number),
	revision: 1,
	updated_at: "",
	body: { number, name: `Page ${number}`, assignments: {} },
}));
const topologyActions = { createPage: vi.fn(), renamePage: vi.fn() };
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: { playbackSetArmed: false }, dispatch: vi.fn() }),
}));
vi.mock("../playbackTopology/PlaybackTopologyProvider", () => ({
	usePlaybackTopologyActions: () => topologyActions,
}));
vi.mock("../playbackTopology/PlaybackTopologyView", () => ({
	usePlaybackPagesView: () => ({ ready: true, pages, error: null }),
}));
vi.mock("./PlaybackRuntimeView", () => ({
	usePlaybackDeskView: () =>
		useSyncExternalStore(store.subscribe, store.getSnapshot).desk,
	usePlaybackRuntimeActions: () => writer,
	usePlaybackRuntimeStatus: () => ({ status: "ready" }),
}));
afterEach(cleanup);
function setup(follow: boolean) {
	store = new PlaybackRuntimeStore();
	store.reset(SHOW_ID, DESK_ID, "main-pager");
	const snapshot = playbackSnapshot([]);
	const original = {
		...deskProjection(follow ? 2 : 4),
		selected_surface_id: "pane:test",
		resolved_pages: { main: 2, "pane:test": follow ? 2 : 4, pinned: 1 },
	};
	store.installSnapshot({ ...snapshot, desk: original }, []);
	const apply = vi.fn(async (_desk: string, page: number, surface: string) => {
		if (surface === "pane:test" && follow)
			throw new Error("Only an Own page selection can be changed directly");
		const current = store.getSnapshot().desk;
		if (!current) throw new Error("Missing test desk");
		store.applyDesk(
			{
				...current,
				command_page:
					surface === "main" && follow ? page : current.command_page,
				resolved_pages: {
					...current.resolved_pages,
					[surface]: page,
					...(surface === "main" && follow ? { "pane:test": page } : {}),
				},
			},
			30,
		);
		return {
			desk_id: DESK_ID,
			page,
			event_sequence: 30,
			page_creation_event_sequence: null,
		};
	});
	writer = new PlaybackRuntimeActionWriter({
		showId: SHOW_ID,
		deskId: DESK_ID,
		store,
		applyAction: vi.fn(),
		applyDeskPage: apply,
	});
	render(
		<ModalProvider>
			<PlaybackPageControl />
		</ModalProvider>,
	);
	return { apply, original };
}
describe("visible Main page controls use their own action boundary", () => {
	it("changes Main while selected follower reflects it, without breaking Follow", async () => {
		const { apply } = setup(true);
		fireEvent.click(screen.getByRole("button", { name: "Next playback page" }));
		await waitFor(() => expect(apply).toHaveBeenCalledWith(DESK_ID, 3, "main"));
		expect(store.getSnapshot().desk).toMatchObject({
			selected_surface_id: "pane:test",
			command_page: 3,
			resolved_pages: { main: 3, "pane:test": 3, pinned: 1 },
		});
		expect(
			screen.queryByText("Playback Page 3 could not be selected."),
		).not.toBeInTheDocument();
	});
	it("shows Main page and isolates an independently selected Own pane", async () => {
		const { apply } = setup(false);
		expect(
			screen.getByRole("button", {
				name: "Select playback page. Page 2 Page 2",
			}),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Next playback page" }));
		await waitFor(() => expect(apply).toHaveBeenCalledWith(DESK_ID, 3, "main"));
		expect(store.getSnapshot().desk).toMatchObject({
			command_page: 4,
			resolved_pages: { main: 3, "pane:test": 4, pinned: 1 },
		});
	});
	it("keeps the Main page picker on Main when another pane is selected", async () => {
		const { apply } = setup(false);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Select playback page. Page 2 Page 2",
			}),
		);
		fireEvent.click(screen.getByRole("button", { name: "1Page 1" }));
		await waitFor(() => expect(apply).toHaveBeenCalledWith(DESK_ID, 1, "main"));
		expect(store.getSnapshot().desk?.resolved_pages["pane:test"]).toBe(4);
	});
	it("does not send a missing explicit surface or corrupt selected-page optimism", async () => {
		const { apply } = setup(false);
		await expect(writer.setActivePage(3, "missing")).resolves.toBe(false);
		await expect(writer.setActivePage(3, "toString")).resolves.toBe(false);
		expect(apply).not.toHaveBeenCalled();
		expect(store.getSnapshot().desk).toMatchObject({
			command_page: 4,
			resolved_pages: { main: 2, "pane:test": 4 },
		});
	});
	it("isolates optimistic Main changes and rollback from an independent selected pane", async () => {
		setup(false);
		let reject!: (error: Error) => void;
		const apply = vi.fn(
			() =>
				new Promise<never>((_resolve, fail) => {
					reject = fail;
				}),
		);
		const action = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: vi.fn(),
			applyDeskPage: apply,
		});
		const result = action.setActivePage(3, "main");
		expect(store.getSnapshot().desk).toMatchObject({
			command_page: 4,
			resolved_pages: { main: 3, "pane:test": 4 },
		});
		reject(new Error("page refused"));
		await expect(result).resolves.toBe(false);
		expect(apply).toHaveBeenCalledOnce();
		expect(store.getSnapshot().desk).toMatchObject({
			command_page: 4,
			resolved_pages: { main: 2, "pane:test": 4 },
		});
	});
	it("keeps the hardware/shared generic picker on its selected surface", async () => {
		const { apply } = setup(false);
		cleanup();
		render(
			<ModalProvider>
				<PlaybackPageMenu open onClose={vi.fn()} />
			</ModalProvider>,
		);
		fireEvent.click(screen.getByRole("button", { name: "1Page 1" }));
		await waitFor(() =>
			expect(apply).toHaveBeenCalledWith(DESK_ID, 1, "pane:test"),
		);
		expect(store.getSnapshot().desk?.resolved_pages.main).toBe(2);
	});
	it("retains generic current-surface page writes for independent pane/OSC callers", async () => {
		const { apply } = setup(false);
		await expect(writer.setActivePage(3)).resolves.toBe(true);
		expect(apply).toHaveBeenCalledWith(DESK_ID, 3, "pane:test");
	});
});
