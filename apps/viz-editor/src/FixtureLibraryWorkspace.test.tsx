import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import type { AttributeDescriptor, FixtureProfile } from "@tosklight/patch";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FixtureLibraryWorkspace } from "./FixtureLibraryWorkspace";
import "./styles.css";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({
	listen: vi.fn(() => Promise.resolve(() => undefined)),
}));
vi.mock("@tauri-apps/api/window", () => ({
	getCurrentWindow: () => ({ startDragging: vi.fn() }),
}));

const REGISTRY: AttributeDescriptor[] = [
	{
		id: "intensity",
		label: "Intensity",
		family: "intensity",
		value_type: "continuous",
		default_unit: "percent",
	},
	{
		id: "shutter",
		label: "Shutter / Strobe",
		family: "intensity",
		value_type: "indexed",
		default_unit: null,
	},
	{
		id: "color.red",
		label: "Red",
		family: "color",
		value_type: "continuous",
		default_unit: "percent",
	},
];

function requiredElement<T extends Element>(element: T | null): T {
	if (!element) throw new Error("Expected a fixture editor element to exist");
	return element;
}

function choose(label: string, option: string) {
	const field = screen
		.getByText(label, { selector: "label", exact: true })
		.closest(".ui-form-field");
	fireEvent.click(
		requiredElement(
			field?.querySelector<HTMLButtonElement>(".ui-select-trigger") ?? null,
		),
	);
	fireEvent.click(screen.getByRole("option", { name: option }));
}

function setNumber(label: string, value: number) {
	fireEvent.change(screen.getByLabelText(label), {
		target: { value: String(value) },
	});
}

function touchDrag(source: HTMLElement, target: HTMLElement, pointerId: number) {
	Object.defineProperties(source, {
		setPointerCapture: { configurable: true, value: vi.fn() },
		hasPointerCapture: { configurable: true, value: vi.fn(() => true) },
		releasePointerCapture: { configurable: true, value: vi.fn() },
	});
	Object.defineProperty(document, "elementFromPoint", {
		configurable: true,
		value: vi.fn(() => target),
	});
	fireEvent.pointerDown(source, { pointerId, pointerType: "touch" });
	fireEvent.pointerMove(source, {
		pointerId,
		pointerType: "touch",
		clientX: 40,
		clientY: 80,
	});
	fireEvent.pointerUp(source, { pointerId, pointerType: "touch" });
}

/** Narrow down the three columns the way an operator does: manufacturer, then fixture. */
async function chooseFixture(manufacturer: string, fixture: string) {
	fireEvent.click(await screen.findByRole("button", { name: new RegExp(`^${manufacturer}`) }));
	fireEvent.click(await screen.findByRole("button", { name: new RegExp(`^${fixture}`) }));
}

/** What the fixture browser already offers, so the list has something in it. */
function existingProfile(): FixtureProfile {
	return {
		schema_version: 3,
		id: "11111111-1111-4111-8111-111111111111",
		revision: 2,
		manufacturer: "Acme",
		name: "Planning Wash",
		short_name: "Wash",
		fixture_type: "wash",
		patch_policy: "dmx",
		notes: "",
		physical: {},
		hazardous: false,
		model_asset: null,
		stage_icon_asset: null,
		direct_control_protocols: [],
		signal_loss_policy: "hold",
		modes: [
			{
				id: "22222222-2222-4222-8222-222222222222",
				name: "Default",
				notes: "",
				splits: [{ number: 1, footprint: 1 }],
				heads: [],
				channels: [],
				color_systems: [],
				control_actions: [],
			},
		],
	} as unknown as FixtureProfile;
}

function renderWorkspace(profiles: FixtureProfile[] = [existingProfile()]) {
	const onReloadProfiles = vi.fn();
	const onError = vi.fn();
	render(
		<ModalProvider>
			<FixtureLibraryWorkspace
				profiles={profiles}
				onReloadProfiles={onReloadProfiles}
				onError={onError}
			/>
		</ModalProvider>,
	);
	return { onReloadProfiles, onError };
}

/** The profile the Architect handed to the library on the last save. */
function savedProfile(): FixtureProfile {
	const call = [...invoke.mock.calls]
		.reverse()
		.find((entry: unknown[]) => entry[0] === "save_library_profile");
	if (!call) throw new Error("Nothing was saved to the fixture library");
	return (call[1] as { profile: FixtureProfile }).profile;
}

beforeEach(() => {
	invoke.mockReset();
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);
	invoke.mockImplementation((command: string, args?: Record<string, unknown>) => {
		switch (command) {
			case "attribute_registry":
				return Promise.resolve(REGISTRY);
			case "save_library_profile":
				return Promise.resolve({
					id: "saved",
					revision: Number(args?.expectedRevision ?? 0) + 1,
					manufacturer: "Acme",
					name: "Saved",
					profile: args?.profile,
				});
			default:
				return Promise.reject(new Error(`unexpected command ${command}`));
		}
	});
});

describe("the Architect fixture library", () => {
	it("narrows down manufacturer, fixture, then what that fixture is", async () => {
		renderWorkspace();
		// Nothing is chosen yet, so the two columns to the right say what to do rather than
		// listing every fixture on the machine.
		expect(await screen.findByText("Choose a manufacturer.")).toBeVisible();
		expect(screen.getByText("Choose a fixture to see what it is.")).toBeVisible();
		// A fixture is not offered until its manufacturer is.
		expect(screen.queryByRole("button", { name: /^Planning Wash/ })).toBeNull();

		fireEvent.click(screen.getByRole("button", { name: /^Acme/ }));
		fireEvent.click(await screen.findByRole("button", { name: /^Planning Wash/ }));

		const info = within(screen.getByRole("region", { name: "Fixture info" }));
		expect(info.getByRole("heading", { name: "Planning Wash" })).toBeVisible();
		expect(info.getByText(/revision 2/)).toBeVisible();
		expect(info.getByText("Default (1)")).toBeVisible();

		fireEvent.click(info.getByRole("button", { name: "Edit as new revision" }));
		expect(
			await screen.findByRole("dialog", { name: "Edit fixture profile" }),
		).toBeVisible();
	});

	it("says the library is empty rather than showing a bare table", () => {
		renderWorkspace([]);
		expect(
			screen.getByText(/fixture library is empty/i),
		).toBeVisible();
	});

	it("filters every column as the operator types", async () => {
		renderWorkspace();
		await chooseFixture("Acme", "Planning Wash");
		fireEvent.change(screen.getByLabelText("Search fixtures"), {
			target: { value: "beam" },
		});
		// The manufacturer column empties too, so no column can offer what search excluded.
		expect(screen.queryByRole("button", { name: /^Acme/ })).toBeNull();
		expect(screen.queryByRole("button", { name: /^Planning Wash/ })).toBeNull();
		expect(screen.getByText("No fixtures yet.")).toBeVisible();
	});

	it("previews the fixture geometry live, as the desk does", async () => {
		renderWorkspace();
		await screen.findByRole("button", { name: /^Acme/ });
		fireEvent.click(screen.getByRole("button", { name: "Create fixture" }));
		await screen.findByRole("dialog", { name: "Create fixture profile" });

		fireEvent.click(screen.getByRole("tab", { name: "Geometry" }));
		fireEvent.click(screen.getByRole("button", { name: "Fixed fixture" }));
		// The Architect draws with the Stage's own geometry code rather than declining to draw.
		expect(
			screen.getByRole("img", {
				name: "Fixture geometry hierarchy and beams in three dimensions",
			}),
		).toBeInTheDocument();
		expect(screen.queryByText(/no Stage renderer/)).toBeNull();
	});

	it("keeps the tabs still when a tab brings its own action", async () => {
		renderWorkspace();
		await screen.findByRole("button", { name: /^Acme/ });
		fireEvent.click(screen.getByRole("button", { name: "Create fixture" }));
		await screen.findByRole("dialog", { name: "Create fixture profile" });

		fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
		const addMode = screen.getByRole("button", { name: "Add mode" });
		const firstTab = screen.getByRole("tab", { name: "Identity" });
		// Left of the tabs: the bar is right-aligned, so a button after them would shove them over.
		expect(
			addMode.compareDocumentPosition(firstTab) &
				Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
	});

	it("shows where in the editing tree each window is, and walks back up it", async () => {
		renderWorkspace();
		await chooseFixture("Acme", "Planning Wash");
		fireEvent.click(screen.getByRole("button", { name: "Edit as new revision" }));
		const editor = await screen.findByRole("dialog", {
			name: "Edit fixture profile",
		});
		const path = (scope: HTMLElement) =>
			within(within(scope).getAllByRole("navigation", { name: "Editing path" })[0])
				.getAllByRole("listitem")
				.map((item) => item.textContent);
		expect(path(editor)).toEqual(["Acme Planning Wash", "Identity"]);

		fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
		expect(path(editor)).toEqual(["Acme Planning Wash", "Modes"]);

		fireEvent.click(screen.getByRole("button", { name: "Edit channels for Default" }));
		const mode = screen.getByRole("dialog", { name: "Edit Default mode" });
		expect(path(mode)).toEqual([
			"Acme Planning Wash",
			"Modes",
			"Default",
			within(mode).getByRole("tab", { selected: true }).textContent,
		]);

		// Choosing an ancestor closes every window below it.
		fireEvent.click(
			within(mode).getByRole("button", { name: "Acme Planning Wash" }),
		);
		expect(screen.queryByRole("dialog", { name: "Edit Default mode" })).toBeNull();
		expect(screen.getByRole("dialog", { name: "Edit fixture profile" })).toBeVisible();
	});

	// The four things an operator has to be able to say about a channel, all in one authored
	// fixture, because they are not independent: they are the same channel's behaviour.
	it("authors channel order, indexed positions, virtual-dimmer response and a mixed channel", async () => {
		renderWorkspace();
		await screen.findByRole("button", { name: /^Acme/ });
		fireEvent.click(screen.getByRole("button", { name: "Create fixture" }));
		const editor = within(
			await screen.findByRole("dialog", { name: "Create fixture profile" }),
		);
		// The registry the Architect reads is the desk's own, not a second list.
		await waitFor(() =>
			expect(
				invoke.mock.calls.some(([command]) => command === "attribute_registry"),
			).toBe(true),
		);

		fireEvent.change(editor.getByLabelText(/^Manufacturer/), {
			target: { value: "Acme" },
		});
		fireEvent.change(editor.getByLabelText(/^Fixture name/), {
			target: { value: "Planning Beam" },
		});

		fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Edit channels for Default" }),
		);

		fireEvent.click(screen.getByRole("button", { name: "Add channel" }));
		fireEvent.click(screen.getByRole("button", { name: "Add channel" }));

		// The second channel becomes Red, so the two rows are distinguishable. The attribute is
		// chosen by its group first, then by name inside it.
		let rows = document.querySelectorAll<HTMLElement>(".fixture-channel-row");
		const chooseAttribute = (slot: number, from: string, group: RegExp, attribute: RegExp) => {
			fireEvent.click(
				screen.getByRole("button", { name: `Attribute for slot ${slot}: ${from}` }),
			);
			const picker = within(
				screen.getByRole("dialog", { name: `Attribute · slot ${slot}` }),
			);
			// Encoder group, then activation group, then the attribute itself.
			fireEvent.click(
				within(picker.getByRole("listbox", { name: "Encoder groups" })).getByRole(
					"option",
					{ name: group },
				),
			);
			fireEvent.click(
				within(picker.getByRole("listbox", { name: "Activation groups" })).getByRole(
					"option",
					{ name: attribute },
				),
			);
			fireEvent.click(
				within(picker.getByRole("listbox", { name: /attributes$/ })).getByRole(
					"option",
					{ name: attribute },
				),
			);
		};
		chooseAttribute(2, "Intensity", /^Color/, /^Red/);

		// Reorder by dragging: Red takes slot 1 and Intensity slot 2.
		rows = document.querySelectorAll<HTMLElement>(".fixture-channel-row");
		touchDrag(
			requiredElement(rows[0].querySelector<HTMLElement>(".touch-drag-handle")),
			rows[1],
			7,
		);
		expect(
			[...document.querySelectorAll<HTMLElement>(".fixture-channel-row")].map(
				(row) => row.textContent,
			),
		).toEqual([
			expect.stringContaining("Red"),
			expect.stringContaining("Intensity"),
		]);

		// The intensity channel reacts to the virtual dimmer, and never fades — both set in the table.
		fireEvent.click(screen.getByRole("button", { name: "Masters for Intensity" }));
		fireEvent.click(
			within(
				screen.getByRole("radiogroup", { name: "React to Virtual Intensity" }),
			).getByRole("radio", { name: "Follow" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Close masters" }));
		fireEvent.click(screen.getByRole("switch", { name: "Snap Intensity" }));

		// One physical channel carrying a dimmer band and a strobe band: a mixed channel. Its
		// functions are a table under its physical range.
		// Every cell shows its value and opens a window to change it; the keyboard types into it.
		const functionRow = (index: number) =>
			within(document.querySelectorAll<HTMLElement>(".fixture-function-row")[index]);
		const enter = (index: number, column: string, value: string) => {
			fireEvent.click(
				functionRow(index).getByRole("button", { name: new RegExp(`^${column} of`) }),
			);
			for (let count = 0; count < 24; count++)
				fireEvent.keyDown(window, { key: "Backspace" });
			for (const key of value) fireEvent.keyDown(window, { key });
			fireEvent.keyDown(window, { key: "Enter" });
		};
		fireEvent.click(screen.getByRole("button", { name: "Edit intensity mapping" }));
		fireEvent.click(screen.getByRole("button", { name: "Add function" }));
		enter(0, "Name", "Dimmer");
		enter(0, "DMX from", "0");
		enter(0, "DMX to", "127");

		fireEvent.click(screen.getByRole("button", { name: "Add function" }));
		enter(1, "Name", "Strobe slow to fast");
		enter(1, "DMX from", "128");
		enter(1, "DMX to", "254");

		fireEvent.click(screen.getByRole("button", { name: "Close channel mapping" }));

		// A third channel whose DMX range is divided into named positions rather than a percentage.
		fireEvent.click(screen.getByRole("button", { name: "Add channel" }));
		chooseAttribute(3, "Intensity", /^Intensity/, /^Shutter \/ Strobe/);
		fireEvent.click(screen.getByRole("button", { name: "Edit shutter mapping" }));
		for (const [index, [name, from, to, raw]] of (
			[
				["Shutter closed", 0, 17, 0],
				["Shutter open", 18, 72, 18],
			] as const
		).entries()) {
			fireEvent.click(screen.getByRole("button", { name: "Add function" }));
			enter(index, "Name", name);
			enter(index, "DMX from", String(from));
			enter(index, "DMX to", String(to));
			fireEvent.click(
				functionRow(index).getByRole("button", { name: /^Behavior of/ }),
			);
			fireEvent.click(
				screen.getByRole("option", { name: "Indexed color or gobo" }),
			);
			// Only the function just added has its details open, so its fields are the only ones shown.
			fireEvent.change(screen.getByLabelText("Fixture label"), {
				target: { value: name },
			});
			fireEvent.change(screen.getByLabelText("Exact raw value"), {
				target: { value: String(raw) },
			});
		}
		fireEvent.click(screen.getByRole("button", { name: "Close channel mapping" }));
		fireEvent.click(screen.getByRole("button", { name: "Close mode editor" }));

		fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));

		expect(document.querySelector(".fixture-profile-errors")).toBeNull();
		await waitFor(() => expect(savedProfile()).toBeTruthy());
		const mode = savedProfile().modes[0];
		// Channel order is the DMX slot order, and it is what the drag produced.
		expect(mode.channels.map((channel) => channel.attribute)).toEqual([
			"color.red",
			"intensity",
			"shutter",
		]);
		const intensity = mode.channels[1];
		expect(intensity.reacts_to_virtual_intensity).toBe(true);
		expect(intensity.snap).toBe(true);
		expect(
			intensity.functions.map((entry) => [
				entry.name,
				entry.dmx_from,
				entry.dmx_to,
			]),
		).toEqual([
			["Dimmer", 0, 127],
			["Strobe slow to fast", 128, 254],
		]);
		// Indexed positions: named bands of one channel, each with its exact raw value.
		const shutter = mode.channels[2];
		expect(shutter.attribute).toBe("shutter");
		expect(
			shutter.functions.map((entry) => [
				entry.dmx_from,
				entry.dmx_to,
				entry.behavior.type,
				"label" in entry.behavior ? entry.behavior.label : null,
				"raw_value" in entry.behavior ? entry.behavior.raw_value : null,
			]),
		).toEqual([
			[0, 17, "indexed", "Shutter closed", 0],
			[18, 72, "indexed", "Shutter open", 18],
		]);

		// A new fixture is revision 0 until the library assigns one.
		const call = [...invoke.mock.calls]
			.reverse()
			.find((entry: unknown[]) => entry[0] === "save_library_profile");
		expect(call?.[1]).toMatchObject({ expectedRevision: 0 });
	});
});
