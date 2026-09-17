import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
	CueMediaPreviewEntry,
	CueMediaPreviewImage,
} from "../../api/client/cueMediaPreviews";
import { useCueMediaPreviews } from "./useCueMediaPreviews";

const mocks = vi.hoisted(() => ({
	actions: {
		available: true,
		canStore: true,
		index: vi.fn(),
		imageUrl: vi.fn(),
		store: vi.fn(),
		mediaIndex: vi.fn(),
		mediaImage: vi.fn(),
	},
	media: {
		mediaServers: [] as Array<{ fixture_id: string; status: { online: boolean } }>,
	},
}));

vi.mock("./CueThumbnailActions", () => ({
	useCueThumbnailActions: () => mocks.actions,
}));
vi.mock("../mediaServers/MediaServersContext", () => ({
	useMediaServers: () => mocks.media,
}));

const PROGRAM_CUE = "11111111-1111-4111-8111-111111111111";
const LAYER_CUE = "22222222-2222-4222-8222-222222222222";
const STAGE_CUE = "33333333-3333-4333-8333-333333333333";

function entry(
	cueId: string,
	overrides: Partial<CueMediaPreviewEntry> = {},
): CueMediaPreviewEntry {
	return {
		cueId,
		cueListId: "list",
		serverFixtureId: "server-a",
		outputId: "output-a",
		scope: "program",
		layer: null,
		layerFixtureId: null,
		previewKey: `${cueId}-key-1`,
		...overrides,
	};
}

function picture(name: string, empty = false): CueMediaPreviewImage {
	return { kind: "picture", blob: new Blob([name]), empty };
}

let objectUrls = 0;

beforeEach(() => {
	objectUrls = 0;
	mocks.actions.available = true;
	mocks.actions.mediaIndex.mockReset().mockResolvedValue([]);
	mocks.actions.mediaImage.mockReset();
	mocks.media.mediaServers = [
		{ fixture_id: "server-a", status: { online: true } },
	];
	vi.stubGlobal("URL", {
		...URL,
		createObjectURL: vi.fn(() => `blob:picture-${++objectUrls}`),
		revokeObjectURL: vi.fn(),
	});
});

afterEach(() => {
	vi.unstubAllGlobals();
	cleanup();
});

describe("Media Server Cue previews", () => {
	it("pictures each media Cue with its own Program or layer image and leaves Stage Cues alone", async () => {
		mocks.actions.mediaIndex.mockResolvedValue([
			entry(PROGRAM_CUE),
			entry(LAYER_CUE, {
				scope: "layer",
				layer: 1,
				layerFixtureId: "layer-2",
				serverFixtureId: "server-b",
				outputId: "output-b",
			}),
			entry("44444444-4444-4444-8444-444444444444"),
		]);
		mocks.actions.mediaImage.mockImplementation(async (cueId: string) =>
			picture(cueId),
		);
		const cueIds = [PROGRAM_CUE, LAYER_CUE, STAGE_CUE];

		const { result } = renderHook(() =>
			useCueMediaPreviews(cueIds, true, "revision-1"),
		);

		await waitFor(() =>
			expect(result.current.previews.get(LAYER_CUE)?.state).toBe("ready"),
		);
		expect(result.current.ready).toBe(true);
		expect([...result.current.mediaCueIds].sort()).toEqual(
			[PROGRAM_CUE, LAYER_CUE].sort(),
		);
		expect(result.current.previews.has(STAGE_CUE)).toBe(false);
		const program = result.current.previews.get(PROGRAM_CUE);
		const layer = result.current.previews.get(LAYER_CUE);
		expect(program?.entry.scope).toBe("program");
		expect(layer?.entry).toMatchObject({
			scope: "layer",
			layer: 1,
			serverFixtureId: "server-b",
		});
		expect(program && "src" in program && program.src).not.toBe(
			layer && "src" in layer && layer.src,
		);
		expect(mocks.actions.mediaImage).toHaveBeenCalledWith(
			PROGRAM_CUE,
			`${PROGRAM_CUE}-key-1`,
			{ width: 320, height: 180 },
		);
		expect(mocks.actions.mediaImage).toHaveBeenCalledTimes(2);
	});

	it("fetches a new picture when the Cue changes and never keeps the previous one", async () => {
		let key = "key-1";
		mocks.actions.mediaIndex.mockImplementation(async () => [
			entry(PROGRAM_CUE, { previewKey: key }),
		]);
		mocks.actions.mediaImage.mockImplementation(async (_cueId, previewKey) =>
			picture(previewKey),
		);
		const cueIds = [PROGRAM_CUE];
		const { result, rerender } = renderHook(
			({ revision }) => useCueMediaPreviews(cueIds, true, revision),
			{ initialProps: { revision: 1 } },
		);
		await waitFor(() =>
			expect(result.current.previews.get(PROGRAM_CUE)?.state).toBe("ready"),
		);
		const before = result.current.previews.get(PROGRAM_CUE);

		key = "key-2";
		rerender({ revision: 2 });

		await waitFor(() =>
			expect(result.current.previews.get(PROGRAM_CUE)?.entry.previewKey).toBe(
				"key-2",
			),
		);
		await waitFor(() =>
			expect(result.current.previews.get(PROGRAM_CUE)?.state).toBe("ready"),
		);
		const after = result.current.previews.get(PROGRAM_CUE);
		expect(after && "src" in after && after.src).not.toBe(
			before && "src" in before && before.src,
		);
		expect(mocks.actions.mediaImage).toHaveBeenLastCalledWith(
			PROGRAM_CUE,
			"key-2",
			{ width: 320, height: 180 },
		);
		expect(URL.revokeObjectURL).toHaveBeenCalledWith(
			before && "src" in before ? before.src : "",
		);
	});

	it("names empty, missing, and offline states, and asks again on retry", async () => {
		const missingCue = "55555555-5555-4555-8555-555555555555";
		const emptyCue = "66666666-6666-4666-8666-666666666666";
		mocks.actions.mediaIndex.mockResolvedValue([
			entry(PROGRAM_CUE),
			entry(missingCue),
			entry(emptyCue),
		]);
		let online = false;
		mocks.actions.mediaImage.mockImplementation(async (cueId: string) => {
			if (cueId === missingCue)
				return {
					kind: "failed",
					state: "missing",
					error: "no output",
					retryable: false,
				};
			if (cueId === emptyCue) return picture("empty", true);
			return online
				? picture("back")
				: {
						kind: "failed",
						state: "offline",
						error: "no answer",
						retryable: true,
					};
		});
		const cueIds = [PROGRAM_CUE, missingCue, emptyCue];
		const { result } = renderHook(() =>
			useCueMediaPreviews(cueIds, true, "revision"),
		);

		await waitFor(() =>
			expect(result.current.previews.get(PROGRAM_CUE)?.state).toBe("offline"),
		);
		await waitFor(() =>
			expect(result.current.previews.get(missingCue)?.state).toBe("missing"),
		);
		expect(result.current.previews.get(emptyCue)?.state).toBe("empty");
		const offline = result.current.previews.get(PROGRAM_CUE);
		expect(offline && "src" in offline).toBe(false);

		online = true;
		act(() => result.current.retry());
		await waitFor(() =>
			expect(result.current.previews.get(PROGRAM_CUE)?.state).toBe("ready"),
		);
		expect(result.current.previews.get(emptyCue)?.state).toBe("empty");
	});

	it("asks again when a Media Server comes back online", async () => {
		mocks.media.mediaServers = [
			{ fixture_id: "server-a", status: { online: false } },
		];
		mocks.actions.mediaIndex.mockResolvedValue([entry(PROGRAM_CUE)]);
		mocks.actions.mediaImage.mockResolvedValue({
			kind: "failed",
			state: "offline",
			error: "no answer",
			retryable: true,
		});
		const cueIds = [PROGRAM_CUE];
		const { result, rerender } = renderHook(() =>
			useCueMediaPreviews(cueIds, true, "revision"),
		);
		await waitFor(() =>
			expect(result.current.previews.get(PROGRAM_CUE)?.state).toBe("offline"),
		);

		mocks.actions.mediaImage.mockResolvedValue(picture("online"));
		mocks.media = {
			mediaServers: [{ fixture_id: "server-a", status: { online: true } }],
		};
		rerender();

		await waitFor(() =>
			expect(result.current.previews.get(PROGRAM_CUE)?.state).toBe("ready"),
		);
	});

	it("waits for a Media Server that is still loading", async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		try {
			mocks.actions.mediaIndex.mockResolvedValue([entry(PROGRAM_CUE)]);
			mocks.actions.mediaImage
				.mockResolvedValueOnce({
					kind: "failed",
					state: "loading",
					error: "loading",
					retryable: true,
				})
				.mockResolvedValue(picture("loaded"));
			const cueIds = [PROGRAM_CUE];
			const { result } = renderHook(() =>
				useCueMediaPreviews(cueIds, true, "revision"),
			);
			await waitFor(() =>
				expect(result.current.previews.get(PROGRAM_CUE)?.state).toBe(
					"loading",
				),
			);
			await act(async () => {
				await vi.advanceTimersByTimeAsync(1_600);
			});
			await waitFor(() =>
				expect(result.current.previews.get(PROGRAM_CUE)?.state).toBe("ready"),
			);
			expect(mocks.actions.mediaImage).toHaveBeenCalledTimes(2);
		} finally {
			vi.useRealTimers();
		}
	});

	it("reports no media Cues when the desk cannot list them", async () => {
		mocks.actions.mediaIndex.mockRejectedValue(new Error("old desk"));
		const cueIds = [PROGRAM_CUE];
		const { result } = renderHook(() =>
			useCueMediaPreviews(cueIds, true, "revision"),
		);
		await waitFor(() => expect(result.current.ready).toBe(true));
		expect(result.current.mediaCueIds.size).toBe(0);
		expect(mocks.actions.mediaImage).not.toHaveBeenCalled();
	});
});
