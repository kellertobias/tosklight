import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
} from "@testing-library/react";
import { Button } from "@tosklight/ui";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { routeControlSurfaceIntent } from "../features/controlSurfaceInteraction/registry";
import { useControlSurfaceTarget } from "../features/controlSurfaceInteraction/useControlSurfaceTarget";
import { AppProvider, useApp } from "./AppContext";

function ModalState() {
	const { state } = useApp();
	return (
		<>
			<span>
				{state.systemControlsOpen ? "running-open" : "running-closed"}
			</span>
			<span>built-in-{state.builtIn ?? "none"}</span>
			<span>{state.shiftArmed ? "shift-held" : "shift-released"}</span>
			<span>control-{state.controlMode}</span>
		</>
	);
}

function PatchState() {
	const { state, dispatch } = useApp();
	return (
		<>
			<Button onClick={() => dispatch({ type: "OPEN_BUILTIN", kind: "patch" })}>
				Open Patch
			</Button>
			<span>{state.patchSetArmed ? "patch-set-armed" : "patch-set-idle"}</span>
		</>
	);
}

function ControlModeState() {
	const { state, dispatch } = useApp();
	return (
		<>
			<Button onClick={() => dispatch({ type: "TOGGLE_CONTROL_MODE" })}>
				Toggle control mode
			</Button>
			<span>
				{state.playbackSetArmed ? "playback-set-armed" : "playback-set-idle"}
			</span>
			<span>
				{state.presetSetArmed ? "preset-set-armed" : "preset-set-idle"}
			</span>
		</>
	);
}

function CompactSetTarget() {
	useControlSurfaceTarget({
		id: "compact-cue-properties",
		priority: 300,
		accepts: ({ type }) => type === "set",
		handle: () => undefined,
	});
	return <section>Compact Cue settings</section>;
}

const values = new Map<string, string>();
beforeEach(() => {
	values.clear();
	vi.stubGlobal("localStorage", {
		getItem: (key: string) => values.get(key) ?? null,
		setItem: (key: string, value: string) => values.set(key, value),
		removeItem: (key: string) => values.delete(key),
		clear: () => values.clear(),
	});
});
afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

describe("desk shortcuts", () => {
	it("routes canonical extension desk commands to the normal application surfaces", () => {
		render(
			<AppProvider>
				<ModalState />
			</AppProvider>,
		);
		act(() =>
			routeControlSurfaceIntent({
				type: "desk_command",
				source: "hardware",
				command: "stage",
			}),
		);
		expect(screen.getByText("built-in-stage")).toBeInTheDocument();
		act(() =>
			routeControlSurfaceIntent({
				type: "desk_command",
				source: "hardware",
				command: "playbacks",
			}),
		);
		expect(screen.getByText("control-playbacks")).toBeInTheDocument();
	});

	it("keeps hardware Shift Clear available for the Programmer Freeze authority", () => {
		render(
			<AppProvider>
				<ModalState />
			</AppProvider>,
		);
		expect(screen.getByText("running-closed")).toBeInTheDocument();

		act(() =>
			routeControlSurfaceIntent({
				type: "desk_shortcut",
				source: "hardware",
				action: "shift_clear",
			}),
		);

		expect(screen.getByText("running-closed")).toBeInTheDocument();
	});

	it("keeps hardware Shift 0 unassigned while retaining the operator Help shortcut", () => {
		render(
			<AppProvider>
				<ModalState />
			</AppProvider>,
		);

		act(() =>
			routeControlSurfaceIntent({
				type: "desk_shortcut",
				source: "hardware",
				action: "shift_0",
			}),
		);
		expect(screen.getByText("built-in-none")).toBeInTheDocument();

		act(() =>
			routeControlSurfaceIntent({
				type: "desk_shortcut",
				source: "hardware",
				action: "shift_9",
			}),
		);
		expect(screen.getByText("built-in-help")).toBeInTheDocument();
	});

	it("tracks attached-hardware Shift press and release for pointer gestures", () => {
		render(
			<AppProvider>
				<ModalState />
			</AppProvider>,
		);
		act(() =>
			routeControlSurfaceIntent({
				type: "desk_shortcut",
				source: "hardware",
				action: "shift_down",
			}),
		);
		expect(screen.getByText("shift-held")).toBeInTheDocument();
		act(() =>
			routeControlSurfaceIntent({
				type: "desk_shortcut",
				source: "hardware",
				action: "shift_up",
			}),
		);
		expect(screen.getByText("shift-released")).toBeInTheDocument();
	});

	it("routes attached-hardware SET into the selected Patch surface", () => {
		render(
			<AppProvider>
				<PatchState />
			</AppProvider>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Open Patch" }));

		act(() => routeControlSurfaceIntent({ type: "set", source: "hardware" }));

		expect(screen.getByText("patch-set-armed")).toBeInTheDocument();
	});

	it("leaves Playback-mode SET ownership to the scoped interaction owner", () => {
		render(
			<AppProvider>
				<ControlModeState />
			</AppProvider>,
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Toggle control mode" }),
		);

		const outcomes: ReturnType<typeof routeControlSurfaceIntent>[] = [];
		act(() => {
			outcomes.push(
				routeControlSurfaceIntent({ type: "set", source: "touch" }),
			);
		});

		expect(outcomes).toEqual([{ status: "missing" }]);
		expect(screen.getByText("playback-set-idle")).toBeInTheDocument();
		expect(screen.getByText("preset-set-idle")).toBeInTheDocument();
	});

	it("reserves attached-hardware SET for the constrained Cue settings editor", () => {
		render(
			<AppProvider>
				<PatchState />
				<CompactSetTarget />
			</AppProvider>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Open Patch" }));

		act(() => routeControlSurfaceIntent({ type: "set", source: "hardware" }));

		expect(screen.getByText("patch-set-idle")).toBeInTheDocument();
	});
});
