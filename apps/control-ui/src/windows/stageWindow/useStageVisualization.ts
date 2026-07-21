import { useMemo } from "react";
import { useServer } from "../../api/ServerContext";
import type { PatchedFixture, VisualizationSnapshot } from "../../api/types";
import { fixtures as visualFixtures } from "../../data/mockData";
import { useVisualizationRuntimeSnapshot } from "../../features/visualizationRuntime/VisualizationRuntimeView";
import { fixtureValue } from "../fixtureVisualization";
import { migrateStagePosition, type Stage3dFixture } from "../stage3dScene";
import type { StageFixturePresentation, StageLayoutModel } from "./types";

function useVisualizationSnapshot(followPreload: boolean, active: boolean) {
	return useVisualizationRuntimeSnapshot({
		lane: followPreload ? "preload" : "normal",
		enabled: active,
		intervalMillis: 200,
	});
}

function usePatchedFixtures(override?: readonly PatchedFixture[]) {
	const server = useServer();
	return useMemo(
		() =>
			[...(override ?? server.patch?.fixtures ?? [])].sort(
				(left, right) =>
					(left.virtual_fixture_number ?? Number.MAX_SAFE_INTEGER) -
						(right.virtual_fixture_number ?? Number.MAX_SAFE_INTEGER) ||
					(left.fixture_number ?? Number.MAX_SAFE_INTEGER) -
						(right.fixture_number ?? Number.MAX_SAFE_INTEGER) ||
					left.fixture_id.localeCompare(right.fixture_id),
			),
		[override, server.patch],
	);
}

function fixturePresentation(
	fixture: PatchedFixture,
	index: number,
	visualization: VisualizationSnapshot | null,
	patchPreviewSelected: boolean,
): StageFixturePresentation {
	const intensity = patchPreviewSelected
		? 1
		: (visualization?.blackout
				? 0
				: fixtureValue(visualization, fixture, "intensity")) *
			(visualization?.grand_master ?? 1);
	const red = fixtureValue(visualization, fixture, "color.red", 1);
	const green = fixtureValue(visualization, fixture, "color.green", 1);
	const blue = fixtureValue(visualization, fixture, "color.blue", 1);
	return {
		fixtureId: fixture.fixture_id,
		fixtureNumber:
			fixture.virtual_fixture_number != null
				? `0.${fixture.virtual_fixture_number}`
				: (fixture.fixture_number ?? index + 1),
		name: fixture.definition.name ?? fixture.definition.model,
		icon: fixture.definition.icon_asset ?? null,
		color: `rgb(${Math.round(red * 255)},${Math.round(green * 255)},${Math.round(blue * 255)})`,
		dimmer: Math.round(intensity * 100),
		pan: fixtureValue(visualization, fixture, "pan"),
		tilt: fixtureValue(visualization, fixture, "tilt"),
	};
}

function fallbackFixturePresentation(): StageFixturePresentation[] {
	return visualFixtures.map((fixture, index) => ({
		fixtureId: "",
		fixtureNumber: index + 1,
		name: fixture.name,
		icon: null,
		color: fixture.color,
		dimmer: fixture.dimmer,
		pan: Math.max(0, Math.min(1, fixture.pan / 360)),
		tilt: Math.max(0, Math.min(1, fixture.tilt / 180)),
	}));
}

function useFixtures3d(
	stageFixtures: PatchedFixture[],
	layout: StageLayoutModel,
) {
	return useMemo(
		() =>
			stageFixtures.flatMap((fixture, fixtureIndex) =>
				[
					{
						id: fixture.fixture_id,
						location: fixture.location,
						rotation: fixture.rotation,
					},
					...(fixture.multipatch ?? []),
				].map((instance, instanceIndex): Stage3dFixture => {
					const index = fixtureIndex * 16 + instanceIndex;
					const located =
						instance.location &&
						(instance.location.x || instance.location.y || instance.location.z)
							? {
									x: instance.location.x / 1000,
									y: instance.location.y / 1000,
									z: instance.location.z / 1000,
									rotationX: instance.rotation?.x ?? 0,
									rotationY: instance.rotation?.y ?? 0,
									rotationZ: instance.rotation?.z ?? 0,
								}
							: null;
					return {
						fixture,
						instanceId: instance.id,
						index,
						position:
							layout.positions3d[instance.id] ??
							located ??
							migrateStagePosition(
								instanceIndex
									? undefined
									: layout.positions[fixture.fixture_id],
								index,
							),
					};
				}),
			),
		[layout.positions, layout.positions3d, stageFixtures],
	);
}

export function patchPreviewFixtureIds(
	stageFixtures: readonly Pick<PatchedFixture, "fixture_id" | "logical_heads">[],
	selectedFixtureIds: ReadonlySet<string>,
) {
	return stageFixtures
		.filter(
			(fixture) =>
				selectedFixtureIds.has(fixture.fixture_id) ||
				fixture.logical_heads.some((head) =>
					selectedFixtureIds.has(head.fixture_id),
				),
		)
		.map((fixture) => fixture.fixture_id);
}

export function useStageVisualization(
	active: boolean,
	followPreload: boolean,
	patchSelectionPreview: boolean,
	layout: StageLayoutModel,
	selectedFixtureIds: ReadonlySet<string>,
	patchedFixtures?: readonly PatchedFixture[],
) {
	const server = useServer();
	const visualization = useVisualizationSnapshot(followPreload, active);
	const stageFixtures = usePatchedFixtures(patchedFixtures);
	const patchPreviewFixtures = useMemo(
		() => patchPreviewFixtureIds(stageFixtures, selectedFixtureIds),
		[selectedFixtureIds, stageFixtures],
	);
	const fixtures = server.bootstrap
		? stageFixtures.map((fixture, index) =>
				fixturePresentation(
					fixture,
					index,
					visualization,
					patchSelectionPreview &&
						patchPreviewFixtures.includes(fixture.fixture_id),
				),
			)
		: fallbackFixturePresentation();
	return {
		visualization,
		fixtures,
		fixtures3d: useFixtures3d(stageFixtures, layout),
		patchPreviewFixtures,
	};
}
