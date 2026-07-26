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
	it("opens the running menu for hardware Shift Clear", () => {
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

		expect(screen.getByText("running-open")).toBeInTheDocument();
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
