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

		// The second channel becomes Red, so the two rows are distinguishable.
		let rows = document.querySelectorAll<HTMLElement>(".fixture-channel-row");
		fireEvent.click(
			within(rows[1]).getByRole("button", { name: "Edit intensity channel" }),
		);
		choose("Channel role", "color · Red");
		fireEvent.click(screen.getByRole("button", { name: "Close channel editor" }));

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

		// The intensity channel reacts to the virtual dimmer, and never fades.
		rows = document.querySelectorAll<HTMLElement>(".fixture-channel-row");
		fireEvent.click(
			within(rows[1]).getByRole("button", { name: "Edit intensity channel" }),
		);
		fireEvent.click(screen.getByRole("checkbox", { name: /virtual intensity/i }));
		fireEvent.click(screen.getByRole("checkbox", { name: /Snap/i }));

		// One physical channel carrying a dimmer band and a strobe band: a mixed channel.
		fireEvent.click(screen.getByRole("button", { name: /Channel functions/ }));
		fireEvent.click(screen.getByRole("button", { name: "Add function" }));
		let names = screen.getAllByLabelText("Function name");
		fireEvent.change(names[0], { target: { value: "Dimmer" } });
		setNumber("DMX from", 0);
		setNumber("DMX to", 127);

		fireEvent.click(screen.getByRole("button", { name: "Add function" }));
		names = screen.getAllByLabelText("Function name");
		fireEvent.change(names[1], { target: { value: "Strobe slow to fast" } });
		const froms = screen.getAllByLabelText("DMX from");
		const tos = screen.getAllByLabelText("DMX to");
		fireEvent.change(froms[1], { target: { value: "128" } });
		fireEvent.change(tos[1], { target: { value: "254" } });

		fireEvent.click(
			screen.getByRole("button", { name: "Close channel functions" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Close channel editor" }));

		// A third channel whose DMX range is divided into named positions rather than a percentage.
		fireEvent.click(screen.getByRole("button", { name: "Add channel" }));
		rows = document.querySelectorAll<HTMLElement>(".fixture-channel-row");
		fireEvent.click(
			within(rows[2]).getByRole("button", { name: "Edit intensity channel" }),
		);
		choose("Channel role", "intensity · Shutter / Strobe");
		fireEvent.click(screen.getByRole("button", { name: /Channel functions/ }));
		for (const [index, [name, from, to, raw]] of (
			[
				["Shutter closed", 0, 17, 0],
				["Shutter open", 18, 72, 18],
			] as const
		).entries()) {
			fireEvent.click(screen.getByRole("button", { name: "Add function" }));
			fireEvent.change(screen.getAllByLabelText("Function name")[index], {
				target: { value: name },
			});
			fireEvent.change(screen.getAllByLabelText("DMX from")[index], {
				target: { value: String(from) },
			});
			fireEvent.change(screen.getAllByLabelText("DMX to")[index], {
				target: { value: String(to) },
			});
			const behaviours = screen.getAllByText("Function behavior", {
				selector: "label",
				exact: true,
			});
			const field = behaviours[index].closest(".ui-form-field");
			fireEvent.click(
				requiredElement(
					field?.querySelector<HTMLButtonElement>(".ui-select-trigger") ?? null,
				),
			);
			fireEvent.click(
				screen.getByRole("option", { name: "Indexed color or gobo" }),
			);
			fireEvent.change(screen.getAllByLabelText("Fixture label")[index], {
				target: { value: name },
			});
			fireEvent.change(screen.getAllByLabelText("Exact raw value")[index], {
				target: { value: String(raw) },
			});
		}
		fireEvent.click(
			screen.getByRole("button", { name: "Close channel functions" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Close channel editor" }));
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
