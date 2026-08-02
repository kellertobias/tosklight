import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { GroupSettingsDialog } from "./GroupSettingsDialog";
import type { Group } from "./model";
import { defaultSpatialMapping } from "./spatialMapping";

const manage = vi.fn();
vi.mock("../../features/groupManagement/GroupManagementProvider", () => ({
	useGroupManagement: () => ({ manage }),
}));

function group(overrides: Record<string, unknown> = {}): Group {
	return {
		kind: "group",
		id: "4",
		revision: 7,
		updated_at: "",
		body: {
			name: "Front Truss",
			color: "#718596",
			icon: "◇",
			fixtures: ["fixture-1", "fixture-2"],
			programming: {},
			...overrides,
		},
	} as Group;
}

function renderDialog(
	target = group(),
	mappingActions: Parameters<
		typeof GroupSettingsDialog
	>[0]["mappingActions"] = null,
) {
	return render(
		<ModalProvider>
			<GroupSettingsDialog
				group={target}
				groups={[target]}
				onClose={vi.fn()}
				mappingActions={mappingActions}
			/>
		</ModalProvider>,
	);
}

describe("Group settings modal", () => {
	afterEach(() => cleanup());
	beforeEach(() => {
		manage.mockReset().mockResolvedValue({
			status: "changed",
			group: { revision: 8 },
			persistenceWarning: null,
		});
	});

	it("has only General, Projection, and Phaser plus an X close control", () => {
		renderDialog();
		const dialog = screen.getByRole("dialog", { name: "Group 4 settings" });
		expect(
			within(dialog)
				.getAllByRole("tab")
				.map((tab) => tab.textContent),
		).toEqual(["General", "Projection", "Phaser"]);
		expect(
			within(dialog).getByRole("button", { name: "Close settings" }),
		).toBeInTheDocument();
		for (const retired of [
			"Master",
			"Select live group",
			"Select frozen group",
			"Replace membership with selection",
			"Undo membership/programming change",
		])
			expect(within(dialog).queryByText(retired)).toBeNull();
		expect(
			within(dialog).queryByRole("button", { name: /Apply|Save|Cancel/ }),
		).toBeNull();
	});

	it("saves a General field immediately with the observed revision", async () => {
		renderDialog();
		fireEvent.change(screen.getByLabelText("Group name"), {
			target: { value: "FOH Movers" },
		});
		fireEvent.blur(screen.getByLabelText("Group name"));

		await waitFor(() =>
			expect(manage).toHaveBeenCalledWith({
				objectId: "4",
				expectedObjectRevision: 7,
				operation: {
					type: "update_properties",
					properties: {
						name: "FOH Movers",
						color: "#718596",
						icon: "◇",
					},
				},
			}),
		);
	});

	it("shows mapping state without enabling writes when the typed server action is absent", () => {
		renderDialog(group({ mapping: defaultSpatialMapping() }));
		fireEvent.click(screen.getByRole("tab", { name: "Projection" }));
		expect(screen.getByText("Local override")).toBeInTheDocument();
		expect(
			screen.getByText(/revisioned Group mapping action/),
		).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Top" })).toBeDisabled();
		expect(
			screen.getByRole("button", { name: "Remove local mapping" }),
		).toBeDisabled();
	});

	it("sends one complete revisioned mapping when a Phaser shape changes", async () => {
		const update = vi.fn().mockResolvedValue({ revision: 8 });
		renderDialog(group({ mapping: defaultSpatialMapping() }), { update });
		fireEvent.click(screen.getByRole("tab", { name: "Phaser" }));
		fireEvent.click(screen.getByRole("radio", { name: "Radial" }));

		await waitFor(() => expect(update).toHaveBeenCalledOnce());
		expect(update).toHaveBeenCalledWith(
			"4",
			7,
			expect.objectContaining({
				shape: {
					type: "radial",
					center_u: 0,
					center_v: 0,
					direction: "outward",
				},
			}),
		);
	});
});
