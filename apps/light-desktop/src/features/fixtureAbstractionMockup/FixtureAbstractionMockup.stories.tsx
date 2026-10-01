import type { Meta, StoryObj } from "@storybook/react-vite";
import { CommandSectionFixture } from "../../../../ui-library/storybook/fixtures/controlSection";
import { ApplicationStateHarness } from "../../../../ui-library/storybook/providers/ApplicationStateHarness";
import { StoryShowObjectsProvider } from "../../../../ui-library/storybook/providers/StoryShowObjectsProvider";
import { FixtureAbstractionMockup, type FixtureAbstractionMockupProps } from "./FixtureAbstractionMockup";

function FixtureAbstractionMockupStory(props: FixtureAbstractionMockupProps) {
	return <ApplicationStateHarness actions={[{ type: "OPEN_BUILTIN", kind: "fixtures" }]}>
		<StoryShowObjectsProvider>
			<FixtureAbstractionMockup {...props} renderControl={({ hardware, programmer }) =>
				<CommandSectionFixture inheritAppState initialMode="programmer" hardware={hardware} programmer={programmer} />} />
		</StoryShowObjectsProvider>
	</ApplicationStateHarness>;
}

const meta = {
	title: "ToskLight/Design/Fixture-independent programming",
	component: FixtureAbstractionMockupStory,
	tags: ["autodocs"],
	parameters: {
		layout: "fullscreen",
		docs: { description: { component: "Interactive UI prototype inside the production shell, keypad, family tabs and four encoders. Local state and fixture results are illustrative; no live API, persisted show, DMX or PSN. Plan: docs/plans/fixture-independent-programming.md. Use the Mode toolbar for software/hardware layouts. The upper Fixture Sheet retains its existing layout. Compact Color Special has a 2D hue/saturation sheet, with a horizontal colored touch fader for White Blend above White balance / Expand. Its second page has Temperature and Duv touch faders, with white at their centers. Repeated Special presses cycle pages. Click the active family to return to encoders without changing page. Expand uses the shared modal with a large hue ring beside Saturation, White Blend, Temperature and Duv touch faders, plus per-fixture approximation below. Hardware and narrow layouts open the same modal. Shift-click the first and last picker or fader values to spread across ordered fixtures; ordinary edits clear that control’s range. Hue follows the shortest arc. Settings and example capabilities are Storybook controls; presets use the existing application pool. Position page two is Point/X/Y/Z: Origin means world coordinates and named points mean offsets. These controls stay on the encoder page; the Position modal has no Aim reference or XYZ row. Editing Angle or Target replaces the other; paging does not activate a mode. Position Special opens directly in a modal with a multi-turn pan circle (illustrative −720° to +720°), −90°/Reset/+90° actions and a regular Tilt fader stacked on the left, with a square velocity joystick on the right. Reset sets only Pan to zero. Hold the joystick away from center to keep moving; the response is gentle near the center and reaches full speed near the edges, and release stops movement. Focus Special also opens directly in a modal and lets you drag beam opening and the 0–100% focus plane. Neither editor displays a fixture model. Mount controls emulate a moving truss without altering the target. Media reuses White Blend for grayscale, with RGB tint and intensity independent. Color and Position each form one activation group. Implementation scope is Position, Color (including Media) and Focus/Zoom; the older Beam/iris/shutter examples are outside that scope." } },
	},
	args: { initialView: "easy", surface: "touch" },
	argTypes: {
		initialView: { control: false }, surface: { control: "inline-radio", options: ["touch", "hardware"] },
		colorMode: { control: "inline-radio", options: ["easy", "advanced"], description: "Operator preference; changes controls without changing the color recipe." },
		easyLayout: { control: "inline-radio", options: ["rgbw", "rgbwauv"] },
		fixtureCapability: { control: "select", options: ["mixed", "rgbw", "rgbwauv", "rgbal", "cmy", "wheel"], description: "Illustrative selected fixture capabilities." },
		mountLift: { control: { type: "range", min: -3, max: 3, step: .1 }, description: "Emulated mounting-point height offset, metres." },
		mountYaw: { control: { type: "range", min: -180, max: 180, step: 1 } },
		performerOffset: { control: { type: "range", min: -4, max: 4, step: .1 } },
	},
	render: (args, context) => <FixtureAbstractionMockupStory {...args} surface={context.globals.mode === "hardware" ? "hardware" : args.surface} />,
} satisfies Meta<typeof FixtureAbstractionMockupStory>;
export default meta;
type Story = StoryObj<typeof meta>;

export const EasyRgbw: Story = { name: "Easy RGBW" };
export const EasyRgbwauv: Story = { name: "Easy RGBWAUV", args: { initialView: "extended" } };
export const AdvancedColor: Story = { args: { initialView: "advanced" } };
export const MixedSelectionMagenta: Story = { args: { initialView: "mixed-magenta" }, parameters: { docs: { description: { story: "One magenta request across JBLED A7 RGB, ROOT PAR 6 RGBWAUV and the user-specified seven-slot AURO wheel. Two nominal continuous mixes; the wheel selects illustrative Dark blue and visibly reports an approximation. Expand shows the ring, all sliders and per-fixture results together. No physical matching is established." } } } };
export const MixedSelectionWarmWhite: Story = { args: { initialView: "mixed-warm-white" }, parameters: { docs: { description: { story: "3200 K, zero tint and 100% White Blend across the same selection: RGB synthesized white, an illustrative RGB/white/amber allocation, and the Warm White wheel slot. Similar swatches do not imply equal spectra, brightness or color rendering. This seven-slot example is not the library's nine-slot AURO SPOT Z300 profile." } } } };
export const PositionAngles: Story = { args: { initialView: "angles" } };
export const PositionFixedTarget: Story = { args: { initialView: "fixed" } };
export const PositionTrackedTarget: Story = { args: { initialView: "tracked" } };
export const FixtureConfiguration: Story = { args: { initialView: "configuration" } };
export const BeamAndShutter: Story = { args: { initialView: "beam" } };
export const MediaColor: Story = { args: { initialView: "media" } };
