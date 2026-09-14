import type {
	InstalledFixtureAppearance,
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
	/** Installed source, gel, and static shaper settings for this exact physical copy. */
	installedAppearance?: InstalledFixtureAppearance;
	/** Installed rotation of a fitted shaper module, in degrees. */
	shaperAngle?: number | null;
}

export type {
	FixtureAttributeValues,
	FixtureValuesById,
	StageShaperState,
} from "@tosklight/patch/stage-geometry";
import type { FixtureValuesById } from "@tosklight/patch/stage-geometry";

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
