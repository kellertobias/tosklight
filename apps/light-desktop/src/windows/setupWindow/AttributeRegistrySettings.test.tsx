import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AttributeConfigurationSnapshot } from "../../api/client/attributeConfiguration";
import {
	FixtureLibraryProvider,
	type FixtureLibraryState,
} from "../../features/fixtureLibrary/FixtureLibraryContext";
import {
	AttributeRegistrySettings,
	updatePushTurnCompanion,
} from "./AttributeRegistrySettings";
import type { SetupWindowController } from "./controller";
import { type AttributeSettingsTab, SetupHeader } from "./SetupChrome";

afterEach(cleanup);

/** The tabs live in the Desk Setup window title, so tests drive them from there. */
function AttributeSettings({
	controller,
}: {
	controller: Partial<SetupWindowController>;
}) {
	const [attributeTab, setAttributeTab] =
		useState<AttributeSettingsTab>("encoder-groups");
	const merged = {
		section: "preferences-attributes",
		...controller,
		attributeTab,
		setAttributeTab,
	} as unknown as SetupWindowController;
	return (
		<>
			<SetupHeader controller={merged} />
			<AttributeRegistrySettings controller={merged} />
		</>
	);
}

function titleTabLabels() {
	return screen
		.getAllByRole("button")
		.map((button) => button.textContent)
		.filter((label): label is string =>
			ATTRIBUTE_TAB_LABELS.includes(label ?? ""),
		);
}

const ATTRIBUTE_TAB_LABELS = [
	"Encoder groups",
	"Attribute activation groups",
	"Attributes",
];

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
			<AttributeSettings
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
		expect(titleTabLabels()).toEqual(ATTRIBUTE_TAB_LABELS);
		// The width now follows the configured encoder placement instead of a preview toggle.
		expect(screen.queryByRole("button", { name: "4 encoders" })).toBeNull();
		const layout = screen.getByLabelText("6-encoder layout editor");
		expect(
			layout.querySelectorAll(".attribute-layout-slot.is-unassigned").length,
		).toBeGreaterThan(0);
		fireEvent.click(screen.getByRole("button", { name: "Attributes" }));
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

	it("assigns an unplaced attribute from an unassigned slot", () => {
		const editAttributeConfiguration = vi.fn();
		const spare = {
			...snapshot.descriptors[0],
			id: "haze",
			label: "Haze",
			encoder_group: "control" as const,
			activation_group_id: "haze",
		};
		render(
			<AttributeSettings
				controller={
					{
						attributeConfiguration: {
							...snapshot,
							descriptors: [...snapshot.descriptors, spare],
						},
						attributeConfigurationError: null,
						editAttributeConfiguration,
					} as unknown as SetupWindowController
				}
			/>,
		);

		fireEvent.click(
			screen.getByRole("button", { name: "Assign control page 1 encoder 2" }),
		);
		fireEvent.click(screen.getByRole("option", { name: "Haze" }));
		expect(editAttributeConfiguration).toHaveBeenCalledWith(
			expect.objectContaining({
				placements: expect.arrayContaining([
					expect.objectContaining({
						attribute: "haze",
						encoder_group: "control",
						encoder_page: 1,
						encoder_slot: 1,
					}),
				]),
			}),
		);
	});

	it("moves a dragged encoder onto the dropped slot", () => {
		const editAttributeConfiguration = vi.fn();
		const second = {
			...snapshot.descriptors[0],
			id: "intensity.fade",
			label: "Intensity Fade",
			encoder_slot: 2,
			activation_group_id: "intensity.fade",
		};
		render(
			<AttributeSettings
				controller={
					{
						attributeConfiguration: {
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
						},
						attributeConfigurationError: null,
						editAttributeConfiguration,
					} as unknown as SetupWindowController
				}
			/>,
		);

		const source = screen.getByLabelText("intensity page 1 encoder 2");
		const target = screen.getByLabelText("intensity page 1 encoder 1");
		fireEvent.dragStart(source);
		fireEvent.drop(target);

		expect(editAttributeConfiguration).toHaveBeenCalledWith(
			expect.objectContaining({
				placements: expect.arrayContaining([
					expect.objectContaining({
						attribute: "intensity.fade",
						encoder_page: 1,
						encoder_slot: 1,
					}),
					expect.objectContaining({
						attribute: "intensity",
						encoder_page: 1,
						encoder_slot: 2,
					}),
				]),
			}),
		);
	});

	it("restores the server-projected recommended activation defaults", () => {
		const editAttributeConfiguration = vi.fn();
		render(
			<AttributeSettings
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
			screen.getByRole("button", { name: "Attribute activation groups" }),
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
			<AttributeSettings
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
			<AttributeSettings
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

	it("lists and retargets remembered fixture-source mappings under Attributes", async () => {
		const rememberFixtureSourceMapping = vi.fn(async (input) =>
			input.targetAttribute
				? {
						source_format: input.sourceFormat,
						source_attribute: input.sourceAttribute,
						target_attribute: input.targetAttribute,
					}
				: null,
		);
		const library = {
			fixtureSourceMappings: vi.fn(async () => [
				{
					source_format: "gdtf",
					source_attribute: "Dimmer",
					target_attribute: "intensity",
				},
			]),
			rememberFixtureSourceMapping,
		} as unknown as FixtureLibraryState;
		render(
			<FixtureLibraryProvider library={library}>
				<AttributeSettings
					controller={
						{
							attributeConfiguration: snapshot,
							attributeConfigurationError: null,
							editAttributeConfiguration: vi.fn(),
						} as unknown as SetupWindowController
					}
				/>
			</FixtureLibraryProvider>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Attributes" }));
		expect(await screen.findByText("GDTF:Dimmer")).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Forget mapping" }));

		expect(rememberFixtureSourceMapping).toHaveBeenCalledWith({
			sourceFormat: "gdtf",
			sourceAttribute: "Dimmer",
			targetAttribute: null,
		});
	});
});
