import type {
	PatchFixtureInput,
	PatchFixtureProjection,
} from "../apps/light-desktop/src/api/generated/light-wire";

export const LARGE_STAGE_FIXTURE_RECORDS: 970;
export const LARGE_STAGE_FIXTURE_INSTANCES: 1_000;
export const LARGE_STAGE_FIRST_UNIVERSE: 101;
export const LARGE_STAGE_DYNAMIC_INSTANCES: 20;

export const LARGE_STAGE_MANIFEST: readonly Array<{
	key: string;
	category: "moving" | "sunstrip" | "static" | "venue";
	manufacturer: string;
	name: string;
	mode: string;
	records: number;
	multipatches: number;
	dynamic: boolean;
}>;

export interface DeterministicLargeStageInputs {
	fixtures: PatchFixtureInput[];
	addedFixtureRecords: number;
	addedMultipatchInstances: number;
	categoryCounts: Record<"sunstrip" | "moving" | "static" | "venue", number>;
	dynamicFixtureIds: string[];
	staticControlFixtureIds: string[];
	inventory: Array<{
		key: string;
		manufacturer: string;
		name: string;
		mode: string;
		records: number;
		instances: number;
		footprint: number;
		patchedSlots: number;
		dynamic: boolean;
	}>;
	patch: {
		firstUniverse: number | null;
		lastUniverse: number | null;
		universeCount: number;
		occupiedSlots: number;
		occupiedByUniverse: Record<string, number>;
	};
}

export interface LargeStageFixtureProfile {
	id: string;
	revision: number;
	manufacturer: string;
	name: string;
	modes: Array<{
		id: string;
		name: string;
		splits: Array<{ number: number; footprint: number }>;
	}>;
}

export function createDeterministicLargeStageInputs(
	seedFixtures: readonly PatchFixtureProjection[],
	profiles: readonly LargeStageFixtureProfile[],
	layerId?: string,
): DeterministicLargeStageInputs;

export function countFixtureInstances(
	fixtures: readonly Array<{ multipatch: readonly unknown[] }>,
): number;
