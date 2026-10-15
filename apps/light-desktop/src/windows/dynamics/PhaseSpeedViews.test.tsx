// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DynamicPhaseOrderingProjection } from "../../api/types";
import type { ShowObject } from "../../features/showObjects/contracts";
import { createDefaultDynamicDefinition } from "./DynamicsEditor";
import { AppProvider } from "../../state/AppContext";
import { DynamicEncoderDeck } from "./DynamicEncoderDeck";
import { PhaseView } from "./PhaseSpeedViews";

vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useHardwareConnected: () => true,
}));

beforeEach(() => {
	const entries = new Map<string, string>();
	vi.stubGlobal("localStorage", {
		getItem: (key: string) => entries.get(key) ?? null,
		setItem: (key: string, value: string) => { entries.set(key, value); },
		removeItem: (key: string) => { entries.delete(key); },
		clear: () => entries.clear(),
	});
});
afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});
type DynamicObject = ShowObject<"dynamic">;
function object(ordering: DynamicPhaseOrderingProjection): DynamicObject {
	const body = createDefaultDynamicDefinition(149, "intensity");
	body.target_binding = { type: "live_group", group_id: "25" };
	body.spatial_mapping = { projection: { type: "inherit" }, shape: { type: "inherit" } };
	body.phase = { ...body.phase, ordering };
	return { kind: "dynamic" as const, id: body.id, revision: 7, updated_at: "", body };
}
function mounted(dynamic: DynamicObject, mutate = vi.fn()) {
	return { ...render(<PhaseView dynamic={dynamic} lane={dynamic.body.lanes[0]} running={false}
		selectionCount={0} onSelectLane={vi.fn()} onTakeSelection={vi.fn()}
		onClearSelection={vi.fn()} onMutate={mutate} />), mutate };
}
const choice = (name: string) => within(screen.getByRole("radiogroup", { name: "Ordering mode" })).getByRole("radio", { name: new RegExp(`^${name}$`) });

describe("authoritative Dynamic Phase ordering", () => {
	it("shows legacy Grid as Grid even with saved live Group inheritance defaults, without writing", () => {
		const { mutate } = mounted(object({ type: "grid_linear", angle_degrees: 45 }));
		expect(choice("Grid")).toHaveAttribute("aria-checked", "true");
		expect(screen.getByLabelText("Direction")).toHaveValue("45");
		expect(mutate).not.toHaveBeenCalled();
	});
	it("Linear emits one explicit source-order phase edit and never changes projection", () => {
		const dynamic = object({ type: "grid_linear", angle_degrees: 45 });
		const { mutate } = mounted(dynamic);
		fireEvent.click(choice("Linear"));
		expect(mutate).toHaveBeenCalledExactlyOnceWith(dynamic, {
			type: "set_phase", phase: { ...dynamic.body.phase, ordering: { type: "source_order" } },
		});
	});
	it("Inherit emits a phase-local inherit shape, leaving the global projection untouched", () => {
		const dynamic = object({ type: "grid_linear", angle_degrees: 45 });
		const { mutate } = mounted(dynamic);
		fireEvent.click(choice("Inherit"));
		expect(mutate).toHaveBeenCalledExactlyOnceWith(dynamic, {
			type: "set_phase", phase: { ...dynamic.body.phase, ordering: { type: "spatial", shape: { type: "inherit" } } },
		});
	});
	it("offers no Inherit for frozen or targetless bindings", () => {
		const dynamic = object({ type: "selection" });
		dynamic.body.target_binding = { type: "frozen_targets", targets: ["fixture-1"] };
		const view = mounted(dynamic);
		expect(screen.queryByRole("radio", { name: /^Inherit$/ })).not.toBeInTheDocument();
		view.unmount();
		dynamic.body.target_binding = { type: "targetless" };
		mounted(dynamic);
		expect(screen.queryByRole("radio", { name: /^Inherit$/ })).not.toBeInTheDocument();
	});
	it("per-lane ordering edits only the selected lane with one request", () => {
		const dynamic = object({ type: "grid_linear", angle_degrees: 45 });
		dynamic.body.phase_mode = "per_lane";
		const lane = dynamic.body.lanes[0];
		lane.phase = { ...dynamic.body.phase, offset_degrees: 90 };
		const { mutate } = mounted(dynamic);
		fireEvent.click(choice("Linear"));
		expect(mutate).toHaveBeenCalledExactlyOnceWith(dynamic, {
			type: "replace_lane", lane_id: lane.id,
			lane: { ...lane, phase: { ...lane.phase, ordering: { type: "source_order" } } },
		});
	});
	it("adopts refreshed legacy shape authority without writing", () => {
		const dynamic = object({ type: "grid_linear", angle_degrees: 45 });
		const view = mounted(dynamic);
		const next = object({ type: "radial_in", center_x: -2, center_z: 3 });
		view.rerender(<PhaseView dynamic={next} lane={next.body.lanes[0]} running={false}
			selectionCount={0} onSelectLane={vi.fn()} onTakeSelection={vi.fn()}
			onClearSelection={vi.fn()} onMutate={view.mutate} />);
		expect(choice("Radial")).toHaveAttribute("aria-checked", "true");
		expect(view.mutate).not.toHaveBeenCalled();
	});
});

it("shows legacy axial parameters and preserves its sweep when editing the centre", () => {
	const dynamic = object({ type: "axial", center_x: -2, center_z: 3 });
	const { mutate } = mounted(dynamic);
	expect(screen.getByText("Start angle −180° · Counter-clockwise")).toBeInTheDocument();
	expect(screen.queryByLabelText("Start angle")).toBeNull();
	fireEvent.change(screen.getByLabelText("Centre U"), { target: { value: "4" } });
	expect(mutate).toHaveBeenLastCalledWith(dynamic, {
		type: "set_phase",
		phase: { ...dynamic.body.phase, ordering: { type: "axial", center_x: 4, center_z: 3 } },
	});
});

it("ignores hardware Phase encoder input when per-lane editing has no selected lane", () => {
	const dynamic = object({ type: "source_order" });
	dynamic.body.phase_mode = "per_lane";
	const mutate = vi.fn(async () => undefined);
	const laneChange = vi.fn(async () => undefined);
	render(<AppProvider><DynamicEncoderDeck view="phase" dynamic={dynamic.body}
		onMutate={mutate} onLaneChange={laneChange} /></AppProvider>);
	fireEvent(window, new CustomEvent("light:encoder-action", {
		detail: { control: "encoder/1", value: "up" },
	}));
	expect(mutate).not.toHaveBeenCalled();
	expect(laneChange).not.toHaveBeenCalled();
});

it("preserves legacy Grid X/Z ordering through a visible angle edit", () => {
	const dynamic = object({ type: "grid_linear", angle_degrees: 45 });
	const { mutate } = mounted(dynamic);
	fireEvent.change(screen.getByLabelText("Direction"), { target: { value: "90" } });
	expect(mutate).toHaveBeenLastCalledWith(dynamic, {
		type: "set_phase", phase: { ...dynamic.body.phase,
			ordering: { type: "grid_linear", angle_degrees: 90 } },
	});
});

it("preserves legacy Grid ordering through the hardware direction encoder", () => {
	const dynamic = object({ type: "grid_linear", angle_degrees: 45 });
	const mutate = vi.fn(async () => undefined);
	render(<AppProvider><DynamicEncoderDeck view="phase" dynamic={dynamic.body}
		lane={dynamic.body.lanes[0]} onMutate={mutate} onLaneChange={vi.fn(async () => undefined)} /></AppProvider>);
	fireEvent(window, new CustomEvent("light:encoder-action", {
		detail: { control: "encoder/6", value: "up" },
	}));
	expect(mutate).toHaveBeenCalledWith({ type: "set_phase",
		phase: { ...dynamic.body.phase, ordering: { type: "grid_linear", angle_degrees: 50 } },
	}, expect.any(String));
});
