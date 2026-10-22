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
		// The settings are grouped into three tabs, so each field is on the one it belongs to.
		const tab = (name: string) =>
			within(settings).getByRole("tab", { name });
		expect(tab("Generic")).toHaveAttribute("aria-selected", "true");
		for (const label of ["Name", "Duration"])
			expect(within(settings).getByLabelText(label)).toBeTruthy();
		expect(
			within(settings).getByRole("button", { name: "Select File" }),
		).toBeTruthy();
		expect(within(settings).queryByLabelText("Transport offset")).toBeNull();
		expect(within(settings).queryByLabelText("Lock markers")).toBeNull();

		fireEvent.click(tab("Sync"));
		expect(within(settings).getByLabelText("Transport offset")).toBeTruthy();
		expect(within(settings).getByLabelText("Arm")).toBeTruthy();
		expect(within(settings).queryByLabelText("Name")).toBeNull();

		fireEvent.click(tab("Markers"));
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
			within(settings).getByRole("button", { name: "Choose marker CSV" }),
		).toBeTruthy();
		// Editing the name means going back to the tab it lives on.
		fireEvent.click(tab("Generic"));
		fireEvent.change(within(settings).getByLabelText("Name"), {
			target: { value: "Opening sequence" },
		});
		expect(screen.queryByText("Saving changes…")).toBeNull();
		await waitFor(() =>
			expect(update).toHaveBeenCalledWith(SHOW_ID, TIMECODE_ID, 4, {
				name: "Opening sequence",
			}),
		);
		// Arming against an external clock is on the Sync tab.
		fireEvent.click(tab("Sync"));
		fireEvent.click(within(settings).getByLabelText("Arm"));
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
								in_fade_frames: 0,
								out_fade_frames: 0,
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
		fireEvent.click(screen.getByRole("button", { name: "Cue 1 First" }));
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

async function compoundResizeRig() {
	const first = "00000000-0000-4000-8000-000000000181";
	const second = "00000000-0000-4000-8000-000000000182";
	let cueRevision = 9;
	let timecodeRevision = 4;
	let storedCueList: CueList = {
		id: "00000000-0000-4000-8000-000000000180",
		name: "Opening",
		mode: "sequence",
		priority: 0,
		looped: false,
		cues: [
			{
				id: first,
				number: "1",
				name: "Keep this name",
				delay_millis: 0,
				fade_millis: 0,
				out_delay_millis: 250,
				out_fade_millis: 1000,
				trigger: { type: "manual" },
				changes: [],
			},
			{
				id: second,
				number: "2",
				name: "Second",
				delay_millis: 500,
				fade_millis: 2000,
				trigger: { type: "manual" },
				changes: [],
			},
		],
	};
	const originalCueList = structuredClone(storedCueList);
	let storedDefinition: TimecodeDefinition = {
		...definition(),
		duration_frame: 2640,
		lanes: [
			{
				id: "lane-compound",
				name: "Opening",
				content: {
					kind: "cue_list",
					cue_list_id: storedCueList.id,
					clips: [
						{
							id: "clip-compound",
							start_frame: 0,
							end_frame: 264,
							start_cue_id: first,
							end_cue_id: second,
							start_behavior: "state",
							end_behavior: "release",
							cue_starts: [{ cue_id: second, offset_frame: 44 }],
							in_fade_frames: 0,
							out_fade_frames: 0,
						},
					],
				},
			},
		],
	};
	const originalDefinition = structuredClone(storedDefinition);
	let rejectCue = false;
	let rejectDefinition = false;
	let deferCue = false;
	let releaseCue: (() => void) | undefined;
	const saveCueList = vi.fn(
		async (basis: { expectedRevision: number }, body: CueList) => {
			expect(basis.expectedRevision).toBe(cueRevision);
			if (deferCue) {
				deferCue = false;
				await new Promise<void>((resolve) => {
					releaseCue = resolve;
				});
			}
			if (rejectCue) {
				rejectCue = false;
				throw new Error("Cue write refused");
			}
			storedCueList = structuredClone(body);
			return {
				kind: "cue_list",
				id: "cue-list-object",
				revision: ++cueRevision,
				updated_at: "",
				body: storedCueList,
			};
		},
	);
	const update = vi.fn(
		async (
			_show: string,
			_id: string,
			revision: number,
			patch: TimecodePatch,
		) => {
			expect(revision).toBe(timecodeRevision);
			if (rejectDefinition) {
				rejectDefinition = false;
				throw new Error("Timecode write refused");
			}
			storedDefinition = { ...storedDefinition, ...structuredClone(patch) };
			return outcome(++timecodeRevision, storedDefinition);
		},
	);
	const view = render(
		<TimecodeEditor
			showId={SHOW_ID}
			item={{ revision: 4, definition: originalDefinition }}
			api={
				{
					create: vi.fn(),
					update,
					objects: vi.fn(),
					delete: vi.fn(),
					transportAction: vi.fn(),
					importAudio: vi.fn(),
					waveform: vi.fn(),
				} as never
			}
			cueLists={[
				{
					id: storedCueList.id,
					name: storedCueList.name,
					cues: storedCueList.cues,
					body: originalCueList,
					objectId: "cue-list-object",
					revision: 9,
				},
			]}
			audioPlayers={[]}
			saveCueList={saveCueList}
			onClose={vi.fn()}
		/>,
	);

	const ui = within(view.container);
	await ui.findByText("Saved");
	const attemptResize = (frame: number) => {
		const pixelsPerFrame = Number(
			view.container.querySelector<HTMLElement>(".timecode-timeline-canvas")
				?.dataset.pixelsPerFrame,
		);
		const edge = view.container.querySelector<HTMLElement>(
			".timecode-clip-edge.start",
		);
		fireEvent.pointerDown(edge as HTMLElement, { pointerId: 91, clientX: 0 });
		fireEvent.pointerMove(window, {
			pointerId: 91,
			clientX: frame * pixelsPerFrame,
		});
		fireEvent.pointerUp(window, { pointerId: 91 });
	};
	return {
		originalDefinition,
		originalCueList,
		saveCueList,
		update,
		ui,
		attemptResize,
		section: () => view.container.querySelector(".timecode-editor"),
		deferCue: () => {
			deferCue = true;
		},
		releaseCue: () => releaseCue?.(),
		definition: () => storedDefinition,
		cues: () => storedCueList,
		rejectCue: () => {
			rejectCue = true;
		},
		rejectDefinition: () => {
			rejectDefinition = true;
		},
		async resize() {
			const canvas = view.container.querySelector<HTMLElement>(
				".timecode-timeline-canvas",
			);
			const pixelsPerFrame = Number(canvas?.dataset.pixelsPerFrame);
			const edge = view.container.querySelector<HTMLElement>(
				".timecode-clip-edge.start",
			);
			expect(edge).toBeTruthy();
			fireEvent.pointerDown(edge as HTMLElement, { pointerId: 91, clientX: 0 });
			fireEvent.pointerMove(window, {
				pointerId: 91,
				clientX: 154 * pixelsPerFrame,
			});
			fireEvent.pointerUp(window, { pointerId: 91 });
			await waitFor(() =>
				expect(storedCueList.cues[0].out_fade_millis).toBe(417),
			);
			await waitFor(() =>
				expect(storedDefinition.lanes[0].content).toMatchObject({
					clips: [expect.objectContaining({ start_frame: 154 })],
				}),
			);
			await waitFor(() =>
				expect(ui.getByRole("button", { name: "Undo" })).toBeEnabled(),
			);
		},
	};
}

it("undoes and redoes a physical clip resize together with its persisted Cue timings", async () => {
	const rig = await compoundResizeRig();
	await rig.resize();
	const resizedDefinition = structuredClone(rig.definition());
	const resizedCueList = structuredClone(rig.cues());
	fireEvent.click(rig.ui.getByRole("button", { name: "Undo" }));
	await waitFor(() => expect(rig.definition()).toEqual(rig.originalDefinition));
	await waitFor(() => expect(rig.cues()).toEqual(rig.originalCueList));
	await waitFor(() =>
		expect(rig.ui.getByRole("button", { name: "Redo" })).toBeEnabled(),
	);
	fireEvent.click(rig.ui.getByRole("button", { name: "Redo" }));
	await waitFor(() => expect(rig.definition()).toEqual(resizedDefinition));
	await waitFor(() => expect(rig.cues()).toEqual(resizedCueList));
});

for (const failure of ["Cue", "Timecode"] as const) {
	it(`retains compound resize history after a refused ${failure} Undo and completes only on explicit retry`, async () => {
		const rig = await compoundResizeRig();
		await rig.resize();
		const resizedDefinition = structuredClone(rig.definition());
		const resizedCueList = structuredClone(rig.cues());
		if (failure === "Cue") rig.rejectCue();
		else rig.rejectDefinition();
		fireEvent.click(rig.ui.getByRole("button", { name: "Undo" }));
		await waitFor(() =>
			expect(rig.ui.getByRole("alert")).toHaveTextContent(
				`${failure} write refused`,
			),
		);
		expect(rig.definition()).toEqual(resizedDefinition);
		expect(rig.cues()).toEqual(
			failure === "Cue" ? resizedCueList : rig.originalCueList,
		);
		expect(rig.ui.getByRole("button", { name: "Redo" })).toBeDisabled();
		await waitFor(() =>
			expect(rig.ui.getByRole("button", { name: "Undo" })).toBeEnabled(),
		);
		const writes = rig.saveCueList.mock.calls.length;
		fireEvent.click(rig.ui.getByRole("button", { name: "Undo" }));
		await waitFor(() =>
			expect(rig.definition()).toEqual(rig.originalDefinition),
		);
		await waitFor(() =>
			expect(rig.ui.getByRole("button", { name: "Redo" })).toBeEnabled(),
		);
		expect(rig.cues()).toEqual(rig.originalCueList);
		expect(rig.saveCueList.mock.calls.length).toBe(
			writes + (failure === "Cue" ? 1 : 0),
		);
		expect(rig.ui.queryByRole("alert")).not.toBeInTheDocument();
		fireEvent.click(rig.ui.getByRole("button", { name: "Redo" }));
		await waitFor(() => expect(rig.definition()).toEqual(resizedDefinition));
		expect(rig.cues()).toEqual(resizedCueList);
	});
}

it("blocks a new physical timeline edit while compound Undo awaits its Cue acknowledgment", async () => {
	const rig = await compoundResizeRig();
	await rig.resize();
	const resized = structuredClone(rig.definition());
	rig.deferCue();
	fireEvent.click(rig.ui.getByRole("button", { name: "Undo" }));
	await waitFor(() => expect(rig.section()).toHaveAttribute("inert"));
	expect(rig.saveCueList).toHaveBeenCalledTimes(2);
	rig.attemptResize(200);
	expect(rig.definition()).toEqual(resized);
	expect(rig.saveCueList).toHaveBeenCalledTimes(2);
	rig.releaseCue();
	await waitFor(() => expect(rig.definition()).toEqual(rig.originalDefinition));
	await waitFor(() =>
		expect(rig.ui.getByRole("button", { name: "Redo" })).toBeEnabled(),
	);
	expect(rig.cues()).toEqual(rig.originalCueList);
	expect(rig.section()).not.toHaveAttribute("inert");
});

it("undoes geometry honestly after the initial linked Cue save refused, then redoes the complete intent", async () => {
	const rig = await compoundResizeRig();
	rig.rejectCue();
	rig.attemptResize(154);
	await waitFor(() =>
		expect(rig.ui.getByRole("alert")).toHaveTextContent("Cue write refused"),
	);
	await waitFor(() =>
		expect(rig.ui.getByRole("button", { name: "Undo" })).toBeEnabled(),
	);
	expect(rig.cues()).toEqual(rig.originalCueList);
	fireEvent.click(rig.ui.getByRole("button", { name: "Undo" }));
	await waitFor(() => expect(rig.definition()).toEqual(rig.originalDefinition));
	await waitFor(() =>
		expect(rig.ui.getByRole("button", { name: "Redo" })).toBeEnabled(),
	);
	expect(rig.saveCueList).toHaveBeenCalledTimes(1);
	fireEvent.click(rig.ui.getByRole("button", { name: "Redo" }));
	await waitFor(() => expect(rig.cues().cues[0].out_fade_millis).toBe(417));
	await waitFor(() =>
		expect(rig.ui.getByRole("button", { name: "Undo" })).toBeEnabled(),
	);
	expect(rig.saveCueList).toHaveBeenCalledTimes(2);
});
