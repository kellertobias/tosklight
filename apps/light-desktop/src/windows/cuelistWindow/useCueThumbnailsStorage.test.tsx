import type { PropsWithChildren } from "react";
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Cue, VisualizationSnapshot } from "../../api/types";
import { ShowObjectsStateProvider } from "../../features/showObjects/ShowObjectsState";
import { ShowObjectsStore } from "../../features/showObjects/store";
import { cueStateHash, stageGeometryTag } from "./cueThumbnailState";
import { useCueThumbnails } from "./useCueThumbnails";

vi.mock("../../features/patch/PatchState", async (importOriginal) => ({
	...(await importOriginal<Record<string, unknown>>()),
	// Stable identity: the hook keys its work on the fixture array, so a fresh array per render
	// would restart it forever.
	usePatchedFixturesView: (enabled = true) => (enabled ? PATCHED : EMPTY),
}));

const { PATCHED, EMPTY, STAGE_POSITION } = vi.hoisted(() => ({
	PATCHED: [
		{
			fixture_id: "stage-fixture",
			universe: 1,
			address: 1,
			logical_heads: [],
			definition: { name: "Stage fixture" },
		},
	],
	EMPTY: [] as unknown[],
	STAGE_POSITION: {
		x: 1,
		y: 2,
		z: 3,
		rotationX: 0,
		rotationY: 0,
		rotationZ: 0,
	},
}));

const mocks = vi.hoisted(() => ({
	canDraw: true,
	cueVisualization: vi.fn(),
	migrateStagePosition: vi.fn(() => STAGE_POSITION),
	previews: {
		available: true,
		canStore: true,
		index: vi.fn(),
		imageUrl: vi.fn(),
		store: vi.fn(),
	},
	readVisualization: vi.fn(),
	renderStageThumbnail: vi.fn(),
}));

vi.mock(
	"../../features/visualizationRuntime/VisualizationRuntimeView",
	async (importOriginal) => ({
		...(await importOriginal<Record<string, unknown>>()),
		useVisualizationRuntimeRead: () => mocks.readVisualization,
	}),
);
vi.mock("../../platform/desktop", () => ({
	useDesktopBridge: () => ({ available: mocks.canDraw }),
}));
vi.mock("../../features/cueThumbnails/CueThumbnailActions", () => ({
	useCueThumbnailActions: () => mocks.previews,
}));
vi.mock("../stage3dScene", () => ({
	cueVisualization: mocks.cueVisualization,
	migrateStagePosition: mocks.migrateStagePosition,
	renderStageThumbnail: mocks.renderStageThumbnail,
}));

const CUE_ID = "11111111-1111-4111-8111-111111111111";
const DRAWN = "data:image/webp;base64,UklGRgAAAABXRUJQ";

function visualization(): VisualizationSnapshot {
	return {
		revision: 1,
		generated_at: "2026-08-07T00:00:00Z",
		grand_master: 1,
		blackout: false,
		values: [],
	};
}

function cue(): Cue {
	return {
		id: CUE_ID,
		number: 1,
		name: "Cue 1",
		fade_millis: 0,
		delay_millis: 0,
		trigger: { type: "manual" },
		changes: [],
	};
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

function readyStore() {
	const store = new ShowObjectsStore();
	store.reset("show-a", "authority-a");
	store.setCollection("show-a", "group", []);
	return store;
}

/**
 * The tag the hook will compute for the single cue in these tests, derived the same way the hook
 * derives it so the test asserts agreement rather than restating the formula.
 */
function expectedHash() {
	return cueStateHash(
		[],
		stageGeometryTag([
			{ instanceId: "stage-fixture", position: STAGE_POSITION },
		]),
	);
}

beforeEach(() => {
	mocks.canDraw = true;
	mocks.cueVisualization.mockReset().mockImplementation((state) => state);
	mocks.renderStageThumbnail.mockReset().mockReturnValue(DRAWN);
	mocks.readVisualization.mockReset().mockResolvedValue(visualization());
	mocks.previews.available = true;
	mocks.previews.canStore = true;
	mocks.previews.index.mockReset().mockResolvedValue([]);
	mocks.previews.imageUrl.mockReset().mockResolvedValue("blob:stored-picture");
	mocks.previews.store.mockReset().mockResolvedValue(undefined);
	vi.stubGlobal("URL", {
		...URL,
		createObjectURL: vi.fn(() => "blob:stored-picture"),
		revokeObjectURL: vi.fn(),
	});
});

afterEach(() => {
	vi.unstubAllGlobals();
	cleanup();
});

describe("persisted cue previews", () => {
	it("draws nothing when the show already holds a current picture", async () => {
		mocks.previews.index.mockResolvedValue([
			{
				cueId: CUE_ID,
				stateHash: expectedHash(),
				updatedAt: "2026-08-07T00:00:00Z",
			},
		]);
		const cues = [cue()];

		const { result } = renderHook(() => useCueThumbnails(cues, true), {
			wrapper: wrapper(readyStore()),
		});

		await waitFor(() =>
			expect(result.current).toEqual({ 0: "blob:stored-picture" }),
		);
		expect(mocks.renderStageThumbnail).not.toHaveBeenCalled();
		expect(mocks.previews.store).not.toHaveBeenCalled();
		expect(mocks.previews.imageUrl).toHaveBeenCalledWith(CUE_ID);
	});

	it("redraws and stores a picture the show does not hold", async () => {
		const cues = [cue()];

		const { result } = renderHook(() => useCueThumbnails(cues, true), {
			wrapper: wrapper(readyStore()),
		});

		await waitFor(() => expect(result.current).toEqual({ 0: DRAWN }));
		expect(mocks.renderStageThumbnail).toHaveBeenCalledOnce();
		await waitFor(() => expect(mocks.previews.store).toHaveBeenCalledOnce());
		expect(mocks.previews.store).toHaveBeenCalledWith([
			{
				cueId: CUE_ID,
				stateHash: expectedHash(),
				imageBase64: "UklGRgAAAABXRUJQ",
				width: 240,
				height: 135,
			},
		]);
	});

	it("redraws a picture whose stored state no longer matches", async () => {
		mocks.previews.index.mockResolvedValue([
			{
				cueId: CUE_ID,
				stateHash: "a-look-that-has-since-changed",
				updatedAt: "2026-08-07T00:00:00Z",
			},
		]);
		const cues = [cue()];

		const { result } = renderHook(() => useCueThumbnails(cues, true), {
			wrapper: wrapper(readyStore()),
		});

		await waitFor(() => expect(result.current).toEqual({ 0: DRAWN }));
		expect(mocks.renderStageThumbnail).toHaveBeenCalledOnce();
		expect(mocks.previews.imageUrl).not.toHaveBeenCalled();
	});

	it("shows stored pictures on a desk that cannot draw them", async () => {
		mocks.canDraw = false;
		mocks.previews.index.mockResolvedValue([
			{
				cueId: CUE_ID,
				stateHash: expectedHash(),
				updatedAt: "2026-08-07T00:00:00Z",
			},
		]);
		const cues = [cue()];

		const { result } = renderHook(() => useCueThumbnails(cues, true), {
			wrapper: wrapper(readyStore()),
		});

		await waitFor(() =>
			expect(result.current).toEqual({ 0: "blob:stored-picture" }),
		);
		expect(mocks.renderStageThumbnail).not.toHaveBeenCalled();
	});

	it("leaves a cue without a picture rather than drawing one it cannot store", async () => {
		mocks.canDraw = false;
		const cues = [cue()];

		const { result } = renderHook(() => useCueThumbnails(cues, true), {
			wrapper: wrapper(readyStore()),
		});

		await waitFor(() => expect(mocks.previews.index).toHaveBeenCalled());
		expect(result.current).toEqual({});
		expect(mocks.renderStageThumbnail).not.toHaveBeenCalled();
	});

	it("still shows what it drew on a desk that may not write the show", async () => {
		mocks.previews.canStore = false;
		const cues = [cue()];

		const { result } = renderHook(() => useCueThumbnails(cues, true), {
			wrapper: wrapper(readyStore()),
		});

		await waitFor(() => expect(result.current).toEqual({ 0: DRAWN }));
		expect(mocks.previews.store).not.toHaveBeenCalled();
	});
});
