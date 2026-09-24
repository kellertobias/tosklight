import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CadAddFlows, type CadAddRequest } from "./CadAddFlows";
import { CadPartMenu } from "./CadPartMenu";
import { holdPart } from "./CadToolbar";
import { CadToolContext, type CadTools } from "./cadTools";

const mocks = vi.hoisted(() => ({
	fixtureProfiles: vi.fn(),
	patchSnapshot: vi.fn(),
	patchFixtures: vi.fn(),
	importVenueModel: vi.fn(),
	open: vi.fn(),
}));

vi.mock("../document/session", () => ({
	documentSession: {
		fixtureProfiles: mocks.fixtureProfiles,
		patchSnapshot: mocks.patchSnapshot,
		importVenueModel: mocks.importVenueModel,
	},
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: mocks.open }));
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
/** The generated flight rack, one part whose rack units and depth are set in Info. */
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
		{
			...profile(THREE_POINT, "Three-Point Truss"),
			scenery: {
				kind: "truss",
				chords: 3,
				default_size_metres: { x: 2, y: 0.29, z: 0.29 },
				adjustable: { width: true, height: false, depth: false },
				minimum_size_metres: { x: 0.5, y: 0.29, z: 0.29 },
				maximum_size_metres: { x: 20, y: 0.29, z: 0.29 },
			},
		},
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
	mocks.open.mockReset();
	mocks.importVenueModel.mockReset();
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

describe("Add Several beside every part in a button's menu", () => {
	it("puts the button at the right of each real part and holds that part, not the checked one", async () => {
		const onChoose = vi.fn();
		const onAddSeveral = vi.fn();
		render(<CadPartMenu kind="truss" onChoose={onChoose} onAddSeveral={onAddSeveral} />);
		const rows = screen.getAllByRole("menuitemradio");
		const buttons = screen.getAllByRole("menuitem", { name: /^Add Several /u });
		// One per part row, each in the same row as its part.
		expect(buttons).toHaveLength(rows.length);
		for (const [index, button] of buttons.entries()) {
			expect(button).toHaveAttribute("title", "Add Several");
			expect(button.closest(".cad-part-menu-row")).toBe(rows[index].closest(".cad-part-menu-row"));
		}
		const fourPoint = await screen.findByRole("group", { name: "4-point" });
		const corner = within(fourPoint).getByRole("menuitem", { name: /^Add Several Corner 2-way/u });
		await waitFor(() => expect(corner).toBeEnabled());
		// A button is a button of its own: reachable by keyboard, and it never chooses the part.
		corner.focus();
		expect(corner).toHaveFocus();
		fireEvent.click(corner);
		expect(onAddSeveral).toHaveBeenCalledWith(CORNER);
		expect(onChoose).not.toHaveBeenCalled();
	});

	it("holds the exact part, options and all, so each press places one more of it", () => {
		const startPlacing = vi.fn();
		holdPart({ startPlacing } as unknown as CadTools, "stage", STAIRS);
		expect(startPlacing).toHaveBeenCalledWith({
			profileId: STAIRS,
			name: "Stairs",
			with: { sceneryOptions: undefined, scenerySizeMetres: undefined },
		});
		startPlacing.mockClear();
		holdPart({ startPlacing } as unknown as CadTools, "curtain", RACK);
		expect(startPlacing).toHaveBeenCalledWith(
			expect.objectContaining({
				profileId: RACK,
				with: expect.objectContaining({ scenerySizeMetres: { x: 600, y: 476, z: 600 } }),
			}),
		);
	});
});

describe("Load model from Add primitive", () => {
	it("offers Load model in the primitive menu and nowhere else", () => {
		const onLoadModel = vi.fn();
		const { unmount } = render(<CadPartMenu kind="primitive" onChoose={vi.fn()} onLoadModel={onLoadModel} />);
		fireEvent.click(screen.getByRole("menuitem", { name: /Load model/u }));
		expect(onLoadModel).toHaveBeenCalledTimes(1);
		unmount();
		render(<CadPartMenu kind="truss" onChoose={vi.fn()} />);
		expect(screen.queryByRole("menuitem", { name: /Load model/u })).toBeNull();
	});

	it("imports the chosen file, shows it loading, and selects what it placed", async () => {
		mocks.open.mockResolvedValue("/models/stage-set.glb");
		let finish: (value: { fixtureId: string; name: string }) => void = () => undefined;
		mocks.importVenueModel.mockReturnValue(new Promise((resolve) => (finish = resolve)));
		const { announcePlaced, onError, press } = renderFlows();
		press("primitive", "load-model");
		expect(await screen.findByRole("status")).toHaveTextContent("Loading the 3D model");
		expect(mocks.importVenueModel).toHaveBeenCalledWith("/models/stage-set.glb", null);
		finish({ fixtureId: "set", name: "Stage Set" });
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledWith(["set"]));
		expect(screen.queryByRole("status")).toBeNull();
		expect(onError).not.toHaveBeenCalled();
		// Nothing from the library was placed, and the button still places its own part next.
		expect(mocks.patchFixtures).not.toHaveBeenCalled();
	});

	it("says why a file could not be loaded, and does nothing when the picker is closed", async () => {
		mocks.open.mockResolvedValueOnce(null);
		const { announcePlaced, onError, press } = renderFlows();
		press("primitive", "load-model");
		await waitFor(() => expect(mocks.open).toHaveBeenCalledTimes(1));
		expect(mocks.importVenueModel).not.toHaveBeenCalled();
		expect(screen.queryByRole("status")).toBeNull();

		mocks.open.mockResolvedValueOnce("/models/broken.obj");
		mocks.importVenueModel.mockRejectedValueOnce(new Error("the OBJ file has no faces"));
		press("primitive", "load-model");
		await waitFor(() =>
			expect(onError).toHaveBeenCalledWith("Could not load the 3D model: Error: the OBJ file has no faces"),
		);
		expect(announcePlaced).not.toHaveBeenCalled();
	});
});

describe("the scenery button's flight rack", () => {
	it("is one Flight rack in the menu, placed at 8U, whatever rack size was remembered before", async () => {
		const onChoose = vi.fn();
		const menu = render(<CadPartMenu kind="curtain" onChoose={onChoose} />);
		// One Flight rack entry, not a heading over rack-unit sizes.
		expect(screen.queryByRole("group", { name: "Flight rack" })).toBeNull();
		expect(screen.queryByRole("menuitemradio", { name: /^\d+U/u })).toBeNull();
		const rack = screen.getByRole("menuitemradio", { name: /^Flight rack/u });
		await waitFor(() => expect(rack).toBeEnabled());
		expect(rack).toHaveTextContent("Units and depth are set in Info");
		fireEvent.click(rack);
		expect(onChoose).toHaveBeenCalledWith(RACK);
		menu.unmount();

		const { announcePlaced, press } = renderFlows();
		press("curtain", RACK);
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledTimes(1));
		// 120 mm of case and 44.45 mm a unit, stored in whole millimetres, 600 mm across and deep.
		expect(placedFixture(0)).toMatchObject({
			profileId: RACK,
			scenerySizeMetres: { x: 600, y: 476, z: 600 },
		});
		// A choice remembered as one of the old sizes places the same one rack.
		localStore.set("tosklight:viz-editor:cad-add-part:curtain:v1", `${RACK}:units-12`);
		press("curtain");
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledTimes(2));
		expect(placedFixture(1)).toMatchObject({ profileId: RACK, scenerySizeMetres: { y: 476 } });
	});
});

describe("the one Stairs part", () => {
	it("is one item in the menu, placed without rails, whatever variant was remembered before", async () => {
		const onChoose = vi.fn();
		const menu = render(<CadPartMenu kind="stage" onChoose={onChoose} />);
		// One Stairs entry, not a heading over handrail variants.
		expect(screen.queryByRole("group", { name: "Stairs" })).toBeNull();
		const stairs = screen.getByRole("menuitemradio", { name: /^Stairs/u });
		await waitFor(() => expect(stairs).toBeEnabled());
		expect(stairs).toHaveTextContent("Handrails are chosen in Info");
		fireEvent.click(stairs);
		expect(onChoose).toHaveBeenCalledWith(STAIRS);
		menu.unmount();

		const { announcePlaced, press } = renderFlows();
		press("stage", STAIRS);
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledTimes(1));
		expect(placedFixture(0)).toMatchObject({ profileId: STAIRS });
		expect(placedFixture(0).sceneryOptions ?? undefined).toBeUndefined();
		// A choice remembered as one of the old variants places the same one Stairs.
		localStore.set("tosklight:viz-editor:cad-add-part:stage:v1", `${STAIRS}:handrails-left`);
		press("stage");
		await waitFor(() => expect(announcePlaced).toHaveBeenCalledTimes(2));
		expect(placedFixture(1)).toMatchObject({ profileId: STAIRS });
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

describe("Place Multiple", () => {
	/** The fixtures of one write, in the order the batch made them. */
	const batch = (call = 0) => mocks.patchFixtures.mock.calls[call][2].fixtures;
	const type = (dialog: HTMLElement, label: string, value: string) => {
		const field = within(dialog).getByLabelText(label);
		fireEvent.change(field, { target: { value } });
		fireEvent.keyDown(field, { key: "Enter" });
	};

	it("is the button beside every truss and stage element, and the menus end with no Place several", async () => {
		const onPlaceMultiple = vi.fn();
		const onAddSeveral = vi.fn();
		const truss = render(
			<CadPartMenu kind="truss" onChoose={vi.fn()} onAddSeveral={onAddSeveral} onPlaceMultiple={onPlaceMultiple} />,
		);
		expect(screen.queryByRole("menuitem", { name: /Place several/u })).toBeNull();
		// Beside every truss part the library holds, each naming its own part.
		await waitFor(() =>
			expect(
				screen.getAllByRole("menuitem", { name: /^Place Multiple/u }).some((each) => !each.hasAttribute("disabled")),
			).toBe(true),
		);
		const beside = screen
			.getAllByRole("menuitem", { name: /^Place Multiple/u })
			.filter((each) => !each.hasAttribute("disabled"));
		for (const each of beside) fireEvent.click(each);
		expect(onPlaceMultiple.mock.calls.map(([key]) => key)).toEqual(expect.arrayContaining([THREE_POINT, STRAIGHT]));
		expect(new Set(onPlaceMultiple.mock.calls.map(([key]) => key)).size).toBe(beside.length);
		onPlaceMultiple.mockClear();
		expect(screen.queryByRole("menuitem", { name: /^Add Several/u })).toBeNull();
		truss.unmount();

		const stage = render(
			<CadPartMenu kind="stage" onChoose={vi.fn()} onAddSeveral={onAddSeveral} onPlaceMultiple={onPlaceMultiple} />,
		);
		expect(screen.getByRole("menuitem", { name: "Place Multiple 2 × 1 m (Scissor feet)" })).toBeInTheDocument();
		// Stairs are placed one at a time: they keep Add Several.
		expect(screen.queryByRole("menuitem", { name: /^Place Multiple Stairs/u })).toBeNull();
		expect(screen.getByRole("menuitem", { name: /^Add Several Stairs/u })).toBeInTheDocument();
		expect(screen.queryByRole("menuitem", { name: /Place several/u })).toBeNull();
		stage.unmount();

		// Parts that are never arranged offer only Add Several.
		render(<CadPartMenu kind="curtain" onChoose={vi.fn()} onAddSeveral={onAddSeveral} onPlaceMultiple={onPlaceMultiple} />);
		expect(screen.queryByRole("menuitem", { name: /^Place Multiple/u })).toBeNull();
		expect(screen.getAllByRole("menuitem", { name: /^Add Several/u }).length).toBeGreaterThan(0);
	});

	it("butts a grid of stage elements edge to edge around its centre and writes it in one go", async () => {
		const { announcePlaced, press } = renderFlows();
		press("stage", DECK, true);
		const dialog = await screen.findByRole("dialog", { name: "Place multiple stage elements" });
		type(dialog, "Across (X)", "3");
		type(dialog, "Grid centre X", "5");
		type(dialog, "Grid centre Y", "-2");
		// The plan shows every element before anything is placed.
		expect(within(dialog).getAllByTestId("cad-bulk-preview-element")).toHaveLength(6);
		expect(mocks.patchFixtures).not.toHaveBeenCalled();
		fireEvent.click(within(dialog).getByRole("button", { name: /^Place 6$/u }));
		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledTimes(1));
		expect(batch().map((fixture: { location: unknown }) => fixture.location)).toEqual([
			{ x: 3000, y: -2500, z: 0 },
			{ x: 5000, y: -2500, z: 0 },
			{ x: 7000, y: -2500, z: 0 },
			{ x: 3000, y: -1500, z: 0 },
			{ x: 5000, y: -1500, z: 0 },
			{ x: 7000, y: -1500, z: 0 },
		]);
		// Every element takes its own free virtual number: 1 and 2 are already used.
		expect(batch().map((fixture: { virtualFixtureNumber: number }) => fixture.virtualFixtureNumber)).toEqual([
			3, 4, 5, 6, 7, 8,
		]);
		// The whole grid is selected, so it can be moved or adjusted as one.
		expect(announcePlaced).toHaveBeenCalledWith(batch().map((fixture: { fixtureId: string }) => fixture.fixtureId));
	});

	it("lays a truss run from its first point to its last, turned about Z and stepped in height", async () => {
		const { press } = renderFlows();
		press("truss", THREE_POINT, true);
		const dialog = await screen.findByRole("dialog", { name: "Place multiple trusses" });
		type(dialog, "First point X", "0");
		type(dialog, "First point Y", "0");
		type(dialog, "First point Z", "5");
		type(dialog, "Last point X", "6");
		type(dialog, "Last point Y", "6");
		type(dialog, "Last point Z", "8");
		type(dialog, "Sections", "3");
		expect(within(dialog).getAllByTestId("cad-bulk-preview-element")).toHaveLength(3);
		fireEvent.click(within(dialog).getByRole("button", { name: /^Place 3$/u }));
		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledTimes(1));
		expect(batch().map((fixture: { location: unknown }) => fixture.location)).toEqual([
			{ x: 1000, y: 1000, z: 5500 },
			{ x: 3000, y: 3000, z: 6500 },
			{ x: 5000, y: 5000, z: 7500 },
		]);
		// Heading along the run on the plan, never pitched or rolled.
		for (const fixture of batch()) expect(fixture.rotation).toEqual({ x: 0, y: 0, z: 45 });
		// Each straight section fills its share of the run: √72 / 3 m.
		expect(batch()[0].scenerySizeMetres).toEqual({ x: 2828, y: 290, z: 290 });
	});

	it("places each section at the length typed for it", async () => {
		const { press } = renderFlows();
		press("truss", THREE_POINT, true);
		const dialog = await screen.findByRole("dialog", { name: "Place multiple trusses" });
		type(dialog, "Section length", "1.5");
		fireEvent.click(within(dialog).getByRole("button", { name: /^Place 4$/u }));
		await waitFor(() => expect(mocks.patchFixtures).toHaveBeenCalledTimes(1));
		expect(batch().map((fixture: { location: { x: number } }) => fixture.location.x)).toEqual([-3000, -1000, 1000, 3000]);
		for (const fixture of batch()) expect(fixture.scenerySizeMetres).toEqual({ x: 1500, y: 290, z: 290 });
	});

	it("places nothing at all when the wizard is cancelled", async () => {
		const { announcePlaced, press } = renderFlows();
		press("stage", DECK, true);
		const dialog = await screen.findByRole("dialog", { name: "Place multiple stage elements" });
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
