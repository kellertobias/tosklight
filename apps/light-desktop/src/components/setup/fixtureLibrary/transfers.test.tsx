import { act, renderHook } from "@testing-library/react";
import type { PropsWithChildren } from "react";
import { describe, expect, it, vi } from "vitest";
import {
	FixtureLibraryProvider,
	type FixtureLibraryState,
} from "../../../features/fixtureLibrary/FixtureLibraryContext";
import { useFixtureLibraryTransfers } from "./transfers";

function fixtureLibrary(
	importFixturePackage: FixtureLibraryState["importFixturePackage"],
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
	};
}

describe("useFixtureLibraryTransfers", () => {
	it("keeps a package import open while the operator resolves unknown attributes", async () => {
		const requirement = {
			attribute: "vendor.test.feature",
			value_type: "indexed" as const,
		};
		const importFixturePackage = vi
			.fn()
			.mockResolvedValue({
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
});
