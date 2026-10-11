import { render, cleanup } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PlaybackArtworkModel } from "@tosklight/ui/playback";
import type { CueList, PlaybackDefinition } from "../../../api/types";
import { usePlaybackArtwork } from "./usePlaybackArtwork";
import { PlaybackArtworkProvider } from "./PlaybackArtworkProvider";

const mocks = vi.hoisted(() => ({
	media: {
		ready: true,
		mediaCueIds: new Set<string>(),
		previews: new Map(),
		retry: vi.fn(),
	},
	mediaHook: vi.fn(),
	stageHook: vi.fn(),
	thumbnails: { 0: "stage.png" },
}));
vi.mock("../../../features/cueThumbnails/useCueMediaPreviews", () => ({
	useCueMediaPreviews: (...args: unknown[]) => {
		mocks.mediaHook(...args);
		return mocks.media;
	},
}));
vi.mock("../../../windows/cuelistWindow/useCueThumbnails", () => ({
	useCueThumbnails: (...args: unknown[]) => {
		mocks.stageHook(...args);
		return mocks.thumbnails;
	},
}));
const playback = {
	target: { type: "cue_list", cue_list_id: "list-a" },
} as PlaybackDefinition;
const cues = [{ id: "cue-a", number: "1" }];
const list = { cues } as CueList;
function Probe({
	assignment,
	value,
	read,
}: {
	assignment: PlaybackDefinition;
	value: CueList;
	read: (artwork: PlaybackArtworkModel | undefined) => void;
}) {
	read(usePlaybackArtwork(assignment, value));
	return null;
}
function renderArtwork() {
	let result: PlaybackArtworkModel | undefined;
	const view = (assignment: PlaybackDefinition, value: CueList) => (
		<PlaybackArtworkProvider
			candidates={[{ playback: assignment, cue: value }]}
		>
			<Probe
				assignment={assignment}
				value={value}
				read={(artwork) => {
					result = artwork;
				}}
			/>
		</PlaybackArtworkProvider>
	);
	const rendered = render(view(playback, list));
	return {
		get result() {
			return result;
		},
		update: (assignment: PlaybackDefinition, value: CueList) =>
			rendered.rerender(view(assignment, value)),
	};
}
afterEach(() => {
	cleanup();
	vi.clearAllMocks();
	mocks.media.previews.clear();
	mocks.media.mediaCueIds.clear();
});

describe("canonical Playback artwork authority", () => {
	it("uses the exact single Cue's media image and scope, never its Stage fallback", () => {
		mocks.media.mediaCueIds.add("cue-a");
		mocks.media.previews.set("cue-a", {
			state: "ready",
			src: "layer.png",
			entry: { cueId: "cue-a", scope: "layer" },
		});
		const state = renderArtwork();
		expect(state.result).toEqual({
			image: "layer.png",
			imageSource: "cue-preview",
			transparent: true,
			notice: undefined,
		});
		expect(mocks.mediaHook).toHaveBeenLastCalledWith(["cue-a"], true, cues);
		expect(mocks.stageHook).toHaveBeenLastCalledWith(cues, true, {
			exclude: mocks.media.mediaCueIds,
			independent: true,
		});
		mocks.media.previews.set("cue-a", {
			state: "offline",
			error: "offline",
			entry: { cueId: "cue-a", scope: "program" },
		});
		state.update(playback, { ...list, cues: [...list.cues] });
		// Test authority mocks mutate their Map; a new authored Cue invalidates the batch like a real edit.
		state.update(playback, { ...list, cues: [{ ...list.cues[0] }] });
		expect(state.result).toEqual({
			image: undefined,
			imageSource: undefined,
			transparent: false,
			notice: "Media Server offline",
		});
	});
	it("chosen icon/image disables automatic requests and a second Cue removes automatic artwork", () => {
		const state = renderArtwork();
		expect(state.result).toEqual({
			image: "stage.png",
			imageSource: "cue-preview",
		});
		state.update({ ...playback, presentation_image: "chosen.png" }, list);
		expect(state.result).toEqual({
			image: "chosen.png",
			imageSource: "configured",
		});
		expect(mocks.mediaHook).toHaveBeenLastCalledWith([], false, []);
		state.update(
			{ ...playback, presentation_icon: "★", presentation_image: "chosen.png" },
			list,
		);
		expect(state.result).toEqual({ icon: "★" });
		state.update(playback, {
			...list,
			cues: [...list.cues, { ...list.cues[0], id: "cue-b", number: "2" }],
		});
		expect(state.result).toBeUndefined();
		state.update(playback, list);
		expect(state.result).toEqual({
			image: "stage.png",
			imageSource: "cue-preview",
		});
	});
	it("280 visible cards share one deduplicated batch stable across runtime-only slot changes", () => {
		const candidates = Array.from({ length: 280 }, () => ({
			playback,
			cue: list,
		}));
		const view = (slots: typeof candidates) => (
			<PlaybackArtworkProvider candidates={slots}>
				{slots.map((slot, index) => (
					<Probe
						key={index}
						assignment={slot.playback}
						value={slot.cue}
						read={() => undefined}
					/>
				))}
			</PlaybackArtworkProvider>
		);
		const rendered = render(view(candidates));
		expect(mocks.mediaHook).toHaveBeenCalledTimes(1);
		expect(mocks.stageHook).toHaveBeenCalledTimes(1);
		const batch = mocks.mediaHook.mock.calls[0][2];
		rendered.rerender(view(candidates.map((slot) => ({ ...slot }))));
		expect(mocks.mediaHook).toHaveBeenLastCalledWith(["cue-a"], true, batch);
		expect(mocks.mediaHook.mock.calls[1][2]).toBe(batch);
	});
});
