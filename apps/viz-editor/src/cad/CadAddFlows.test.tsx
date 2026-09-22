import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CadAddFlows, type CadAddRequest } from "./CadAddFlows";
import { CadPartMenu } from "./CadPartMenu";
import { CadToolContext, type CadTools } from "./cadTools";

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

const THREE_POINT = "44097b39-11b4-5bd4-af61-8adb97d426b1";
const CORNER = "3ea0f8ad-c38d-5ec6-a4f7-6d918a1e974e";
const STRAIGHT = "562e7947-8284-5ec8-9750-3cd3fe6c1c6d";
const CROWD = "a0e75c30-92e5-4c20-bcd1-9a51ddbc6257";
const RAILING = "9fc82162-c31c-4a34-bb2c-01fcc2254e37";
const PAR = "par-profile";

/** The smallest profile the patch sheet can turn into a definition. */
function profile(
	id: string,
	name: string,
	{ revision = 1, manufacturer = "Venue", fixture_type = "rigging", patch_policy = "visual_only" } = {},
) {
	return {
		id,
		revision,
		manufacturer,
		name,
		short_name: name,
		fixture_type,
		patch_policy,
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

const localStore = new Map<string, string>();

beforeEach(() => {
	localStore.clear();
	vi.stubGlobal("localStorage", {
		getItem: (key: string) => localStore.get(key) ?? null,
		setItem: (key: string, value: string) => localStore.set(key, value),
		removeItem: (key: string) => localStore.delete(key),
	});
	mocks.fixtureProfiles.mockReset().mockResolvedValue([
		profile(THREE_POINT, "Three-Point Truss"),
		profile(STRAIGHT, "Four-Point Truss", { revision: 3 }),
		profile(CORNER, "Four-Point Truss Corner 2-Way"),
		profile(CROWD, "Crowd Area", { fixture_type: "venue" }),
		profile(RAILING, "Stage Railing 2 m", { fixture_type: "venue" }),
		profile(PAR, "LED Par", { manufacturer: "Generic", fixture_type: "par", patch_policy: "dmx" }),
	]);
	mocks.patchSnapshot.mockReset().mockResolvedValue({
		showId: "show",
		patchRevision: 12,
		fixtures: [{ virtualFixtureNumber: 1 }, { virtualFixtureNumber: 2 }, { virtualFixtureNumber: null }],
	});
	mocks.patchFixtures.mockReset().mockResolvedValue({});
});

function renderFlows() {
	const announcePlaced = vi.fn();
	const onError = vi.fn();
	const tools = { announcePlaced } as unknown as CadTools;
	const view = (add: CadAddRequest) => (
		<ModalProvider>
			<CadToolContext.Provider value={tools}>
				<CadAddFlows add={add} onError={onError} />
			</CadToolContext.Provider>
		</ModalProvider>
	);
	const { rerender } = render(view({ kind: "venue", request: 0 }));
	let request = 0;
	const press = (kind: CadAddRequest["kind"], profileId?: string) =>
		rerender(view({ kind, profileId, request: ++request }));
	return { announcePlaced, onError, press };
}

const placedFixture = (call = 0) => mocks.patchFixtures.mock.calls[call][2].fixtures[0];

describe("the CAD add buttons", () => {
	it("places the button's default part at once, then the part its caret menu chose", async () => {
		const { announcePlaced, onError, press } = renderFlows();
		press("truss");
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledTimes(1));
		expect(placedFixture(0)).toMatchObject({ profileId: THREE_POINT, virtualFixtureNumber: 3 });
		expect(announcePlaced).toHaveBeenCalledWith(placedFixture(0).fixtureId);
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

		// A part chosen from the menu is placed and becomes what a plain press places.
		press("truss", CORNER);
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledTimes(2));
		expect(placedFixture(1)).toMatchObject({ profileId: CORNER, profileRevision: 1 });
		press("truss");
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledTimes(3));
		expect(placedFixture(2).profileId).toBe(CORNER);
		expect(onError).not.toHaveBeenCalled();
	});

	it("says why the show refused a part, and names a part the library lacks", async () => {
		mocks.patchFixtures.mockRejectedValueOnce(new Error("layer default is locked"));
		const { announcePlaced, onError, press } = renderFlows();
		press("truss", STRAIGHT);
		await waitFor(() =>
			expect(onError).toHaveBeenCalledWith(expect.stringContaining("layer default is locked")),
		);
		press("stage");
		await waitFor(() =>
			expect(onError).toHaveBeenLastCalledWith(
				"The 2 × 1 m (Scissor feet) is not in this computer's fixture library.",
			),
		);
		expect(announcePlaced).not.toHaveBeenCalled();
	});

	it("lists the Venue profiles no add button offers, narrows them by search and places the chosen one", async () => {
		const { announcePlaced, press } = renderFlows();
		press("venue");
		const dialog = await screen.findByRole("dialog", { name: "Add venue element" });
		const list = await within(dialog).findByRole("list", { name: "Venue elements" });
		await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(2));
		const names = within(list)
			.getAllByRole("listitem")
			.map((item) => item.querySelector("strong")?.textContent);
		// The trusses, decks, curtains and primitives are placed from their own buttons, not here.
		expect(names).toEqual(["Crowd Area", "Stage Railing 2 m"]);
		expect(within(list).getAllByRole("listitem")[0].querySelector("img")).toHaveAttribute(
			"src",
			`data:image/png;base64,${CROWD.slice(0, 4)}`,
		);

		fireEvent.change(screen.getByLabelText("Search venue elements"), {
			target: { value: "crowd" },
		});
		await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(1));
		fireEvent.click(within(list).getByRole("button", { name: /Crowd Area/u }));
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledTimes(1));
		expect(placedFixture().profileId).toBe(CROWD);
		await waitFor(() =>
			expect(screen.queryByRole("dialog", { name: "Add venue element" })).not.toBeInTheDocument(),
		);
	});
});

describe("a part button's caret menu", () => {
	it("lists the parts with pictures, checks the current one and disables parts the library lacks", async () => {
		localStore.set("tosklight:viz-editor:cad-add-part:truss:v1", CORNER);
		const onChoose = vi.fn();
		render(<CadPartMenu kind="truss" onChoose={onChoose} />);
		const fourPoint = await screen.findByRole("group", { name: "4-point" });
		const corner = within(fourPoint).getByRole("menuitemradio", { name: /Corner 2-way/u });
		await waitFor(() => expect(corner).toBeEnabled());
		expect(corner).toHaveAttribute("aria-checked", "true");
		expect(corner.querySelector("img")).toHaveAttribute("src", `data:image/png;base64,${CORNER.slice(0, 4)}`);
		expect(within(fourPoint).getByRole("menuitemradio", { name: /Node 6-way/u })).toBeDisabled();
		// A section with only its straight truss is one entry, not a heading.
		expect(screen.getByRole("menuitemradio", { name: /^Pipe/u })).toBeDisabled();

		fireEvent.click(within(fourPoint).getByRole("menuitemradio", { name: /Straight truss/u }));
		expect(onChoose).toHaveBeenCalledWith(STRAIGHT);
	});
});
