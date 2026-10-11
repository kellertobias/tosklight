// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { ControlSection } from "./ControlSection";

const state = vi.hoisted(() => ({ mode: "playbacks", hardware: false }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state: { controlMode: state.mode, midiProfile: null } }) }));
vi.mock("../../features/screens/encoderPlacement", () => ({ useEncoderPlacement: () => ({ holdsEncoders: false, encoderScreen: { name: "Screen 1" } }) }));
vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useHardwareConnected: () => state.hardware,
	useSessionSnapshot: () => ({ desk: { surface_id: "main" } }),
}));
vi.mock("./ControlSurfaceMode", () => ({ useControlSurfacePolicy: () => null }));
vi.mock("./parameterControls/VisibleEncoderCount", () => ({
	useVisibleEncoderCount: () => 6,
	resolveVisibleEncoderCount: () => 6,
	VisibleEncoderCountProvider: ({ children }: { children: ReactNode }) => children,
}));
vi.mock("./PlaybackFaderBank", () => ({ PlaybackFaderBank: (props: { scrollable?: boolean; surfaceId: string }) => <div data-testid="bank" data-scrollable={String(Boolean(props.scrollable))} data-surface={props.surfaceId} /> }));
vi.mock("./CommandLineBar", () => ({ CommandLineBar: () => <div /> }));
vi.mock("./HardwareControlSummary", () => ({ HardwareControlSummary: () => <div /> }));
vi.mock("./NumericPad", () => ({ NumericPad: () => <div /> }));
vi.mock("./ParameterControls", () => ({ ParameterControls: () => <div /> }));
vi.mock("./PatchParameterControls", () => ({ PatchParameterControls: () => <div /> }));
vi.mock("./PlaybackTools", () => ({ PlaybackTools: () => <div /> }));

afterEach(cleanup);
it.each([false, true])("fits the Main bank without pane scrolling in hardware=%s", (hardware) => {
	state.hardware = hardware;
	for (const mode of ["playbacks", "programmer"]) {
		state.mode = mode;
		const { container, unmount } = render(<ControlSection />);
		expect(screen.getByTestId("bank")).toHaveAttribute("data-scrollable", "false");
		expect(screen.getByTestId("bank")).toHaveAttribute("data-surface", "main");
		expect(container.querySelector(".screen-bank-fit")).not.toBeNull();
		expect(container.querySelector(".screen-playback-fit")).not.toBeNull();
		unmount();
	}
});
