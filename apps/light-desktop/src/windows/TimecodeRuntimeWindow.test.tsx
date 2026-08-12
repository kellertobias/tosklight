// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ShowObjectActionOutcome } from "../api/generated/light-wire";
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
		render(
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
								number: 1,
								name: "First",
							},
						],
					},
				]}
				onClose={vi.fn()}
			/>,
		);
		await screen.findByText("Saved");

		expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
		const transport = screen.getByLabelText("Timecode transport");
		for (const label of ["Go", "Pause", "Stop", "Rewind"])
			expect(
				within(transport).getByRole("button", { name: label }),
			).toBeTruthy();
		fireEvent.click(within(transport).getByRole("button", { name: "Go" }));
		await waitFor(() =>
			expect(api.transportAction).toHaveBeenCalledWith(SHOW_ID, TIMECODE_ID, {
				type: "go",
			}),
		);

		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		const settings = screen.getByRole("dialog", {
			name: "Timecode Settings",
		});
		for (const label of ["Name", "Duration", "Transport offset", "Auto-start"])
			expect(within(settings).getByLabelText(label)).toBeTruthy();
		expect(within(settings).queryByLabelText("Number")).toBeNull();
		expect(within(settings).queryByLabelText("Frames")).toBeNull();
		expect(within(settings).queryByRole("textbox", { name: "Marker CSV" })).toBeNull();
		expect(within(settings).getByRole("button", { name: "Choose audio file" })).toBeTruthy();
		expect(within(settings).getByRole("button", { name: "Choose marker CSV" })).toBeTruthy();
		fireEvent.change(within(settings).getByLabelText("Name"), {
			target: { value: "Opening sequence" },
		});
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

		fireEvent.click(screen.getByRole("button", { name: "Add" }));
		const add = screen.getByRole("menu", { name: "Add" });
		expect(
			within(add)
				.getAllByRole("menuitem")
				.map((item) => item.textContent),
		).toEqual([
			"Add Marker",
			"Add Audio Lane",
			"Add Speed Lane",
			"Add Cuelist Lane",
		]);
		expect(within(add).queryByText("Add Playhead")).toBeNull();
		fireEvent.click(within(add).getByRole("menuitem", { name: "Add Marker" }));
		await waitFor(() =>
			expect(update.mock.calls.at(-1)?.[3]).toEqual({
				markers: [expect.objectContaining({ frame: 0, name: "Marker 1" })],
			}),
		);
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
