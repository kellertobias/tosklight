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
	it("persists and reloads the browser-local Cue only preference", () => {
		saveRecordSettings({ cueOnly: true });
		expect(loadRecordSettings()).toEqual({ cueOnly: true });
		expect(localStorage.getItem("light.store-mode")).toBeNull();
		expect(localStorage.getItem("light.store-merge-active-cue")).toBeNull();
	});

	it("offers the four Record defaults and Cue only, and no retired settings", () => {
		const change = vi.fn();
		const recordDefault = vi.fn();
		render(
			<RecordDefaultsFields
				settings={{ cueOnly: false }}
				onChange={change}
				recordDefault="smart"
				onRecordDefault={recordDefault}
			/>,
		);
		const group = screen.getByRole("radiogroup", {
			name: "Default Record mode",
		});
		expect(
			Array.from(group.querySelectorAll('[role="radio"]')).map(
				(radio) => radio.textContent,
			),
		).toEqual(["Smart", "Merge", "Add Existing", "Add Cue"]);
		expect(screen.getByRole("radio", { name: "Smart" })).toHaveAttribute(
			"aria-checked",
			"true",
		);
		expect(
			screen.getByText(/A Cuelist with one Cue asks whether to add/),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("radio", { name: "Add Existing" }));
		expect(recordDefault).toHaveBeenCalledWith("add_existing");
		fireEvent.click(screen.getByRole("switch", { name: "Cue only" }));
		expect(change).toHaveBeenCalledWith({ cueOnly: true });
		expect(screen.queryByText("Merge into active Cue")).toBeNull();
		expect(screen.queryByText("Record mode")).toBeNull();
		expect(
			screen.getByText(/the recorded values last for this Cue only/),
		).toHaveTextContent(
			"The next Cue returns those fixture attributes to their earlier values, or releases them. Everything else keeps tracking.",
		);
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
		fireEvent.click(screen.getByRole("radio", { name: "Merge" }));
		expect(change).toHaveBeenCalledWith({
			...defaultUpdateSettings,
			update_default: "merge",
		});
		expect(screen.getByRole("button", { name: "Update" })).toBeInTheDocument();
	});
});
