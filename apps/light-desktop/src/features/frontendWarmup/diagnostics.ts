import type { FrontendWarmupDiagnostics } from "./coordinator";

export interface FrontendSnapshotRequestDiagnostic {
	feature: string;
	startedAt: number;
	finishedAt: number | null;
	payloadBytes: number | null;
	status: "running" | "ready" | "error";
}

export interface FrontendSurfaceSwitchDiagnostic {
	surface: string;
	startedAt: number;
	paintedAt: number;
	durationMs: number;
}

export interface FrontendLongTaskDiagnostic {
	startedAt: number;
	durationMs: number;
}

export interface FrontendEventLagDiagnostic {
	feature: string;
	occurredAt: string;
	receivedAt: number;
	lagMs: number;
}

export interface FrontendPatchMutationDiagnostic {
	requestId: string;
	fixtureCount: number;
	startedAt: number;
	optimisticStoreAt: number | null;
	responseDecodedAt: number | null;
	authoritativeStoreAt: number | null;
	visiblePaintAt: number | null;
	actionToVisibleMs: number | null;
}

export interface FrontendStageVisualizationDiagnostic {
	lane: "normal" | "preload";
	startedAt: number;
	receivedAt: number | null;
	durationMs: number | null;
	payloadBytes: number | null;
	sourceGeneratedAt: string | null;
	sourceAgeMs: number | null;
	status: "running" | "ready" | "error";
}

export interface FrontendStageSceneBuildDiagnostic {
	startedAt: number;
	finishedAt: number;
	durationMs: number;
	fixtureCount: number;
	objectCount: number;
	geometryCount: number;
	materialCount: number;
}

export interface FrontendStageRenderDiagnostic {
	submittedAt: number;
	durationMs: number;
	calls: number;
	triangles: number;
	lines: number;
	points: number;
	geometries: number;
	textures: number;
}

export interface FrontendStageModelLoadDiagnostic {
	startedAt: number;
	finishedAt: number | null;
	durationMs: number | null;
	status: "loading" | "ready" | "error";
}

export interface FrontendStageFrameDiagnostic {
	lane: "normal" | "preload";
	sourceFrame: number | null;
	sourceGeneratedAt: string;
	publishedAt: string | null;
	receivedAt: number;
	firstAppliedAt: number | null;
	settledAppliedAt: number | null;
	firstCanvasSubmittedAt: number | null;
	settledCanvasSubmittedAt: number | null;
	sourceToReceiveMs: number | null;
	projectionToReceiveMs: number | null;
	sourceToSettledCanvasMs: number | null;
}

export interface FrontendStageDiagnostics {
	visualizationRequests: readonly FrontendStageVisualizationDiagnostic[];
	frames: readonly FrontendStageFrameDiagnostic[];
	sceneBuilds: readonly FrontendStageSceneBuildDiagnostic[];
	modelLoads: readonly FrontendStageModelLoadDiagnostic[];
	modelCacheHits: number;
	modelCacheMisses: number;
	modelClones: number;
	modelCacheDisposals: number;
	renders: readonly FrontendStageRenderDiagnostic[];
	sceneDisposals: number;
	rendererContextsCreated: number;
	rendererContextsDisposed: number;
	rafCallbacks: number;
}

export interface FrontendPerformanceSnapshot {
	startedAt: number;
	firstUsablePaintAt: number | null;
	warmup: FrontendWarmupDiagnostics | null;
	snapshotRequests: readonly FrontendSnapshotRequestDiagnostic[];
	snapshotRequestCount: number;
	snapshotPayloadBytes: number;
	maxSnapshotConcurrency: number;
	surfaceSwitches: readonly FrontendSurfaceSwitchDiagnostic[];
	longTasks: readonly FrontendLongTaskDiagnostic[];
	eventLags: readonly FrontendEventLagDiagnostic[];
	patchMutations: readonly FrontendPatchMutationDiagnostic[];
	patchActionToVisible: {
		samples: number;
		p50Ms: number | null;
		p95Ms: number | null;
		gateEnforced: false;
	};
	stage: FrontendStageDiagnostics;
}

type PerformanceWithMemory = Performance & {
	measureUserAgentSpecificMemory?: () => Promise<{ bytes: number }>;
};

class FrontendPerformanceDiagnostics {
	private readonly startedAt = now();
	private firstUsablePaintAt: number | null = null;
	private warmup: FrontendWarmupDiagnostics | null = null;
	private readonly requests: FrontendSnapshotRequestDiagnostic[] = [];
	private readonly switches: FrontendSurfaceSwitchDiagnostic[] = [];
	private readonly longTasks: FrontendLongTaskDiagnostic[] = [];
	private readonly eventLags: FrontendEventLagDiagnostic[] = [];
	private readonly patchMutations: FrontendPatchMutationDiagnostic[] = [];
	private readonly stageVisualizationRequests: FrontendStageVisualizationDiagnostic[] =
		[];
	private readonly stageFrames: FrontendStageFrameDiagnostic[] = [];
	private readonly stageSceneBuilds: FrontendStageSceneBuildDiagnostic[] = [];
	private readonly stageModelLoads: FrontendStageModelLoadDiagnostic[] = [];
	private readonly stageRenders: FrontendStageRenderDiagnostic[] = [];
	private stageSceneDisposals = 0;
	private stageModelCacheHits = 0;
	private stageModelCacheMisses = 0;
	private stageModelClones = 0;
	private stageModelCacheDisposals = 0;
	private stageRendererContextsCreated = 0;
	private stageRendererContextsDisposed = 0;
	private stageRafCallbacks = 0;
	private activeRequests = 0;
	private maxSnapshotConcurrency = 0;
	private longTaskObserver: PerformanceObserver | null = null;

	constructor() {
		mark("tosklight:frontend:start");
		this.observeLongTasks();
	}

	beginSnapshotRequest(feature: string) {
		const request: FrontendSnapshotRequestDiagnostic = {
			feature,
			startedAt: now(),
			finishedAt: null,
			payloadBytes: null,
			status: "running",
		};
		this.requests.push(request);
		this.activeRequests++;
		this.maxSnapshotConcurrency = Math.max(
			this.maxSnapshotConcurrency,
			this.activeRequests,
		);
		const markName = `tosklight:snapshot:${feature}:${this.requests.length}`;
		mark(`${markName}:start`);
		let finished = false;
		return (result?: unknown, error?: unknown) => {
			if (finished) return;
			finished = true;
			this.activeRequests = Math.max(0, this.activeRequests - 1);
			request.finishedAt = now();
			request.status = error == null ? "ready" : "error";
			request.payloadBytes =
				result === undefined ? null : serializedModelBytes(result);
			mark(`${markName}:end`);
			measure(markName, `${markName}:start`, `${markName}:end`);
		};
	}

	markFirstUsablePaint() {
		if (this.firstUsablePaintAt !== null) return;
		this.firstUsablePaintAt = now();
		mark("tosklight:frontend:first-usable-paint");
		measure(
			"tosklight:frontend:time-to-first-usable-paint",
			"tosklight:frontend:start",
			"tosklight:frontend:first-usable-paint",
		);
	}

	beginPhase(name: string) {
		const markName = `tosklight:startup:${name}`;
		mark(`${markName}:start`);
		let finished = false;
		return () => {
			if (finished) return;
			finished = true;
			mark(`${markName}:end`);
			measure(markName, `${markName}:start`, `${markName}:end`);
		};
	}

	setWarmup(diagnostics: FrontendWarmupDiagnostics) {
		const wasReady = this.warmup?.status === "ready";
		this.warmup = diagnostics;
		if (diagnostics.status === "running" && !this.warmupStarted()) {
			mark("tosklight:warmup:start");
		}
		if (diagnostics.status === "ready" && !wasReady) {
			mark("tosklight:warmup:ready");
			measure(
				"tosklight:warmup:duration",
				"tosklight:warmup:start",
				"tosklight:warmup:ready",
			);
		}
	}

	beginSurfaceSwitch(surface: string) {
		const startedAt = now();
		const markName = `tosklight:surface-switch:${surface}:${this.switches.length + 1}`;
		mark(`${markName}:start`);
		return () => {
			const paintedAt = now();
			this.switches.push({
				surface,
				startedAt,
				paintedAt,
				durationMs: paintedAt - startedAt,
			});
			mark(`${markName}:paint`);
			measure(markName, `${markName}:start`, `${markName}:paint`);
		};
	}

	recordEventReceipt(feature: string, value: unknown) {
		const occurredAt = rawEventOccurredAt(value);
		if (!occurredAt) return;
		const occurredAtMillis = Date.parse(occurredAt);
		if (!Number.isFinite(occurredAtMillis)) return;
		const receivedAt = Date.now();
		this.eventLags.push({
			feature,
			occurredAt,
			receivedAt,
			lagMs: Math.max(0, receivedAt - occurredAtMillis),
		});
		if (this.eventLags.length > 2048) this.eventLags.shift();
	}

	beginPatchMutation(requestId: string, fixtureCount: number) {
		const sample: FrontendPatchMutationDiagnostic = {
			requestId,
			fixtureCount,
			startedAt: now(),
			optimisticStoreAt: null,
			responseDecodedAt: null,
			authoritativeStoreAt: null,
			visiblePaintAt: null,
			actionToVisibleMs: null,
		};
		this.patchMutations.push(sample);
		if (this.patchMutations.length > 512) this.patchMutations.shift();
		const markName = `tosklight:patch:${requestId}`;
		mark(`${markName}:start`);
		return {
			optimisticStorePublished: () => {
				sample.optimisticStoreAt ??= now();
				mark(`${markName}:optimistic-store`);
			},
			responseDecoded: () => {
				sample.responseDecodedAt ??= now();
				mark(`${markName}:response`);
			},
			authoritativeStorePublished: () => {
				sample.authoritativeStoreAt ??= now();
				mark(`${markName}:authoritative-store`);
			},
			visiblePainted: () => {
				if (sample.visiblePaintAt !== null) return;
				sample.visiblePaintAt = now();
				sample.actionToVisibleMs = sample.visiblePaintAt - sample.startedAt;
				mark(`${markName}:visible`);
				measure(markName, `${markName}:start`, `${markName}:visible`);
			},
		};
	}

	beginStageVisualizationRequest(lane: "normal" | "preload") {
		const sample: FrontendStageVisualizationDiagnostic = {
			lane,
			startedAt: now(),
			receivedAt: null,
			durationMs: null,
			payloadBytes: null,
			sourceGeneratedAt: null,
			sourceAgeMs: null,
			status: "running",
		};
		pushBounded(this.stageVisualizationRequests, sample, 2_048);
		let finished = false;
		return (
			result?: { generated_at: string },
			error?: unknown,
			payloadBytes?: number,
		) => {
			if (finished) return;
			finished = true;
			const receivedAt = Date.now();
			sample.receivedAt = receivedAt;
			sample.durationMs = now() - sample.startedAt;
			sample.status = error == null ? "ready" : "error";
			if (!result) return;
			sample.payloadBytes = payloadBytes ?? serializedModelBytes(result);
			sample.sourceGeneratedAt = result.generated_at;
			const sourceAt = Date.parse(result.generated_at);
			sample.sourceAgeMs = Number.isFinite(sourceAt)
				? Math.max(0, receivedAt - sourceAt)
				: null;
		};
	}

	recordStageSceneBuild(sample: FrontendStageSceneBuildDiagnostic) {
		pushBounded(this.stageSceneBuilds, sample, 1_024);
	}

	beginStageModelLoad() {
		const sample: FrontendStageModelLoadDiagnostic = {
			startedAt: now(),
			finishedAt: null,
			durationMs: null,
			status: "loading",
		};
		pushBounded(this.stageModelLoads, sample, 1_024);
		let finished = false;
		return (error?: unknown) => {
			if (finished) return;
			finished = true;
			sample.finishedAt = now();
			sample.durationMs = sample.finishedAt - sample.startedAt;
			sample.status = error == null ? "ready" : "error";
		};
	}

	recordStageModelCacheLookup(hit: boolean) {
		if (hit) this.stageModelCacheHits++;
		else this.stageModelCacheMisses++;
	}

	recordStageModelClone() {
		this.stageModelClones++;
	}

	recordStageModelCacheDisposal() {
		this.stageModelCacheDisposals++;
	}

	recordStageFrameReceived({
		lane,
		sourceFrame = null,
		sourceGeneratedAt,
		publishedAt = null,
	}: {
		lane: "normal" | "preload";
		sourceFrame?: number | null;
		sourceGeneratedAt: string;
		publishedAt?: string | null;
	}) {
		const receivedAt = Date.now();
		const sourceAt = Date.parse(sourceGeneratedAt);
		const projectionAt =
			publishedAt === null ? Number.NaN : Date.parse(publishedAt);
		pushBounded(
			this.stageFrames,
			{
				lane,
				sourceFrame,
				sourceGeneratedAt,
				publishedAt,
				receivedAt,
				firstAppliedAt: null,
				settledAppliedAt: null,
				firstCanvasSubmittedAt: null,
				settledCanvasSubmittedAt: null,
				sourceToReceiveMs: Number.isFinite(sourceAt)
					? Math.max(0, receivedAt - sourceAt)
					: null,
				projectionToReceiveMs: Number.isFinite(projectionAt)
					? Math.max(0, receivedAt - projectionAt)
					: null,
				sourceToSettledCanvasMs: null,
			},
			4_096,
		);
	}

	recordStageFrameApplied(
		sourceGeneratedAt: string | undefined,
		settled: boolean,
		lane?: "normal" | "preload",
	) {
		const sample = this.latestStageFrame(sourceGeneratedAt, lane);
		if (!sample) return;
		const appliedAt = Date.now();
		sample.firstAppliedAt ??= appliedAt;
		if (settled) sample.settledAppliedAt ??= appliedAt;
	}

	recordStageFrameCanvasSubmitted(
		sourceGeneratedAt: string | undefined,
		settled: boolean,
		lane?: "normal" | "preload",
	) {
		const sample = this.latestStageFrame(sourceGeneratedAt, lane);
		if (!sample) return;
		const submittedAt = Date.now();
		sample.firstCanvasSubmittedAt ??= submittedAt;
		if (settled && sample.settledCanvasSubmittedAt === null) {
			sample.settledCanvasSubmittedAt = submittedAt;
			const sourceAt = Date.parse(sample.sourceGeneratedAt);
			sample.sourceToSettledCanvasMs = Number.isFinite(sourceAt)
				? Math.max(0, submittedAt - sourceAt)
				: null;
		}
	}

	recordStageSceneDisposal() {
		this.stageSceneDisposals++;
	}

	recordStageRendererCreated() {
		this.stageRendererContextsCreated++;
	}

	recordStageRendererDisposed() {
		this.stageRendererContextsDisposed++;
	}

	recordStageRafCallback() {
		this.stageRafCallbacks++;
	}

	recordStageRender(sample: FrontendStageRenderDiagnostic) {
		pushBounded(this.stageRenders, sample, 4_096);
	}

	snapshot(): FrontendPerformanceSnapshot {
		const patchDurations = this.patchMutations
			.flatMap(({ actionToVisibleMs }) =>
				actionToVisibleMs == null ? [] : [actionToVisibleMs],
			)
			.sort((left, right) => left - right);
		return {
			startedAt: this.startedAt,
			firstUsablePaintAt: this.firstUsablePaintAt,
			warmup: this.warmup,
			snapshotRequests: this.requests.map((request) => ({ ...request })),
			snapshotRequestCount: this.requests.length,
			snapshotPayloadBytes: this.requests.reduce(
				(total, request) => total + (request.payloadBytes ?? 0),
				0,
			),
			maxSnapshotConcurrency: this.maxSnapshotConcurrency,
			surfaceSwitches: this.switches.map((sample) => ({ ...sample })),
			longTasks: this.longTasks.map((task) => ({ ...task })),
			eventLags: this.eventLags.map((sample) => ({ ...sample })),
			patchMutations: this.patchMutations.map((sample) => ({ ...sample })),
			patchActionToVisible: {
				samples: patchDurations.length,
				p50Ms: percentile(patchDurations, 50),
				p95Ms: percentile(patchDurations, 95),
				gateEnforced: false,
			},
			stage: {
				visualizationRequests: this.stageVisualizationRequests.map(
					(sample) => ({
						...sample,
					}),
				),
				frames: this.stageFrames.map((sample) => ({ ...sample })),
				sceneBuilds: this.stageSceneBuilds.map((sample) => ({ ...sample })),
				modelLoads: this.stageModelLoads.map((sample) => ({ ...sample })),
				modelCacheHits: this.stageModelCacheHits,
				modelCacheMisses: this.stageModelCacheMisses,
				modelClones: this.stageModelClones,
				modelCacheDisposals: this.stageModelCacheDisposals,
				renders: this.stageRenders.map((sample) => ({ ...sample })),
				sceneDisposals: this.stageSceneDisposals,
				rendererContextsCreated: this.stageRendererContextsCreated,
				rendererContextsDisposed: this.stageRendererContextsDisposed,
				rafCallbacks: this.stageRafCallbacks,
			},
		};
	}

	async browserMemoryBytes() {
		const performance = globalThis.performance as
			| PerformanceWithMemory
			| undefined;
		return (
			(await performance?.measureUserAgentSpecificMemory?.())?.bytes ?? null
		);
	}

	private warmupStarted() {
		return (
			(globalThis.performance?.getEntriesByName(
				"tosklight:warmup:start",
				"mark",
			).length ?? 0) > 0
		);
	}

	private latestStageFrame(
		sourceGeneratedAt: string | undefined,
		lane?: "normal" | "preload",
	) {
		if (!sourceGeneratedAt) return undefined;
		for (let index = this.stageFrames.length - 1; index >= 0; index--) {
			const sample = this.stageFrames[index];
			if (
				sample?.sourceGeneratedAt === sourceGeneratedAt &&
				(lane === undefined || sample.lane === lane)
			)
				return sample;
		}
		return undefined;
	}

	private observeLongTasks() {
		if (typeof PerformanceObserver === "undefined") return;
		try {
			this.longTaskObserver = new PerformanceObserver((list) => {
				for (const entry of list.getEntries())
					this.longTasks.push({
						startedAt: entry.startTime,
						durationMs: entry.duration,
					});
			});
			this.longTaskObserver.observe({ type: "longtask", buffered: true });
		} catch {
			this.longTaskObserver = null;
		}
	}
}

export const frontendPerformanceDiagnostics =
	new FrontendPerformanceDiagnostics();

export async function measureFrontendSnapshot<T>(
	feature: string,
	load: () => Promise<T>,
) {
	const finish = frontendPerformanceDiagnostics.beginSnapshotRequest(feature);
	try {
		const result = await load();
		finish(result);
		return result;
	} catch (error) {
		finish(undefined, error);
		throw error;
	}
}

if (typeof window !== "undefined") {
	Object.defineProperty(window, "__TOSKLIGHT_FRONTEND_PERFORMANCE__", {
		configurable: true,
		value: {
			snapshot: () => frontendPerformanceDiagnostics.snapshot(),
			browserMemoryBytes: () =>
				frontendPerformanceDiagnostics.browserMemoryBytes(),
		},
	});
}

export function serializedModelBytes(value: unknown) {
	try {
		return new TextEncoder().encode(
			JSON.stringify(value, (_key, candidate) => {
				if (candidate instanceof Map) return [...candidate.entries()];
				if (candidate instanceof Set) return [...candidate.values()];
				if (candidate instanceof Error)
					return { name: candidate.name, message: candidate.message };
				return candidate;
			}),
		).byteLength;
	} catch {
		return 0;
	}
}

function rawEventOccurredAt(value: unknown) {
	if (!value || typeof value !== "object" || Array.isArray(value)) return null;
	const event = (value as Record<string, unknown>).event;
	if (!event || typeof event !== "object" || Array.isArray(event)) return null;
	const occurredAt = (event as Record<string, unknown>).occurred_at;
	return typeof occurredAt === "string" ? occurredAt : null;
}

function now() {
	return globalThis.performance?.now() ?? Date.now();
}

function percentile(sorted: readonly number[], value: number) {
	if (!sorted.length) return null;
	const rank = Math.ceil((value / 100) * sorted.length);
	return sorted[Math.max(0, rank - 1)];
}

function pushBounded<T>(target: T[], value: T, maximum: number) {
	target.push(value);
	if (target.length > maximum) target.splice(0, target.length - maximum);
}

function mark(name: string) {
	try {
		globalThis.performance?.mark(name);
	} catch {
		// Diagnostics must never block desk operation.
	}
}

function measure(name: string, start: string, end: string) {
	try {
		globalThis.performance?.measure(name, start, end);
	} catch {
		// A missing mark is diagnostic loss, not an operator-facing failure.
	}
}

declare global {
	interface Window {
		__TOSKLIGHT_FRONTEND_PERFORMANCE__?: {
			snapshot(): FrontendPerformanceSnapshot;
			browserMemoryBytes(): Promise<number | null>;
		};
	}
}
