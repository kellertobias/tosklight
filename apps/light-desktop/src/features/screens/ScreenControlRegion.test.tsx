// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useEffect, useMemo, useState } from "react";
import { PlaybackSoloEditorProvider, usePlaybackSoloEditorRegistry } from "../playbackSolo/PlaybackSoloEditorContext";
import type { ScreenConfiguration } from "../../api/types";
import { ScreenControlRegion } from "./ScreenControlRegion";
import { ScreensProvider } from "./ScreensContext";
import type { ScreensContextValue } from "./types";

const soloMocks = vi.hoisted(() => ({ saved: vi.fn(), regions: [] }));
vi.mock("../playbackSolo/PlaybackSoloContext", () => ({
	usePlaybackSolo: () => ({ authorityGeneration: 1, activate: () => {}, subscribe: () => () => {},
		getRegions: () => soloMocks.regions, isSaving: () => false, error: null }),
}));

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
vi.mock("../../components/control/CommandLineBar", async (original) => {
	const actual = await original<typeof import("../../components/control/CommandLineBar")>();
	return { CommandLineBar: () => <div data-testid="command-line" />, SoloCommandEditor: actual.SoloCommandEditor };
});
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
		ScreenPlaybackSection: ({ screen: configuration, encoders }: { screen: ScreenConfiguration; encoders?: React.ReactNode }) => {
			const registry = usePlaybackSoloEditorRegistry();
			const [selecting, setSelecting] = useState(false);
			const [name, setName] = useState("");
			const editor = useMemo(() => ({
				available: true, ready: true, selecting, selected: [{ page: 2, position: 101 }, { page: 2, position: 102 }],
				name, saving: false, error: null, setName,
				begin: () => setSelecting(true), beginEdit: () => setSelecting(true),
				save: async () => { soloMocks.saved({ name, members: [{ page: 2, position: 101 }, { page: 2, position: 102 }] }); setSelecting(false); },
				cancel: () => setSelecting(false),
			}), [selecting, name]);
			useEffect(() => registry?.register(configuration.surface_id, editor), [registry, configuration.surface_id, editor]);
			return <div data-testid="playbacks">{useLowerSectionSwitch()}{encoders}</div>;
		},
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
	surface_id: "screen:test",
	show_page_controls: false,
	show_programmer: true,

	desired_open: true,
	display_id: null,
	bounds: null,
	fullscreen: false,
} as unknown as ScreenConfiguration;

function mount(
	screenConfiguration: ScreenConfiguration,
	encoderScreenId: string | null,
) {
	const source: ScreensContextValue = {
		screens: {
			screens: [screenConfiguration],
			surfaces: {
				playback_surfaces: {},
				selected_surface_id: "main",
				resolved_pages: { main: 1 },
				command_page: 1,
			},
			programmer_control_surface: {
				owner_screen_id: encoderScreenId,
				visible_encoders: 6,
			},
		},
		bootstrap: null,
		session: null,
		saveScreen: vi.fn(),
		deleteScreen: vi.fn(),
		setSurfacePage: vi.fn(),
		surfaces: null,
		selectSurface: vi.fn(),
		updatePlaybackSurface: vi.fn(),
		updateProgrammerControlSurface: vi.fn(),
		updateControlDesk: vi.fn(),
		selectControlDesk: vi.fn(),
		removeClient: vi.fn(),
	};
	return render(
		<PlaybackSoloEditorProvider>
			<ScreensProvider source={source}>
				<ScreenControlRegion screen={screenConfiguration} />
			</ScreensProvider>
		</PlaybackSoloEditorProvider>,
	);
}

describe("ScreenControlRegion", () => {
	beforeEach(() => {
		soloMocks.saved.mockClear();
		compactMocks.hardware = false;
		compactMocks.programmerContext = true;
		compactMocks.bindings = [];
		compactMocks.full = [];
		compactMocks.miniature = [];
	});
	afterEach(() => { cleanup(); vi.useRealTimers(); });

	it("holds optional Programmer settings open through Manage, then exposes local Solo Save and Cancel", async () => {
		vi.useFakeTimers();
		mount(screenTemplate, "screen-1");
		const toggle = screen.getByRole("button", { name: "Lower section" });
		fireEvent.pointerDown(toggle);
		act(() => vi.advanceTimersByTime(650));
		fireEvent.pointerUp(toggle);
		fireEvent.click(toggle, { detail: 1 });
		expect(screen.getByTestId("encoders-only")).toBeInTheDocument();
		expect(screen.getByRole("dialog", { name: "Playback section settings" })).toBeInTheDocument();
		vi.useRealTimers();
		fireEvent.click(screen.getByRole("tab", { name: "Manage Solo Regions" }));
		await waitFor(() => expect(screen.getByTestId("playbacks")).toBeInTheDocument());
		expect(screen.getByRole("dialog")).toBeInTheDocument();
		fireEvent.click(await screen.findByRole("button", { name: "Create New Solo Region" }));
		fireEvent.change(screen.getByLabelText("Solo Region name"), { target: { value: "Local pair" } });
		fireEvent.click(screen.getByRole("button", { name: "Save Solo Region" }));
		await waitFor(() => expect(soloMocks.saved).toHaveBeenCalledExactlyOnceWith({
			name: "Local pair", members: [{ page: 2, position: 101 }, { page: 2, position: 102 }],
		}));
		expect(screen.queryByRole("button", { name: "Save Solo Region" })).not.toBeInTheDocument();
		fireEvent.pointerDown(screen.getByRole("button", { name: "Lower section" }));
		await new Promise((resolve) => setTimeout(resolve, 670));
		fireEvent.pointerUp(screen.getByRole("button", { name: "Lower section" }));
		fireEvent.click(screen.getByRole("tab", { name: "Manage Solo Regions" }));
		fireEvent.click(await screen.findByRole("button", { name: "Create New Solo Region" }));
		fireEvent.click(screen.getByRole("button", { name: /^Cancel$/ }));
		expect(screen.queryByRole("button", { name: "Save Solo Region" })).not.toBeInTheDocument();
		expect(soloMocks.saved).toHaveBeenCalledOnce();
		expect(screen.getByTestId("playbacks")).toBeInTheDocument();
	});

	it("shows only Playbacks while the encoders live elsewhere", () => {
		mount(screenTemplate, null);
		expect(screen.getByTestId("playbacks")).toBeInTheDocument();
		const draftLayout = screen.getByTestId("playbacks").parentElement!;
		expect(draftLayout.style.gridTemplateRows).toBe("auto minmax(0, 1fr)");
		expect(draftLayout.firstElementChild).toBeEmptyDOMElement();
		expect(draftLayout.children[1]).toBe(screen.getByTestId("playbacks"));
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
