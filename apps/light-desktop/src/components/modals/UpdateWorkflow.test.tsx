import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	UpdateMode,
	UpdatePreview,
	UpdateSettings,
	UpdateTargetRequest,
} from "../../api/types";
import type { ProgrammingUpdateMenuEntry } from "../../features/programmingUpdate/contracts";
import {
	configuredUpdateMode,
	cueUpdateTarget,
	defaultUpdateSettings,
} from "../control/updateWorkflow";
import {
	UpdateOperationDialog,
	UpdateSettingsDialog,
	UpdateTargetMenu,
	updatePreviewStats,
} from "./UpdateWorkflow";
import { cueMenuEntryFor } from "./updateWorkflowTestFixtures";

const request: UpdateTargetRequest = cueUpdateTarget("cue-list-a", 7, {
	id: "cue-2",
	number: "2",
});
const existingOnly: UpdateMode = { target_type: "cue", mode: "existing_only" };
const target = {
	family: { type: "cue" as const },
	object_id: "cue-list-a",
	name: "Main Cuelist",
	playback_number: 7,
	cue: { id: "cue-2", number: "2" },
};
const preview: UpdatePreview = {
	revision: 4,
	show_revision: 12,
	programmer_revision: "programmer-a",
	target,
	mode: existingOnly,
	items: [
		{
			address: {
				type: "fixture_attribute",
				fixture_id: "fixture-1",
				attribute: "intensity",
			},
			outcome: {
				outcome: "change_at_source",
				source: { cue_id: "cue-1", cue_number: "1", cue_index: 0 },
			},
		},
		{
			address: {
				type: "fixture_attribute",
				fixture_id: "fixture-1",
				attribute: "color.red",
			},
			outcome: { outcome: "ignored", reason: "new_address" },
		},
	],
};

function titleBarOf(dialog: HTMLElement) {
	const header = dialog.querySelector<HTMLElement>("header.ui-modal-titlebar");
	if (!header) throw new Error("dialog has no modal title bar");
	return header;
}

afterEach(cleanup);

describe("Update workflow", () => {
	it("shows the four literal Cue modes and authoritative source/ignored preview before applying", () => {
		const onMode = vi.fn();
		const onApply = vi.fn();
		const onCancel = vi.fn();
		render(
			<UpdateOperationDialog
				operation={{ request, preview }}
				busy={false}
				error={null}
				onMode={onMode}
				onApply={onApply}
				onCancel={onCancel}
			/>,
		);

		const dialog = screen.getByRole("dialog", { name: "Update Main Cuelist" });
		expect(dialog).toHaveClass("workflow-theme", "update-workflow");
		expect(within(dialog).getByText("UPDATE")).toBeInTheDocument();
		expect(
			within(dialog).getByText("Cuelist · Playback 7 · Current Cue 2"),
		).toBeInTheDocument();
		for (const label of ["Update", "Tracked", "Known", "All"])
			expect(
				within(dialog).getByRole("button", { name: label }),
			).toBeInTheDocument();
		expect(
			within(dialog).getByText("Change at source Cue 1"),
		).toBeInTheDocument();
		expect(
			within(dialog).getByText("Ignored · address is new to this target"),
		).toBeInTheDocument();
		expect(updatePreviewStats(preview)).toMatchObject({
			eligible: 1,
			changed: 1,
			ignored: 1,
			source: 1,
		});

		fireEvent.click(within(dialog).getByRole("button", { name: "Known" }));
		expect(onMode).toHaveBeenCalledWith({
			target_type: "cue",
			mode: "add_to_current_cue",
		});
		const titleBar = within(titleBarOf(dialog));
		expect(titleBar.getByRole("heading")).toHaveTextContent(
			"UPDATE Main Cuelist",
		);
		expect(
			within(dialog).getByText(/nothing is stored until you press Update/),
		).toBeInTheDocument();
		expect(dialog.querySelector(".modal-actions")).toBeNull();
		fireEvent.click(titleBar.getByRole("button", { name: "Update Cuelist" }));
		expect(onApply).toHaveBeenCalledTimes(1);
		fireEvent.click(titleBar.getByRole("button", { name: "Cancel" }));
		expect(onCancel).toHaveBeenCalledTimes(1);
	});

	it("disables the title Update action when nothing would change and while busy", () => {
		const noChange: UpdatePreview = {
			...preview,
			items: [preview.items[1]],
		};
		const { rerender } = render(
			<UpdateOperationDialog
				operation={{ request, preview: noChange }}
				busy={false}
				error={null}
				onMode={vi.fn()}
				onApply={vi.fn()}
				onCancel={vi.fn()}
			/>,
		);
		let titleBar = within(
			titleBarOf(screen.getByRole("dialog", { name: "Update Main Cuelist" })),
		);
		expect(
			titleBar.getByRole("button", { name: "Update Cuelist" }),
		).toBeDisabled();
		expect(titleBar.getByRole("button", { name: "Cancel" })).toBeEnabled();
		rerender(
			<UpdateOperationDialog
				operation={{ request, preview }}
				busy
				error={null}
				onMode={vi.fn()}
				onApply={vi.fn()}
				onCancel={vi.fn()}
			/>,
		);
		titleBar = within(
			titleBarOf(screen.getByRole("dialog", { name: "Update Main Cuelist" })),
		);
		expect(titleBar.getByRole("button", { name: "Updating…" })).toBeDisabled();
		expect(titleBar.getByRole("button", { name: "Cancel" })).toBeDisabled();
	});

	it("keeps deterministic desk defaults separate from show programming", () => {
		const onChange = vi.fn();
		const onSave = vi.fn();
		render(
			<UpdateSettingsDialog
				settings={defaultUpdateSettings}
				busy={false}
				error={null}
				onChange={onChange}
				onSave={onSave}
				onCancel={vi.fn()}
			/>,
		);
		const dialog = screen.getByRole("dialog", { name: "Update Settings" });
		expect(dialog).toHaveClass("workflow-theme", "update-workflow");
		expect(within(dialog).getByText("UPDATE")).toHaveClass("workflow-badge");
		const titleBar = within(titleBarOf(dialog));
		expect(titleBar.getByRole("heading")).toHaveTextContent("UPDATE Settings");
		expect(
			within(dialog).getByText(
				"Which Update mode the desk uses for each kind of target. Saved for this desk; show programming does not change.",
			),
		).toBeInTheDocument();
		expect(dialog.querySelector(".modal-actions")).toBeNull();
		expect(
			within(dialog).getByRole("button", { name: "Update" }),
		).toBeInTheDocument();
		expect(
			within(dialog).getAllByRole("button", { name: /Update Existing/ }),
		).toHaveLength(2);
		fireEvent.click(
			within(dialog).getByRole("switch", {
				name: "Show Update modal on touch",
			}),
		);
		expect(onChange).toHaveBeenCalledWith(
			expect.objectContaining({ show_update_modal_on_touch: false }),
		);
		fireEvent.click(titleBar.getByRole("button", { name: "Done" }));
		expect(onSave).toHaveBeenCalledTimes(1);
	});

	it("cancels Update Settings from the title bar and locks it while saving", () => {
		const onCancel = vi.fn();
		const { rerender } = render(
			<UpdateSettingsDialog
				settings={defaultUpdateSettings}
				busy={false}
				error={null}
				onChange={vi.fn()}
				onSave={vi.fn()}
				onCancel={onCancel}
			/>,
		);
		const dialog = () =>
			screen.getByRole("dialog", { name: "Update Settings" });
		fireEvent.click(
			within(titleBarOf(dialog())).getByRole("button", { name: "Cancel" }),
		);
		expect(onCancel).toHaveBeenCalledTimes(1);
		rerender(
			<UpdateSettingsDialog
				settings={defaultUpdateSettings}
				busy
				error={null}
				onChange={vi.fn()}
				onSave={vi.fn()}
				onCancel={onCancel}
			/>,
		);
		const titleBar = within(titleBarOf(dialog()));
		expect(titleBar.getByRole("button", { name: "Saving…" })).toBeDisabled();
		expect(titleBar.getByRole("button", { name: "Cancel" })).toBeDisabled();
	});

	it("distinguishes eligible targets from visible no-ops and applies the shown concrete context", () => {
		const noOp: UpdatePreview = {
			...preview,
			target: {
				...target,
				object_id: "cue-list-b",
				name: "No-op Cuelist",
				playback_number: 8,
			},
			items: [
				{
					address: {
						type: "fixture_attribute",
						fixture_id: "fixture-1",
						attribute: "intensity",
					},
					outcome: { outcome: "unchanged" },
				},
			],
		};
		const entries: ProgrammingUpdateMenuEntry[] = [
			cueMenuEntryFor(
				preview,
				{ ...preview, mode: { target_type: "cue", mode: "add_new" } },
				"legacy-cue-list-a",
			),
			cueMenuEntryFor(
				noOp,
				{ ...noOp, mode: { target_type: "cue", mode: "add_new" } },
				"legacy-cue-list-b",
			),
		];
		const onFilter = vi.fn();
		const onApply = vi.fn();
		render(
			<UpdateTargetMenu
				entries={entries}
				filter="eligible_for_update_existing"
				modes={{}}
				busyKey={null}
				error={null}
				onFilter={onFilter}
				onMode={vi.fn()}
				onApply={onApply}
				onCancel={vi.fn()}
			/>,
		);
		const dialog = screen.getByRole("dialog", { name: "Update Targets" });
		const titleBar = within(titleBarOf(dialog));
		expect(titleBar.getByRole("heading")).toHaveTextContent("UPDATE Targets");
		expect(titleBar.getByRole("button", { name: "Cancel" })).toBeEnabled();
		expect(dialog.querySelector(".modal-actions")).toBeNull();
		expect(
			within(dialog).getByText("Cuelist · Playback 7 · Current Cue 2"),
		).toBeInTheDocument();
		expect(
			within(dialog).getByText("No-op Cuelist").closest("article"),
		).toHaveTextContent("No eligible change");
		expect(
			within(dialog).getByRole("button", { name: "No changes" }),
		).toBeDisabled();
		fireEvent.click(
			within(dialog).getByRole("button", { name: "Show All Active" }),
		);
		expect(onFilter).toHaveBeenCalledWith("show_all_active");
		fireEvent.click(
			within(dialog).getAllByRole("button", { name: "Update" })[0],
		);
		expect(onApply).toHaveBeenCalledWith(entries[0], existingOnly);
	});

	it("maps each target family to its configured default without changing the target", () => {
		const settings: UpdateSettings = {
			...defaultUpdateSettings,
			cue_mode: "existing_in_current_cue",
			preset_mode: "add_new",
			group_mode: "update_existing",
		};
		expect(configuredUpdateMode(settings, request)).toEqual({
			target_type: "cue",
			mode: "existing_in_current_cue",
		});
		expect(
			configuredUpdateMode(settings, {
				family: { type: "preset" },
				object_id: "4",
			}),
		).toEqual({ target_type: "existing_content", mode: "add_new" });
		expect(
			configuredUpdateMode(settings, {
				family: { type: "group" },
				object_id: "3",
			}),
		).toEqual({ target_type: "existing_content", mode: "update_existing" });
	});
});
