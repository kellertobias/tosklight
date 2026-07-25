import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ControlDesk } from "../../../api/types";
import {
	type ScreenUndoHandle,
	useDefaultScreenDraft,
} from "./useDefaultScreenDraft";

const desk: ControlDesk = {
	id: "desk-1",
	name: "Main desk",
	osc_alias: "main",
	columns: 8,
	rows: 1,
	buttons: 3,
};

describe("default screen mutation history", () => {
	it("persists immediately and undoes only a real editing gesture", async () => {
		const undoRef: ScreenUndoHandle = { current: null };
		const availability = vi.fn();
		const persist = vi.fn(async () => undefined);
		const updateKeyboardShortcuts = vi.fn();
		const updatePlaybackLayout = vi.fn();
		const { result } = renderHook(() =>
			useDefaultScreenDraft({
				desk,
				regularNumberShortcuts: true,
				onKeyboardShortcuts: updateKeyboardShortcuts,
				onPlaybackLayout: updatePlaybackLayout,
				onPersistDesk: persist,
				undoRef,
				onUndoAvailabilityChange: availability,
			}),
		);

		act(() => result.current.beginTextEdit("osc_alias"));
		act(() => result.current.updateText("osc_alias", "main-a"));
		act(() => result.current.updateText("osc_alias", "main-ab"));
		await waitFor(() => expect(persist).toHaveBeenCalledTimes(2));
		expect(result.current.draft?.osc_alias).toBe("main-ab");
		expect(availability).toHaveBeenLastCalledWith(true);

		act(() => undoRef.current?.());
		await waitFor(() => expect(persist).toHaveBeenCalledTimes(3));
		expect(persist).toHaveBeenLastCalledWith(desk);
		expect(result.current.draft?.osc_alias).toBe("main");
		expect(availability).toHaveBeenLastCalledWith(false);

		act(() => result.current.updateDesk({ name: desk.name }));
		expect(persist).toHaveBeenCalledTimes(3);
		expect(availability).toHaveBeenLastCalledWith(false);
	});
});
