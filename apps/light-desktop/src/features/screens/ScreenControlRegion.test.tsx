// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ScreenConfiguration } from "../../api/types";
import { ScreenControlRegion } from "./ScreenControlRegion";
import { ScreensProvider } from "./ScreensContext";
import type { ScreensContextValue } from "./types";

const compactMocks = vi.hoisted(() => ({ hardware: false, programmerContext: true, bindings: [] as { active: boolean; bindHardware: boolean }[], full: [] as unknown[], miniature: [] as unknown[], controller: { id: "shared-controller" } }));

vi.mock("../../components/control/parameterControls/useParameterController", () => ({
	useParameterController: (active: boolean, options: { bindHardware: boolean }) => {
		compactMocks.bindings.push({ active, bindHardware: options.bindHardware });
		return compactMocks.controller;
	},
}));
vi.mock("../../components/control/parameterControls/CompactEncoderAssignments", () => ({
	CompactEncoderAssignments: ({ controller }: { controller: unknown }) => {
		compactMocks.miniature.push(controller);
		return <div data-testid="compact-encoders" />;
	},
}));

/** Both sections host the switch where the operator's own controls already are. */
vi.mock("../../components/control/CommandLineBar", () => ({
	CommandLineBar: () => <div data-testid="command-line" />,
}));
vi.mock("../../components/control/ControlSection", () => ({
	ControlSection: () => <div data-testid="main-control-section" />,
}));

vi.mock("../../components/control/ParameterControls", async () => {
	const { useLowerSectionSwitch } = await import("./LowerSectionSwitch");
	return {
		useProgrammerEncoderContext: () => compactMocks.programmerContext,
		ParameterControls: ({ controller }: { controller: unknown }) => {
			compactMocks.full.push(controller);
			return <div data-testid="encoders-only">{useLowerSectionSwitch()}</div>;
		},
	};
});

vi.mock("./ScreenPlaybackSection", async () => {
	const { useLowerSectionSwitch } = await import("./LowerSectionSwitch");
	return {
		ScreenPlaybackSection: ({ encoders }: { encoders?: React.ReactNode }) => (
			<div data-testid="playbacks">{useLowerSectionSwitch()}{encoders}</div>
		),
	};
});

vi.mock("../deskSnapshot/DeskSnapshotState", () => ({
	useHardwareConnected: () => compactMocks.hardware,
}));

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: { midiProfile: null }, dispatch: vi.fn() }),
}));

const screenTemplate = {
	id: "screen-1",
	name: "Stage manager",
	layout: { desks: [], activeDeskId: "main" },
	content: { type: "desktop" },
	show_dock: false,
	show_playbacks: true,
	show_page_controls: false,
	show_programmer: true,
	playback_count: 8,
	playback_rows: 1,
	first_playback_slot: 1,
	page_mode: "follow_main",
	desired_open: true,
	display_id: null,
	bounds: null,
	fullscreen: false,
} as unknown as ScreenConfiguration;

function mount(screenConfiguration: ScreenConfiguration, encoderScreenId: string | null) {
	const source: ScreensContextValue = {
		screens: {
			screens: [screenConfiguration],
			active_pages: {},
			programmer_control_surface: {
				owner_screen_id: encoderScreenId,
				visible_encoders: 6,
			},
		},
		bootstrap: null,
		session: null,
		saveScreen: vi.fn(),
		deleteScreen: vi.fn(),
		setScreenPage: vi.fn(),
		updateProgrammerControlSurface: vi.fn(),
		updateControlDesk: vi.fn(),
		selectControlDesk: vi.fn(),
		removeClient: vi.fn(),
	};
	return render(
		<ScreensProvider source={source}>
			<ScreenControlRegion screen={screenConfiguration} />
		</ScreensProvider>,
	);
}

describe("ScreenControlRegion", () => {
	beforeEach(() => {
		compactMocks.hardware = false;
		compactMocks.programmerContext = true;
		compactMocks.bindings = [];
		compactMocks.full = [];
		compactMocks.miniature = [];
	});
	afterEach(cleanup);

	it("shows only Playbacks while the encoders live elsewhere", () => {
		mount(screenTemplate, null);
		expect(screen.getByTestId("playbacks")).toBeInTheDocument();
		expect(screen.queryByTestId("encoders")).toBeNull();
		expect(screen.queryByRole("button", { name: "Lower section" })).toBeNull();
	});

	it("shows only the encoders when the screen carries no Playbacks", () => {
		mount(
			{ ...screenTemplate, show_playbacks: false } as ScreenConfiguration,
			"screen-1",
		);
		expect(screen.getByTestId("encoders-only")).toBeInTheDocument();
		expect(screen.queryByTestId("playbacks")).toBeNull();
		expect(screen.queryByRole("button", { name: "Lower section" })).toBeNull();
	});

	it("keeps the keypad and the programmer tools off an optional screen", () => {
		mount(
			{
				...screenTemplate,
				show_playbacks: false,
				show_programmer: false,
			} as ScreenConfiguration,
			"screen-1",
		);
		expect(screen.getByTestId("encoders-only")).toBeInTheDocument();
		expect(screen.queryByTestId("command-line")).toBeNull();
	});

	it("adds the command line only where the screen enables it", () => {
		mount(
			{ ...screenTemplate, show_playbacks: false } as ScreenConfiguration,
			"screen-1",
		);
		expect(screen.getByTestId("command-line")).toBeInTheDocument();
		expect(screen.getByTestId("encoders-only")).toBeInTheDocument();
	});

	it("hands one switch button to whichever section is visible", () => {
		const { container } = mount(screenTemplate, "screen-1");
		const encoders = container.querySelector(".screen-encoders-only");
		const control = screen.getByRole("button", { name: "Lower section" });
		expect(encoders).toContainElement(control);
		expect(screen.queryByTestId("playbacks")).toBeNull();

		/* Both labels are always present; only the active colour moves. */
		expect(control).toHaveTextContent("Playback");
		expect(control).toHaveTextContent("Encoders");
		expect(control).toHaveAttribute("data-section", "encoders");

		fireEvent.click(control);
		const playbacks = screen.getByTestId("playbacks");
		const moved = screen.getByRole("button", { name: "Lower section" });
		expect(playbacks).toContainElement(moved);
		expect(moved).toHaveAttribute("data-section", "playbacks");
		expect(screen.queryByTestId("encoders-only")).toBeNull();

		fireEvent.click(moved);
		expect(screen.getByTestId("encoders-only")).toBeInTheDocument();
	});

	it("keeps one owner controller while switching into opted-in hardware playback readouts", () => {
		compactMocks.hardware = true;
		mount({ ...screenTemplate, show_compact_encoders: true }, "screen-1");
		expect(screen.queryByTestId("compact-encoders")).toBeNull();
		fireEvent.click(screen.getByRole("button", { name: "Lower section" }));
		expect(screen.getByTestId("compact-encoders")).toBeInTheDocument();
		expect(compactMocks.full[0]).toBe(compactMocks.miniature[0]);
		expect(compactMocks.bindings.at(-1)).toEqual({ active: true, bindHardware: true });
	});

	it("makes a nonowner compact screen a readout without extra hardware binding", () => {
		compactMocks.hardware = true;
		mount({ ...screenTemplate, show_compact_encoders: true }, null);
		expect(screen.getByTestId("compact-encoders")).toBeInTheDocument();
		expect(compactMocks.bindings.at(-1)).toEqual({ active: true, bindHardware: false });
	});

	it.each(["disabled", "software"])("suppresses compact readouts for %s", (condition) => {
		compactMocks.hardware = condition !== "software";
		mount({ ...screenTemplate, show_compact_encoders: condition !== "disabled" }, null);
		expect(screen.queryByTestId("compact-encoders")).toBeNull();
		expect(compactMocks.bindings.at(-1)?.active).toBe(false);
	});

	it("does not suppress a nonowner mirror merely because its own workspace has special controls", () => {
		compactMocks.hardware = true;
		compactMocks.programmerContext = false;
		mount({ ...screenTemplate, show_compact_encoders: true }, null);
		expect(screen.getByTestId("compact-encoders")).toBeInTheDocument();
		expect(compactMocks.bindings.at(-1)).toEqual({ active: true, bindHardware: false });
	});
});
