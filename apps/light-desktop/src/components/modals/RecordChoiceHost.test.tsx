import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createCommandLineTestAuthority } from "../../features/programmingInteraction/testing/commandLineTestAuthority";
import { defaultUpdateSettings } from "../control/updateWorkflow";
import { RecordChoiceHost } from "./RecordChoiceHost";

const host = vi.hoisted(() => {
	const state = {
		recordChoiceOpen: true,
		storeArmed: true,
		cueListSetArmed: false,
	};
	return {
		state,
		dispatch: vi.fn((action: { type: string; modal?: string; value?: boolean }) => {
			if (action.type === "SET_MODAL" && action.modal === "recordChoiceOpen")
				state.recordChoiceOpen = Boolean(action.value);
		}),
		update: {
			scopeKey: "authority-a",
			loadSettings: vi.fn(),
			saveSettings: vi.fn(),
		},
	};
});

vi.mock("../../features/programmingUpdate/ProgrammingUpdateProvider", () => ({
	useProgrammingUpdate: () => host.update,
}));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: host.state, dispatch: host.dispatch }),
}));

let storage: Map<string, string>;
beforeEach(() => {
	vi.clearAllMocks();
	storage = new Map();
	vi.stubGlobal("localStorage", {
		getItem: (key: string) => storage.get(key) ?? null,
		setItem: (key: string, value: string) => storage.set(key, value),
		removeItem: (key: string) => storage.delete(key),
	});
	host.state.recordChoiceOpen = true;
	host.update.loadSettings.mockResolvedValue(defaultUpdateSettings);
	host.update.saveSettings.mockImplementation(async (settings) => settings);
});
afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

async function renderHost(text = "RECORD ") {
	const authority = createCommandLineTestAuthority({ text });
	host.state.recordChoiceOpen = false;
	const view = render(authority.wrap(<RecordChoiceHost />));
	await act(authority.settle);
	// RECORD RECORD opens the choice on a settled, armed command line.
	host.state.recordChoiceOpen = true;
	view.rerender(authority.wrap(<RecordChoiceHost />));
	const dialog = await screen.findByRole("dialog", { name: "Record" });
	await waitFor(() =>
		expect(within(dialog).getByRole("button", { name: "Record" })).toBeEnabled(),
	);
	return { authority, dialog };
}

describe("RECORD RECORD choice", () => {
	it("arms a one-off Merge without touching the stored default", async () => {
		const { authority, dialog } = await renderHost();
		fireEvent.click(within(dialog).getByRole("radio", { name: "Merge" }));
		fireEvent.click(within(dialog).getByRole("button", { name: "Record" }));
		await waitFor(() =>
			expect(authority.writes.at(-1)?.text).toBe("RECORD MERGE "),
		);
		expect(host.update.saveSettings).not.toHaveBeenCalled();
		expect(host.dispatch).toHaveBeenCalledWith({
			type: "SET_STORE_ARMED",
			value: true,
		});
		expect(host.dispatch).toHaveBeenCalledWith({
			type: "SET_MODAL",
			modal: "recordChoiceOpen",
			value: false,
		});
	});

	it("stores Add Cue as the default, so the line stays a plain RECORD", async () => {
		const { authority, dialog } = await renderHost();
		fireEvent.click(within(dialog).getByRole("radio", { name: "Add Cue" }));
		fireEvent.click(within(dialog).getByRole("switch", { name: "Set as default" }));
		fireEvent.click(within(dialog).getByRole("button", { name: "Record" }));
		await waitFor(() =>
			expect(host.update.saveSettings).toHaveBeenCalledWith({
				...defaultUpdateSettings,
				record_default: "add_cue",
			}),
		);
		await waitFor(() =>
			expect(host.dispatch).toHaveBeenCalledWith({
				type: "SET_STORE_ARMED",
				value: true,
			}),
		);
		expect(authority.writes.every((write) => write.text === "RECORD ")).toBe(
			true,
		);
	});

	it("shows the stored default and the line's one-off option", async () => {
		host.update.loadSettings.mockResolvedValue({
			...defaultUpdateSettings,
			record_default: "add_cue",
		});
		const { dialog } = await renderHost("RECORD ADD EXISTING ");
		await waitFor(() => {
			expect(
				within(dialog).getByText("Add Cue", { selector: "b" }),
			).toBeInTheDocument();
			expect(
				within(dialog).getByRole("radio", { name: "Add Existing" }),
			).toHaveAttribute("aria-checked", "true");
		});
	});

	it("moves the retired Merge into active Cue setting into the desk default once", async () => {
		storage.set("light.store-merge-active-cue", "true");
		storage.set("light.store-mode", "merge");
		host.state.recordChoiceOpen = false;
		const authority = createCommandLineTestAuthority({ text: "" });
		render(authority.wrap(<RecordChoiceHost />));
		await act(authority.settle);
		await waitFor(() =>
			expect(host.update.saveSettings).toHaveBeenCalledWith({
				...defaultUpdateSettings,
				record_default: "merge",
			}),
		);
		await waitFor(() => expect(storage.size).toBe(0));
	});
});
