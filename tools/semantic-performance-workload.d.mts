import type {
	DynamicDefinitionProjection,
	PatchSnapshot,
	ProgrammingAttributeValue,
	ProgrammingTargetReference,
} from "../apps/light-desktop/src/api/generated/light-wire";

export const SEMANTIC_WORKLOAD_VERSION: "tosklight.semantic-performance-workload/1";
export const SEMANTIC_TRACKING_RATES_HZ: readonly number[];
export const DEFAULT_OUTPUT_RATES_HZ: readonly number[];
export const SEMANTIC_DYNAMIC_POOL_BASE: number;

export interface SemanticWorkloadRequest {
	angle?: number;
	position?: Partial<{
		fixed: number;
		referenced: number;
		sharedMountGroups: number;
		sharedMountGroupSize: number;
		separateMounts: number;
	}>;
	color?: Partial<Record<"rgb" | "rgbw" | "cmy" | "wheel", number>>;
	uv?: number;
	focus?: number;
	zoom?: number;
	dirtySubsetPoints?: number;
	trackingRatesHz?: number[];
	outputRatesHz?: number[];
	trackingDurationSeconds?: number;
}

export type Unavailable = { status: "unavailable"; reason: string };

export interface SemanticActivation {
	key: string;
	family: string;
	dynamicId: string;
	poolNumber: number;
	targets: string[];
	start: {
		targets: string[];
		overrides: {
			size: number;
			speed_multiplier: { numerator: number; denominator: number };
			phase_offset_degrees: number;
		};
		timing: Record<string, never>;
	};
}

export interface SemanticStaticValue {
	type: "set_fixture";
	fixture_id: string;
	attribute: "position" | "color" | "focus" | "zoom";
	value: ProgrammingAttributeValue;
	timing: { fade: false; fade_millis: null; delay_millis: null };
}

export interface SemanticDirtyScenario {
	key: "static-points" | "small-subset" | "all-points-move";
	requestedMovingPoints: number;
	movingPointIds: string[];
	dirtyTargetCount: number;
	positionTargetCount: number;
	dirtyTargetIds: string[];
}

export interface SemanticTrackingFrame {
	sequence: number;
	t_micros: number;
	points: Array<{ point_id: string; position_metres: [number, number, number] }>;
}

/** The manifest is intentionally open: consumers should read it as recorded evidence. */
export interface SemanticWorkloadManifest {
	workloadVersion: string;
	workloadId: string;
	seed: string;
	identitySource: "live-patch" | "synthetic-deterministic";
	manifestSha256: string;
	counts: Record<string, unknown>;
	capabilityMix: Record<string, { requested: number; realized: number; status: "covered" | "partial" | "missing" | "not-requested" }>;
	rates: { kind: "configured-targets"; trackingHz: number[]; outputHz: number[]; matrix: Array<{ trackingHz: number; outputHz: number }> };
	shortfalls: Array<Record<string, unknown>>;
	limitations: Array<Record<string, unknown>>;
	[key: string]: unknown;
}

export interface SemanticPerformanceWorkload {
	definitions: DynamicDefinitionProjection[];
	activations: SemanticActivation[];
	staticValues: SemanticStaticValue[];
	positionScenarios: Array<{
		key: string;
		kind: "fixed" | "referenced" | "shared-moving-mount" | "separate-moving-mount";
		reference: ProgrammingTargetReference | null;
		mountPoint: string | null;
		aimPoint: string | null;
		requestedTargets: number;
		targets: string[];
	}>;
	dirty: { points: string[]; dependents: Record<string, string[]>; scenarios: SemanticDirtyScenario[] };
	tracking: { ratesHz: number[]; durationSeconds: number; staticPointsEmitted: true; motion: unknown[] };
	manifest: SemanticWorkloadManifest;
}

export function buildSemanticPerformanceWorkload(
	patch: PatchSnapshot | (Omit<PatchSnapshot, "show_id" | "show_revision" | "patch_revision" | "cursor"> & Record<string, unknown>),
	options: { seed: string | number; request?: SemanticWorkloadRequest; productionSupport?: Record<string, unknown> },
): SemanticPerformanceWorkload;
export function buildSyntheticSemanticWorkload(options: {
	seed: string | number;
	request?: SemanticWorkloadRequest;
}): SemanticPerformanceWorkload;
export function validateSemanticWorkload(workload: SemanticPerformanceWorkload): string[];
export function trackingFrames(
	workload: SemanticPerformanceWorkload,
	options: { scenario?: SemanticDirtyScenario["key"]; rateHz: number },
): Generator<SemanticTrackingFrame>;
/** `<root>/semantic-workloads/<workloadId>/<manifestSha256[0..16]>`: one directory per exact input set. */
export function semanticWorkloadDirectory(
	root: string,
	manifest: { workloadId: string; manifestSha256: string },
): string;
/** Refuses (throws) when the directory already holds a different manifest or manifest-less evidence. */
export function writeSemanticWorkload(
	workload: SemanticPerformanceWorkload,
	directory: string,
	options?: { emitTracking?: boolean },
): string;
