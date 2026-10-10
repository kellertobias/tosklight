import {
	act,
	cleanup,
	fireEvent,
	render,
	renderHook,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import type { ComponentProps, PropsWithChildren } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	AttributeConfigurationApiClient,
	AttributeConfigurationSnapshot,
} from "../../../api/client/attributeConfiguration";
import { AttributeConfigurationActionsProvider } from "../../../features/attributeConfiguration/AttributeConfigurationActions";
import {
	FixtureLibraryProvider,
	type FixtureLibraryState,
} from "../../../features/fixtureLibrary/FixtureLibraryContext";
import { blankFixtureProfile } from "../fixtureProfileModel";
import { FixtureImportDialogs, useFixtureLibraryTransfers } from "./transfers";

const picker = vi.hoisted(() => ({ open: vi.fn(), content: vi.fn() }));
vi.mock("../../../windows/FileManagerPickerHost", () => ({
	openFileManagerPicker: picker.open,
}));
vi.mock("../../../features/files/FilesContext", () => ({
	useFiles: () => ({ fileContent: picker.content }),
}));

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

function gdtfPreview() {
	const profile = blankFixtureProfile();
	profile.manufacturer = "Acme";
	profile.name = "Mapped";
	return {
		profile,
		diagnostics: [
			{ node: "Optics", message: "Optical calibration remains unknown." },
		],
		unknown_attributes: [
			{ attribute: "gdtf.Gobo", value_type: "indexed" as const },
		],
	};
}

async function gdtfFile(_attribute: string) {
	return new File([new Uint8Array([80, 75, 3, 4])], "mapped.gdtf");
}

afterEach(cleanup);

describe("useFixtureLibraryTransfers", () => {
	it("dismisses the mapping popup before outer GDTF cancellation and preserves its draft", () => {
		const close = vi.fn();
		render(
			<FixtureImportDialogs
				busy={false}
				error={null}
				modal="gdtf"
				pendingGdtf={{
					...gdtfPreview(),
					source: new Uint8Array([80, 75]),
					expectedRevision: 0,
				}}
				close={close}
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

		const trigger = screen.getByLabelText("Map vendor.test.feature");
		fireEvent.click(trigger);
		expect(screen.getByRole("listbox")).toBeInTheDocument();
		fireEvent.keyDown(trigger, { key: "Escape" });
		expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
		expect(close).not.toHaveBeenCalled();
		expect(screen.getByLabelText("Display label")).toHaveValue(
			"Vendor Feature",
		);
		fireEvent.keyDown(trigger, { key: "Escape" });
		expect(close).toHaveBeenCalledOnce();
		fireEvent.click(screen.getByRole("button", { name: "Close Import GDTF" }));
		expect(close).toHaveBeenCalledTimes(2);
	});

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
		const preview = gdtfPreview();
		const importFixtureGdtf = vi.fn(async () => ({
			...preview.profile,
			revision: 1,
		}));
		const rememberFixtureSourceMapping = vi.fn(async () => null);
		const library = fixtureLibrary(vi.fn(), {
			previewFixtureGdtf: vi.fn(async () => preview),
			importFixtureGdtf,
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
			{ attribute: "gdtf.Gobo", value_type: "indexed" },
		]);

		act(() => result.current.setMapping("gdtf.Gobo", "gobo.1"));
		await act(() => result.current.confirmGdtfMappings());

		expect(rememberFixtureSourceMapping).toHaveBeenCalledWith({
			sourceFormat: "gdtf",
			sourceAttribute: "Gobo",
			targetAttribute: "gobo.1",
		});
		expect(importFixtureGdtf).toHaveBeenCalledWith(
			expect.objectContaining({
				profileId: preview.profile.id,
				expectedRevision: 0,
				attributeMappings: [
					{ source_attribute: "gdtf.Gobo", target_attribute: "gobo.1" },
				],
			}),
		);
		expect(library.saveFixtureProfile).not.toHaveBeenCalled();
		expect(library.saveFixtureProfileSourceGdtf).not.toHaveBeenCalled();
	});

	it("prefills a compatible remembered mapping while still presenting import limitations", async () => {
		const preview = gdtfPreview();
		const importFixtureGdtf = vi.fn(async () => ({
			...preview.profile,
			revision: 1,
		}));
		const library = fixtureLibrary(vi.fn(), {
			previewFixtureGdtf: vi.fn(async () => preview),
			importFixtureGdtf,
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

		expect(result.current.mappings).toEqual({ "gdtf.Gobo": "gobo.1" });
		expect(result.current.pendingGdtf?.diagnostics).toEqual(
			preview.diagnostics,
		);
		expect(importFixtureGdtf).not.toHaveBeenCalled();
		await act(() => result.current.confirmGdtfMappings());
		expect(importFixtureGdtf).toHaveBeenCalledTimes(1);
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

function gdtfDialogProps(
	overrides: Partial<ComponentProps<typeof FixtureImportDialogs>> = {},
): ComponentProps<typeof FixtureImportDialogs> {
	return {
		busy: false,
		error: null,
		modal: "gdtf",
		pendingGdtf: {
			...gdtfPreview(),
			source: new Uint8Array([80, 75]),
			expectedRevision: 0,
		},
		close: vi.fn(),
		confirmGdtfMappings: vi.fn(),
		confirmPackageMappings: vi.fn(),
		importGdtfFile: vi.fn(),
		importPackage: vi.fn(),
		mappingCandidates: [
			{ id: "gobo.1", label: "Gobo 1", value_type: "indexed" },
		],
		mappings: {},
		requirements: [{ attribute: "gdtf.Gobo", value_type: "indexed" }],
		setMapping: vi.fn(),
		activationGroupOptions: [],
		beginCustomAttribute: vi.fn(),
		cancelCustomAttribute: vi.fn(),
		createCustomAttribute: vi.fn(),
		customAttributeDraft: null,
		editCustomAttribute: vi.fn(),
		placementOptions: [],
		...overrides,
	};
}

describe("GDTF import decision hierarchy", () => {
	it("navigates past a remembered mapping to the actual unresolved destination control", () => {
		const props = gdtfDialogProps({
			requirements: [
				{ attribute: "gdtf.Remembered", value_type: "indexed" },
				{ attribute: "gdtf.Next", value_type: "indexed" },
			],
			mappings: { "gdtf.Remembered": "gobo.1" },
		});
		render(<FixtureImportDialogs {...props} />, { wrapper: ModalProvider });
		const target = screen.getByLabelText("Map gdtf.Next");
		const scroll = vi.fn();
		Object.defineProperty(target, "scrollIntoView", { value: scroll });
		expect(
			screen.getByText("1 unresolved of 2 required mappings"),
		).toBeVisible();
		expect(
			screen.getByRole("button", { name: "Import and remember mappings" }),
		).toBeDisabled();
		fireEvent.click(screen.getByRole("button", { name: "Next unresolved" }));
		expect(scroll).toHaveBeenCalledWith({ block: "center" });
		expect(target).toHaveFocus();
		expect(
			screen.getByText(
				"Choose a destination for 1 remaining attribute to enable import.",
			),
		).toBeVisible();
		expect(props.confirmGdtfMappings).not.toHaveBeenCalled();
	});

	it("keeps compatible remembered mappings editable and enables import without reconfirmation", () => {
		const props = gdtfDialogProps({ mappings: { "gdtf.Gobo": "gobo.1" } });
		render(<FixtureImportDialogs {...props} />, { wrapper: ModalProvider });
		expect(
			screen.getByText("0 unresolved of 1 required mappings"),
		).toBeVisible();
		expect(screen.getByText("Mapped")).toBeVisible();
		expect(screen.getByLabelText("Map gdtf.Gobo")).toBeEnabled();
		expect(
			screen.queryByRole("button", { name: "Next unresolved" }),
		).not.toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Import and remember mappings" }),
		);
		expect(props.confirmGdtfMappings).toHaveBeenCalledOnce();
	});

	it("retains every diagnostic in expandable details and a visible limitation summary", () => {
		const preview = gdtfPreview();
		preview.diagnostics.push({
			node: "Geometry",
			message: "Model retained only in the source archive.",
		});
		const props = gdtfDialogProps({
			pendingGdtf: {
				...preview,
				source: new Uint8Array(),
				expectedRevision: 2,
			},
		});
		render(<FixtureImportDialogs {...props} />, { wrapper: ModalProvider });
		expect(
			screen.getByText(
				"2 import limitations. Review the source details before importing.",
			),
		).toBeVisible();
		const summary = screen.getByText("Import limitations — show all 2");
		const details = summary.closest("details");
		expect(details).not.toHaveAttribute("open");
		expect(details).toHaveTextContent(
			"Optics: Optical calibration remains unknown.",
		);
		expect(details).toHaveTextContent(
			"Geometry: Model retained only in the source archive.",
		);
		fireEvent.click(summary);
		expect(
			within(details as HTMLElement).getByText(
				"Model retained only in the source archive.",
				{
					exact: false,
				},
			),
		).toBeVisible();
		expect(
			screen.getByText(
				"Creates a new library revision. Patched fixtures keep their current revision.",
			),
		).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
		expect(props.close).toHaveBeenCalledOnce();
	});

	it("keeps the public import guard and mapping control disabled while importing", () => {
		const props = gdtfDialogProps({
			busy: true,
			mappings: { "gdtf.Gobo": "gobo.1" },
		});
		render(<FixtureImportDialogs {...props} />, { wrapper: ModalProvider });
		expect(screen.getByRole("button", { name: "Importing…" })).toBeDisabled();
		expect(screen.getByLabelText("Map gdtf.Gobo")).toBeDisabled();
		expect(props.confirmGdtfMappings).not.toHaveBeenCalled();
	});
});

describe("fixture import busy ownership", () => {
	it("blocks close and duplicate package work while a potentially committing request is pending, then closes only on owned success", async () => {
		let finish: (
			value: Awaited<ReturnType<FixtureLibraryState["importFixturePackage"]>>,
		) => void = () => undefined;
		const request = vi.fn(
			() =>
				new Promise<
					Awaited<ReturnType<FixtureLibraryState["importFixturePackage"]>>
				>((resolve) => {
					finish = resolve;
				}),
		);
		const library = fixtureLibrary(request);
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
		const file = new File([new Uint8Array([1, 2, 3])], "owned.toskfixture");
		let running: Promise<void> = Promise.resolve();
		await act(async () => {
			running = result.current.importPackage(file);
			await Promise.resolve();
		});
		act(() => result.current.setModal(null));
		expect(result.current.modal).toBe("package");
		expect(result.current.busy).toBe(true);
		await act(() => result.current.importPackage(file));
		expect(request).toHaveBeenCalledOnce();
		await act(async () => {
			finish({ type: "profile", profile: blankFixtureProfile() });
			await running;
		});
		expect(result.current.modal).toBeNull();
		expect(result.current.busy).toBe(false);
	});
	it("guards every GDTF dialog close route while reading and importing", () => {
		const props = gdtfDialogProps({ busy: true });
		render(<FixtureImportDialogs {...props} />, { wrapper: ModalProvider });
		expect(
			screen.getByRole("status", { name: "Importing fixture…" }),
		).toBeVisible();
		expect(
			screen.getByRole("button", { name: "Close Import GDTF", hidden: true }),
		).toBeDisabled();
		expect(
			screen.getByRole("button", { name: "Cancel", hidden: true }),
		).toBeDisabled();
		fireEvent.keyDown(window, { key: "Escape" });
		expect(props.close).not.toHaveBeenCalled();
	});
	it("shows the GDTF selected-file read overlay before import starts and blocks closing until a failed read settles", async () => {
		let fail: (reason: Error) => void = () => {};
		picker.open.mockResolvedValue([
			{ rootId: "fixtures", entry: { name: "tour.gdtf", path: "tour.gdtf" } },
		]);
		picker.content.mockImplementation(
			() =>
				new Promise((_resolve, reject) => {
					fail = reject;
				}),
		);
		const props = gdtfDialogProps({ pendingGdtf: null, requirements: [] });
		render(<FixtureImportDialogs {...props} />, { wrapper: ModalProvider });
		fireEvent.click(screen.getByRole("button", { name: "Choose GDTF file" }));
		await waitFor(() =>
			expect(
				screen.getByRole("status", { name: "Loading selected GDTF file…" }),
			).toBeVisible(),
		);
		expect(props.importGdtfFile).not.toHaveBeenCalled();
		expect(
			screen.getByRole("button", { name: "Close Import GDTF", hidden: true }),
		).toBeDisabled();
		fireEvent.keyDown(window, { key: "Escape" });
		expect(props.close).not.toHaveBeenCalled();
		await act(async () => {
			fail(new Error("Read failed"));
		});
		expect(screen.getByRole("alert")).toHaveTextContent("Read failed");
		expect(
			screen.queryByRole("status", { name: "Loading selected GDTF file…" }),
		).toBeNull();
		expect(
			screen.getByRole("button", { name: "Close Import GDTF" }),
		).toBeEnabled();
	});
});
