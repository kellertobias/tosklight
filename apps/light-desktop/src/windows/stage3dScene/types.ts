import type {
	AttributeValue,
	PatchedFixture,
	VisualizationSnapshot,
} from "../../api/types";
import type { StagePosition3d } from "../../features/server/contracts";
import type { StageRenderQuality } from "../../types";
import type { StageProceduralResourceCache } from "./resources";

export interface Stage3dFixture {
	fixture: PatchedFixture;
	position: StagePosition3d;
	index: number;
	instanceId?: string;
	invertPan?: boolean;
	invertTilt?: boolean;
	/**
	 * Degrees the mounting bracket is set to, positive nose-down.
	 *
	 * It turns the fixture about its own transverse axis after the placement rotation, which is
	 * what a clamp or a yoke does: the bar decides which way the lantern faces and the bracket
	 * decides how far down it looks.
	 */
	bracketAngle?: number;
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
	resources: StageProceduralResourceCache;
}
