import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	PlaybackSoloEditorProvider,
	usePlaybackSoloEditor,
} from "../../../features/playbackSolo/PlaybackSoloEditorContext";
import { usePlaybackGridSolo } from "./usePlaybackGridSolo";
const mocks = vi.hoisted(() => {
	const regions: [] = [];
	return { regions, save: vi.fn(async () => regions), operate: vi.fn() };
});
vi.mock("../../../features/playbackSolo/PlaybackSoloContext", () => ({
	usePlaybackSoloOptional: () => ({
		authorityGeneration: 1, available: true, error: null,
		activate: () => () => {}, subscribe: () => () => {},
		getRegions: () => mocks.regions, isSaving: () => false, save: mocks.save,
	}),
}));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
function Harness() {
	const [page, setPage] = useState(2);
	const editor = usePlaybackSoloEditor("main");
	const pane = usePlaybackSoloEditor("pane:other");
	return <>
		<button onClick={() => editor?.begin()}>Create</button>
		<button onClick={() => setPage(4)}>Page 4</button>
		<button onClick={() => editor?.cancel()}>Cancel</button>
		<output aria-label="Selected">{JSON.stringify(editor?.selected)}</output>
		<output aria-label="Other surface">{String(pane?.selecting)}</output>
		<Grid page={page} surfaceId="main" />
		<Grid page={3} surfaceId="pane:other" />
	</>;
}
function Grid({ page, surfaceId }: { page: number; surfaceId: string }) {
	const solo = usePlaybackGridSolo({ surfaceId, activePageNumber: page, columns: 2,
		slots: [1, 2].map((slot) => ({ slot, playback: null, cue: null, group: null,
			row: null, rowIndex: 0, columnIndex: slot - 1, footprint: null })),
	});
	return <section aria-label={surfaceId} onPointerDownCapture={solo.pointerDown}
		onClickCapture={solo.block} onKeyDownCapture={solo.keyDown}>
		{[1, 2].map((slot) => <button key={slot} data-playback-slot={slot}
			onPointerDown={mocks.operate} onClick={mocks.operate}>
			{surfaceId} {page}.{slot}
		</button>)}
	</section>;
}
describe("surface-scoped Solo selection", () => {
	it("selects cells across page changes, blocks live pointer actions, and cancels without writes", async () => {
		const view = render(<PlaybackSoloEditorProvider><Harness /></PlaybackSoloEditorProvider>);
		await waitFor(() => expect(view.getByLabelText("Other surface").textContent).toBe("false"));
		fireEvent.click(view.getByText("Create"));
		fireEvent.pointerDown(view.getByText("main 2.1"), { button: 0 });
		fireEvent.click(view.getByText("main 2.1"));
		fireEvent.click(view.getByText("Page 4"));
		fireEvent.pointerDown(view.getByText("main 4.2"), { button: 0 });
		expect(view.getByLabelText("Selected").textContent).toBe(JSON.stringify([
			{ page: 2, position: 1 }, { page: 4, position: 2 },
		]));
		expect(view.getByLabelText("Other surface").textContent).toBe("false");
		expect(mocks.operate).not.toHaveBeenCalled();
		fireEvent.click(view.getByText("Cancel"));
		expect(mocks.save).not.toHaveBeenCalled();
		fireEvent.pointerDown(view.getByText("main 4.2"), { button: 0 });
		expect(mocks.operate).toHaveBeenCalledOnce();
	});
});
