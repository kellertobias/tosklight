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
	it("shows saved Group and no-Group bases with dedicated controls and Random guidance", async () => {
		const groupDynamic = dynamic({ type: "live_group", group_id: "front" });
		const loadGroup = vi.fn(
			async (draft: DynamicSpatialMappingOverrideProjection) =>
				preview(groupDynamic.body.target_binding, draft),
		);
		const view = render(
			<DynamicProjectionView
				dynamic={groupDynamic}
				busy={false}
				loadPreview={loadGroup}
				apply={vi.fn(
					async (_draft: DynamicSpatialMappingOverrideProjection) =>
						"applied" as const,
				)}
			/>,
		);

		expect(
			await screen.findByText("Inherit group mapping · Group front"),
		).toBeTruthy();
		expect(
			screen.getByRole("heading", { name: "Projection stage" }),
		).toBeTruthy();
		expect(screen.getByRole("heading", { name: "Phase shape" })).toBeTruthy();
		const shape = screen
			.getByRole("heading", { name: "Phase shape" })
			.closest("section");
		if (!shape) throw new Error("missing Shape section");
		fireEvent.click(within(shape).getByRole("radio", { name: "Override" }));
		fireEvent.click(within(shape).getByRole("button", { name: "Shape" }));
		fireEvent.click(screen.getByRole("option", { name: "Random" }));
		expect(
			screen.getByText(
				"Random ignores fixture positions and the Projection stage.",
			),
		).toBeTruthy();

		const frozen = dynamic(
			{ type: "frozen_targets", targets: ["fixture-1"] },
			"dynamic-2",
		);
		view.rerender(
			<DynamicProjectionView
				dynamic={frozen}
				busy={false}
				loadPreview={async (draft) =>
					preview(frozen.body.target_binding, draft)
				}
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

	it("keeps a conflicted draft for deliberate reapply without replaying automatically", async () => {
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

		await screen.findByText("Inherit group mapping · Group front");
		const projection = screen
			.getByRole("heading", { name: "Projection stage" })
			.closest("section");
		if (!projection) throw new Error("missing Projection section");
		fireEvent.click(
			within(projection).getByRole("radio", { name: "Override" }),
		);
		const preset = within(projection).getByRole("button", {
			name: "View preset",
		});
		expect(preset).toHaveTextContent("Top");
		fireEvent.click(preset);
		fireEvent.click(screen.getByRole("option", { name: "Front" }));
		await waitFor(() =>
			expect(
				(screen.getByRole("button", { name: "Apply" }) as HTMLButtonElement)
					.disabled,
			).toBe(false),
		);
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));

		await screen.findByText(/Authoritative values were reloaded/);
		expect(apply).toHaveBeenCalledOnce();
		expect(apply.mock.calls[0]?.[0]).toMatchObject({
			projection: {
				type: "replace",
				value: {
					preset: "front",
					view_direction: { x: 0, y: 1, z: 0 },
				},
			},
		});
		expect(
			(
				within(projection).getByRole("radio", {
					name: "Override",
				}) as HTMLInputElement
			).checked,
		).toBe(true);
		expect(
			(screen.getByRole("button", { name: "Apply" }) as HTMLButtonElement)
				.disabled,
		).toBe(false);

		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() => expect(apply).toHaveBeenCalledTimes(2));
	});
});
