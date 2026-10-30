import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { PatchController } from "./controller";
import { FixtureEditDialog } from "./PatchEditSurfaces";
import { saveMultipatchEdit } from "./multipatchActions";

const probe = vi.hoisted(() => ({ controller: null as unknown as PatchController }));
vi.mock("./controller", () => ({ usePatchController: () => probe.controller }));
afterEach(cleanup);

it("the full location Set button confirms an unchanged origin and closes after persistence", async () => {
	const fixture = { fixture_id: "root", location: { x: 0, y: 0, z: 0 }, stage_position_set: false };
	let saved = { ...fixture };
	const updateFixture = vi.fn(async (_id, changes) => {
		saved = { ...saved, ...changes };
		return true;
	});
	const setEdit = vi.fn();
	probe.controller = {
		data: { selected: fixture, all: [fixture] },
		patch: { updateFixture },
		selection: { orderedFixtureIds: [] },
		ui: { edit: "location", editAxis: null, vector: { x: 0, y: 0, z: 0 },
			setEdit, setEditError: vi.fn(), setEditingSplit: vi.fn(), setPending: vi.fn(), setBlockedBy: vi.fn() },
		dispatch: vi.fn(),
	} as unknown as PatchController;
	const view = render(<FixtureEditDialog />);
	fireEvent.click(screen.getByRole("button", { name: /^Set$/ }));
	await waitFor(() => expect(setEdit).toHaveBeenCalledWith(null));
	expect(saved).toMatchObject({ location: { x: 0, y: 0, z: 0 }, stage_position_set: true });
	expect(updateFixture).toHaveBeenCalledWith("root", { location: { x: 0, y: 0, z: 0 }, stage_position_set: true });
	probe.controller.ui.edit = null;
	view.rerender(<FixtureEditDialog />);
	expect(screen.queryByText("Set fixture location")).toBeNull();
});

it("full location confirmation marks only the selected multipatch copy", async () => {
	const copy = (id: string) => ({ id, location: { x: 0, y: 0, z: 0 }, stage_position_set: false });
	const fixture = { fixture_id: "root", stage_position_set: false, multipatch: [copy("first"), copy("second")] };
	const updateFixture = vi.fn().mockResolvedValue(true);
	const setMultipatchEdit = vi.fn();
	const controller = {
		data: { all: [fixture] }, patch: { updateFixture },
		ui: { multipatchEdit: { fixtureId: "root", instanceId: "first", kind: "location" },
			vector: { x: 0, y: 0, z: 0 }, setEditError: vi.fn(), setMultipatchEdit },
	} as unknown as PatchController;
	await saveMultipatchEdit(controller);
	const [id, changes] = updateFixture.mock.calls[0];
	expect(id).toBe("root");
	expect(changes).not.toHaveProperty("stage_position_set");
	expect(changes.multipatch.map((item: { stage_position_set: boolean }) => item.stage_position_set))
		.toEqual([true, false]);
	expect(setMultipatchEdit).toHaveBeenCalledWith(null);
});
