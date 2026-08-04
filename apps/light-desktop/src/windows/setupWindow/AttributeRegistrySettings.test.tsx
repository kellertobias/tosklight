import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AttributeConfigurationSnapshot } from "../../api/client/attributeConfiguration";
import {
	AttributeRegistrySettings,
	updatePushTurnCompanion,
} from "./AttributeRegistrySettings";
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
	recommended_configuration: {
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
			{
				id: "recommended.intensity",
				label: "Recommended intensity",
				members: ["intensity"],
			},
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
			screen.getByText("intensity", { selector: "code" }),
		).toBeInTheDocument();
		expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
			"Encoder groups",
			"Attribute activation groups",
			"Custom attributes",
		]);
		fireEvent.click(screen.getByRole("button", { name: "4 encoders" }));
		const fourEncoderPreview = screen.getByLabelText(
			"4-encoder layout preview",
		);
		expect(
			fourEncoderPreview.querySelectorAll(".attribute-layout-slot"),
		).toHaveLength(4);
		fireEvent.click(screen.getByRole("tab", { name: "Custom attributes" }));
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
					push_turn_of: null,
				},
			],
			activation_groups: [
				...snapshot.configuration.activation_groups,
				{ id, label: "House Light", members: [id] },
			],
		});
	});

	it("restores the server-projected recommended activation defaults", () => {
		const editAttributeConfiguration = vi.fn();
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

		fireEvent.click(
			screen.getByRole("tab", { name: "Attribute activation groups" }),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Restore recommended defaults" }),
		);

		expect(editAttributeConfiguration).toHaveBeenCalledWith({
			...snapshot.configuration,
			activation_groups: snapshot.recommended_configuration.activation_groups,
		});
	});

	it("reorders semantic controls from the visual encoder pages", () => {
		const editAttributeConfiguration = vi.fn();
		const second = {
			...snapshot.descriptors[0],
			id: "intensity.fade",
			label: "Intensity Fade",
			encoder_slot: 2,
			activation_group_id: "intensity.fade",
		};
		const configured = {
			...snapshot,
			configuration: {
				...snapshot.configuration,
				placements: [
					...snapshot.configuration.placements,
					{
						attribute: second.id,
						encoder_group: "intensity" as const,
						encoder_page: 1,
						encoder_slot: 2,
					},
				],
			},
			descriptors: [...snapshot.descriptors, second],
		};
		render(
			<AttributeRegistrySettings
				controller={
					{
						attributeConfiguration: configured,
						attributeConfigurationError: null,
						editAttributeConfiguration,
					} as unknown as SetupWindowController
				}
			/>,
		);

		expect(screen.getAllByText("Built-in")).toHaveLength(2);
		fireEvent.click(
			screen.getByRole("button", { name: "Move Intensity Fade earlier" }),
		);

		expect(editAttributeConfiguration).toHaveBeenCalledWith({
			...configured.configuration,
			placements: [
				{
					attribute: "intensity.fade",
					encoder_group: "intensity",
					encoder_page: 1,
					encoder_slot: 1,
				},
				{
					attribute: "intensity",
					encoder_group: "intensity",
					encoder_page: 1,
					encoder_slot: 2,
				},
			],
		});
	});

	it("edits a compound push-turn relationship from the visual encoder page", () => {
		const configuration = {
			...snapshot.configuration,
			placements: [
				{
					attribute: "prism.1",
					encoder_group: "beam" as const,
					encoder_page: 1,
					encoder_slot: 1,
				},
				{
					attribute: "prism.1.rotation",
					encoder_group: "beam" as const,
					encoder_page: 1,
					encoder_slot: 2,
				},
			],
		};
		const descriptors = [
			{
				...snapshot.descriptors[0],
				id: "prism.1",
				label: "Prism 1",
				encoder_group: "beam" as const,
				encoder_slot: 1,
			},
			{
				...snapshot.descriptors[0],
				id: "prism.1.rotation",
				label: "Prism 1 Rotation",
				encoder_group: "beam" as const,
				encoder_slot: 2,
			},
		];
		const editAttributeConfiguration = vi.fn();
		render(
			<AttributeRegistrySettings
				controller={
					{
						attributeConfiguration: {
							...snapshot,
							configuration,
							descriptors,
						},
						attributeConfigurationError: null,
						editAttributeConfiguration,
					} as unknown as SetupWindowController
				}
			/>,
		);

		fireEvent.click(
			screen.getByRole("button", {
				name: "Prism 1 push-turn companion",
			}),
		);
		fireEvent.click(screen.getByRole("option", { name: "Prism 1 Rotation" }));

		expect(editAttributeConfiguration).toHaveBeenCalledWith(
			updatePushTurnCompanion(configuration, "prism.1", "prism.1.rotation"),
		);
	});
});
