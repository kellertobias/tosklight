// @vitest-environment jsdom
import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ShowObjectActionOutcome } from "../api/types";
import type {
	TimecodeAudioImportResult,
	TimecodeDefinition,
	TimecodePatch,
} from "../api/types/timecode";
import { TimecodeEditor } from "./TimecodeRuntimeWindow";

const observed = vi.hoisted(() => ({
	onFiles: null as ((files: File[]) => Promise<void>) | null,
	definition: null as TimecodeDefinition | null,
	peaks: undefined as readonly number[] | undefined,
}));
vi.mock("../components/files/RootConfinedFilePickerButton", () => ({
	RootConfinedFilePickerButton: (props: {
		label: string;
		disabled: boolean;
		onFiles(files: File[]): Promise<void>;
	}) => {
		observed.onFiles = props.onFiles;
		return (
			<button type="button" disabled={props.disabled}>
				{props.label}
			</button>
		);
	},
}));
vi.mock("../features/timecode/TimecodeTimelineEditor", () => ({
	TimecodeTimelineEditor: (props: {
		definition: TimecodeDefinition;
		waveformPeaks?: readonly number[];
	}) => {
		observed.definition = props.definition;
		observed.peaks = props.waveformPeaks;
		return <div aria-label="Timeline" />;
	},
}));
afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((yes) => {
		resolve = yes;
	});
	return { promise, resolve };
}

describe("Timecode audio replacement", () => {
	it.each([
		{ extent: "clip", markerFrame: 660, clipEnd: 880 },
		{ extent: "marker", markerFrame: 990, clipEnd: 880 },
	])("preserves concurrent name edits and the final $extent, then loads the saved waveform", async ({
		markerFrame,
		clipEnd,
	}) => {
		// Browser decoding is unavailable: the editor must use the managed server waveform.
		vi.stubGlobal("AudioContext", undefined);
		const imported = deferred<TimecodeAudioImportResult>();
		const nameSaved = deferred<void>();
		const audioSaved = deferred<void>();
		let revision = 4;
		let server = definition(markerFrame, clipEnd);
		const item = { revision, definition: structuredClone(server) };
		const update = vi.fn(
			async (
				_showId: string,
				_id: string,
				expectedRevision: number,
				patch: TimecodePatch,
			) => {
				expect(expectedRevision).toBe(revision);
				if (patch.audio) await audioSaved.promise;
				else await nameSaved.promise;
				server = { ...server, ...patch };
				revision += 1;
				return outcome(revision, server);
			},
		);
		const waveform = vi.fn(async () => ({
			peaks: server.audio?.asset_id === "replacement" ? [0.8, 0.3] : [0.1, 0.2],
		}));
		const api = {
			create: vi.fn(),
			update,
			importAudio: vi.fn(() => imported.promise),
			waveform,
			objects: vi.fn(async () => ({
				show_revision: revision,
				objects: [{ revision, definition: server }],
			})),
		};
		render(
			<TimecodeEditor
				showId="show"
				item={item}
				api={api as never}
				cueLists={[]}
				audioPlayers={[]}
				onClose={vi.fn()}
			/>,
		);
		await screen.findByText("Saved");
		await waitFor(() => expect(observed.peaks).toEqual([0.1, 0.2]));
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		await screen.findByRole("button", { name: "Change Selected File" });
		let importing!: Promise<void>;
		act(() => {
			importing = observed.onFiles!([new File(["mp3"], "short.mp3")]);
		});
		fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
			target: { value: "Edited during import" },
		});
		await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
		expect(update.mock.calls[0]?.[3]).toEqual({ name: "Edited during import" });
		await act(async () => {
			imported.resolve({
				asset_id: "replacement",
				asset_revision: 2,
				name: "short.mp3",
				media_type: "audio/wav",
				sample_rate: 48000,
				sample_frames: 48000,
				channels: 2,
			});
			await importing;
		});
		expect(observed.definition?.name).toBe("Edited during import");
		expect(observed.definition?.duration_frame).toBe(
			Math.max(markerFrame, clipEnd),
		);
		expect(observed.definition?.markers).toEqual(item.definition.markers);
		expect(observed.definition?.lanes).toEqual(item.definition.lanes);
		expect(observed.peaks).toBeUndefined();
		expect(waveform).toHaveBeenCalledTimes(1);
		await act(async () => {
			nameSaved.resolve();
		});
		await waitFor(() => expect(update).toHaveBeenCalledTimes(2));
		expect(waveform).toHaveBeenCalledTimes(1);
		expect(update.mock.calls[1]?.[2]).toBe(5);
		expect(update.mock.calls[1]?.[3].audio).toEqual({
			asset_id: "replacement",
			asset_revision: 2,
			file_name: "short.mp3",
		});
		await act(async () => {
			audioSaved.resolve();
		});
		await waitFor(() => expect(observed.peaks).toEqual([0.8, 0.3]));
		expect(waveform).toHaveBeenCalledTimes(2);
		expect(waveform).toHaveBeenLastCalledWith("show", "timecode");
		expect(server.name).toBe("Edited during import");
		expect(server.markers).toEqual(item.definition.markers);
		expect(server.lanes).toEqual(item.definition.lanes);
	});
});

function definition(markerFrame: number, clipEnd: number): TimecodeDefinition {
	return {
		id: "timecode",
		number: 1,
		name: "Original",
		duration_frame: 1320,
		transport_offset_frame: 0,
		auto_start: false,
		audio: { asset_id: "original", asset_revision: 1, file_name: "long.wav" },
		markers: [{ id: "marker", name: "Final marker", frame: markerFrame }],
		lanes: [
			{
				id: "audio",
				name: "Audio",
				content: { kind: "audio_volume", keyframes: [] },
			},
			{
				id: "cues",
				name: "Cues",
				content: {
					kind: "cue_list",
					cue_list_id: "cuelist",
					clips: [
						{
							id: "clip",
							start_frame: 44,
							end_frame: clipEnd,
							start_cue_id: "cue",
							end_cue_id: "cue",
							start_behavior: "state",
							end_behavior: "hold",
							cue_starts: [],
							in_fade_frames: 0,
							out_fade_frames: 0,
						},
					],
				},
			},
		],
	};
}

function outcome(
	revision: number,
	body: TimecodeDefinition,
): ShowObjectActionOutcome {
	return {
		request_id: `save-${revision}`,
		replayed: false,
		show_id: "show",
		show_revision: revision,
		object: {
			kind: "timecode",
			id: body.id,
			revision,
			updated_at: "2026-09-05T00:00:00Z",
			body,
		},
	};
}
