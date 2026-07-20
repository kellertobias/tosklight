import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { UpdateSettings, UpdateTargetRequest } from "../../api/types";
import {
	defaultUpdateSettings,
	UPDATE_SETTINGS_EVENT,
	UPDATE_TARGET_EVENT,
	UPDATE_TARGET_MENU_EVENT,
} from "../control/updateWorkflow";
import { UpdateWorkflow } from "./UpdateWorkflow";
import {
	addNewAuthority,
	cueEntry,
	existingAuthority,
	mutationFor,
	targetsFor,
} from "./updateWorkflowTestFixtures";

const workflow = vi.hoisted(() => {
	const state = { updateArmed: false, shiftArmed: false };
	const dispatch = vi.fn((action: { type: string; value?: boolean }) => {
		if (action.type === "SET_UPDATE_ARMED")
			state.updateArmed = Boolean(action.value);
		if (action.type === "SET_SHIFT_ARMED")
			state.shiftArmed = Boolean(action.value);
	});
	return {
		state,
		dispatch,
		update: {
			scopeKey: "authority-a",
			loadSettings: vi.fn(),
			saveSettings: vi.fn(),
			preview: vi.fn(),
			targets: vi.fn(),
			confirm: vi.fn(),
			applyDirect: vi.fn(),
		},
		server: {
			commandLine: "",
			commandTargetMode: "Fixture",
			commandLinePristine: true,
			setCommandLine: vi.fn(),
			resetCommandLine: vi.fn(),
			executeCommandLine: vi.fn(),
			cancelCommandChoice: vi.fn(),
		},
	};
});

vi.mock("../../features/programmingUpdate/ProgrammingUpdateProvider", () => ({
	useProgrammingUpdate: () => workflow.update,
}));
vi.mock("../../api/ServerContext", () => ({
	useServer: () => workflow.server,
}));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: workflow.state, dispatch: workflow.dispatch }),
}));

beforeEach(() => {
	workflow.state.updateArmed = false;
	workflow.state.shiftArmed = false;
	workflow.update.scopeKey = "authority-a";
	workflow.server.commandLine = "";
	vi.clearAllMocks();
	workflow.update.loadSettings.mockResolvedValue(defaultUpdateSettings);
	workflow.update.saveSettings.mockResolvedValue(defaultUpdateSettings);
	workflow.update.targets.mockResolvedValue(targetsFor([]));
	workflow.update.preview.mockResolvedValue(null);
	workflow.update.confirm.mockResolvedValue(null);
	workflow.update.applyDirect.mockResolvedValue(null);
});

afterEach(cleanup);

describe("Update workflow integration", () => {
	it("loads the typed menu and confirms the exact Add New authority", async () => {
		workflow.update.targets.mockResolvedValue(targetsFor());
		workflow.update.confirm.mockResolvedValue(mutationFor());
		render(<UpdateWorkflow />);

		fireEvent(window, new Event(UPDATE_TARGET_MENU_EVENT));
		const dialog = await screen.findByRole("dialog", { name: "Update Update" });
		expect(workflow.update.targets).toHaveBeenCalledWith(
			"eligible_for_update_existing",
		);
		expect(
			within(dialog).queryByText("Mode for Main Cuelist", {
				selector: "label",
			}),
		).not.toBeInTheDocument();

		fireEvent.click(
			within(dialog).getByRole("button", { name: "Show All Active" }),
		);
		await waitFor(() =>
			expect(workflow.update.targets).toHaveBeenLastCalledWith(
				"show_all_active",
			),
		);
		const modeLabel = within(dialog).getByText("Mode for Main Cuelist", {
			selector: "label",
		});
		const modeTrigger = modeLabel
			.closest(".ui-form-field")
			?.querySelector(".ui-select-trigger") as HTMLButtonElement;
		expect(modeTrigger).toHaveTextContent("Existing Only");

		fireEvent.click(modeTrigger);
		fireEvent.click(screen.getByRole("option", { name: "Add New" }));
		fireEvent.click(within(dialog).getByRole("button", { name: "Update" }));

		await waitFor(() =>
			expect(workflow.update.confirm).toHaveBeenCalledWith(addNewAuthority),
		);
		expect(addNewAuthority.requestTarget).toMatchObject({
			type: "cue",
			cue_id: "cue-2",
			cue_number: 2,
			validate_active_context: true,
		});
		expect(addNewAuthority.object).toEqual({
			kind: "cue_list",
			object_id: "legacy-cue-list-a",
			object_revision: 4,
		});
		expect(addNewAuthority.scopeKey).toBe(workflow.update.scopeKey);
		expect(workflow.update.applyDirect).not.toHaveBeenCalled();
		expect(
			await screen.findByRole("dialog", { name: "Update complete" }),
		).toBeInTheDocument();
	});

	it("uses applyDirect for a touched target whose configured modal is disabled", async () => {
		const request: UpdateTargetRequest = {
			family: { type: "group" },
			object_id: "3",
		};
		const settings: UpdateSettings = {
			...defaultUpdateSettings,
			group_mode: "add_new",
			show_update_modal_on_touch: false,
		};
		const target = {
			family: { type: "group" as const },
			object_id: "3",
			name: "Group 3",
		};
		workflow.state.updateArmed = true;
		workflow.state.shiftArmed = true;
		workflow.server.commandLine = "UPDATE GROUP 3";
		workflow.update.loadSettings.mockResolvedValue(settings);
		workflow.update.applyDirect.mockResolvedValue(mutationFor(target));
		render(<UpdateWorkflow />);

		expect(screen.getByRole("status")).toHaveTextContent("UPDATE armed");
		fireEvent(
			window,
			new CustomEvent<UpdateTargetRequest>(UPDATE_TARGET_EVENT, {
				detail: request,
			}),
		);

		await waitFor(() =>
			expect(workflow.update.applyDirect).toHaveBeenCalledWith(request, {
				target_type: "existing_content",
				mode: "add_new",
			}),
		);
		expect(workflow.update.preview).not.toHaveBeenCalled();
		expect(workflow.update.confirm).not.toHaveBeenCalled();
		expect(
			await screen.findByRole("dialog", { name: "Update complete" }),
		).toBeInTheDocument();
		expect(workflow.dispatch).toHaveBeenCalledWith({
			type: "SET_UPDATE_ARMED",
			value: false,
		});
		expect(workflow.dispatch).toHaveBeenCalledWith({
			type: "SET_SHIFT_ARMED",
			value: false,
		});
		expect(workflow.server.resetCommandLine).toHaveBeenCalledOnce();
	});

	it("confirms the exact preview authority for a Cue-less touched request", async () => {
		const request: UpdateTargetRequest = {
			family: { type: "cue" },
			object_id: "cue-list-a",
			playback_number: 7,
			validate_active_context: true,
		};
		workflow.state.updateArmed = true;
		workflow.update.loadSettings.mockResolvedValue({
			...defaultUpdateSettings,
			cue_mode: "existing_only",
		});
		workflow.update.preview.mockResolvedValue(existingAuthority);
		workflow.update.confirm.mockResolvedValue(mutationFor());
		render(<UpdateWorkflow />);

		fireEvent(
			window,
			new CustomEvent<UpdateTargetRequest>(UPDATE_TARGET_EVENT, {
				detail: request,
			}),
		);
		const dialog = await screen.findByRole("dialog", {
			name: "Update Main Cuelist",
		});
		fireEvent.click(
			within(dialog).getByRole("button", { name: "Update Cuelist" }),
		);

		expect(workflow.update.preview).toHaveBeenCalledWith(
			request,
			existingAuthority.preview.mode,
		);
		await waitFor(() =>
			expect(workflow.update.confirm).toHaveBeenCalledWith(existingAuthority),
		);
		expect(existingAuthority.requestTarget).toMatchObject({
			cue_id: "cue-2",
			cue_number: 2,
		});
	});

	it("loads and saves settings through the scoped capability", async () => {
		workflow.update.saveSettings.mockImplementation(
			async (settings) => settings,
		);
		render(<UpdateWorkflow />);

		fireEvent(window, new Event(UPDATE_SETTINGS_EVENT));
		const dialog = await screen.findByRole("dialog", {
			name: "Update Settings",
		});
		expect(workflow.update.loadSettings).toHaveBeenCalledOnce();
		fireEvent.click(
			within(dialog).getByRole("switch", {
				name: "Show Update modal on touch",
			}),
		);
		fireEvent.click(
			within(dialog).getByRole("button", { name: "Save Update Settings" }),
		);

		await waitFor(() =>
			expect(workflow.update.saveSettings).toHaveBeenCalledWith({
				...defaultUpdateSettings,
				show_update_modal_on_touch: false,
			}),
		);
	});

	it("shows a local capability error without falling back to legacy Update methods", async () => {
		workflow.update.targets.mockRejectedValue(
			new Error("target query rejected"),
		);
		render(<UpdateWorkflow />);

		fireEvent(window, new Event(UPDATE_TARGET_MENU_EVENT));
		const dialog = await screen.findByRole("dialog", { name: "Update Update" });
		expect(await within(dialog).findByRole("alert")).toHaveTextContent(
			"target query rejected",
		);
	});

	it("ignores a late target response after the capability scope changes", async () => {
		const pending = deferred<ReturnType<typeof targetsFor>>();
		workflow.update.targets.mockReturnValue(pending.promise);
		const view = render(<UpdateWorkflow />);

		fireEvent(window, new Event(UPDATE_TARGET_MENU_EVENT));
		await screen.findByRole("dialog", { name: "Update Update" });
		workflow.update.scopeKey = "authority-b";
		view.rerender(<UpdateWorkflow />);
		await waitFor(() =>
			expect(
				screen.queryByRole("dialog", { name: "Update Update" }),
			).not.toBeInTheDocument(),
		);

		const lateAuthority = targetsFor([cueEntry]);
		expect(lateAuthority.scopeKey).toBe("authority-a");
		pending.resolve(lateAuthority);
		await Promise.resolve();
		expect(screen.queryByText("Main Cuelist")).not.toBeInTheDocument();
	});
});

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((finish) => {
		resolve = finish;
	});
	return { promise, resolve };
}
