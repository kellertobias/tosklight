import type { StagePosition3d } from "../../api/ServerContext";
import type { PatchedFixture } from "../../api/types";
import type { StageMode, StageView } from "../../types";
import type { WindowProps } from "../windowTypes";

export interface StageWindowProps extends WindowProps {
	showSelection?: boolean;
	showFloorGrid?: boolean;
	environmentBrightness?: number;
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
	followPreload: boolean;
	toggleFollowPreload: () => void;
	groupsVisible: boolean;
	showSelection: boolean;
	showFloorGrid: boolean;
	showBeamGuides: boolean;
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
	updatePosition2d: (
		fixtureId: string,
		position: { x: number; y: number; rotation: number },
	) => void;
	updatePosition3d: (fixtureId: string, position: StagePosition3d) => void;
	save: () => Promise<void>;
	savePosition3d: (
		fixtureId: string,
		position: StagePosition3d,
	) => Promise<void>;
}
