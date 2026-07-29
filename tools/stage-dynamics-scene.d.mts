import type {
	DynamicDefinitionProjection,
	PatchSnapshot,
} from "../apps/light-desktop/src/api/generated/light-wire";
import type { DeterministicLargeStageInputs } from "./stage-large-scene.mjs";

export interface StageDynamicTargetDescriptor {
	target: string;
	fixtureId: string;
	headId: string;
	attributes: string[];
	signature: string;
}

export interface LargeStageDynamicsPlan {
	definitions: DynamicDefinitionProjection[];
	targetDescriptors: StageDynamicTargetDescriptor[];
	dynamicTargetCount: number;
	staticControlFixtureIds: string[];
	laneCoverage: Record<string, number>;
}

export function createLargeStageDynamicsPlan(
	patch: PatchSnapshot,
	largeScene: Pick<
		DeterministicLargeStageInputs,
		"dynamicFixtureIds" | "staticControlFixtureIds"
	>,
): LargeStageDynamicsPlan;
