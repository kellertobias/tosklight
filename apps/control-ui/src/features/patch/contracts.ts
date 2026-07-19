/** Feature-owned Patch models. Serialized wire DTOs are mapped at the API boundary. */
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

export interface PatchMultiPatch {
	id: string;
	name: string;
	splitPatches: readonly PatchSplitAssignment[];
	location: PatchFixtureLocation;
	rotation: PatchFixtureRotation;
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
	moveInBlackEnabled: boolean;
	moveInBlackDelayMillis: number;
	highlightOverrides: readonly PatchHighlightOverride[];
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
}

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
