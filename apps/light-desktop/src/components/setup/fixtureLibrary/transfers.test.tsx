import { act, render, renderHook, screen } from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import JSZip from "jszip";
import type { PropsWithChildren } from "react";
import { describe, expect, it, vi } from "vitest";
import type {
	AttributeConfigurationApiClient,
	AttributeConfigurationSnapshot,
} from "../../../api/client/attributeConfiguration";
import { AttributeConfigurationActionsProvider } from "../../../features/attributeConfiguration/AttributeConfigurationActions";
import {
	FixtureLibraryProvider,
	type FixtureLibraryState,
} from "../../../features/fixtureLibrary/FixtureLibraryContext";
import { FixtureImportDialogs, useFixtureLibraryTransfers } from "./transfers";

vi.mock("../../../features/deskSnapshot/DeskSnapshotState", () => ({
	useAttributeRegistry: () => [
		{
			id: "gobo.1",
			label: "Gobo 1",
			value_type: "indexed",
			retired: false,
		},
	],
}));

function fixtureLibrary(
	importFixturePackage: FixtureLibraryState["importFixturePackage"],
	overrides: Partial<FixtureLibraryState> = {},
): FixtureLibraryState {
	return {
		fixtureLibrary: [],
		fixtureProfiles: [],
		fixtureProfileWarnings: [],
		patchLayers: [],
		unresolvedMvrFixtures: [],
		savePatchLayer: vi.fn(),
		saveFixtureProfile: vi.fn(),
		deleteFixtureProfile: vi.fn(),
		fixtureProfileRevisions: vi.fn(),
		saveFixtureProfileSourceGdtf: vi.fn(),
		importFixturePackage,
		exportFixturePackage: vi.fn(),
		...overrides,
	};
}

async function gdtfFile(attribute: string) {
	const zip = new JSZip();
	zip.file(
		"description.xml",
		`<GDTF><FixtureType Manufacturer="Acme" Name="Mapped"><DMXModes><DMXMode Name="Standard"><DMXChannels><DMXChannel Offset="1" Geometry="Main"><LogicalChannel Attribute="${attribute}"><ChannelFunction><ChannelSet Name="Open" DMXFrom="0/1" DMXTo="255/1"/></ChannelFunction></LogicalChannel></DMXChannel></DMXChannels></DMXMode></DMXModes></FixtureType></GDTF>`,
	);
	const archive = await zip.generateAsync({ type: "uint8array" });
	return new File(
		[
			archive.buffer.slice(
				archive.byteOffset,
				archive.byteOffset + archive.byteLength,
			) as ArrayBuffer,
		],
		"mapped.gdtf",
	);
}

describe("useFixtureLibraryTransfers", () => {
	it("renders the complete imported custom-attribute authoring form", () => {
		render(
			<FixtureImportDialogs
				busy={false}
				error={null}
				modal="package"
				close={vi.fn()}
				confirmGdtfMappings={vi.fn()}
				confirmPackageMappings={vi.fn()}
				importGdtfFile={vi.fn()}
				importPackage={vi.fn()}
				mappingCandidates={[]}
				mappings={{}}
				requirements={[
					{ attribute: "vendor.test.feature", value_type: "indexed" },
				]}
				setMapping={vi.fn()}
				activationGroupOptions={[
					{ value: "activation.beam", label: "Beam mode" },
				]}
				beginCustomAttribute={vi.fn()}
				cancelCustomAttribute={vi.fn()}
				createCustomAttribute={vi.fn()}
				customAttributeDraft={{
					sourceAttribute: "vendor.test.feature",
					label: "Vendor Feature",
					valueType: "indexed",
					encoderGroup: "beam",
					encoderPage: 2,
					encoderSlot: 3,
					activationGroupId: "activation.beam",
					displayUnit: "mode",
					physicalUnit: "vendor-mode",
				}}
				editCustomAttribute={vi.fn()}
				placementOptions={[{ value: "2:3", label: "Page 2, encoder 3" }]}
			/>,
			{ wrapper: ModalProvider },
		);

		expect(screen.getByLabelText("Display label")).toHaveValue(
			"Vendor Feature",
		);
		expect(screen.getByLabelText("Attribute type")).toHaveTextContent(
			"indexed",
		);
		expect(screen.getByLabelText("Encoder group")).toHaveTextContent("Beam");
		expect(screen.getByLabelText("Semantic placement")).toHaveTextContent(
			"Page 2, encoder 3",
		);
		expect(screen.getByLabelText("Activation group")).toHaveTextContent(
			"Beam mode",
		);
		expect(screen.getByLabelText("Display unit")).toHaveValue("mode");
		expect(screen.getByLabelText("Physical unit")).toHaveValue("vendor-mode");
		expect(
			screen.getByRole("button", { name: "Create and use attribute" }),
		).toBeEnabled();
	});

	it("creates, places, and selects a compatible custom attribute inside import", async () => {
		const requirement = {
			attribute: "vendor.test.feature",
			value_type: "indexed" as const,
		};
		const importFixturePackage = vi.fn().mockResolvedValueOnce({
			type: "import_required",
			unknown_attributes: [requirement],
		});
		const snapshot = attributeSnapshot();
		const update = vi.fn(async (_showId, _snapshot, patch) => {
			const custom = patch.custom_attributes.at(-1);
			const placement = patch.placements.at(-1);
			return {
				snapshot: {
					...snapshot,
					configuration: {
						...snapshot.configuration,
						custom_attributes: patch.custom_attributes,
						placements: patch.placements,
						activation_groups: patch.activation_groups,
					},
					descriptors: [
						{
							id: custom.id,
							label: custom.label,
							encoder_group: placement.encoder_group,
							encoder_page: placement.encoder_page,
							encoder_slot: placement.encoder_slot,
							value_type: custom.value_type,
							display_unit: custom.display_unit,
							physical_unit: custom.physical_unit,
							normalized_min: null,
							normalized_max: null,
							domain_min: null,
							domain_max: null,
							cyclic: false,
							recordable: true,
							built_in: false,
							retired: false,
							activation_group_id: custom.id,
							push_turn_of: null,
						},
					],
				},
			};
		});
		const attributeClient = {
			snapshot: vi.fn(async () => snapshot),
			update,
		} as unknown as AttributeConfigurationApiClient;
		const library = fixtureLibrary(importFixturePackage);
		const wrapper = ({ children }: PropsWithChildren) => (
			<AttributeConfigurationActionsProvider
				client={attributeClient}
				showId="show-1"
				canWrite
				onApplied={vi.fn(async () => undefined)}
			>
				<FixtureLibraryProvider library={library}>
					{children}
				</FixtureLibraryProvider>
			</AttributeConfigurationActionsProvider>
		);
		const { result } = renderHook(
			() =>
				useFixtureLibraryTransfers({
					selectedMode: null,
					setSelectedFamilyKey: vi.fn(),
					setSelectedModeKey: vi.fn(),
				}),
			{ wrapper },
		);

		act(() => result.current.setModal("package"));
		await act(() =>
			result.current.importPackage(
				new File([new Uint8Array([1, 2, 3])], "unknown.toskfixture"),
			),
		);
		await act(() => result.current.beginCustomAttribute(requirement));
		act(() =>
			result.current.editCustomAttribute({
				label: "Vendor Feature",
				encoderGroup: "control",
				displayUnit: "mode",
				physicalUnit: "vendor-mode",
			}),
		);
		await act(() => result.current.createCustomAttribute());

		const patch = update.mock.calls[0]?.[2];
		const custom = patch.custom_attributes.at(-1);
		expect(custom).toMatchObject({
			label: "Vendor Feature",
			value_type: "indexed",
			display_unit: "mode",
			physical_unit: "vendor-mode",
			recordable: true,
		});
		expect(patch.placements.at(-1)).toMatchObject({
			attribute: custom.id,
			encoder_group: "control",
			encoder_page: 1,
			encoder_slot: 1,
		});
		expect(patch.activation_groups.at(-1)).toEqual({
			id: custom.id,
			label: "Vendor Feature",
			members: [custom.id],
		});
		expect(result.current.mappings[requirement.attribute]).toBe(custom.id);

		await act(() => result.current.confirmPackageMappings());
		expect(importFixturePackage).toHaveBeenLastCalledWith(
			expect.any(Uint8Array),
			[
				{
					source_attribute: requirement.attribute,
					target_attribute: custom.id,
				},
			],
		);
	});

	it("keeps a package import open while the operator resolves unknown attributes", async () => {
		const requirement = {
			attribute: "vendor.test.feature",
			value_type: "indexed" as const,
		};
		const importFixturePackage = vi.fn().mockResolvedValue({
			type: "import_required",
			unknown_attributes: [requirement],
		});
		const library = fixtureLibrary(importFixturePackage);
		const wrapper = ({ children }: PropsWithChildren) => (
			<FixtureLibraryProvider library={library}>
				{children}
			</FixtureLibraryProvider>
		);
		const { result } = renderHook(
			() =>
				useFixtureLibraryTransfers({
					selectedMode: null,
					setSelectedFamilyKey: vi.fn(),
					setSelectedModeKey: vi.fn(),
				}),
			{ wrapper },
		);

		act(() => result.current.setModal("package"));
		await act(() =>
			result.current.importPackage(
				new File([new Uint8Array([1, 2, 3])], "unknown.toskfixture"),
			),
		);

		expect(result.current.modal).toBe("package");
		expect(result.current.error).toBeNull();
		expect(result.current.busy).toBe(false);
		expect(result.current.requirements).toEqual([requirement]);

		act(() => result.current.setMapping(requirement.attribute, "shutter"));
		await act(() => result.current.confirmPackageMappings());

		expect(importFixturePackage).toHaveBeenLastCalledWith(
			expect.any(Uint8Array),
			[
				{
					source_attribute: "vendor.test.feature",
					target_attribute: "shutter",
				},
			],
		);
	});

	it("maps an unknown GDTF source identity and remembers the explicit target", async () => {
		const saveFixtureProfile = vi.fn(async (profile) => ({
			...profile,
			revision: 1,
		}));
		const rememberFixtureSourceMapping = vi.fn(async () => null);
		const library = fixtureLibrary(vi.fn(), {
			saveFixtureProfile,
			saveFixtureProfileSourceGdtf: vi.fn(async () => true),
			fixtureSourceMappings: vi.fn(async () => []),
			rememberFixtureSourceMapping,
		});
		const wrapper = ({ children }: PropsWithChildren) => (
			<FixtureLibraryProvider library={library}>
				{children}
			</FixtureLibraryProvider>
		);
		const { result } = renderHook(
			() =>
				useFixtureLibraryTransfers({
					selectedMode: null,
					setSelectedFamilyKey: vi.fn(),
					setSelectedModeKey: vi.fn(),
				}),
			{ wrapper },
		);

		act(() => result.current.setModal("gdtf"));
		const file = await gdtfFile("Gobo");
		await act(() => result.current.importGdtfFile(file));
		expect(result.current.requirements).toEqual([
			{ attribute: "GDTF:Gobo", value_type: "indexed" },
		]);

		act(() => result.current.setMapping("GDTF:Gobo", "gobo.1"));
		await act(() => result.current.confirmGdtfMappings());

		expect(rememberFixtureSourceMapping).toHaveBeenCalledWith({
			sourceFormat: "gdtf",
			sourceAttribute: "Gobo",
			targetAttribute: "gobo.1",
		});
		const saved = saveFixtureProfile.mock.calls[0]?.[0];
		expect(saved.modes[0].channels[0]).toMatchObject({
			fixture_attribute: "GDTF:Gobo",
			attribute: "gobo.1",
			functions: [expect.objectContaining({ attribute: "gobo.1" })],
		});
	});

	it("reuses a compatible remembered GDTF mapping without another prompt", async () => {
		const saveFixtureProfile = vi.fn(async (profile) => ({
			...profile,
			revision: 1,
		}));
		const library = fixtureLibrary(vi.fn(), {
			saveFixtureProfile,
			saveFixtureProfileSourceGdtf: vi.fn(async () => true),
			fixtureSourceMappings: vi.fn(async () => [
				{
					source_format: "gdtf",
					source_attribute: "Gobo",
					target_attribute: "gobo.1",
				},
			]),
		});
		const wrapper = ({ children }: PropsWithChildren) => (
			<FixtureLibraryProvider library={library}>
				{children}
			</FixtureLibraryProvider>
		);
		const { result } = renderHook(
			() =>
				useFixtureLibraryTransfers({
					selectedMode: null,
					setSelectedFamilyKey: vi.fn(),
					setSelectedModeKey: vi.fn(),
				}),
			{ wrapper },
		);

		act(() => result.current.setModal("gdtf"));
		const file = await gdtfFile("Gobo");
		await act(() => result.current.importGdtfFile(file));

		expect(result.current.requirements).toEqual([]);
		expect(
			saveFixtureProfile.mock.calls[0]?.[0].modes[0].channels[0],
		).toMatchObject({
			fixture_attribute: "GDTF:Gobo",
			attribute: "gobo.1",
		});
	});
});

function attributeSnapshot(): AttributeConfigurationSnapshot {
	const configuration = {
		version: 1,
		custom_attributes: [],
		placements: [],
		activation_groups: [],
	};
	return {
		show_id: "show-1",
		show_revision: 1,
		object_revision: 1,
		configuration,
		recommended_configuration: configuration,
		descriptors: [],
		validation_error: null,
	};
}
