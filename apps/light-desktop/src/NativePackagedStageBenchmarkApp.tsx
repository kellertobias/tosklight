import { useEffect, useMemo, useRef, useState } from "react";
import { ServerRuntime } from "./api/ServerRuntime";
import { ConnectionState } from "./components/shell/ConnectionState";
import { DeskLoadingOverlay } from "./components/shell/DeskLoadingOverlay";
import { PatchFeatureBoundary } from "./features/patch/PatchFeatureBoundary";
import {
	benchmarkStageEnabled,
	benchmarkSurfaces,
	NativeHostObservations,
} from "./nativePackagedBenchmarkMode";
import { NativeStageReadiness } from "./nativePackagedStageReadiness";
import {
	NativeStageGpuObservations,
	nativeStageMeasurementBoundaries,
} from "./nativeStageGpuObservations";
import { DesktopProvider, useDesktopBridge } from "./platform/desktop";
import { baselineStageBridge, baselineStageLayout } from "./nativePackagedStageLayout";
import type {
	DesktopBridge,
	PackagedStageBenchmarkConfig,
	StagePaneBenchmarkHeartbeat,
	StagePaneBenchmarkObservations,
	StagePaneBenchmarkSample,
} from "./platform/desktop/types";
import { AppProvider, useApp } from "./state/AppContext";
import { FixtureSheetBenchmarkDiagnostics } from "./windows/FixtureSheetBenchmarkDiagnostics";
import { FixtureSheetWindow } from "./windows/FixtureSheetWindow";
import { StageWindow } from "./windows/StageWindow";

interface NativeFrameSample extends StagePaneBenchmarkSample {
	lane: "normal" | "preload";
	sourceGeneratedAt: string | null;
	settledCanvasSubmittedAt: number;
	sourceToSettledCanvasMs: number | null;
}

function appendNativeFrames(
	target: NativeFrameSample[],
	samples: StagePaneBenchmarkSample[],
	latestByPane: Map<string, NativeFrameSample>,
) {
	for (const frame of samples) {
		const converted: NativeFrameSample = {
			...frame,
			lane: frame.followPreload ? "preload" : "normal",
			sourceGeneratedAt:
				frame.sourceInputEpochMicros > 0
					? new Date(frame.sourceInputEpochMicros / 1_000).toISOString()
					: null,
			settledCanvasSubmittedAt: frame.presentedEpochMicros / 1_000,
			sourceToSettledCanvasMs:
				frame.sourceInputEpochMicros > 0
					? Math.max(
							0,
							(frame.presentedEpochMicros - frame.sourceInputEpochMicros) /
								1_000,
						)
					: null,
		};
		const previous = latestByPane.get(frame.paneId);
		if (
			previous &&
			frame.sequence > previous.sequence &&
			frame.sourceFrame === previous.sourceFrame &&
			frame.quality === previous.quality &&
			frame.mode === previous.mode &&
			frame.width === previous.width &&
			frame.height === previous.height
		) {
			continue;
		}
		target.push(converted);
		latestByPane.set(frame.paneId, converted);
	}
}

const MAX_ACTIVITY_OBSERVATIONS = 20_000;
const MAX_HEARTBEAT_OBSERVATIONS = 10_000;

function retainNativeObservations(
	batch: StagePaneBenchmarkObservations,
	activity: StagePaneBenchmarkSample[],
	heartbeats: StagePaneBenchmarkHeartbeat[],
	overflow: { activity: number; heartbeats: number; errors: number },
) {
	// Preserve actual presentation activity before appendNativeFrames filters unchanged
	// source frames. One latest observation per attachment per poll is sufficient for
	// liveness; it is not a renderer-FPS sample stream.
	const latest = new Map<string, StagePaneBenchmarkSample>();
	for (const frame of batch.frames) {
		latest.set(`${frame.paneId}\0${frame.attachmentId}`, frame);
	}
	for (const frame of latest.values()) {
		if (activity.length < MAX_ACTIVITY_OBSERVATIONS) activity.push(frame);
		else overflow.activity += 1;
	}
	// Keep every heartbeat in arrival order so foreign/reordered generations cannot be
	// hidden by deduplication. An overflow preserves a bounded prefix and fails validation.
	for (const heartbeat of batch.heartbeats) {
		if (heartbeats.length < MAX_HEARTBEAT_OBSERVATIONS)
			heartbeats.push(heartbeat);
		else overflow.heartbeats += 1;
	}
}

interface NativeObservationContext {
	config: PackagedStageBenchmarkConfig;
	hostObservations: { current: NativeHostObservations };
	modeAcknowledged: { current: boolean };
	desktop: DesktopBridge;
	frames: { current: NativeFrameSample[] };
	gpuObservations: { current: NativeStageGpuObservations };
	latestFrameByPane: { current: Map<string, NativeFrameSample> };
	activity: { current: StagePaneBenchmarkSample[] };
	heartbeats: { current: StagePaneBenchmarkHeartbeat[] };
	observerOverflow: {
		current: { activity: number; heartbeats: number; errors: number };
	};
	timeline: { current: Array<Record<string, unknown>> };
	quality: { current: StagePaneBenchmarkSample["quality"] };
	sampleInFlight: { current: Promise<void> | null };
	finishing: { current: boolean };
	readiness: { current: NativeStageReadiness };
	stageBeganAt: { current: number | null };
}

function sampleNativeBenchmark(context: NativeObservationContext) {
	const {
		desktop,
		frames,
		latestFrameByPane,
		activity,
		heartbeats,
		observerOverflow,
		timeline,
		quality,
		sampleInFlight,
		finishing,
		readiness,
		stageBeganAt,
	} = context;
	if (finishing.current || sampleInFlight.current) return;
	sampleInFlight.current = desktop
		.takeStagePaneBenchmarkSamples()
		.then(async (batch) => {
			context.gpuObservations.current.record(batch.frames, batch.compositors);
			context.hostObservations.current.record(batch);
			if (!context.modeAcknowledged.current) {
				await desktop.appendPackagedStageBenchmarkSample({
					schemaVersion: 2,
					kind: "stage-mode-ack",
					measurementSurface: "packaged-tauri-native-stage",
					stageEnabled: context.config.stageEnabled,
					profile: context.config.profile,
					recordedAt: new Date().toISOString(),
					hostObservationCount: context.hostObservations.current.rows.length,
				});
				context.modeAcknowledged.current = true;
			}
			retainNativeObservations(
				batch,
				activity.current,
				heartbeats.current,
				observerOverflow.current,
			);
			const ready =
				context.config.stageEnabled &&
				readiness.current.observe(
					batch,
					stageBeganAt.current,
					Date.now(),
					Object.values(observerOverflow.current).some((value) => value !== 0),
				);
			if (ready) await desktop.appendPackagedStageBenchmarkSample({ ...ready });
			const first = frames.current.length;
			appendNativeFrames(
				frames.current,
				batch.frames,
				latestFrameByPane.current,
			);
			const renderSamples = frames.current.slice(first).map((frame) => ({
				benchmarkSequence: `${frame.paneId}:${frame.sequence}`,
				paneId: frame.paneId,
				submittedAt: frame.presentedEpochMicros / 1_000,
				recordedAt: frame.presentedEpochMicros / 1_000,
				renderQuality: frame.quality,
				durationMs: frame.cpuMicros / 1_000,
				calls: frame.drawCalls,
				geometries: frame.instances,
				textures: 0,
			}));
			timeline.current.push({
				recordedAt: Date.now(),
				quality: quality.current,
				renders: frames.current.length,
				newRenders: renderSamples,
				latestRender: renderSamples.at(-1) ?? null,
				mainDocumentFocused: document.hasFocus(),
			});
		})
		.catch(() => {
			observerOverflow.current.errors += 1;
		})
		.finally(() => {
			sampleInFlight.current = null;
		});
}

async function finishNativeBenchmark(
	context: NativeObservationContext,
	config: PackagedStageBenchmarkConfig,
	startedAt: string | null,
	onFinished: () => void,
) {
	const {
		desktop,
		frames,
		latestFrameByPane,
		activity,
		heartbeats,
		observerOverflow,
		timeline,
		sampleInFlight,
		finishing,
	} = context;
	finishing.current = true;
	await sampleInFlight.current;
	const batch = await desktop
		.takeStagePaneBenchmarkSamples()
		.then((batch) => {
			context.gpuObservations.current.record(batch.frames, batch.compositors);
			context.hostObservations.current.record(batch);
			return batch;
		})
		.catch(() => {
			observerOverflow.current.errors += 1;
			return null;
		});
	if (batch)
		retainNativeObservations(
			batch,
			activity.current,
			heartbeats.current,
			observerOverflow.current,
		);
	const samples = batch?.frames ?? [];
	appendNativeFrames(frames.current, samples, latestFrameByPane.current);
	await desktop.appendPackagedStageBenchmarkSample({
		schemaVersion: 2,
		kind: "complete",
		measurementSurface: "packaged-tauri-native-stage",
		profile: config.profile,
		config,
		stageEnabled: config.stageEnabled,
		startedAt,
		recordedAt: new Date().toISOString(),
		activeUiSurfaces: benchmarkSurfaces(config),
		nativeStage: {
			measurementBoundaries: nativeStageMeasurementBoundaries,
			gpuObservations: context.gpuObservations.current.report(),
			frames: frames.current,
			activity: activity.current,
			heartbeats: heartbeats.current,
			observerOverflow: observerOverflow.current,
			hostObservationCount: context.hostObservations.current.rows.length,
			hostObservations: context.hostObservations.current.rows,
			rendererLifecycleExercise:
				config.stageEnabled && !config.baselineLayout
					? "unmount-and-recreate"
					: "disabled",
		},
		timeline: timeline.current,
		capabilities: {
			userAgent: navigator.userAgent,
			renderer: config.stageEnabled ? "native-helper" : "disabled",
		},
	});
	onFinished();
}

function beginNativeStage(
	desktop: DesktopBridge,
	profile: string,
	enable: () => void,
) {
	void desktop.appendPackagedStageBenchmarkSample({
		schemaVersion: 2,
		kind: "stage-started",
		measurementSurface: "packaged-tauri-native-stage",
		profile,
		recordedAt: new Date().toISOString(),
	});
	enable();
}

export function NativePackagedStageBenchmarkApp({
	config,
}: {
	config: PackagedStageBenchmarkConfig;
}) {
	const desktop = useDesktopBridge();
	const benchmarkDesktop = useMemo(
		() => baselineStageBridge(desktop, config),
		[desktop, config],
	);
	return (
		<DesktopProvider bridge={benchmarkDesktop}>
			<ServerRuntime sessionRole="primary">
				<AppProvider>
					<PatchFeatureBoundary>
						<NativeBenchmark config={config} />
						<ConnectionState />
						<DeskLoadingOverlay />
					</PatchFeatureBoundary>
				</AppProvider>
			</ServerRuntime>
		</DesktopProvider>
	);
}

function useNativeFrameObservations() {
	const hostObservations = useRef(new NativeHostObservations());
	const frames = useRef<NativeFrameSample[]>([]);
	const gpuObservations = useRef(new NativeStageGpuObservations());
	const activity = useRef<StagePaneBenchmarkSample[]>([]);
	return { hostObservations, frames, gpuObservations, activity };
}

function NativeBenchmark({ config }: { config: PackagedStageBenchmarkConfig }) {
	const desktop = useDesktopBridge();
	const stageMode = benchmarkStageEnabled(config);
	const { dispatch } = useApp();
	const [prepared, setPrepared] = useState(false);
	const [stageEnabled, setStageEnabled] = useState(false);
	const [liveVisible, setLiveVisible] = useState(true);
	const [surfaceInset, setSurfaceInset] = useState(false);
	const { hostObservations, frames, gpuObservations, activity } =
		useNativeFrameObservations();
	const modeAcknowledged = useRef(false);
	const heartbeats = useRef<StagePaneBenchmarkHeartbeat[]>([]);
	const observerOverflow = useRef({ activity: 0, heartbeats: 0, errors: 0 });
	const sampleInFlight = useRef<Promise<void> | null>(null);
	const finishing = useRef(false);
	const readiness = useRef(new NativeStageReadiness());
	const stageBeganAt = useRef<number | null>(null);
	const latestFrameByPane = useRef(new Map<string, NativeFrameSample>());
	const timeline = useRef<Array<Record<string, unknown>>>([]);
	const startedAt = useRef<string | null>(null);
	const quality = useRef<StagePaneBenchmarkSample["quality"]>(
		config.baselineQuality ?? "draft",
	);
	const observation: NativeObservationContext = {
		config,
		hostObservations,
		modeAcknowledged,
		desktop,
		frames,
		gpuObservations,
		latestFrameByPane,
		activity,
		heartbeats,
		observerOverflow,
		timeline,
		quality,
		sampleInFlight,
		finishing,
		readiness,
		stageBeganAt,
	};

	useEffect(() => {
		let cancelled = false;
		const poll = async () => {
			if (await desktop.packagedStageBenchmarkPrepared()) {
				if (!cancelled) setPrepared(true);
				return;
			}
			if (!cancelled) window.setTimeout(() => void poll(), 100);
		};
		void poll();
		return () => {
			cancelled = true;
		};
	}, [desktop]);

	useEffect(() => {
		if (!prepared) return;
		startedAt.current = new Date().toISOString();
		void desktop.appendPackagedStageBenchmarkSample({
			schemaVersion: 2,
			kind: "started",
			measurementSurface: "packaged-tauri-native-stage",
			profile: config.profile,
			startedAt: startedAt.current,
			stageEnabled: stageMode,
			config,
		});
		const beginStage = stageMode
			? window.setTimeout(
					() =>
						beginNativeStage(desktop, config.profile, () =>
							setStageEnabled(true),
						),
					config.controlDurationSeconds * 1_000,
				)
			: undefined;

		sampleNativeBenchmark(observation);
		const sample = window.setInterval(
			() => sampleNativeBenchmark(observation),
			stageMode ? 100 : 1_000,
		);

		return () => {
			if (beginStage !== undefined) window.clearTimeout(beginStage);
			window.clearInterval(sample);
		};
	}, [config, desktop, prepared]);

	useEffect(() => {
		if (!(stageMode ? stageEnabled : prepared)) return;
		const finish = window.setTimeout(() => {
			void finishNativeBenchmark(observation, config, startedAt.current, () =>
				setStageEnabled(false),
			).catch(() => {
				observerOverflow.current.errors += 1;
			});
		}, config.durationSeconds * 1_000);
		return () => window.clearTimeout(finish);
	}, [config, desktop, stageEnabled, stageMode, prepared]);

	useEffect(() => {
		if (!stageMode || !stageEnabled) return;
		stageBeganAt.current = Date.now();
		dispatch({
			type: "SET_STAGE_OPTIONS",
			vizQuality: config.baselineQuality ?? "draft",
		});
		if (config.baselineLayout) return;
		const lifecycleStep = Math.max(
			100,
			Math.min(1_000, (config.durationSeconds * 1_000) / 8),
		);
		const releaseAt = Math.min(15_000, config.durationSeconds * 350);
		const resizeAt = Math.min(8_000, config.durationSeconds * 150);
		const release = window.setTimeout(() => setLiveVisible(false), releaseAt);
		const recreate = window.setTimeout(
			() => setLiveVisible(true),
			releaseAt + Math.max(250, lifecycleStep),
		);
		const resize = window.setTimeout(() => setSurfaceInset(true), resizeAt);
		const restoreSize = window.setTimeout(
			() => setSurfaceInset(false),
			resizeAt + Math.max(250, lifecycleStep),
		);
		return () => {
			window.clearTimeout(release);
			window.clearTimeout(recreate);
			window.clearTimeout(resize);
			window.clearTimeout(restoreSize);
		};
	}, [config, dispatch, stageEnabled, stageMode]);

	return (
		<NativeBenchmarkSurfaces
			config={config}
			stageMode={stageMode}
			stageEnabled={stageEnabled}
			prepared={prepared}
			liveVisible={liveVisible}
			surfaceInset={surfaceInset}
		/>
	);
}

function NativeBenchmarkSurfaces({
	config,
	stageMode,
	stageEnabled,
	prepared,
	liveVisible,
	surfaceInset,
}: {
	config: PackagedStageBenchmarkConfig;
	stageMode: boolean;
	stageEnabled: boolean;
	prepared: boolean;
	liveVisible: boolean;
	surfaceInset: boolean;
}) {
	const [scale, setScale] = useState(window.devicePixelRatio || 1);
	useEffect(() => {
		const resized = () => setScale(window.devicePixelRatio || 1);
		window.addEventListener("resize", resized);
		return () => window.removeEventListener("resize", resized);
	}, []);
	const baseline = config.baselineLayout
		? baselineStageLayout(config.baselineLayout, scale)
		: null;
	return (
		<div
			data-baseline-layout={config.baselineLayout ?? undefined}
			style={{
				display: "grid",
				gridTemplateColumns:
					config.fixtureSheet && !baseline
						? "minmax(0, 2fr) minmax(0, 1fr)"
						: "minmax(0, 1fr)",
				width: baseline?.width ?? "100vw",
				height: baseline?.height ?? "100vh",
				minWidth: 0,
				minHeight: 0,
				overflow: "hidden",
			}}
		>
			{stageMode && stageEnabled ? (
				<div
					style={{
						display: "grid",
						gridTemplateRows:
							baseline?.panes === 1
								? "minmax(0, 1fr)"
								: "minmax(0, 1fr) minmax(0, 1fr)",
						minWidth: 0,
						minHeight: 0,
						paddingRight: surfaceInset ? 48 : 0,
					}}
				>
					{liveVisible ? (
						<StageWindow
							compact
							stageView="3d"
							followPreload={false}
							showGroupShortcuts={false}
							showSelection
							showFloorGrid
						/>
					) : (
						<div data-testid="native-stage-released" />
					)}
					{baseline?.panes !== 1 && (
						<StageWindow
							compact
							stageView={baseline ? "3d" : "2d"}
							followPreload
							showGroupShortcuts={false}
							showSelection={false}
							showFloorGrid
						/>
					)}
				</div>
			) : (
				<div data-testid="packaged-stage-output-baseline" />
			)}
			{config.fixtureSheet && !baseline && (
				<div style={{ minWidth: 0, minHeight: 0, overflow: "hidden" }}>
					<FixtureSheetBenchmarkDiagnostics profile={config.profile}>
						<FixtureSheetWindow
							compact
							active={stageMode ? stageEnabled : prepared}
						/>
					</FixtureSheetBenchmarkDiagnostics>
				</div>
			)}
		</div>
	);
}
