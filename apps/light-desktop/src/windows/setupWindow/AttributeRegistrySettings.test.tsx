import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AttributeConfigurationSnapshot } from "../../api/generated/light-wire";
import { AttributeRegistrySettings } from "./AttributeRegistrySettings";
import type { SetupWindowController } from "./controller";

afterEach(cleanup);

const snapshot: AttributeConfigurationSnapshot = {
	show_id: "show-29",
	show_revision: 8,
	object_revision: 3,
	configuration: {
		version: 1,
		custom_attributes: [],
		placements: [
			{
				attribute: "intensity",
				encoder_group: "intensity",
				encoder_page: 1,
				encoder_slot: 1,
			},
		],
		activation_groups: [
			{ id: "intensity", label: "Intensity", members: ["intensity"] },
		],
	},
	descriptors: [
		{
			id: "intensity",
			label: "Intensity",
			encoder_group: "intensity",
			encoder_page: 1,
			encoder_slot: 1,
			value_type: "continuous",
			display_unit: "percent",
			physical_unit: null,
			normalized_min: 0,
			normalized_max: 1,
			domain_min: 0,
			domain_max: 100,
			cyclic: false,
			recordable: true,
			built_in: true,
			retired: false,
			activation_group_id: "intensity",
		},
	],
	validation_error: null,
};

describe("Desk Setup attribute registry", () => {
	it("shows stable IDs and creates a placed single-member custom attribute", () => {
		const editAttributeConfiguration = vi.fn();
		vi.spyOn(crypto, "randomUUID").mockReturnValue(
			"00000000-0000-4000-8000-000000000029",
		);
		render(
			<AttributeRegistrySettings
				controller={
					{
						attributeConfiguration: snapshot,
						attributeConfigurationError: null,
						editAttributeConfiguration,
					} as unknown as SetupWindowController
				}
			/>,
		);

		expect(
			screen.getByText(/intensity · page 1, encoder 1/),
		).toBeInTheDocument();
		fireEvent.change(screen.getByLabelText("New custom attribute"), {
			target: { value: "House Light" },
		});
		fireEvent.click(
			screen.getByRole("button", { name: "Create custom attribute" }),
		);

		const id = "custom.house.light.00000000-0000-4000-8000-000000000029";
		expect(editAttributeConfiguration).toHaveBeenCalledWith({
			...snapshot.configuration,
			custom_attributes: [
				{
					id,
					label: "House Light",
					value_type: "continuous",
					display_unit: null,
					physical_unit: null,
					normalized_bounds: { min: 0, max: 1 },
					domain_bounds: null,
					cyclic: false,
					recordable: true,
					lifecycle: "active",
				},
			],
			placements: [
				...snapshot.configuration.placements,
				{
					attribute: id,
					encoder_group: "intensity",
					encoder_page: 1,
					encoder_slot: 2,
				},
			],
			activation_groups: [
				...snapshot.configuration.activation_groups,
				{ id, label: "House Light", members: [id] },
			],
		});
	});
});
