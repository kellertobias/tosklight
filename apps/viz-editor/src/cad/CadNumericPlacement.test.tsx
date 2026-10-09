import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { CadEntity } from "./types";
import { CadInfoPanel } from "./CadInfoPanel";
import { SeveralPlacement } from "./CadInfoSeveral";
const mocks = vi.hoisted(() => ({
	snapshot: vi.fn(),
	patch: vi.fn(),
	numeric: vi.fn().mockResolvedValue({}),
}));
vi.mock("../document/session", () => ({
	documentSession: {
		patchSnapshot: mocks.snapshot,
		fixtureNotes: async () => [],
		fixtureProfileUpdate: async () => null,
	},
}));
vi.mock("../document/transport", () => ({
	TauriPatchTransport: class {
		patchFixtures = mocks.patch;
	},
}));
vi.mock("./session", () => ({
	cadSession: { setNumericTransforms: mocks.numeric },
}));
afterEach(() => {
	cleanup();
	vi.clearAllMocks();
});
const id = "11111111-1111-4111-8111-111111111111";
const copy = "22222222-2222-4222-8222-222222222222";
function fixture(fixtureId = id) {
	return {
		fixtureId,
		name: "Back Truss Segment1",
		profileId: "profile",
		profileRevision: 1,
		location: { x: -3000, y: 4000, z: 4150 },
		rotation: { x: 0, y: 0, z: 0 },
		multipatch: [
			{
				id: copy,
				name: "Copy",
				location: { x: 5000, y: 6000, z: 7000 },
				rotation: { x: 0, y: 0, z: 10 },
				splitPatches: [],
			},
		],
		splitPatches: [],
	};
}
const entity = {
	id,
	logicalFixtureId: id,
	name: "Back Truss Segment1",
	kind: "venue",
	positionMillimetres: [-3000, 4000, 4150],
	rotationDegrees: [0, 0, 0],
} as CadEntity;
function commit(label: string, value: string) {
	const input = screen.getByLabelText(label);
	fireEvent.change(input, { target: { value } });
	fireEvent.keyDown(input, { key: "Enter" });
	return input;
}
it("literal single Position Enter records CAD history instead of a generic patch", async () => {
	mocks.snapshot.mockResolvedValue({
		fixtures: [fixture()],
		profileRevisions: [],
	});
	render(<CadInfoPanel entity={entity} selectionCount={1} sceneRevision={9} tab="placement" onError={vi.fn()} />);
	await waitFor(() => expect(screen.getByLabelText("Position X")).toHaveValue("-3"));
	const input = commit("Position X", "-2.5");
	await waitFor(() =>
		expect(mocks.numeric).toHaveBeenCalledWith(9, [
			{
				id,
				positionMillimetres: [-2500, 4000, 4150],
				rotationDegrees: [0, 0, 0],
			},
		]),
	);
	fireEvent.blur(input);
	expect(mocks.numeric).toHaveBeenCalledTimes(1);
	expect(mocks.patch).not.toHaveBeenCalled();
});
it("numeric copy Rotation addresses just the selected physical copy", async () => {
	mocks.snapshot.mockResolvedValue({
		fixtures: [fixture()],
		profileRevisions: [],
	});
	render(
		<CadInfoPanel
			entity={{ ...entity, id: copy }}
			selectionCount={1}
			sceneRevision={9}
			tab="placement"
			onError={vi.fn()}
		/>,
	);
	await waitFor(() => expect(screen.getByLabelText("Rotation Z")).toHaveValue("10"));
	commit("Rotation Z", "30");
	await waitFor(() =>
		expect(mocks.numeric).toHaveBeenCalledWith(9, [
			{
				id: copy,
				positionMillimetres: [5000, 6000, 7000],
				rotationDegrees: [0, 0, 30],
			},
		]),
	);
	expect(mocks.patch).not.toHaveBeenCalled();
});
it("multi-selection Enter spreads poses in selected order as one history step", async () => {
	const second = "33333333-3333-4333-8333-333333333333";
	mocks.snapshot.mockResolvedValue({
		fixtures: [fixture(), fixture(second)],
		profileRevisions: [],
	});
	render(
		<SeveralPlacement
			elements={
				[
					{ id: second, isFixture: true },
					{ id, isFixture: true },
				] as never
			}
			sceneRevision={9}
			onError={vi.fn()}
		/>,
	);
	await waitFor(() => expect(screen.getByLabelText("Position X")).toHaveValue("-3"));
	commit("Position X", "1 THRU 2");
	await waitFor(() =>
		expect(mocks.numeric).toHaveBeenCalledWith(9, [
			{
				id: second,
				positionMillimetres: [1000, 4000, 4150],
				rotationDegrees: [0, 0, 0],
			},
			{
				id,
				positionMillimetres: [2000, 4000, 4150],
				rotationDegrees: [0, 0, 0],
			},
		]),
	);
	expect(mocks.patch).not.toHaveBeenCalled();
});
