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
const settings = vi.fn();
const managementActions = { manage, settings };
let managementAvailable = true;
vi.mock("../../features/groupManagement/GroupManagementProvider", () => ({
	useGroupManagement: () => (managementAvailable ? managementActions : null),
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

function renderDialog(target = group()) {
	return render(
		<ModalProvider>
			<GroupSettingsDialog group={target} groups={[target]} onClose={vi.fn()} />
		</ModalProvider>,
	);
}

function settingsSnapshot(target = group()) {
	const mapping = target.body.mapping ?? null;
	return {
		showId: "11111111-1111-4111-8111-111111111111",
		showRevision: 12,
		group: { id: target.id, revision: target.revision, object: target },
		resolvedSpatial: {
			source_order: target.body.fixtures,
			effective_mapping: mapping,
			mapping_provenance: mapping ? { type: "local" } : { type: "none" },
			ordered_fixture_ids: target.body.fixtures,
			projected_positions: [],
			ranks: [],
			rank_count: target.body.fixtures.length,
			warnings: [],
		},
	};
}

describe("Group settings modal", () => {
	afterEach(() => cleanup());
	beforeEach(() => {
		managementAvailable = true;
		settings.mockReset().mockResolvedValue(settingsSnapshot());
		manage.mockReset().mockResolvedValue({
			status: "changed",
			group: { revision: 8 },
			showRevision: 13,
			persistenceWarning: null,
		});
	});

	it("reconciles mapping provenance and ranks from the authoritative settings snapshot", async () => {
		settings.mockResolvedValue({
			showId: "11111111-1111-4111-8111-111111111111",
			showRevision: 12,
			group: { id: "4", revision: 9, object: group() },
			resolvedSpatial: {
				source_order: ["11111111-1111-4111-8111-111111111111"],
				effective_mapping: defaultSpatialMapping(),
				mapping_provenance: {
					type: "inherited",
					source_group_ids: ["2"],
				},
				ordered_fixture_ids: ["11111111-1111-4111-8111-111111111111"],
				projected_positions: [
					{
						fixture_id: "11111111-1111-4111-8111-111111111111",
						u: 1.25,
						v: -2.5,
					},
				],
				ranks: [
					{
						fixture_id: "11111111-1111-4111-8111-111111111111",
						rank: 0,
					},
				],
				rank_count: 1,
				warnings: [],
			},
		});
		renderDialog();
		fireEvent.click(screen.getByRole("tab", { name: "Projection" }));

		await waitFor(() =>
			expect(screen.getByText("Inherited from Group 2")).toBeInTheDocument(),
		);
		expect(screen.getByText(/1 authoritative ranks/)).toBeInTheDocument();
		expect(screen.getByText("U 1.25 · V -2.5")).toBeInTheDocument();
		expect(settings).toHaveBeenCalledWith("4");
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
				expectedShowRevision: 12,
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
		managementAvailable = false;
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
		const target = group({ mapping: defaultSpatialMapping() });
		settings.mockResolvedValue(settingsSnapshot(target));
		renderDialog(target);
		fireEvent.click(screen.getByRole("tab", { name: "Phaser" }));
		fireEvent.click(screen.getByRole("radio", { name: "Radial" }));

		await waitFor(() => expect(manage).toHaveBeenCalledOnce());
		expect(manage).toHaveBeenCalledWith({
			objectId: "4",
			expectedObjectRevision: 7,
			expectedShowRevision: 12,
			operation: expect.objectContaining({
				type: "set_spatial_mapping",
				mapping: expect.objectContaining({
					shape: {
						type: "radial",
						center_u: 0,
						center_v: 0,
						direction: "outward",
					},
				}),
			}),
		});
	});
});
