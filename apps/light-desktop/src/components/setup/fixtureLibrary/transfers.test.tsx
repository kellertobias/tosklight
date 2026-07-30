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
	it("keeps a paused package import open with its actionable server error", async () => {
		const reason = new Error(
			"Fixture import paused: map or create canonical descriptors for vendor.test.feature in Show > Desk Setup > Programmer > Attributes, then retry the import",
		);
		const library = fixtureLibrary(vi.fn().mockRejectedValue(reason));
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
		expect(result.current.error).toBe(reason.message);
		expect(result.current.busy).toBe(false);
	});
});
