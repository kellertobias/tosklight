// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ScreenConfiguration } from "../../api/types";
import { ScreenControlRegion } from "./ScreenControlRegion";
import { ScreensProvider } from "./ScreensContext";
import type { ScreensContextValue } from "./types";

/** Both sections host the switch where the operator's own controls already are. */
vi.mock("../../components/control/ControlSection", async () => {
	const { useLowerSectionSwitch } = await import("./LowerSectionSwitch");
	return {
		ControlSection: () => (
			<div data-testid="encoders">{useLowerSectionSwitch()}</div>
		),
	};
});

vi.mock("../../components/control/ParameterControls", async () => {
	const { useLowerSectionSwitch } = await import("./LowerSectionSwitch");
	return {
		ParameterControls: () => (
			<div data-testid="encoders-only">{useLowerSectionSwitch()}</div>
		),
	};
});

vi.mock("./ScreenPlaybackSection", async () => {
	const { useLowerSectionSwitch } = await import("./LowerSectionSwitch");
	return {
		ScreenPlaybackSection: () => (
			<div data-testid="playbacks">{useLowerSectionSwitch()}</div>
		),
	};
});

vi.mock("../deskSnapshot/DeskSnapshotState", () => ({
	useHardwareConnected: () => false,
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
		expect(screen.getByTestId("encoders")).toBeInTheDocument();
		expect(screen.queryByTestId("playbacks")).toBeNull();
		expect(screen.queryByRole("button", { name: "Lower section" })).toBeNull();
	});

	it("drops the programmer surface while the screen shows the encoders alone", () => {
		mount(
			{
				...screenTemplate,
				show_playbacks: false,
				show_programmer: false,
			} as ScreenConfiguration,
			"screen-1",
		);
		expect(screen.getByTestId("encoders-only")).toBeInTheDocument();
		expect(screen.queryByTestId("encoders")).toBeNull();
	});

	it("hands one switch button to whichever section is visible", () => {
		mount(screenTemplate, "screen-1");
		const encoders = screen.getByTestId("encoders");
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
		expect(screen.queryByTestId("encoders")).toBeNull();

		fireEvent.click(moved);
		expect(screen.getByTestId("encoders")).toBeInTheDocument();
	});
});
