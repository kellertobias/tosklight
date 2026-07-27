import type {
	AttributeValue,
	PatchedFixture,
	VisualizationSnapshot,
} from "../../api/types";
import type { StagePosition3d } from "../../features/server/contracts";
import type { StageRenderQuality } from "../../types";

export interface Stage3dFixture {
	fixture: PatchedFixture;
	position: StagePosition3d;
	index: number;
	instanceId?: string;
}

export type FixtureAttributeValues = Map<string, AttributeValue>;
export type FixtureValuesById = Map<string, FixtureAttributeValues>;

export interface StageSceneContext {
	snapshot: VisualizationSnapshot | null;
	selected: Set<string>;
	byFixture: FixtureValuesById;
	projectedOwners: Set<string>;
	showBeamGuides: boolean;
	renderQuality: StageRenderQuality;
	virtualHighlight: Set<string>;
}
