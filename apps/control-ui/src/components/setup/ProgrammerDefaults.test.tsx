import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defaultUpdateSettings } from "../control/updateWorkflow";
import {
	loadRecordSettings,
	RecordDefaultsFields,
	saveRecordSettings,
	UpdateDefaultsFields,
} from "./ProgrammerDefaults";

beforeEach(() => {
	const values = new Map<string, string>();
	vi.stubGlobal("localStorage", {
		getItem: (key: string) => values.get(key) ?? null,
		setItem: (key: string, value: string) => values.set(key, value),
		removeItem: (key: string) => values.delete(key),
		clear: () => values.clear(),
	});
});
afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

describe("Programmer defaults", () => {
	it("persists and reloads the Record defaults shared with the hold dialog", () => {
		saveRecordSettings({
			mode: "overwrite",
			cueOnly: true,
			mergeActiveCue: true,
		});
		expect(loadRecordSettings()).toEqual({
			mode: "overwrite",
			cueOnly: true,
			mergeActiveCue: true,
		});
	});

	it("exposes every Record default through the reusable form", () => {
		const change = vi.fn();
		render(
			<RecordDefaultsFields
				settings={{ mode: "merge", cueOnly: false, mergeActiveCue: false }}
				onChange={change}
			/>,
		);
		fireEvent.click(screen.getByRole("radio", { name: "Overwrite" }));
		expect(change).toHaveBeenCalledWith({
			mode: "overwrite",
			cueOnly: false,
			mergeActiveCue: false,
		});
		fireEvent.click(screen.getByRole("switch", { name: "Cue only" }));
		expect(change).toHaveBeenCalledWith({
			mode: "merge",
			cueOnly: true,
			mergeActiveCue: false,
		});
	});

	it("exposes the Update defaults shared with the hold dialog", () => {
		const change = vi.fn();
		render(
			<UpdateDefaultsFields
				settings={defaultUpdateSettings}
				onChange={change}
			/>,
		);
		fireEvent.click(
			screen.getByRole("switch", { name: "Show Update modal on touch" }),
		);
		expect(change).toHaveBeenCalledWith({
			...defaultUpdateSettings,
			show_update_modal_on_touch: false,
		});
		expect(
			screen.getByRole("button", { name: /Add to Current Cue/ }),
		).toBeInTheDocument();
	});
});
