// @vitest-environment jsdom
import {
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { CueList, ShowObjectActionOutcome } from "../api/types";
import type { TimecodeDefinition, TimecodePatch } from "../api/types/timecode";
import { TimecodeEditor } from "./TimecodeRuntimeWindow";

vi.mock("../components/files/RootConfinedFilePickerButton", () => ({
	RootConfinedFilePickerButton: ({ label }: { label: string }) => (
		<button type="button">{label}</button>
	),
}));

const SHOW_ID = "00000000-0000-4000-8000-000000000161";
const TIMECODE_ID = "00000000-0000-4000-8000-000000000162";

describe("TimecodeEditor title and settings", () => {
	it("keeps the editor playhead when an unchanged transport frame is republished", async () => {
		const serverDefinition = definition();
		const api = {
			create: vi.fn(),
			update: vi.fn(),
			delete: vi.fn(),
			objects: vi.fn(async () => ({
				show_revision: 4,
				objects: [{ revision: 4, definition: serverDefinition }],
			})),
			transportAction: vi.fn(),
			importAudio: vi.fn(),
			waveform: vi.fn(),
		};
		const renderEditor = (snapshotRevision: number) => (
			<TimecodeEditor
				showId={SHOW_ID}
				item={{ revision: 4, definition: serverDefinition }}
				api={api as never}
				cueLists={[]}
				audioPlayers={[]}
				snapshot={{
					timecode_id: TIMECODE_ID,
					revision: snapshotRevision,
					state: "paused",
					frame: 44,
					duration_frame: 440,
					audio_linked: false,
					cue_list_clips: [],
				}}
				onClose={vi.fn()}
			/>
		);
		const view = render(renderEditor(1));
		await screen.findByText("Saved");
		const viewport = screen.getByLabelText("Timecode timeline viewport");
		vi.spyOn(viewport, "getBoundingClientRect").mockReturnValue({
			x: 0,
			y: 0,
			left: 0,
			top: 0,
			right: 720,
			bottom: 400,
			width: 720,
			height: 400,
			toJSON: () => ({}),
		});
		const canvas = viewport.querySelector<HTMLElement>(
			".timecode-timeline-canvas",
		);
		const pixelsPerFrame = Number(canvas?.dataset.pixelsPerFrame);
		const playhead = screen.getByRole("button", {
			name: "Drag playhead to seek",
		}) as HTMLButtonElement;
		playhead.setPointerCapture = vi.fn();
		fireEvent.pointerDown(playhead, {
			pointerId: 1,
			clientX: 160 + 88 * pixelsPerFrame,
		});
		expect(playhead).toHaveTextContent("00:00:02.00");

		view.rerender(renderEditor(2));
		expect(
			screen.getByRole("button", { name: "Drag playhead to seek" }),
		).toHaveTextContent("00:00:02.00");
		view.unmount();
	});

	it("autosaves Settings, routes title transport, and owns the exact Add menu", async () => {
		let revision = 4;
		let serverDefinition = definition();
		const update = vi.fn(
			async (
				_showId: string,
				_id: string,
				expectedRevision: number,
				patch: TimecodePatch,
			) => {
				expect(expectedRevision).toBe(revision);
				serverDefinition = { ...serverDefinition, ...patch };
				revision += 1;
				return outcome(revision, serverDefinition);
			},
		);
		const api = {
			create: vi.fn(),
			update,
			delete: vi.fn(),
			objects: vi.fn(async () => ({
				show_revision: revision,
				objects: [{ revision, definition: serverDefinition }],
			})),
			transportAction: vi.fn(async () => ({
				timecode_id: TIMECODE_ID,
				revision: 1,
				state: "playing" as const,
				frame: 0,
				duration_frame: 440,
				audio_linked: false,
			})),
			importAudio: vi.fn(),
			waveform: vi.fn(),
		};
		const view = render(
			<TimecodeEditor
				showId={SHOW_ID}
				item={{ revision, definition: serverDefinition }}
				api={api as never}
				cueLists={[
					{
						id: "00000000-0000-4000-8000-000000000170",
						name: "Opening",
						cues: [
							{
								id: "00000000-0000-4000-8000-000000000171",
								number: "1",
								name: "First",
							},
						],
					},
				]}
				audioPlayers={[
					{
						fixtureId: "00000000-0000-4000-8000-000000000180",
						name: "Audio Player 201",
					},
				]}
				onClose={vi.fn()}
			/>,
		);
		await screen.findByText("Saved");

		expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
		for (const label of ["Rewind to start", "Stop", "Play", "Pause"])
			expect(screen.getByRole("button", { name: label })).toHaveClass(
				"timecode-transport-action",
			);
		expect(
			screen.getByRole("button", { name: "Rewind to start" }),
		).toHaveTextContent("▏▶");
		fireEvent.click(
			screen.getByRole("button", { name: "Rewind to start" }),
		);
		await waitFor(() =>
			expect(api.transportAction).toHaveBeenCalledWith(SHOW_ID, TIMECODE_ID, {
				type: "seek",
				frame: 0,
			}),
		);
		expect(
			screen.getByRole("button", { name: "Timecode position" }),
		).toHaveTextContent("00:00:00.00");
		expect(
			screen.getByRole("button", { name: "Timecode position" }),
		).toHaveClass("timecode-position-action");
		fireEvent.click(screen.getByRole("button", { name: "Play" }));
		await waitFor(() =>
			expect(api.transportAction).toHaveBeenCalledWith(SHOW_ID, TIMECODE_ID, {
				type: "go",
			}),
		);
		view.rerender(
			<TimecodeEditor
				showId={SHOW_ID}
				item={{ revision, definition: serverDefinition }}
				api={api as never}
				cueLists={[]}
				audioPlayers={[
					{
						fixtureId: "00000000-0000-4000-8000-000000000180",
						name: "Audio Player 201",
					},
				]}
				snapshot={{
					timecode_id: TIMECODE_ID,
					revision: 2,
					state: "playing",
					frame: 44,
					duration_frame: 440,
					audio_linked: false,
					cue_list_clips: [],
				}}
				onClose={vi.fn()}
			/>,
		);
		expect(
			screen.getByRole("button", { name: "Timecode position" }),
		).toHaveTextContent("00:00:01.00");
		expect(
			view.container.querySelector(".ui-window-info > b"),
		).toHaveTextContent("playing");
		expect(
			view.container.querySelector(".ui-window-info > b"),
		).not.toHaveTextContent("00:00:01.00");
		expect(
			view.container.querySelector(".timecode-editor-playhead"),
		).toHaveTextContent("00:00:01.00");
		expect(screen.queryByLabelText("Speed Group")).toBeNull();
		expect(screen.queryByLabelText("Cuelist")).toBeNull();
		expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();

		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		const settings = screen.getByRole("dialog", {
			name: "Timecode Settings",
		});
		for (const label of ["Name", "Duration", "Transport offset", "Auto-start"])
			expect(within(settings).getByLabelText(label)).toBeTruthy();
		const markerLock = within(settings).getByLabelText("Lock markers");
		expect(markerLock).not.toBeChecked();
		fireEvent.click(markerLock);
		expect(markerLock).toBeChecked();
		expect(within(settings).queryByLabelText("Number")).toBeNull();
		expect(within(settings).queryByLabelText("Frames")).toBeNull();
		expect(
			within(settings).queryByRole("textbox", { name: "Marker CSV" }),
		).toBeNull();
		expect(
			within(settings).getByRole("button", { name: "Choose audio file" }),
		).toBeTruthy();
		expect(
			within(settings).getByRole("button", { name: "Choose marker CSV" }),
		).toBeTruthy();
		fireEvent.change(within(settings).getByLabelText("Name"), {
			target: { value: "Opening sequence" },
		});
		expect(screen.queryByText("Saving changes…")).toBeNull();
		await waitFor(() =>
			expect(update).toHaveBeenCalledWith(SHOW_ID, TIMECODE_ID, 4, {
				name: "Opening sequence",
			}),
		);
		fireEvent.click(within(settings).getByLabelText("Auto-start"));
		await waitFor(() =>
			expect(update).toHaveBeenLastCalledWith(SHOW_ID, TIMECODE_ID, 5, {
				auto_start: true,
			}),
		);
		const addTrigger = screen.getByRole("button", { name: "Add" });
		fireEvent.click(addTrigger);
		fireEvent.click(addTrigger);
		expect(view.container.querySelector(".timecode-window")).toBeTruthy();
		expect(screen.queryAllByRole("menu", { name: "Add" })).toHaveLength(0);

		fireEvent.click(addTrigger);
		expect(screen.getByRole("button", { name: "Add" })).toHaveClass(
			"ui-title-chrome-dropdown-trigger",
		);
		expect(screen.getByRole("button", { name: "Add" })).toHaveClass(
			"timecode-add-title-action",
		);
		const add = screen.getByRole("menu", { name: "Add" });
		expect(
			within(add)
				.getAllByRole("menuitem")
				.map((item) => item.textContent),
		).toEqual([
			"Add Marker",
			"Add Audio Player 201 Lane",
			"Add Speed Lane",
			"Add Cuelist Lane",
		]);
		expect(within(add).queryByText("Add Playhead")).toBeNull();
		const addMarker = within(add).getByRole("menuitem", { name: "Add Marker" });
		const randomUuid = vi
			.spyOn(crypto, "randomUUID")
			.mockImplementationOnce(() => {
				throw new Error("UUID generation failed");
			});
		fireEvent.click(addMarker);
		fireEvent.click(addMarker);
		expect(randomUuid).toHaveBeenCalledTimes(1);
		expect(screen.getByRole("alert")).toHaveTextContent(
			"Could not add marker: UUID generation failed",
		);
		expect(view.container.querySelector(".timecode-window")).toBeTruthy();
		randomUuid.mockRestore();

		await new Promise((resolve) => window.setTimeout(resolve, 275));
		fireEvent.click(addTrigger);
		fireEvent.click(
			within(screen.getByRole("menu", { name: "Add" })).getByRole("menuitem", {
				name: "Add Marker",
			}),
		);
		await waitFor(() =>
			expect(update.mock.calls.at(-1)?.[3]).toEqual({
				markers: [expect.objectContaining({ frame: 44, name: "Marker 1" })],
			}),
		);
	});

	it("uses the CueList revision writer and reports a rejected nested timing edit", async () => {
		const cueList: CueList = {
			id: "00000000-0000-4000-8000-000000000170",
			name: "Opening",
			mode: "sequence",
			priority: 0,
			looped: false,
			cues: [
				{
					id: "00000000-0000-4000-8000-000000000171",
					number: "1",
					name: "First",
					fade_millis: 2_000,
					delay_millis: 0,
					trigger: { type: "wait", delay_millis: 0 },
					changes: [],
				},
			],
		};
		const cueId = cueList.cues[0]?.id;
		if (!cueId) throw new Error("expected Cue identity");
		const timecode = {
			...definition(),
			lanes: [
				{
					id: "lane-1",
					name: "Opening",
					content: {
						kind: "cue_list" as const,
						cue_list_id: cueList.id,
						clips: [
							{
								id: "clip-1",
								start_frame: 44,
								end_frame: 396,
								start_cue_id: cueId,
								end_cue_id: cueId,
								start_behavior: "state" as const,
								end_behavior: "release" as const,
								cue_starts: [],
							},
						],
					},
				},
			],
		};
		const saveCueList = vi.fn(async (_basis: unknown, _body: CueList) => {
			throw new Error("Cue List changed on another desk");
		});
		render(
			<TimecodeEditor
				showId={SHOW_ID}
				item={{ revision: 4, definition: timecode }}
				api={
					{
						create: vi.fn(),
						update: vi.fn(),
						delete: vi.fn(),
						objects: vi.fn(),
						transportAction: vi.fn(),
						importAudio: vi.fn(),
						waveform: vi.fn(),
					} as never
				}
				cueLists={[
					{
						id: cueList.id,
						name: cueList.name,
						cues: cueList.cues,
						objectId: "cue-list-object",
						revision: 9,
						body: cueList,
					},
				]}
				audioPlayers={[]}
				saveCueList={saveCueList}
				onClose={vi.fn()}
			/>,
		);
		await waitFor(() =>
			expect(screen.getAllByText("Saved").length).toBeGreaterThan(0),
		);
		const canvas = document.querySelector<HTMLElement>(
			".timecode-timeline-canvas",
		);
		const pixelsPerFrame = Number(canvas?.dataset.pixelsPerFrame);
		const handle = screen.getByRole("slider", {
			// Each handle is named for what it sets: the boundary sets the delay, the far edge the fade.
			name: "Cue 1 In fade",
		});
		fireEvent.pointerDown(handle, { pointerId: 30, clientX: 100 });
		fireEvent.pointerMove(window, {
			pointerId: 30,
			clientX: 100 + 44 * pixelsPerFrame,
		});
		fireEvent.pointerUp(window, { pointerId: 30 });
		await waitFor(() => expect(saveCueList).toHaveBeenCalledTimes(1));
		expect(saveCueList.mock.calls[0]?.[0]).toEqual({
			cueListId: cueList.id,
			expectedRevision: 9,
			expectedObjectId: "cue-list-object",
		});
		await waitFor(() =>
			expect(screen.getByRole("alert")).toHaveTextContent(
				"Cue 1 In fade was not saved: Cue List changed on another desk",
			),
		);
		expect(
			screen.getByRole("slider", { // Each handle is named for what it sets: the boundary sets the delay, the far edge the fade.
			name: "Cue 1 In fade" }),
		).toHaveAttribute("aria-valuenow", "132");
	});
});

function definition(): TimecodeDefinition {
	return {
		id: TIMECODE_ID,
		number: 1,
		name: "Timecode 1",
		duration_frame: 440,
		transport_offset_frame: 0,
		auto_start: false,
		markers: [],
		lanes: [],
	};
}

function outcome(
	revision: number,
	body: TimecodeDefinition,
): ShowObjectActionOutcome {
	return {
		request_id: `save-${revision}`,
		replayed: false,
		show_id: SHOW_ID,
		show_revision: revision,
		object: {
			kind: "timecode",
			id: TIMECODE_ID,
			revision,
			updated_at: "2026-08-11T00:00:00Z",
			body,
		},
	};
}
