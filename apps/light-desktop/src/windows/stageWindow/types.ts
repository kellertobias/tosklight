import type { StageProjection2d } from "../../api/client/stageLayout";
import type { PatchedFixture } from "../../api/types";
import type { StagePosition3d } from "../../features/server/contracts";
import type { Stage2dSide, StageMode, StageView } from "../../types";
import type { WindowProps } from "../windowTypes";

export interface StageWindowProps extends WindowProps {
	showSelection?: boolean;
	showFloorGrid?: boolean;
	environmentBrightness?: number;
	visualizationIntervalMillis?: number;
	pixelRatioCap?: number;
	camera3d?: {
		position: readonly [number, number, number];
		target: readonly [number, number, number];
	};
	patchSelectionPreview?: boolean;
	patchedFixtures?: readonly PatchedFixture[];
}

export interface StageOptionsModel {
	mode: StageMode;
	setMode: (mode: StageMode) => void;
	view: StageView;
	setView: (view: StageView) => void;
	/** Which side a 2D Stage is the plan from. */
	side2d: Stage2dSide;
	setSide2d: (side: Stage2dSide) => void;
	followPreload: boolean;
	toggleFollowPreload: () => void;
	groupsVisible: boolean;
	showSelection: boolean;
	showFloorGrid: boolean;
	environmentBrightness: number;
}

export interface StageFixturePresentation {
	fixtureId: string;
	fixtureNumber: number | string;
	name: string;
	icon: string | null;
	color: string;
	dimmer: number;
	pan: number;
	tilt: number;
}

export interface StageLayoutModel {
	positions: Record<string, { x: number; y: number; rotation: number }>;
	positions3d: Record<string, StagePosition3d>;
	positions2dConfig: {
		provenance: "automatic" | "manual";
		projection: StageProjection2d;
	};
}
