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
const DRUMS = "d2a9d52d-0000-4000-8000-000000000001";
/** The generated flight rack, chosen by the rack units it holds. */
const RACK = "448af0db-7419-557e-a621-26eadcd05eed";
const PAR = "par-profile";
/** The one flight of stairs, placed with the handrails chosen for it. */
const STAIRS = "d5982d33-9723-5749-ade6-7be0e6b4adf1";
/** The 2 × 1 m stage element, which is the stage button's own default part. */
const DECK = "ae45dcb3-cd94-59db-b3b1-0e8a5adb9141";

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
		profile(DRUMS, "Drum Kit", { fixture_type: "venue" }),
		profile(RACK, "Flight Rack", { fixture_type: "venue" }),
		profile(STAIRS, "Stage Stairs", { fixture_type: "venue" }),
		profile(PAR, "LED Par", { manufacturer: "Generic", fixture_type: "par", patch_policy: "dmx" }),
		{
			...profile(DECK, "Stage Element 2 × 1 m", { fixture_type: "venue" }),
			scenery: {
				kind: "riser",
				chords: 0,
				default_size_metres: { x: 2, y: 0.6, z: 1 },
				adjustable: { width: false, height: true, depth: false },
				minimum_size_metres: { x: 2, y: 0.1, z: 1 },
				maximum_size_metres: { x: 2, y: 1.2, z: 1 },
			},
		},
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
	const startPlacing = vi.fn();
	const onError = vi.fn();
	const tools = { announcePlaced, startPlacing } as unknown as CadTools;
	const view = (add: CadAddRequest) => (
		<ModalProvider>
			<CadToolContext.Provider value={tools}>
				<CadAddFlows add={add} onError={onError} />
			</CadToolContext.Provider>
		</ModalProvider>
	);
	const { rerender } = render(view({ kind: "venue", request: 0 }));
	let request = 0;
	const press = (kind: CadAddRequest["kind"], profileId?: string, several?: boolean) =>
		rerender(view({ kind, profileId, several, request: ++request }));
	return { announcePlaced, startPlacing, onError, press };
}

const placedFixture = (call = 0) => mocks.patchFixtures.mock.calls[call][2].fixtures[0];

describe("the CAD add buttons", () => {
	it("places the button's default part at once, then the part its caret menu chose", async () => {
		const { announcePlaced, onError, press } = renderFlows();
		press("truss");
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledTimes(1));
		expect(placedFixture(0)).toMatchObject({ profileId: THREE_POINT, virtualFixtureNumber: 3 });
		expect(announcePlaced).toHaveBeenCalledWith([placedFixture(0).fixtureId]);
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

	it("lists the Venue profiles no add button offers, narrows them by search and adds the selected one", async () => {
		const { announcePlaced, press } = renderFlows();
		press("venue");
		const dialog = await screen.findByRole("dialog", { name: "Add venue element" });
		const list = await within(dialog).findByRole("list", { name: "Venue elements" });
		await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(2));
		const names = within(list)
			.getAllByRole("listitem")
			.map((item) => item.querySelector("strong")?.textContent);
		// The trusses, decks, scenic elements and primitives are placed from their own buttons, not
		// here: the railing is a scenic element now, and the backline stays in this dialog.
		expect(names).toEqual(["Crowd Area", "Drum Kit"]);
		expect(within(list).getAllByRole("listitem")[0].querySelector("img")).toHaveAttribute(
			"src",
			`data:image/png;base64,${CROWD.slice(0, 4)}`,
		);

		fireEvent.change(screen.getByLabelText("Search venue elements"), {
			target: { value: "crowd" },
		});
		await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(1));
		const tile = within(list).getByRole("button", { name: /^Crowd Area/u });
		// Choosing an element only selects it.
		fireEvent.click(tile);
		expect(tile).toHaveAttribute("aria-pressed", "true");
		await Promise.resolve();
		expect(announcePlaced).not.toHaveBeenCalled();
		expect(mocks.patchFixtures).not.toHaveBeenCalled();
		fireEvent.click(within(dialog).getByRole("button", { name: "Add Crowd Area" }));
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledTimes(1));
		expect(placedFixture().profileId).toBe(CROWD);
		await waitFor(() =>
			expect(screen.queryByRole("dialog", { name: "Add venue element" })).not.toBeInTheDocument(),
		);
	});
});

describe("Add Several", () => {
	it("holds the row's own element for repeated placement without placing a first copy", async () => {
		const { announcePlaced, startPlacing, press } = renderFlows();
		press("venue");
		const dialog = await screen.findByRole("dialog", { name: "Add venue element" });
		const list = await within(dialog).findByRole("list", { name: "Venue elements" });
		await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(2));
		const drums = within(list).getAllByRole("listitem")[1];
		const several = within(drums).getByRole("button", {
			name: "Add Several Drum Kit",
		});
		expect(several).toHaveAttribute("title", "Add Several");
		expect(several).toHaveTextContent("++");
		// A keyboard press is a click on the real button.
		several.focus();
		fireEvent.click(several);
		expect(startPlacing).toHaveBeenCalledWith({
			profileId: DRUMS,
			name: "Drum Kit",
		});
		expect(announcePlaced).not.toHaveBeenCalled();
		expect(mocks.patchFixtures).not.toHaveBeenCalled();
		await waitFor(() =>
			expect(screen.queryByRole("dialog", { name: "Add venue element" })).not.toBeInTheDocument(),
		);
	});
});

describe("the scenery button's flight rack", () => {
	it("places the rack at the height the chosen rack units need", async () => {
		const { announcePlaced, press } = renderFlows();
		press("curtain", `${RACK}:units-12`);
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledTimes(1));
		// 120 mm of case and 44.45 mm a unit, stored in whole millimetres, 600 mm across and deep.
		expect(placedFixture(0)).toMatchObject({
			profileId: RACK,
			scenerySizeMetres: { x: 600, y: 653, z: 600 },
		});
	});
});

describe("the one Stairs part", () => {
	it("is chosen with its handrails and placed with them, each side remembered on its own", async () => {
		const onChoose = vi.fn();
		const menu = render(<CadPartMenu kind="stage" onChoose={onChoose} />);
		const stairs = await screen.findByRole("group", { name: "Stairs" });
		expect(within(stairs).getAllByRole("menuitemradio").map((item) => item.textContent)).toEqual([
			"No handrails",
			"LeftSeen climbing",
			"RightSeen climbing",
			"Both sides",
		]);
		const left = within(stairs).getByRole("menuitemradio", { name: /^Left/u });
		await waitFor(() => expect(left).toBeEnabled());
		fireEvent.click(left);
		const [key] = onChoose.mock.calls[0];
		expect(key).toBe(`${STAIRS}:handrails-left`);
		menu.unmount();

		const { announcePlaced, press } = renderFlows();
		press("stage", key);
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledTimes(1));
		expect(placedFixture(0)).toMatchObject({
			profileId: STAIRS,
			sceneryOptions: { handrails: "left" },
		});
		// A plain press places the same choice again, and the menu checks it.
		press("stage");
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledTimes(2));
		expect(placedFixture(1).sceneryOptions).toEqual({ handrails: "left" });
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

describe("the Place several wizards", () => {
	/** The fixtures of one write, in the order the batch made them. */
	const batch = (call = 0) => mocks.patchFixtures.mock.calls[call][2].fixtures;

	it("butts a field of stage elements edge to edge and writes them in one go", async () => {
		const { announcePlaced, press } = renderFlows();
		press("stage", DECK, true);
		const dialog = await screen.findByRole("dialog", {
			name: "Place several stage elements",
		});
		// Two across and two deep is what the wizard opens on.
		fireEvent.click(within(dialog).getByRole("button", { name: /^Place 4$/u }));
		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledTimes(1));
		expect(batch()).toHaveLength(4);
		expect(batch().map((fixture: { location: unknown }) => fixture.location)).toEqual([
			{ x: 0, y: 0, z: 0 },
			{ x: 2000, y: 0, z: 0 },
			{ x: 0, y: 1000, z: 0 },
			{ x: 2000, y: 1000, z: 0 },
		]);
		// Every element takes its own free virtual number: 1 and 2 are already used.
		expect(batch().map((fixture: { virtualFixtureNumber: number }) => fixture.virtualFixtureNumber)).toEqual([
			3, 4, 5, 6,
		]);
		// The whole field is selected, so it can be moved or adjusted as one.
		expect(announcePlaced).toHaveBeenCalledWith(
			batch().map((fixture: { fixtureId: string }) => fixture.fixtureId),
		);
	});

	it("flies a truss run again at every height, over every line", async () => {
		const { press } = renderFlows();
		press("truss", THREE_POINT, true);
		const dialog = await screen.findByRole("dialog", { name: "Place several trusses" });
		const heights = within(dialog).getByLabelText("Heights");
		fireEvent.change(heights, { target: { value: "5 7" } });
		fireEvent.blur(heights);
		const back = within(dialog).getByLabelText("Positions back");
		fireEvent.change(back, { target: { value: "0 4" } });
		fireEvent.blur(back);
		fireEvent.click(await within(dialog).findByRole("button", { name: /^Place 4$/u }));
		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledTimes(1));
		expect(batch().map((fixture: { location: unknown }) => fixture.location)).toEqual([
			{ x: 0, y: 0, z: 5000 },
			{ x: 0, y: 4000, z: 5000 },
			{ x: 0, y: 0, z: 7000 },
			{ x: 0, y: 4000, z: 7000 },
		]);
	});

	it("crosses the heights with positions across the room when the runs are turned", async () => {
		const { press } = renderFlows();
		press("truss", THREE_POINT, true);
		const dialog = await screen.findByRole("dialog", { name: "Place several trusses" });
		fireEvent.click(within(dialog).getByRole("button", { name: "Runs deep" }));
		const across = within(dialog).getByLabelText("Positions across");
		fireEvent.change(across, { target: { value: "-3 3" } });
		fireEvent.blur(across);
		fireEvent.click(await within(dialog).findByRole("button", { name: /^Place 2$/u }));
		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledTimes(1));
		expect(batch().map((fixture: { location: unknown }) => fixture.location)).toEqual([
			{ x: -3000, y: 0, z: 5000 },
			{ x: 3000, y: 0, z: 5000 },
		]);
		expect(batch().every((fixture: { rotation: { z: number } }) => fixture.rotation.z === 90)).toBe(true);
	});

	it("places nothing at all when the wizard is cancelled", async () => {
		const { announcePlaced, press } = renderFlows();
		press("stage", DECK, true);
		const dialog = await screen.findByRole("dialog", {
			name: "Place several stage elements",
		});
		fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
		await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
		expect(mocks.patchFixtures).not.toHaveBeenCalled();
		expect(announcePlaced).not.toHaveBeenCalled();
	});

	it("leaves a single press placing one element as it always did", async () => {
		const { press } = renderFlows();
		press("stage", DECK);
		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledTimes(1));
		expect(batch()).toHaveLength(1);
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});
});
