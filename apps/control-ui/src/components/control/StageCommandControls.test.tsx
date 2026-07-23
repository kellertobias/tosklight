import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { StageLayoutActionRequest } from "../../api/stageLayoutTypes";
import { StageLayoutActionsProvider } from "../../features/stageLayout/StageLayoutActionsProvider";
import { StageLayoutStore } from "../../features/stageLayout/store";
import { StageCommandControls } from "./StageCommandControls";

const mocks = vi.hoisted(() => ({
	selected: ["fixture-b", "fixture-a"],
	positions3d: {} as Record<
		string,
		{ x: number; y: number; z: number; rotationX: number; rotationY: number; rotationZ: number }
	>,
	faderValue: 0,
	appState: {
		stageMode: "position",
		stageView: "2d",
		stageZoom: 1,
		stagePanX: 0,
		stagePanY: 0,
		stageOrbitX: 0,
		stageOrbitY: 0,
		midiProfile: null,
	},
	fixtures: [{ fixture_id: "fixture-a" }, { fixture_id: "fixture-b" }],
}));

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: mocks.appState, dispatch: vi.fn() }),
}));
vi.mock("../../features/patch/PatchState", () => ({
	usePatchedFixturesView: () => mocks.fixtures,
}));
vi.mock("../../features/stageLayout/StageLayoutState", () => ({
	useStagePositions: () => ({}),
	useStagePositions3d: () => mocks.positions3d,
}));
vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useHardwareConnected: () => false,
}));
vi.mock("../../windows/stageWindow/useStageSelection", () => ({
	useStageSelection: () => ({ fixtureIds: mocks.selected }),
}));
vi.mock("../../windows/stage3dScene", () => ({
	migrateStagePosition: () => ({ x: 0, y: 0, z: 0, rotationX: 0, rotationY: 0, rotationZ: 0 }),
}));
vi.mock("./VerticalTouchFader", () => ({
	VerticalTouchFader: ({
		label,
		onChange,
	}: {
		label: string;
		onChange?: (value: number) => void;
	}) => (
		<button type="button" onClick={() => onChange?.(mocks.faderValue)}>
			{label}
		</button>
	),
}));

function position(x: number) {
	return { x, y: 0, z: 0, rotationX: 0, rotationY: 0, rotationZ: 0 };
}

function renderControls() {
	const moveStageSelection = vi.fn(async (request: StageLayoutActionRequest) => ({
		request_id: request.request_id,
		revision: 2,
		moved_fixture_ids: request.action.fixture_ids,
		replayed: false,
		changed: true,
	}));
	const putStageLayout = vi.fn(async () => undefined);
	const readStageLayout = vi.fn(async () => null);
	render(
		<StageLayoutActionsProvider
			store={new StageLayoutStore()}
			showId="show-1"
			putStageLayout={putStageLayout}
			moveStageSelection={moveStageSelection}
			readStageLayout={readStageLayout}
			onApplied={vi.fn()}
			onError={vi.fn()}
		>
			<StageCommandControls />
		</StageLayoutActionsProvider>,
	);
	return { moveStageSelection, putStageLayout };
}

describe("stage command position controls", () => {
	afterEach(() => {
		cleanup();
		mocks.selected = ["fixture-b", "fixture-a"];
		mocks.positions3d = {};
	});

	it("sends one move-selection intent with the ordered selection instead of a layout save", () => {
		mocks.positions3d = {
			"fixture-a": position(4),
			"fixture-b": position(1),
		};
		mocks.faderValue = 13;
		const harness = renderControls();

		fireEvent.click(screen.getByRole("button", { name: "X Position" }));

		expect(harness.moveStageSelection).toHaveBeenCalledTimes(1);
		expect(harness.moveStageSelection).toHaveBeenCalledWith({
			request_id: expect.stringMatching(/[0-9a-f-]{36}/),
			action: {
				type: "move_selection",
				fixture_ids: ["fixture-b", "fixture-a"],
				axis: "x",
				delta: 2,
			},
		});
		expect(harness.putStageLayout).not.toHaveBeenCalled();
	});

	it("sends nothing when the value matches the anchor position", () => {
		mocks.positions3d = {
			"fixture-a": position(4),
			"fixture-b": position(1),
		};
		mocks.faderValue = 11;
		const harness = renderControls();

		fireEvent.click(screen.getByRole("button", { name: "X Position" }));

		expect(harness.moveStageSelection).not.toHaveBeenCalled();
		expect(harness.putStageLayout).not.toHaveBeenCalled();
	});

	it("sends nothing without an anchor position", () => {
		mocks.selected = ["fixture-missing"];
		mocks.faderValue = 13;
		const harness = renderControls();

		fireEvent.click(screen.getByRole("button", { name: "X Position" }));

		expect(harness.moveStageSelection).not.toHaveBeenCalled();
		expect(harness.putStageLayout).not.toHaveBeenCalled();
	});
});
