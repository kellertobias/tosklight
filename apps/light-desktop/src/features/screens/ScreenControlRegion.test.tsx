// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ScreenConfiguration } from "../../api/types";
import { ScreenControlRegion } from "./ScreenControlRegion";
import { ScreensProvider } from "./ScreensContext";
import type { ScreensContextValue } from "./types";

vi.mock("../../components/control/ControlSection", () => ({
	ControlSection: () => <div data-testid="encoders" />,
}));

vi.mock("./ScreenPlaybackSection", () => ({
	ScreenPlaybackSection: () => <div data-testid="playbacks" />,
}));

const screenTemplate = {
	id: "screen-1",
	name: "Stage manager",
	layout: { desks: [], activeDeskId: "main" },
	content: { type: "desktop" },
	show_dock: false,
	show_playbacks: true,
	show_page_controls: false,
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
		expect(screen.queryByRole("group", { name: "Lower section" })).toBeNull();
	});

	it("shows only the encoders when the screen carries no Playbacks", () => {
		mount(
			{ ...screenTemplate, show_playbacks: false } as ScreenConfiguration,
			"screen-1",
		);
		expect(screen.getByTestId("encoders")).toBeInTheDocument();
		expect(screen.queryByTestId("playbacks")).toBeNull();
		expect(screen.queryByRole("group", { name: "Lower section" })).toBeNull();
	});

	it("switches between Playback and Encoders when the screen carries both", () => {
		mount(screenTemplate, "screen-1");
		expect(screen.getByRole("group", { name: "Lower section" })).toBeVisible();
		expect(screen.getByTestId("encoders")).toBeInTheDocument();
		expect(screen.queryByTestId("playbacks")).toBeNull();

		fireEvent.click(screen.getByRole("button", { name: "Playback" }));
		expect(screen.getByTestId("playbacks")).toBeInTheDocument();
		expect(screen.queryByTestId("encoders")).toBeNull();

		fireEvent.click(screen.getByRole("button", { name: "Encoders" }));
		expect(screen.getByTestId("encoders")).toBeInTheDocument();
	});
});
