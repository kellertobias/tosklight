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
		// The Phase preview plots the authoritative projected positions rather than listing
		// them, so the snapshot is read through the picture the operator actually looks at.
		fireEvent.click(screen.getByRole("tab", { name: "Phase" }));
		const preview = await screen.findByRole("img", {
			name: /Phase order across the projected plane/,
		});
		const plotted = preview.querySelectorAll("circle.phase-order-fixture");
		expect(plotted).toHaveLength(1);
		// One rank out of one, so the only fixture sits at the black end of the shading.
		expect(plotted[0]?.getAttribute("fill")).toBe("hsl(0 0% 0.0%)");
		expect(screen.getByText(/1 rank/)).toBeInTheDocument();
		expect(settings).toHaveBeenCalledWith("4");
	});

	it("has only General, Projection, and Phase plus an X close control", () => {
		renderDialog();
		const dialog = screen.getByRole("dialog", { name: "Group 4 settings" });
		expect(
			within(dialog)
				.getAllByRole("tab")
				.map((tab) => tab.textContent),
		).toEqual(["General", "Projection", "Phase"]);
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
		const status = document.querySelector(".group-settings-status");
		expect(status).toBeInTheDocument();
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
		expect(document.querySelector(".group-settings-status")).toBe(status);
		expect(screen.queryByText("Saved.")).toBeNull();
	});

	it("shows the projection without enabling writes when the typed server action is absent", () => {
		managementAvailable = false;
		renderDialog(group({ mapping: defaultSpatialMapping() }));
		fireEvent.click(screen.getByRole("tab", { name: "Projection" }));
		expect(
			screen.getByText(/revisioned Group mapping action/),
		).toBeInTheDocument();
		// Everything that would write is unusable, but the values are still readable.
		expect(screen.getByRole("button", { name: "Top" })).toBeDisabled();
		expect(screen.getByRole("radio", { name: "Cylindrical" })).toBeDisabled();
		expect(screen.getByLabelText("Azimuth")).toBeDisabled();
	});

	it("gives Projection type a full row and separates position from direction", () => {
		const base = defaultSpatialMapping();
		const mapping = {
			...base,
			projection: {
				...base.projection,
				kind: "cylindrical" as const,
				preset: null,
			},
		};
		const target = group({ mapping });
		settings.mockResolvedValue(settingsSnapshot(target));
		renderDialog(target);
		fireEvent.click(screen.getByRole("tab", { name: "Projection" }));

		const projectionType = document.querySelector(".group-projection-kinds");
		expect(projectionType).toBeInTheDocument();
		expect(
			projectionType?.closest(".group-mapping-fields"),
		).toBeInTheDocument();

		const position = screen.getByRole("group", { name: "Position" });
		const direction = screen.getByRole("group", { name: "Direction" });
		expect(within(position).getByLabelText("Position X")).toBeInTheDocument();
		expect(within(position).getByLabelText("Position Y")).toBeInTheDocument();
		expect(within(position).getByLabelText("Position Z")).toBeInTheDocument();
		expect(
			within(position).queryByRole("button", { name: "Decrease value" }),
		).toBeNull();
		expect(within(position).queryByLabelText("Azimuth")).toBeNull();
		expect(within(direction).getByLabelText("Azimuth")).toBeInTheDocument();
		expect(within(direction).getByLabelText("Elevation")).toBeInTheDocument();
		expect(within(direction).getByLabelText("Rotation")).toBeInTheDocument();
		expect(within(direction).queryByLabelText("Position X")).toBeNull();
	});

	it("changes projection kind immediately while its revisioned save is in flight", async () => {
		const base = defaultSpatialMapping();
		const mapping = {
			...base,
			projection: { ...base.projection, kind: "cylindrical" as const },
		};
		const target = group({ mapping });
		settings.mockResolvedValue(settingsSnapshot(target));
		let settleSave: ((value: unknown) => void) | undefined;
		manage.mockReturnValueOnce(
			new Promise((resolve) => {
				settleSave = resolve;
			}),
		);
		renderDialog(target);
		fireEvent.click(screen.getByRole("tab", { name: "Projection" }));
		await waitFor(() => expect(settings).toHaveBeenCalled());

		fireEvent.click(screen.getByRole("radio", { name: "Planar" }));

		expect(screen.getByRole("radio", { name: "Planar" })).toHaveAttribute(
			"aria-checked",
			"true",
		);
		expect(screen.getByLabelText("Azimuth")).toBeInTheDocument();
		expect(screen.queryByLabelText("Position X")).toBeNull();
		expect(manage).toHaveBeenCalledWith(
			expect.objectContaining({
				operation: expect.objectContaining({
					type: "set_spatial_mapping",
					mapping: expect.objectContaining({
						projection: expect.objectContaining({ kind: "planar" }),
					}),
				}),
			}),
		);
		settleSave?.({
			status: "changed",
			group: { revision: 8 },
			showRevision: 13,
			persistenceWarning: null,
		});
	});

	it("keeps stable-height containers while Projection and Phase variants change", async () => {
		const base = defaultSpatialMapping();
		const mapping = {
			...base,
			projection: { ...base.projection, kind: "cylindrical" as const },
		};
		const target = group({ mapping });
		settings.mockResolvedValue(settingsSnapshot(target));
		renderDialog(target);

		fireEvent.click(screen.getByRole("tab", { name: "Projection" }));
		await waitFor(() => expect(settings).toHaveBeenCalled());
		const projection = document.querySelector(".group-projection-fields");
		expect(projection).toBeInTheDocument();
		for (const kind of ["Planar", "Spherical", "Cylindrical"]) {
			fireEvent.click(screen.getByRole("radio", { name: kind }));
			expect(document.querySelector(".group-projection-fields")).toBe(
				projection,
			);
		}

		fireEvent.click(screen.getByRole("tab", { name: "Phase" }));
		const phase = document.querySelector(".group-phase-fields");
		expect(phase).toBeInTheDocument();
		for (const shape of ["Radial", "Radar", "Grid"]) {
			fireEvent.click(screen.getByRole("radio", { name: shape }));
			expect(document.querySelector(".group-phase-fields")).toBe(phase);
		}
	});

	it("keeps six Planar presets in three paired rows and compact stepped directions", async () => {
		const target = group({ mapping: defaultSpatialMapping() });
		settings.mockResolvedValue(settingsSnapshot(target));
		renderDialog(target);
		fireEvent.click(screen.getByRole("tab", { name: "Projection" }));
		const presets = screen.getByRole("group", { name: "Presets" });
		const direction = screen.getByRole("group", { name: "Direction" });
		expect(
			within(presets)
				.getAllByRole("button")
				.map((button) => button.textContent),
		).toEqual(["Top", "Bottom", "Front", "Back", "Left", "Right"]);
		expect(
			presets.querySelector(".group-projection-presets"),
		).toBeInTheDocument();
		const azimuth = screen
			.getByLabelText("Azimuth")
			.closest<HTMLElement>(".ui-form-field")!;
		fireEvent.click(
			within(azimuth).getByRole("button", { name: "Decrease value" }),
		);
		await waitFor(() => expect(manage).toHaveBeenCalled());

		manage.mockClear();
		fireEvent.click(
			within(azimuth).getByRole("button", { name: "Open number pad" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "9" }));
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));
		await waitFor(() =>
			expect(manage).toHaveBeenCalledWith(
				expect.objectContaining({
					operation: expect.objectContaining({
						type: "set_spatial_mapping",
						mapping: expect.objectContaining({
							projection: expect.objectContaining({
								view_direction: expect.any(Object),
							}),
						}),
					}),
				}),
			),
		);
	});

	it("uses the default Selection Order placeholder and aligned Phase rows", async () => {
		renderDialog(group({ mapping: defaultSpatialMapping() }));
		fireEvent.click(screen.getByRole("tab", { name: "Phase" }));
		expect(
			screen.getByRole("radio", { name: "Selection Order" }),
		).toHaveAttribute("aria-checked", "true");
		expect(
			screen.getByText(/order in which the fixtures were selected/),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("radio", { name: "Radar" }));
		await waitFor(() =>
			expect(document.querySelectorAll(".group-phase-row")).toHaveLength(2),
		);
		expect(screen.getByLabelText("Centre U")).toBeInTheDocument();
		expect(screen.getByLabelText("Sweep")).toBeInTheDocument();
		expect(
			screen.getByLabelText("Sweep").closest(".ui-form-field"),
		).toHaveClass("labels-top");
		fireEvent.click(screen.getByRole("radio", { name: "Fixture ID" }));
		expect(screen.getByText(/lowest ID to the highest/)).toBeInTheDocument();
	});

	it("sends one complete revisioned mapping when a Phase shape changes", async () => {
		const target = group({ mapping: defaultSpatialMapping() });
		settings.mockResolvedValue(settingsSnapshot(target));
		renderDialog(target);
		fireEvent.click(screen.getByRole("tab", { name: "Phase" }));
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
