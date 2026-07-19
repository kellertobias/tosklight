import type { Dispatch, SetStateAction } from "react";
import type { StagePosition3d } from "../../api/ServerContext";
import type { VisualizationSnapshot } from "../../api/types";
import { NumberField } from "../../components/common";
import { Stage3dCanvas } from "../Stage3dCanvas";
import type { Stage3dFixture } from "../stage3dScene";
import type {
	StageLayoutModel,
	StageOptionsModel,
	StageWindowProps,
} from "./types";
import type { StageSelectionModel } from "./useStageSelection";

function Stage3dInspector({
	fixture,
	layout,
}: {
	fixture: Stage3dFixture;
	layout: StageLayoutModel;
}) {
	const fixtureId = fixture.fixture.fixture_id;
	const update = (key: keyof StagePosition3d, value: number) => {
		layout.updatePosition3d(fixtureId, {
			...fixture.position,
			[key]: value,
		});
	};
	return (
		<aside className="stage-3d-inspector">
			<b>Fixture position</b>
			{(["x", "y", "z", "rotationX", "rotationY", "rotationZ"] as const).map(
				(key) => (
					<NumberField
						key={key}
						label={key}
						allowDecimal={!key.startsWith("rotation")}
						step={key.startsWith("rotation") ? 1 : 0.1}
						value={fixture.position[key]}
						onChange={(event) => update(key, Number(event.target.value))}
						onBlur={() => void layout.save()}
					/>
				),
			)}
		</aside>
	);
}

export function Stage3dView({
	fixtures,
	visualization,
	options,
	layout,
	patchSelectionPreview,
	patchPreviewFixtures,
	camera3d,
	setupFixtureId,
	setSetupFixtureId,
	selection,
}: {
	fixtures: Stage3dFixture[];
	visualization: VisualizationSnapshot | null;
	options: StageOptionsModel;
	layout: StageLayoutModel;
	patchSelectionPreview: boolean;
	patchPreviewFixtures: string[];
	camera3d: StageWindowProps["camera3d"];
	setupFixtureId: string | null;
	setSetupFixtureId: Dispatch<SetStateAction<string | null>>;
	selection: StageSelectionModel;
}) {
	const inspectorFixtureId = setupFixtureId ?? selection.firstFixtureId;
	const inspectorFixture = fixtures.find(
		(item) => item.fixture.fixture_id === inspectorFixtureId,
	);
	return (
		<div className="stage-canvas stage-canvas-3d">
			<Stage3dCanvas
				fixtures={fixtures}
				visualization={visualization}
				selected={
					options.mode === "setup" && setupFixtureId
						? [setupFixtureId]
						: selection.fixtureIds
				}
				virtualHighlight={patchSelectionPreview ? patchPreviewFixtures : []}
				setup={options.mode === "setup"}
				showSelection={options.showSelection}
				showFloorGrid={options.showFloorGrid}
				showBeamGuides={options.showBeamGuides}
				environmentBrightness={options.environmentBrightness}
				camera3d={camera3d}
				onSelect={(fixtureId, additive) => {
					if (options.mode === "setup") {
						setSetupFixtureId(fixtureId);
						return;
					}
					void selection.applyFixtureGesture(
						fixtureId,
						additive && selection.fixtureIdSet.has(fixtureId)
							? "remove"
							: "add",
					);
				}}
				onMove={layout.updatePosition3d}
				onMoveEnd={(fixtureId, position) =>
					void layout.savePosition3d(fixtureId, position)
				}
			/>
			{options.mode === "setup" && inspectorFixture && (
				<Stage3dInspector fixture={inspectorFixture} layout={layout} />
			)}
		</div>
	);
}
