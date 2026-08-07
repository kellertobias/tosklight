// @vitest-environment jsdom

import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	DynamicSpatialMappingOverrideProjection,
	DynamicSpatialPreviewResponse,
} from "../../api/types";
import type { ShowObject } from "../../features/showObjects/contracts";
import { DynamicProjectionView } from "./DynamicProjectionView";

type DynamicObject = ShowObject<"dynamic">;

afterEach(cleanup);

/** The projection kind is a segmented toggle, so each kind is a radio inside its group. */
function kindOption(name: string) {
	return within(
		screen.getByRole("radiogroup", { name: "Projection" }),
	).getByRole("radio", { name });
}

const inheritedMapping = {
	projection: {
		anchor: { x: 0, y: 0, z: 0 },
		view_direction: { x: 0, y: 0, z: -1 },
		rotation_degrees: 0,
		preset: "top" as const,
	},
	shape: {
		type: "grid" as const,
		angle_degrees: 0,
		direction: "ascending" as const,
	},
};

function dynamic(
	target_binding: DynamicObject["body"]["target_binding"],
	id = "dynamic-1",
): DynamicObject {
	return {
		id,
		revision: 7,
		body: {
			target_binding,
			spatial_mapping: {
				projection: { type: "inherit" },
				shape: { type: "inherit" },
			},
			lanes: [],
		},
	} as unknown as DynamicObject;
}

function preview(
	target: DynamicObject["body"]["target_binding"],
	draft: DynamicSpatialMappingOverrideProjection,
): DynamicSpatialPreviewResponse {
	const live = target.type === "live_group";
	return {
		show_id: "show-1",
		show_revision: 19,
		dynamic_id: "dynamic-1",
		dynamic_revision: 7,
		target_binding: target,
		base: live
			? {
					type: "live_group",
					group_id: target.group_id,
					mapping_provenance: { type: "local", group_id: target.group_id },
				}
			: { type: target.type },
		inherited_mapping: live ? inheritedMapping : null,
		draft,
		source_order: ["fixture-1"],
		ordered_fixture_ids: ["fixture-1"],
		ranks: [{ fixture_id: "fixture-1", rank: 0 }],
		rank_count: 1,
		warnings: [],
	};
}

describe("DynamicProjectionView", () => {
	it("shows the saved base and switches projection kind, offering only that kind's fields", async () => {
		const groupDynamic = dynamic({ type: "live_group", group_id: "front" });
		const view = render(
			<DynamicProjectionView
				dynamic={groupDynamic}
				busy={false}
				loadPreview={vi.fn(
					async (draft: DynamicSpatialMappingOverrideProjection) =>
						preview(groupDynamic.body.target_binding, draft),
				)}
				apply={vi.fn(
					async (_draft: DynamicSpatialMappingOverrideProjection) =>
						"applied" as const,
				)}
			/>,
		);

		expect(
			await screen.findByText("Inherit group mapping \u00b7 Group front"),
		).toBeTruthy();
		// Shape configuration lives on its own tab now.
		expect(screen.queryByText("Phase shape")).toBeNull();
		// Nothing to apply: changes persist as they are made.
		expect(screen.queryByRole("button", { name: "Apply" })).toBeNull();

		fireEvent.click(screen.getByRole("radio", { name: "Override" }));
		// Planar looks along a direction and takes a view preset, and reads no position.
		expect(screen.getByRole("radiogroup", { name: "View preset" })).toBeTruthy();
		expect(screen.getByLabelText("Direction X")).toBeTruthy();
		expect(screen.queryByLabelText("Position X")).toBeNull();

		fireEvent.click(kindOption("Cylindrical"));

		// A cylinder is placed and oriented by the same two, and has no view to preset.
		expect(await screen.findByLabelText("Position X")).toBeTruthy();
		expect(screen.getByLabelText("Direction X")).toBeTruthy();
		expect(screen.getByLabelText("Rotation")).toBeTruthy();
		expect(
			screen.queryByRole("radiogroup", { name: "View preset" }),
		).toBeNull();

		fireEvent.click(kindOption("Spherical"));
		// A roll about the centre of a spherical spread does not move it, so it has none.
		expect(await screen.findByLabelText("Direction X")).toBeTruthy();
		expect(screen.getByLabelText("Position X")).toBeTruthy();
		expect(screen.queryByLabelText("Rotation")).toBeNull();

		const frozen = dynamic(
			{ type: "frozen_targets", targets: ["fixture-1"] },
			"dynamic-2",
		);
		view.rerender(
			<DynamicProjectionView
				dynamic={frozen}
				busy={false}
				loadPreview={async (draft) => preview(frozen.body.target_binding, draft)}
				apply={vi.fn(
					async (_draft: DynamicSpatialMappingOverrideProjection) =>
						"applied" as const,
				)}
			/>,
		);
		expect(
			await screen.findByText("Selection order (no Group mapping)"),
		).toBeTruthy();
	});

	it("persists a change without an Apply button and keeps the draft on conflict", async () => {
		const value = dynamic({ type: "live_group", group_id: "front" });
		const apply = vi.fn(
			async (_draft: DynamicSpatialMappingOverrideProjection) =>
				"conflict" as const,
		);
		render(
			<DynamicProjectionView
				dynamic={value}
				busy={false}
				loadPreview={async (draft) => preview(value.body.target_binding, draft)}
				apply={apply}
			/>,
		);

		await screen.findByText("Inherit group mapping \u00b7 Group front");
		fireEvent.click(screen.getByRole("radio", { name: "Override" }));
		const presets = screen.getByRole("radiogroup", { name: "View preset" });
		expect(within(presets).getByRole("radio", { name: "Top" })).toBeChecked();
		fireEvent.click(within(presets).getByRole("radio", { name: "Front" }));

		// No Apply: the change saves itself once the operator stops editing.
		await waitFor(() => expect(apply).toHaveBeenCalled());
		expect(apply.mock.calls[0]?.[0]).toMatchObject({
			projection: {
				type: "replace",
				value: { preset: "front", view_direction: { x: 0, y: 1, z: 0 } },
			},
		});

		// A conflict says so and leaves the operator's override in place.
		await screen.findByText(/Authoritative values were reloaded/);
		expect(
			(screen.getByRole("radio", { name: "Override" }) as HTMLInputElement)
				.checked,
		).toBe(true);
	});

	it("sends the new projection kinds to the server", async () => {
		const value = dynamic({ type: "live_group", group_id: "front" });
		const apply = vi.fn(
			async (_draft: DynamicSpatialMappingOverrideProjection) =>
				"applied" as const,
		);
		render(
			<DynamicProjectionView
				dynamic={value}
				busy={false}
				loadPreview={async (draft) => preview(value.body.target_binding, draft)}
				apply={apply}
			/>,
		);

		await screen.findByText("Inherit group mapping \u00b7 Group front");
		fireEvent.click(screen.getByRole("radio", { name: "Override" }));
		fireEvent.click(kindOption("Cylindrical"));

		await waitFor(() => expect(apply).toHaveBeenCalled());
		const sent = apply.mock.calls.at(-1)?.[0];
		expect(sent).toMatchObject({
			projection: { type: "replace", value: { kind: "cylindrical" } },
		});
	});
});
