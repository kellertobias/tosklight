// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import { ControlSection } from "./ControlSection";

const mocks = vi.hoisted(() => ({
	hardware: true,
	enabled: true,
	programmerContext: true,
	owner: true,
	state: { controlMode: "playbacks", builtIn: null as string | null, midiProfile: null },
	controller: { dynamicsMode: false, encoderContext: "programmer" },
	bindings: [] as { active: boolean; bindHardware: boolean }[],
	full: [] as unknown[],
	miniature: [] as unknown[],
}));

vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state: mocks.state }) }));
vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useHardwareConnected: () => mocks.hardware,
	useSessionSnapshot: () => ({ desk: { show_compact_encoders: mocks.enabled } }),
}));
vi.mock("../../features/screens/encoderPlacement", () => ({
	useEncoderPlacement: () => ({ holdsEncoders: mocks.owner, encoderScreen: mocks.owner ? null : { id: "other-screen" } }),
}));
vi.mock("./ControlSurfaceMode", () => ({ useControlSurfacePolicy: () => null }));
vi.mock("./parameterControls/useParameterController", () => ({
	useParameterController: (active: boolean, options: { bindHardware: boolean; ownerContext: string }) => {
		mocks.bindings.push({ active, bindHardware: options.bindHardware });
		if (options.bindHardware) mocks.controller.encoderContext = options.ownerContext;
		return mocks.controller;
	},
}));
vi.mock("./ParameterControls", () => ({
	useProgrammerEncoderContext: () => mocks.programmerContext,
	ParameterControls: ({ controller }: { controller: unknown }) => { mocks.full.push(controller); return <div data-testid="full-encoders" />; },
}));
vi.mock("./parameterControls/CompactEncoderAssignments", () => ({
	CompactEncoderAssignments: ({ controller }: { controller: unknown }) => { mocks.miniature.push(controller); return <div data-testid="mini-encoders" />; },
}));
vi.mock("./HardwareControlSummary", () => ({
	HardwareControlSummary: ({ encoders }: { encoders?: ReactNode }) => <div data-testid="hardware-summary">{encoders}</div>,
}));
vi.mock("./PlaybackFaderBank", () => ({ PlaybackFaderBank: () => <div data-testid="playbacks" /> }));
vi.mock("./CommandLineBar", () => ({ CommandLineBar: () => <div /> }));
vi.mock("./NumericPad", () => ({ NumericPad: () => <div data-testid="numeric-pad" /> }));
vi.mock("./PlaybackTools", () => ({ PlaybackTools: () => <div data-testid="playback-tools" /> }));
vi.mock("./PatchParameterControls", () => ({ PatchParameterControls: () => <div data-testid="patch-controls" /> }));

beforeEach(() => {
	mocks.hardware = true;
	mocks.enabled = true;
	mocks.owner = true;
	mocks.programmerContext = true;
	mocks.state.controlMode = "playbacks";
	mocks.state.builtIn = null;
	mocks.controller.dynamicsMode = false;
	mocks.controller.encoderContext = "programmer";
	mocks.bindings = [];
	mocks.full = [];
	mocks.miniature = [];
});
afterEach(cleanup);

describe("main hardware compact encoder integration", () => {
	it("shows the miniature with active Playbacks", () => {
		render(<ControlSection />);
		expect(screen.getByTestId("playbacks")).toBeInTheDocument();
		expect(screen.getByTestId("mini-encoders")).toBeInTheDocument();
		expect(screen.queryByTestId("full-encoders")).toBeNull();
		expect(mocks.bindings.at(-1)).toEqual({ active: true, bindHardware: true });
	});

	it("hides the miniature with full encoders and preserves the shared controller", () => {
		mocks.state.controlMode = "programmer";
		const view = render(<ControlSection />);
		expect(screen.getByTestId("full-encoders")).toBeInTheDocument();
		expect(screen.queryByTestId("mini-encoders")).toBeNull();
		mocks.state.controlMode = "playbacks";
		view.rerender(<ControlSection />);
		expect(screen.getByTestId("mini-encoders")).toBeInTheDocument();
		expect(mocks.full[0]).toBe(mocks.miniature[0]);
		mocks.state.controlMode = "programmer";
		view.rerender(<ControlSection />);
		expect(mocks.full.at(-1)).toBe(mocks.miniature[0]);
	});

	it.each(["disabled", "software"])("does not show the miniature for %s", (condition) => {
		mocks.enabled = condition !== "disabled";
		mocks.hardware = condition !== "software";
		render(<ControlSection />);
		expect(screen.queryByTestId("mini-encoders")).toBeNull();
		expect(screen.getByTestId("playbacks")).toBeInTheDocument();
	});

	it("disables programmer bindings and passes the special owner context to the miniature", () => {
		mocks.programmerContext = false;
		render(<ControlSection />);
		expect(screen.getByTestId("mini-encoders")).toBeInTheDocument();
		expect(mocks.controller.encoderContext).toBe("stage");
		expect(mocks.bindings.at(-1)).toEqual({ active: false, bindHardware: true });
	});

	it("mirrors an encoder owner on another screen without binding hardware twice", () => {
		mocks.owner = false;
		mocks.state.controlMode = "programmer";
		render(<ControlSection />);
		expect(screen.getByTestId("playbacks")).toBeInTheDocument();
		expect(screen.getByTestId("mini-encoders")).toBeInTheDocument();
		expect(mocks.bindings.at(-1)).toEqual({ active: true, bindHardware: false });
	});

	it("keeps the miniature mounted in Dynamics so family controls remain available", () => {
		mocks.controller.dynamicsMode = true;
		render(<ControlSection />);
		expect(screen.getByTestId("mini-encoders")).toBeInTheDocument();
		expect(mocks.miniature[0]).toMatchObject({ dynamicsMode: true });
	});

	it("does not use a nonowner's local Stage context to hide the owner's assignments", () => {
		mocks.owner = false;
		mocks.programmerContext = false;
		render(<ControlSection />);
		expect(screen.getByTestId("mini-encoders")).toBeInTheDocument();
		expect(mocks.bindings.at(-1)).toEqual({ active: true, bindHardware: false });
	});
});
