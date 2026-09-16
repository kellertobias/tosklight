import {
	cleanup,
	fireEvent,
	render as rtlRender,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { copyColorSystemToOtherHeads } from "@tosklight/patch/library";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { FixtureChannel, FixtureMode, FixtureProfile } from "../../api/types";
import { FixtureProfileEditor } from "./FixtureProfileEditor";
import {
	blankChannel,
	blankFixtureProfile,
	blankFunction,
	blankHead,
	hexToXyz,
	nominalWheelSrgb,
	srgbToXyz,
	wheelSlotDisplayXyz,
	xyzToHex,
} from "./fixtureProfileModel";

const render = (ui: Parameters<typeof rtlRender>[0]) =>
	rtlRender(ui, { wrapper: ModalProvider });

vi.mock("../files/RootConfinedFilePickerButton", () => ({
	RootConfinedFilePickerButton: ({ label }: { label: string }) => (
		<span>{label}</span>
	),
}));

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

function channel(
	mode: FixtureMode,
	headId: string,
	attribute: string,
): FixtureChannel {
	return {
		...blankChannel(mode),
		head_id: headId,
		attribute,
		fixture_attribute: attribute,
	};
}

/** A one-head spot whose wheel channel names Open, Deep Red and a rotation range. */
function wheelProfile(): FixtureProfile {
	const profile = blankFixtureProfile();
	profile.manufacturer = "Acme";
	profile.name = "Wheel";
	const mode = profile.modes[0];
	const wheel = channel(mode, mode.heads[0].id, "color.wheel.1");
	const indexed = (label: string, from: number, to: number) => ({
		...blankFunction(wheel, "indexed"),
		name: label,
		dmx_from: from,
		dmx_to: to,
		behavior: {
			type: "indexed" as const,
			semantic_id: "",
			label,
			raw_value: from,
		},
	});
	wheel.functions = [
		indexed("Deep Red", 10, 19),
		indexed("Open", 0, 9),
		{ ...blankFunction(wheel), dmx_from: 128, dmx_to: 255 },
	];
	mode.channels = [wheel];
	mode.color_systems = [
		{
			head_id: mode.heads[0].id,
			correction_matrix: [
				[1, 0, 0],
				[0, 1, 0],
				[0, 0, 1],
			],
			system: { type: "discrete_wheel", channel_id: wheel.id, slots: [] },
		},
	];
	return profile;
}

/** A master head plus two RGB pixels, only the first of which has a color system. */
function pixelMode(): FixtureMode {
	const mode = blankFixtureProfile().modes[0];
	mode.heads = [blankHead(0), blankHead(1), blankHead(2)];
	mode.channels = mode.heads
		.slice(1)
		.flatMap((head) =>
			["color.red", "color.green", "color.blue"].map((attribute) =>
				channel(mode, head.id, attribute),
			),
		);
	mode.splits[0].footprint = 6;
	mode.color_systems = [
		{
			head_id: mode.heads[1].id,
			correction_matrix: [
				[1, 0, 0],
				[0, 1, 0],
				[0, 0, 1],
			],
			system: {
				type: "additive",
				emitters: mode.channels.slice(0, 3).map((candidate, index) => ({
					channel_id: candidate.id,
					name: ["Red", "Green", "Blue"][index],
					xyz: srgbToXyz(
						index === 0 ? 1 : 0,
						index === 1 ? 1 : 0,
						index === 2 ? 1 : 0,
					),
					maximum_level: 1,
					response_curve: 1,
					visible: true,
				})),
			},
		},
	];
	return mode;
}

describe("discrete color wheel definitions", () => {
	it("names describe the same nominal colors the simulator uses", () => {
		expect(nominalWheelSrgb("Open")).toEqual([1, 1, 1]);
		expect(nominalWheelSrgb("deep_red")).toEqual([0.7, 0, 0]);
		expect(nominalWheelSrgb("Congo Blue")).toEqual([0.25, 0, 0.6]);
		expect(nominalWheelSrgb("Rotation stop")).toBeNull();
		expect(xyzToHex(hexToXyz("#ff8000"))).toBe("#ff8000");
		const slot = {
			semantic_id: "effect",
			label: "Effect",
			dmx_from: 0,
			dmx_to: 9,
			measured_xyz: null,
		};
		expect(wheelSlotDisplayXyz(slot)).toBeNull();
		expect(
			wheelSlotDisplayXyz({ ...slot, measured_xyz: { x: 1, y: 2, z: 3 } }),
		).toEqual({ x: 1, y: 2, z: 3 });
	});

	it("fills slots from the wheel functions and persists an edited display color", async () => {
		const profile = wheelProfile();
		const save = vi.fn(async (draft: FixtureProfile) => draft);
		render(
			<FixtureProfileEditor
				initialProfile={profile}
				manufacturers={[]}
				onSave={save}
				onClose={vi.fn()}
			/>,
		);
		fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Edit channels for Default" }),
		);
		fireEvent.click(screen.getByRole("tab", { name: "Color" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Fill slots from wheel functions" }),
		);
		const slots = document.querySelectorAll(".color-wheel-editor > article");
		expect(slots).toHaveLength(2);
		// Unmeasured slots already show the color their names describe.
		const open = within(slots[0] as HTMLElement);
		expect(open.getByText("From the slot name")).toBeInTheDocument();
		expect(open.getByRole("button", { name: /#FFFFFF/ })).toBeInTheDocument();
		const red = within(slots[1] as HTMLElement);
		fireEvent.click(red.getByRole("button", { name: /#/ }));
		fireEvent.click(
			await screen.findByRole("option", { name: "Use color #06b6d4" }),
		);
		expect(red.getByText("Defined color")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
		await waitFor(() => expect(save).toHaveBeenCalled());
		const system = save.mock.calls[0][0].modes[0].color_systems[0].system;
		expect(system).toMatchObject({
			type: "discrete_wheel",
			slots: [
				{
					semantic_id: "open",
					label: "Open",
					dmx_from: 0,
					dmx_to: 9,
					measured_xyz: null,
				},
				{
					semantic_id: "deep_red",
					label: "Deep Red",
					dmx_from: 10,
					dmx_to: 19,
					measured_xyz: hexToXyz("#06b6d4"),
				},
			],
		});
	});
});

describe("per-head color systems", () => {
	it("copies a head's system to every matching head, bound to that head's own channels", () => {
		const mode = pixelMode();
		const systems = copyColorSystemToOtherHeads(mode, mode.heads[1].id);
		// The master head has no RGB channels, so it keeps having no color system.
		expect(systems.map((system) => system.head_id)).toEqual([
			mode.heads[1].id,
			mode.heads[2].id,
		]);
		const copy = systems[1].system;
		if (copy.type !== "additive") throw new Error("expected additive copy");
		expect(copy.emitters.map((emitter) => emitter.channel_id)).toEqual(
			mode.channels.slice(3).map((candidate) => candidate.id),
		);
		// The copy is independent: editing it leaves the source untouched.
		copy.emitters[0].name = "Deep red";
		const source = systems[0].system;
		expect(source.type === "additive" && source.emitters[0].name).toBe("Red");
	});

	it("configures each head of a multi-head fixture on its own", async () => {
		const profile = blankFixtureProfile();
		profile.manufacturer = "Acme";
		profile.name = "Pixels";
		profile.modes[0] = pixelMode();
		const save = vi.fn(async (draft: FixtureProfile) => draft);
		render(
			<FixtureProfileEditor
				initialProfile={profile}
				manufacturers={[]}
				onSave={save}
				onClose={vi.fn()}
			/>,
		);
		fireEvent.click(screen.getByRole("tab", { name: "Modes" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Edit channels for Default" }),
		);
		fireEvent.click(screen.getByRole("tab", { name: "Color" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Copy to other heads" }),
		);
		const sections = document.querySelectorAll(".fixture-color-editor > section");
		expect(sections).toHaveLength(3);
		const second = within(sections[2] as HTMLElement);
		expect(second.getAllByLabelText("Emitter name")).toHaveLength(3);
		fireEvent.change(second.getAllByLabelText("Emitter name")[0], {
			target: { value: "Pixel 2 red" },
		});

		fireEvent.click(screen.getByRole("button", { name: "Save fixture" }));
		await waitFor(() => expect(save).toHaveBeenCalled());
		const saved = save.mock.calls[0][0].modes[0];
		const names = saved.color_systems.map((system) =>
			system.system.type === "additive"
				? system.system.emitters.map((emitter) => emitter.name)
				: [],
		);
		expect(names).toEqual([
			["Red", "Green", "Blue"],
			["Pixel 2 red", "Green", "Blue"],
		]);
	});
});
