import fs from "node:fs/promises";
import path from "node:path";
import type { Page, TestInfo } from "@playwright/test";
import type { RuntimeDiagnosticsSnapshot } from "../../../apps/light-desktop/src/api/generated/light-wire";
import type {
	FrontendPerformanceSnapshot,
	FrontendStageDiagnostics,
} from "../../../apps/light-desktop/src/features/frontendWarmup/diagnostics";
import artifactResolver from "../../../tools/artifact-paths.cjs";
import {
	histogramPercentileMicros,
	type OutputWindow,
	outputWindow,
} from "../../../tools/output-histogram.mjs";
import type { ApiDriver } from "../core/api";

const { artifactPaths } = artifactResolver;

export type StageMeasurementProfile = "default-stage" | "large-stage";

export interface StagePerformanceEvidence {
	schemaVersion: 2;
	profile: StageMeasurementProfile;
	measurementSurface: "browser-playwright";
	packagedWebView: {
		controlled: false;
		measured: false;
		reason: string;
	};
	acceptanceGateEnforced: false;
	scene: {
		fixtureRecords: number;
		fixtureInstances: number;
	};
	window: {
		startedAt: string;
		finishedAt: string;
		elapsedMs: number;
	};
	frontend: {
		visualizationRequests: FrontendStageDiagnostics["visualizationRequests"];
		frames: FrontendStageDiagnostics["frames"];
		sceneBuilds: FrontendStageDiagnostics["sceneBuilds"];
		modelLoads: FrontendStageDiagnostics["modelLoads"];
		modelCacheHits: number;
		modelCacheMisses: number;
		modelClones: number;
		modelCacheDisposals: number;
		renders: FrontendStageDiagnostics["renders"];
		sourceToSettledCanvasMs: Distribution;
		presentationGapMs: Distribution;
		requestDurationMs: Distribution;
		requestPayloadBytes: number;
		maxDrawCalls: number;
		maxTriangles: number;
		maxGeometries: number;
		maxTextures: number;
		sceneDisposals: number;
		rendererContextsCreated: number;
		rendererContextsDisposed: number;
		rafCallbacks: number;
		browserMemoryBytes: number | null;
	};
	server: {
		before: RuntimeDiagnosticsSnapshot;
		afterNoStage: RuntimeDiagnosticsSnapshot;
		after: RuntimeDiagnosticsSnapshot;
		noStageOutputDelta: OutputWindow;
		outputDelta: OutputWindow;
		outputComparison: {
			latestTickDifferenceMicros: number;
			latestTickDifferencePercent: number | null;
			cumulativeMaximumTickIncreaseMicros: number;
			noStageP99TickMicros: number | null;
			stageP99TickMicros: number | null;
			allowedP99RegressionMicros: number | null;
			p99RegressionMicros: number | null;
			boundedWindowGatePassed: boolean;
			stageWindowDeadlineMisses: number;
			releaseGateEnforced: false;
		};
		visualizationWindow: {
			projections: number;
			skippedSourceFrames: number;
			latestProjectionMicros: number;
			latestPayloadBytes: number;
			latestSourceAgeMillis: number;
			streamSendFailures: number;
			streamQueueDrops: number;
			finalStreamQueueDepth: number;
		};
	};
	limitations: string[];
}

interface Distribution {
	samples: number;
	p50Ms: number | null;
	p95Ms: number | null;
	maxMs: number | null;
}

interface StageMeasurementOptions {
	page: Page;
	api: ApiDriver;
	testInfo: TestInfo;
	profile: StageMeasurementProfile;
	fixtureRecords: number;
	fixtureInstances: number;
	noStageExercise: () => Promise<void>;
	exercise: () => Promise<void>;
}

export async function measureStagePerformance(
	options: StageMeasurementOptions,
): Promise<StagePerformanceEvidence> {
	const serverBefore = await runtimeDiagnostics(options.api);
	const startedAt = new Date();
	const monotonicStartedAt = performance.now();

	await options.noStageExercise();
	const serverAfterNoStage = await runtimeDiagnostics(options.api);
	const frontendBefore = await frontendDiagnostics(options.page);
	await options.exercise();
	await options.page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
			),
	);
	await options.page.waitForTimeout(250);

	const frontendAfter = await frontendDiagnostics(options.page);
	const browserMemoryBytes =
		(await options.page.evaluate(() =>
			window.__TOSKLIGHT_FRONTEND_PERFORMANCE__?.browserMemoryBytes(),
		)) ?? null;
	const serverAfter = await runtimeDiagnostics(options.api);
	const finishedAt = new Date();
	const stage = stageDelta(frontendBefore.stage, frontendAfter.stage);
	const noStageOutputDelta = outputWindow(
		serverBefore.output,
		serverAfterNoStage.output,
	);
	const outputDelta = outputWindow(
		serverAfterNoStage.output,
		serverAfter.output,
	);
	const noStageP99TickMicros = histogramPercentileMicros(
		noStageOutputDelta,
		99,
	);
	const stageP99TickMicros = histogramPercentileMicros(outputDelta, 99);
	const allowedP99RegressionMicros =
		noStageP99TickMicros === null
			? null
			: Math.max(1_000, noStageP99TickMicros * 0.05);
	const p99RegressionMicros =
		noStageP99TickMicros === null || stageP99TickMicros === null
			? null
			: stageP99TickMicros - noStageP99TickMicros;
	const canvasTimestamps = stage.frames
		.map(({ settledCanvasSubmittedAt }) => settledCanvasSubmittedAt)
		.filter((value): value is number => value !== null)
		.sort((left, right) => left - right);
	const evidence: StagePerformanceEvidence = {
		schemaVersion: 2,
		profile: options.profile,
		measurementSurface: "browser-playwright",
		packagedWebView: {
			controlled: false,
			measured: false,
			reason:
				"The repository Playwright bench controls Chromium and has no automation bridge into the packaged Tauri WebView.",
		},
		acceptanceGateEnforced: false,
		scene: {
			fixtureRecords: options.fixtureRecords,
			fixtureInstances: options.fixtureInstances,
		},
		window: {
			startedAt: startedAt.toISOString(),
			finishedAt: finishedAt.toISOString(),
			elapsedMs: performance.now() - monotonicStartedAt,
		},
		frontend: {
			visualizationRequests: stage.visualizationRequests,
			frames: stage.frames,
			sceneBuilds: stage.sceneBuilds,
			modelLoads: stage.modelLoads,
			modelCacheHits: stage.modelCacheHits,
			modelCacheMisses: stage.modelCacheMisses,
			modelClones: stage.modelClones,
			modelCacheDisposals: stage.modelCacheDisposals,
			renders: stage.renders,
			sourceToSettledCanvasMs: distribution(
				stage.frames.flatMap(({ sourceToSettledCanvasMs }) =>
					sourceToSettledCanvasMs === null ? [] : [sourceToSettledCanvasMs],
				),
			),
			presentationGapMs: distribution(
				canvasTimestamps
					.slice(1)
					.map((timestamp, index) => timestamp - canvasTimestamps[index]),
			),
			requestDurationMs: distribution(
				stage.visualizationRequests.flatMap(({ durationMs }) =>
					durationMs === null ? [] : [durationMs],
				),
			),
			requestPayloadBytes: stage.visualizationRequests.reduce(
				(total, request) => total + (request.payloadBytes ?? 0),
				0,
			),
			maxDrawCalls: maximum(stage.renders.map(({ calls }) => calls)),
			maxTriangles: maximum(stage.renders.map(({ triangles }) => triangles)),
			maxGeometries: maximum(stage.renders.map(({ geometries }) => geometries)),
			maxTextures: maximum(stage.renders.map(({ textures }) => textures)),
			sceneDisposals: stage.sceneDisposals,
			rendererContextsCreated: stage.rendererContextsCreated,
			rendererContextsDisposed: stage.rendererContextsDisposed,
			rafCallbacks: stage.rafCallbacks,
			browserMemoryBytes,
		},
		server: {
			before: serverBefore,
			afterNoStage: serverAfterNoStage,
			after: serverAfter,
			noStageOutputDelta,
			outputDelta,
			outputComparison: {
				latestTickDifferenceMicros:
					serverAfter.output.last_tick_micros -
					serverAfterNoStage.output.last_tick_micros,
				latestTickDifferencePercent: percentageDifference(
					serverAfterNoStage.output.last_tick_micros,
					serverAfter.output.last_tick_micros,
				),
				cumulativeMaximumTickIncreaseMicros:
					serverAfter.output.maximum_tick_micros -
					serverAfterNoStage.output.maximum_tick_micros,
				noStageP99TickMicros,
				stageP99TickMicros,
				allowedP99RegressionMicros,
				p99RegressionMicros,
				boundedWindowGatePassed:
					p99RegressionMicros !== null &&
					allowedP99RegressionMicros !== null &&
					p99RegressionMicros <= allowedP99RegressionMicros,
				stageWindowDeadlineMisses:
					serverAfter.output.deadline_misses -
					serverAfterNoStage.output.deadline_misses,
				releaseGateEnforced: false,
			},
			visualizationWindow: {
				projections:
					serverAfter.visualization.projections -
					serverAfterNoStage.visualization.projections,
				skippedSourceFrames:
					serverAfter.visualization.skipped_source_frames -
					serverAfterNoStage.visualization.skipped_source_frames,
				latestProjectionMicros: serverAfter.visualization.projection_micros,
				latestPayloadBytes: serverAfter.visualization.payload_bytes,
				latestSourceAgeMillis: serverAfter.visualization.source_age_millis,
				streamSendFailures:
					serverAfter.visualization.stream_send_failures -
					serverAfterNoStage.visualization.stream_send_failures,
				streamQueueDrops:
					serverAfter.visualization.stream_queue_drops -
					serverAfterNoStage.visualization.stream_queue_drops,
				finalStreamQueueDepth: serverAfter.visualization.stream_queue_depth,
			},
		},
		limitations: [
			"This evidence is a Chromium/Playwright engineering baseline, not packaged macOS WebView evidence.",
			"The server output p99 uses paired bounded transition windows derived from cumulative fixed-bucket scheduler histograms; it is not the five-minute packaged release gate.",
			"The large profile adds a separate authenticated WebSocket whose underlying TCP reader is paused; queue replacement or send timeout is recorded independently from the browser message-delivery recovery case.",
			"Thresholds remain informational until the packaged WebView can be controlled and sampled.",
		],
	};
	await writeEvidence(evidence, options.testInfo);
	return evidence;
}

function stageDelta(
	before: FrontendStageDiagnostics,
	after: FrontendStageDiagnostics,
): FrontendStageDiagnostics {
	return {
		visualizationRequests: after.visualizationRequests.slice(
			before.visualizationRequests.length,
		),
		frames: after.frames.slice(before.frames.length),
		sceneBuilds: after.sceneBuilds.slice(before.sceneBuilds.length),
		modelLoads: after.modelLoads.slice(before.modelLoads.length),
		modelCacheHits: after.modelCacheHits - before.modelCacheHits,
		modelCacheMisses: after.modelCacheMisses - before.modelCacheMisses,
		modelClones: after.modelClones - before.modelClones,
		modelCacheDisposals: after.modelCacheDisposals - before.modelCacheDisposals,
		renders: after.renders.slice(before.renders.length),
		sceneDisposals: after.sceneDisposals - before.sceneDisposals,
		rendererContextsCreated:
			after.rendererContextsCreated - before.rendererContextsCreated,
		rendererContextsDisposed:
			after.rendererContextsDisposed - before.rendererContextsDisposed,
		rafCallbacks: after.rafCallbacks - before.rafCallbacks,
	};
}

async function frontendDiagnostics(
	page: Page,
): Promise<FrontendPerformanceSnapshot> {
	return page.evaluate(() => {
		const diagnostics = window.__TOSKLIGHT_FRONTEND_PERFORMANCE__?.snapshot();
		if (!diagnostics)
			throw new Error("Frontend performance diagnostics are unavailable");
		return diagnostics;
	});
}

function runtimeDiagnostics(
	api: ApiDriver,
): Promise<RuntimeDiagnosticsSnapshot> {
	return api.request("GET", "/api/v2/diagnostics");
}

function distribution(values: readonly number[]): Distribution {
	const sorted = [...values].sort((left, right) => left - right);
	return {
		samples: sorted.length,
		p50Ms: percentile(sorted, 50),
		p95Ms: percentile(sorted, 95),
		maxMs: sorted.at(-1) ?? null,
	};
}

function percentile(values: readonly number[], value: number): number | null {
	if (values.length === 0) return null;
	const rank = Math.ceil((value / 100) * values.length);
	return values[Math.max(0, rank - 1)];
}

function maximum(values: readonly number[]): number {
	return values.length === 0 ? 0 : Math.max(...values);
}

function percentageDifference(
	baseline: number,
	candidate: number,
): number | null {
	return baseline <= 0 ? null : ((candidate - baseline) / baseline) * 100;
}

async function writeEvidence(
	evidence: StagePerformanceEvidence,
	testInfo: TestInfo,
): Promise<void> {
	const directory = path.join(artifactPaths.performance, "stage");
	await fs.mkdir(directory, { recursive: true });
	const filename = [
		evidence.profile,
		testInfo.project.name,
		`worker-${testInfo.workerIndex}`,
		`retry-${testInfo.retry}`,
		`${Date.now()}.json`,
	]
		.map(safeArtifactSegment)
		.filter(Boolean)
		.join("-");
	const evidencePath = path.join(directory, filename);
	await fs.writeFile(evidencePath, JSON.stringify(evidence, null, 2));
	await testInfo.attach(`stage-performance-${evidence.profile}.json`, {
		path: evidencePath,
		contentType: "application/json",
	});
}

function safeArtifactSegment(value: string | number): string {
	return String(value)
		.toLowerCase()
		.replace(/[^a-z0-9.-]+/gu, "-")
		.replace(/^-+|-+$/gu, "");
}
