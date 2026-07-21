import type { PropsWithChildren } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Cue, VisualizationSnapshot } from "../../api/types";
import type { ShowObject } from "../../features/showObjects/contracts";
import { ShowObjectsStateProvider } from "../../features/showObjects/ShowObjectsState";
import { ShowObjectsStore } from "../../features/showObjects/store";
import { useCueThumbnails } from "./useCueThumbnails";

vi.mock("../../features/patch/PatchState", async (importOriginal) => ({
	...(await importOriginal<Record<string, unknown>>()),
	usePatchedFixturesView: (enabled = true) =>
		enabled ? (mocks.server.patch as { fixtures: unknown[] }).fixtures : [],
}));

const mocks = vi.hoisted(() => {
	const legacyPlaybackAccess = vi.fn();
	const server = {
		patch: {
			fixtures: [
				{
					fixture_id: "stage-fixture",
					universe: 1,
					address: 1,
					logical_heads: [],
					definition: { name: "Stage fixture" },
				},
			],
		},
		stageLayout: null,
		readVisualization: vi.fn(),
	} as Record<string, unknown> & {
		readVisualization: ReturnType<typeof vi.fn>;
	};
	Object.defineProperty(server, "playbacks", {
		get() {
			legacyPlaybackAccess();
			return { authoritative_controls: { groups: [] } };
		},
	});
	return {
		cueVisualization: vi.fn(),
		desktopAvailable: true,
		legacyPlaybackAccess,
		migrateStagePosition: vi.fn(() => ({ x: 0, y: 0, z: 0 })),
		renderStageThumbnail: vi.fn(),
		server,
	};
});

vi.mock("../../api/ServerContext", () => ({
	useServer: () => mocks.server,
}));
vi.mock("../../platform/desktop", () => ({
	useDesktopBridge: () => ({ available: mocks.desktopAvailable }),
}));
vi.mock("../stage3dScene", () => ({
	cueVisualization: mocks.cueVisualization,
	migrateStagePosition: mocks.migrateStagePosition,
	renderStageThumbnail: mocks.renderStageThumbnail,
}));

function visualization(): VisualizationSnapshot {
	return {
		revision: 1,
		generated_at: "2026-07-21T00:00:00Z",
		grand_master: 1,
		blackout: false,
		values: [],
	};
}

function cue(
	number: number,
	groupId = "group-a",
	directFixture = `direct-${number}`,
): Cue {
	return {
		number,
		name: `Cue ${number}`,
		fade_millis: 0,
		delay_millis: 0,
		trigger: { type: "manual" },
		changes: [
			{
				fixture_id: directFixture,
				attribute: "intensity",
				value: { kind: "normalized", value: number / 10 },
			},
		],
		group_changes: [
			{
				group_id: groupId,
				attribute: "tilt",
				value: { kind: "normalized", value: number / 20 },
			},
		],
	};
}

function group(
	id: string,
	fixtures: string[],
	revision = 1,
): ShowObject<"group"> {
	return {
		kind: "group",
		id,
		revision,
		updated_at: `revision-${revision}`,
		body: { name: id, fixtures },
	};
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((complete) => {
		resolve = complete;
	});
	return { promise, resolve };
}

function wrapper(store: ShowObjectsStore) {
	return function Wrapper({ children }: PropsWithChildren) {
		return (
			<ShowObjectsStateProvider store={store}>
				{children}
			</ShowObjectsStateProvider>
		);
	};
}

beforeEach(() => {
	mocks.cueVisualization.mockReset().mockImplementation((state) => state);
	mocks.renderStageThumbnail.mockReset().mockReturnValue("thumbnail");
	mocks.migrateStagePosition.mockClear();
	mocks.server.readVisualization.mockReset();
	mocks.legacyPlaybackAccess.mockClear();
	mocks.desktopAvailable = true;
});

afterEach(cleanup);

describe("useCueThumbnails portable Group authority", () => {
	it("stays dormant while inactive or Group authority is loading", async () => {
		const store = new ShowObjectsStore();
		store.reset("show-a", "authority-a");
		const subscribe = vi.spyOn(store, "subscribe");
		mocks.server.readVisualization.mockResolvedValue(visualization());
		const cues = [cue(1)];
		const { result, rerender } = renderHook(
			({ active }) => useCueThumbnails(cues, active),
			{ initialProps: { active: false }, wrapper: wrapper(store) },
		);

		expect(result.current).toEqual({});
		expect(mocks.server.readVisualization).not.toHaveBeenCalled();
		expect(subscribe).not.toHaveBeenCalled();
		rerender({ active: true });
		expect(result.current).toEqual({});
		expect(mocks.server.readVisualization).not.toHaveBeenCalled();
		expect(subscribe).toHaveBeenCalled();

		act(() => store.setCollection("show-a", "group", []));
		await waitFor(() =>
			expect(mocks.server.readVisualization).toHaveBeenCalledOnce(),
		);
		expect(mocks.legacyPlaybackAccess).not.toHaveBeenCalled();
	});

	it("expands portable Group fixtures in their stored order", async () => {
		const store = new ShowObjectsStore();
		store.reset("show-a", "authority-a");
		store.setCollection("show-a", "group", [
			group("group-a", ["fixture-8", "fixture-2", "fixture-5"]),
		]);
		mocks.server.readVisualization.mockResolvedValue(visualization());
		const cues = [cue(1)];
		const { result, rerender } = renderHook(
			() => useCueThumbnails(cues, true),
			{
				wrapper: wrapper(store),
			},
		);

		await waitFor(() => expect(result.current).toEqual({ 0: "thumbnail" }));
		expect(mocks.cueVisualization).toHaveBeenCalledWith(visualization(), [
			{
				fixture_id: "direct-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.1 },
			},
			{
				fixture_id: "fixture-8",
				attribute: "tilt",
				value: { kind: "normalized", value: 0.05 },
			},
			{
				fixture_id: "fixture-2",
				attribute: "tilt",
				value: { kind: "normalized", value: 0.05 },
			},
			{
				fixture_id: "fixture-5",
				attribute: "tilt",
				value: { kind: "normalized", value: 0.05 },
			},
		]);
		expect(mocks.legacyPlaybackAccess).not.toHaveBeenCalled();
		mocks.desktopAvailable = false;
		rerender();
		expect(result.current).toEqual({});
	});

	it("clears completed thumbnails and cancels pending work on deactivation", async () => {
		const store = new ShowObjectsStore();
		store.reset("show-a", "authority-a");
		store.setCollection("show-a", "group", [group("group-a", ["old"])]);
		mocks.server.readVisualization.mockResolvedValueOnce(visualization());
		const cues = [cue(1)];
		const { result, rerender } = renderHook(
			({ active }) => useCueThumbnails(cues, active),
			{ initialProps: { active: true }, wrapper: wrapper(store) },
		);
		await waitFor(() => expect(result.current).toEqual({ 0: "thumbnail" }));

		const pending = deferred<VisualizationSnapshot>();
		mocks.server.readVisualization.mockReturnValueOnce(pending.promise);
		rerender({ active: false });
		expect(result.current).toEqual({});
		rerender({ active: true });
		await waitFor(() =>
			expect(mocks.server.readVisualization).toHaveBeenCalledTimes(2),
		);
		rerender({ active: false });
		pending.resolve(visualization());
		await act(async () => pending.promise);

		expect(result.current).toEqual({});
		expect(mocks.renderStageThumbnail).toHaveBeenCalledOnce();
		expect(mocks.server.readVisualization).toHaveBeenCalledTimes(2);
	});

	it("accepts only the newest Cue and Group authority after replacements", async () => {
		const store = new ShowObjectsStore();
		store.reset("show-a", "authority-a");
		store.setCollection("show-a", "group", [group("group-a", ["old"])]);
		const first = deferred<VisualizationSnapshot>();
		const second = deferred<VisualizationSnapshot>();
		const third = deferred<VisualizationSnapshot>();
		mocks.server.readVisualization
			.mockReturnValueOnce(first.promise)
			.mockReturnValueOnce(second.promise)
			.mockReturnValueOnce(third.promise);
		const firstCues = [cue(1)];
		const secondCues = [cue(2)];
		const { result, rerender } = renderHook(
			({ cues }) => useCueThumbnails(cues, true),
			{ initialProps: { cues: firstCues }, wrapper: wrapper(store) },
		);
		await waitFor(() =>
			expect(mocks.server.readVisualization).toHaveBeenCalledOnce(),
		);

		rerender({ cues: secondCues });
		await waitFor(() =>
			expect(mocks.server.readVisualization).toHaveBeenCalledTimes(2),
		);
		expect(result.current).toEqual({});
		act(() => {
			store.reset("show-b", "authority-b");
			store.setCollection("show-b", "group", [
				group("group-a", ["new-3", "new-1"], 2),
			]);
		});
		await waitFor(() =>
			expect(mocks.server.readVisualization).toHaveBeenCalledTimes(3),
		);

		third.resolve(visualization());
		await waitFor(() => expect(result.current).toEqual({ 0: "thumbnail" }));
		expect(mocks.cueVisualization).toHaveBeenCalledWith(visualization(), [
			{
				fixture_id: "direct-2",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.2 },
			},
			{
				fixture_id: "new-3",
				attribute: "tilt",
				value: { kind: "normalized", value: 0.1 },
			},
			{
				fixture_id: "new-1",
				attribute: "tilt",
				value: { kind: "normalized", value: 0.1 },
			},
		]);

		first.resolve(visualization());
		second.resolve(visualization());
		await act(async () => Promise.all([first.promise, second.promise]));
		expect(mocks.cueVisualization).toHaveBeenCalledOnce();
		expect(result.current).toEqual({ 0: "thumbnail" });
	});
});
