import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CadAddPartModal } from "./CadAddPartModal";

const mocks = vi.hoisted(() => ({
	fixtureProfiles: vi.fn(),
	patchSnapshot: vi.fn(),
	patchFixtures: vi.fn(),
}));

vi.mock("../document/session", () => ({
	documentSession: {
		fixtureProfiles: mocks.fixtureProfiles,
		patchSnapshot: mocks.patchSnapshot,
	},
}));
vi.mock("../document/transport", () => ({
	TauriPatchTransport: class {
		patchFixtures = mocks.patchFixtures;
	},
}));

const CORNER = "3ea0f8ad-c38d-5ec6-a4f7-6d918a1e974e";
const STRAIGHT = "562e7947-8284-5ec8-9750-3cd3fe6c1c6d";

/** The smallest visual-only profile the patch sheet can turn into a definition. */
function profile(id: string, name: string, revision = 1) {
	return {
		id,
		revision,
		manufacturer: "Venue",
		name,
		short_name: name,
		fixture_type: "rigging",
		physical: {},
		photograph_asset: `data:image/png;base64,${id.slice(0, 4)}`,
		modes: [
			{
				id: `${id}-mode`,
				name: "Default",
				splits: [{ number: 1, footprint: 0 }],
				heads: [],
				channels: [],
				color_systems: [],
			},
		],
	};
}

function renderModal(kind: "truss" | "curtain") {
	const onPlaced = vi.fn();
	const onError = vi.fn();
	const view = (request: number) => (
		<ModalProvider>
			<CadAddPartModal kind={kind} request={request} onPlaced={onPlaced} onError={onError} />
		</ModalProvider>
	);
	const { rerender } = render(view(0));
	rerender(view(1));
	return { onPlaced, onError };
}

beforeEach(() => {
	mocks.fixtureProfiles.mockReset().mockResolvedValue([
		profile(STRAIGHT, "Four-Point Truss", 3),
		profile(CORNER, "Four-Point Truss Corner 2-Way"),
		profile("6f34b81e-3f71-5d35-b8fb-b4b0b7cce859", "Curtain (parametric)"),
	]);
	mocks.patchSnapshot.mockReset().mockResolvedValue({
		showId: "show",
		patchRevision: 12,
		fixtures: [{ virtualFixtureNumber: 1 }, { virtualFixtureNumber: 2 }, { virtualFixtureNumber: null }],
	});
	mocks.patchFixtures.mockReset().mockResolvedValue({});
});

describe("the CAD add parts dialog", () => {
	it("reads the library when it opens and places a corner the library holds", async () => {
		const { onPlaced, onError } = renderModal("truss");
		const dialog = await screen.findByRole("dialog", { name: "Add truss" });
		expect(mocks.fixtureProfiles).toHaveBeenCalledTimes(1);

		// "4-point" and not "4-point large": the tile's name runs on into its detail line.
		const fourPoint = await within(dialog).findByRole("button", { name: /^4-point(?! large)/u });
		// The tiles wait for the library the dialog reads as it opens.
		await waitFor(() => expect(fourPoint).toBeEnabled());
		fireEvent.click(fourPoint);
		const corner = await within(dialog).findByRole("button", { name: /Corner 2-way/u });
		expect(corner).toBeEnabled();
		expect(corner).not.toHaveTextContent("Not in this library");
		// A part this library does not hold is shown but cannot be chosen.
		expect(within(dialog).getByRole("button", { name: /Node 6-way/u })).toBeDisabled();

		fireEvent.click(corner);
		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledTimes(1));
		const [showId, revision, mutation] = mocks.patchFixtures.mock.calls[0];
		expect([showId, revision]).toEqual(["show", 12]);
		expect(mutation.fixtures[0]).toMatchObject({
			profileId: CORNER,
			profileRevision: 1,
			modeId: `${CORNER}-mode`,
			virtualFixtureNumber: 3,
			fixtureNumber: null,
		});
		expect(onPlaced).toHaveBeenCalledWith(mutation.fixtures[0].fixtureId);
		expect(onError).not.toHaveBeenCalled();
		expect(screen.queryByRole("dialog", { name: "Add truss" })).not.toBeInTheDocument();
	});

	it("says why the show refused a part instead of a bare failure", async () => {
		mocks.patchFixtures.mockRejectedValue(new Error("layer default is locked"));
		const { onError } = renderModal("truss");
		const dialog = await screen.findByRole("dialog", { name: "Add truss" });
		// "4-point" and not "4-point large": the tile's name runs on into its detail line.
		const fourPoint = await within(dialog).findByRole("button", { name: /^4-point(?! large)/u });
		// The tiles wait for the library the dialog reads as it opens.
		await waitFor(() => expect(fourPoint).toBeEnabled());
		fireEvent.click(fourPoint);
		fireEvent.click(await within(dialog).findByRole("button", { name: /Straight truss/u }));
		await waitFor(() =>
			expect(onError).toHaveBeenCalledWith(expect.stringContaining("layer default is locked")),
		);
		expect(screen.getByRole("dialog", { name: "Add truss" })).toBeInTheDocument();
	});

	it("places a curtain at once from the library it has just read", async () => {
		const { onPlaced } = renderModal("curtain");
		await waitFor(() => expect(onPlaced).toHaveBeenCalledTimes(1));
		expect(mocks.patchFixtures.mock.calls[0][2].fixtures[0].name).toBe("Curtain (parametric)");
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});
});
