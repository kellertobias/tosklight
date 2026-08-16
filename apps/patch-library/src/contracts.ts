/** Feature-owned Patch models. Serialized wire DTOs are mapped at the API boundary. */
import type { FixtureProfile } from "./wire";

export interface PatchSplitAssignment {
	split: number;
	universe: number | null;
	address: number | null;
}

export interface PatchDirectControlEndpoint {
	protocol: "citp";
	ipAddress: string;
	port: number;
}

export interface PatchFixtureLocation {
	x: number;
	y: number;
	z: number;
}

export interface PatchFixtureRotation {
	x: number;
	y: number;
	z: number;
}

export type PatchInstalledLightSource =
	| { type: "profile_default" }
	| { type: "tungsten" }
	| { type: "halogen" }
	| { type: "discharge" }
	| { type: "led" }
	| { type: "fluorescent" }
	| { type: "arc" }
	| { type: "other"; label: string };

export interface PatchGelDefinitionSnapshot {
	number: string;
	name: string;
	displaySrgb: string;
	visualizerSrgb: string;
}

export type PatchGelAssignment =
	| { type: "open_white" }
	| {
			type: "built_in";
			catalogId: string;
			entryId: string;
			embeddedFallback: PatchGelDefinitionSnapshot;
	  }
	| {
			type: "custom";
			name: string;
			colorSrgb: string;
			note: string | null;
	  };

export interface PatchInstalledFixtureAppearance {
	lightSource: PatchInstalledLightSource;
	colorTemperatureKelvin: number | null;
	luminousOutputLumens: number | null;
	gel: PatchGelAssignment;
	shaperAnglesDegrees: [number, number, number, number];
}

export interface PatchMultiPatch {
	id: string;
	name: string;
	splitPatches: readonly PatchSplitAssignment[];
	location: PatchFixtureLocation;
	rotation: PatchFixtureRotation;
	invertPan?: boolean;
	invertTilt?: boolean;
	bracketAngle?: number;
	shaperAngle?: number | null;
	installedAppearance?: PatchInstalledFixtureAppearance;
}

export interface PatchHighlightOverride {
	channelId: string;
	rawValue: number;
}

export interface PatchFixtureWrite {
	fixtureId: string;
	fixtureNumber: number | null;
	virtualFixtureNumber: number | null;
	name: string;
	profileId: string;
	profileRevision: number;
	modeId: string;
	splitPatches: readonly PatchSplitAssignment[];
	layerId: string;
	directControl: PatchDirectControlEndpoint | null;
	location: PatchFixtureLocation;
	rotation: PatchFixtureRotation;
	multipatch: readonly PatchMultiPatch[];
	groupMastersEnabled?: boolean;
	grandMasterEnabled?: boolean;
	invertPan?: boolean;
	invertTilt?: boolean;
	/** Degrees the mounting bracket is set to, positive nose-down. */
	bracketAngle?: number;
	/** Degrees a fitted shaper or barn-door module is turned to; `null` when none is fitted. */
	shaperAngle?: number | null;
	installedAppearance?: PatchInstalledFixtureAppearance;
	moveInBlackEnabled: boolean;
	moveInBlackDelayMillis: number;
	highlightOverrides: readonly PatchHighlightOverride[];
}

export interface PatchPlacementOverride {
	fixtureId: string;
	universe: number;
	address: number;
}

export type PatchPlacementMode =
	| { type: "consecutive" }
	| {
			type: "operator_overrides";
			overrides: readonly PatchPlacementOverride[];
	  };

export interface PatchPlacementSplit {
	split: number;
	universe: number | null;
	address: number | null;
	mode: PatchPlacementMode;
}

export interface PatchPlacement {
	fixtureIds: readonly string[];
	splits: readonly PatchPlacementSplit[];
}

export interface PatchLogicalHead {
	profileHeadId: string | null;
	headIndex: number;
	fixtureId: string;
}

export interface PatchFixtureProjection extends PatchFixtureWrite {
	fixtureRevision: number;
	logicalHeads: readonly PatchLogicalHead[];
}

export interface PatchModeProjection {
	modeId: string;
	name: string;
	splits: readonly { split: number; footprint: number }[];
}

export interface PatchProfileRevision {
	profileId: string;
	profileRevision: number;
	contentDigest: string;
	manufacturer: string;
	name: string;
	fixtureType: string;
	patchPolicy: "dmx" | "visual_only";
	referencedModes: readonly PatchModeProjection[];
	/**
	 * Server-resolved parameterized profile snapshot for this exact revision. Programmer-surface
	 * consumers build head parameters, channels, and control actions from it when the live library
	 * lacks the patched revision. Absent (null) on older payloads.
	 */
	profileSnapshot: FixtureProfile | null;
}

export interface PatchChange {
	showId: string;
	showRevision: number;
	patchRevision: number;
	eventSequence: number | null;
	fixtures: readonly PatchFixtureProjection[];
	removedFixtureIds: readonly string[];
	profileRevisions: readonly PatchProfileRevision[];
}

export interface PatchMutationOutcome extends PatchChange {
	requestId: string;
	replayed: boolean;
	changed: boolean;
}

export interface PatchMutation {
	requestId: string;
	fixtures: readonly PatchFixtureWrite[];
	removeFixtureIds: readonly string[];
	placements?: readonly PatchPlacement[];
}

export type PatchFixturePolicyAction =
	| { type: "group_masters"; controlled: boolean }
	| { type: "grand_master"; controlled: boolean }
	| {
			type: "axis_inversion";
			axis: "pan" | "tilt";
			inverted: boolean;
			multipatchInstanceId: string | null;
	  };

export interface PatchSnapshot {
	showId: string;
	showRevision: number;
	patchRevision: number;
	cursor: number;
	fixtures: readonly PatchFixtureProjection[];
	profileRevisions: readonly PatchProfileRevision[];
}

export type PatchEventMessage =
	| { type: "ready"; cursor: number }
	| { type: "repaired"; cursor: number }
	| {
			type: "gap";
			afterSequence: number;
			oldestAvailable: number;
			latestSequence: number;
	  }
	| { type: "event"; sequence: number; change: PatchChange }
	| { type: "error"; error: string };

export interface PatchError {
	error: string;
	currentRevision: number | null;
	retryable: boolean;
}
