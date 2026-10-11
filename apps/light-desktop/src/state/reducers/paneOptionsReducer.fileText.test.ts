import { describe, expect, it } from "vitest";
import type { PaneModel } from "../../types";
import { appReducer, initialState } from "../appReducer";

function layout() {
	const pane = (id: string, kind: PaneModel["kind"]): PaneModel => ({
		id,
		kind,
		title: id,
		x: 1,
		y: 1,
		width: 8,
		height: 8,
	});
	return appReducer(initialState, {
		type: "HYDRATE_LAYOUT",
		activeDeskId: "active",
		desks: [
			{
				id: "active",
				name: "Active",
				panes: [
					pane("files", "file_manager"),
					pane("editor", "text_editor"),
					pane("other-files", "file_manager"),
				],
			},
			{
				id: "inactive",
				name: "Inactive",
				panes: [
					pane("inactive-files", "file_manager"),
					pane("inactive-editor", "text_editor"),
				],
			},
		],
	});
}

function roundTrip(state: ReturnType<typeof layout>) {
	return appReducer(initialState, {
		type: "HYDRATE_LAYOUT",
		activeDeskId: state.activeDeskId,
		desks: JSON.parse(JSON.stringify(state.desks)),
	});
}

describe("File Manager pane options", () => {
	it("toggles hidden files only in the addressed active pane and restores filtering", () => {
		const before = layout();
		const visible = appReducer(before, {
			type: "SET_FILE_MANAGER_SHOW_HIDDEN",
			id: "files",
			value: true,
		});
		expect(visible.desks[0].panes[0].fileManagerShowHidden).toBe(true);
		expect(visible.desks[0].panes[1]).toBe(before.desks[0].panes[1]);
		expect(visible.desks[0].panes[2]).toBe(before.desks[0].panes[2]);
		expect(visible.desks[1]).toBe(before.desks[1]);
		const hidden = appReducer(visible, {
			type: "SET_FILE_MANAGER_SHOW_HIDDEN",
			id: "files",
			value: false,
		});
		expect(hidden.desks[0].panes[0].fileManagerShowHidden).toBe(false);
	});

	it("retains the stored setting through layout hydration without inventing legacy defaults", () => {
		const legacy = layout();
		expect(legacy.desks[0].panes[0].fileManagerShowHidden ?? false).toBe(false);
		expect(legacy.desks[0].panes[0]).not.toHaveProperty("fileManagerShowHidden");
		const changed = appReducer(legacy, {
			type: "SET_FILE_MANAGER_SHOW_HIDDEN",
			id: "files",
			value: true,
		});
		expect(roundTrip(changed).desks[0].panes[0].fileManagerShowHidden).toBe(true);
	});

	it("does not alter an inactive Desktop when its pane is addressed", () => {
		const before = layout();
		const after = appReducer(before, {
			type: "SET_FILE_MANAGER_SHOW_HIDDEN",
			id: "inactive-files",
			value: true,
		});
		expect(after.desks).toEqual(before.desks);
	});
});

describe("existing Text Editor actions", () => {
	it("sets the file and preserves unrelated panes and inactive Desktops", () => {
		const before = layout();
		const after = appReducer(before, {
			type: "SET_TEXT_EDITOR_FILE",
			id: "editor",
			root: "shows",
			path: "notes/run.md",
		});
		expect(after.desks[0].panes[1]).toMatchObject({
			textFileRoot: "shows",
			textFilePath: "notes/run.md",
		});
		expect(after.desks[0].panes[0]).toEqual(before.desks[0].panes[0]);
		expect(after.desks[1]).toEqual(before.desks[1]);
	});

	it("preserves partial settings and safe defaults through hydration", () => {
		const before = layout();
		expect(before.desks[0].panes[1].textEditorMode ?? "plain").toBe("plain");
		expect(before.desks[0].panes[1].textEditorReadOnly ?? false).toBe(false);
		const readOnly = appReducer(before, {
			type: "SET_TEXT_EDITOR_SETTINGS",
			id: "editor",
			readOnly: true,
		});
		const split = appReducer(readOnly, {
			type: "SET_TEXT_EDITOR_SETTINGS",
			id: "editor",
			mode: "split",
		});
		expect(roundTrip(split).desks[0].panes[1]).toMatchObject({
			textEditorReadOnly: true,
			textEditorMode: "split",
		});
		expect(split.desks[0].panes[0]).toEqual(before.desks[0].panes[0]);
		expect(split.desks[1]).toEqual(before.desks[1]);
	});

	it("retains the precise file-bound view and selection across hydration", () => {
		const before = layout();
		const view = {
			root: "shows",
			path: "notes/run.md",
			selectionStart: 4,
			selectionEnd: 12,
			scrollTop: 240,
		};
		const after = appReducer(before, {
			type: "SET_TEXT_EDITOR_VIEW",
			id: "editor",
			...view,
		});
		expect(roundTrip(after).desks[0].panes[1].textEditorView).toEqual(view);
		expect(after.desks[0].panes[1]).not.toHaveProperty("text");
		expect(after.desks[1]).toEqual(before.desks[1]);
	});
});
