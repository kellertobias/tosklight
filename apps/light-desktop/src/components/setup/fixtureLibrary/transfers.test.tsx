import { act, renderHook } from "@testing-library/react";
import JSZip from "jszip";
import type { PropsWithChildren } from "react";
import { describe, expect, it, vi } from "vitest";
import {
	FixtureLibraryProvider,
	type FixtureLibraryState,
} from "../../../features/fixtureLibrary/FixtureLibraryContext";
import { useFixtureLibraryTransfers } from "./transfers";

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
