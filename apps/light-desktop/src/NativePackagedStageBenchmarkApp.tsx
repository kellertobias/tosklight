import { useEffect, useRef, useState } from "react";
import { ServerRuntime } from "./api/ServerRuntime";
import { ConnectionState } from "./components/shell/ConnectionState";
import { DeskLoadingOverlay } from "./components/shell/DeskLoadingOverlay";
import { PatchFeatureBoundary } from "./features/patch/PatchFeatureBoundary";
import { useDesktopBridge } from "./platform/desktop";
import type {
	PackagedStageBenchmarkConfig,
	StagePaneBenchmarkSample,
} from "./platform/desktop/types";
import { AppProvider, useApp } from "./state/AppContext";
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
			frame.width === previous.width &&
			frame.height === previous.height
		) {
			continue;
		}
		target.push(converted);
		latestByPane.set(frame.paneId, converted);
	}
}

export function NativePackagedStageBenchmarkApp({
	config,
}: {
	config: PackagedStageBenchmarkConfig;
}) {
	return (
		<ServerRuntime sessionRole="primary">
			<AppProvider>
				<PatchFeatureBoundary>
					<NativeBenchmark config={config} />
					<ConnectionState />
					<DeskLoadingOverlay />
				</PatchFeatureBoundary>
			</AppProvider>
		</ServerRuntime>
	);
}

function NativeBenchmark({ config }: { config: PackagedStageBenchmarkConfig }) {
	const desktop = useDesktopBridge();
	const { state, dispatch } = useApp();
	const [prepared, setPrepared] = useState(false);
	const [stageEnabled, setStageEnabled] = useState(false);
	const [liveVisible, setLiveVisible] = useState(true);
	const [surfaceInset, setSurfaceInset] = useState(false);
	const frames = useRef<NativeFrameSample[]>([]);
	const latestFrameByPane = useRef(new Map<string, NativeFrameSample>());
	const timeline = useRef<Array<Record<string, unknown>>>([]);
	const startedAt = useRef<string | null>(null);
	const quality = useRef(state.stageVizQuality);
	quality.current = state.stageVizQuality;

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
			config,
		});
		const beginStage = window.setTimeout(() => {
			void desktop.appendPackagedStageBenchmarkSample({
				schemaVersion: 2,
				kind: "stage-started",
				measurementSurface: "packaged-tauri-native-stage",
				profile: config.profile,
				recordedAt: new Date().toISOString(),
			});
			setStageEnabled(true);
		}, config.controlDurationSeconds * 1_000);

		const sample = window.setInterval(() => {
			void desktop.takeStagePaneBenchmarkSamples().then((samples) => {
				const first = frames.current.length;
				appendNativeFrames(
					frames.current,
					samples,
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
			});
		}, 100);

		return () => {
			window.clearTimeout(beginStage);
			window.clearInterval(sample);
		};
	}, [config, desktop, prepared]);

	useEffect(() => {
		if (!stageEnabled) return;
		const finish = window.setTimeout(() => {
			void desktop.takeStagePaneBenchmarkSamples().then(async (samples) => {
				appendNativeFrames(
					frames.current,
					samples,
					latestFrameByPane.current,
				);
				await desktop.appendPackagedStageBenchmarkSample({
					schemaVersion: 2,
					kind: "complete",
					measurementSurface: "packaged-tauri-native-stage",
					profile: config.profile,
					startedAt: startedAt.current,
					recordedAt: new Date().toISOString(),
					activeUiSurfaces: config.fixtureSheet
						? ["stage-native-live", "stage-native-preload", "fixture-sheet"]
						: ["stage-native-live", "stage-native-preload"],
					nativeStage: {
						frames: frames.current,
						rendererLifecycleExercise: "unmount-and-recreate",
					},
					timeline: timeline.current,
					capabilities: {
						userAgent: navigator.userAgent,
						platform: navigator.platform,
						renderer: "native-helper",
					},
				});
				setStageEnabled(false);
			});
		}, config.durationSeconds * 1_000);
		return () => window.clearTimeout(finish);
	}, [config, desktop, stageEnabled]);

	useEffect(() => {
		if (!stageEnabled) return;
		const qualities = ["draft", "standard", "high", "ultra", "extreme"] as const;
		let index = 0;
		const qualityStep = Math.max(
			100,
			Math.min(1_000, (config.durationSeconds * 1_000) / 8),
		);
		const exercise = window.setInterval(() => {
			dispatch({ type: "SET_STAGE_OPTIONS", vizQuality: qualities[index] });
			index += 1;
			if (index >= qualities.length) window.clearInterval(exercise);
		}, qualityStep);
		const releaseAt = Math.min(15_000, config.durationSeconds * 350);
		const resizeAt = Math.min(8_000, config.durationSeconds * 150);
		const release = window.setTimeout(() => setLiveVisible(false), releaseAt);
		const recreate = window.setTimeout(
			() => setLiveVisible(true),
			releaseAt + Math.max(250, qualityStep),
		);
		const resize = window.setTimeout(() => setSurfaceInset(true), resizeAt);
		const restoreSize = window.setTimeout(
			() => setSurfaceInset(false),
			resizeAt + Math.max(250, qualityStep),
		);
		return () => {
			window.clearInterval(exercise);
			window.clearTimeout(release);
			window.clearTimeout(recreate);
			window.clearTimeout(resize);
			window.clearTimeout(restoreSize);
		};
	}, [config.durationSeconds, dispatch, stageEnabled]);

	return (
		<div
			style={{
				display: "grid",
				gridTemplateColumns: config.fixtureSheet
					? "minmax(0, 2fr) minmax(0, 1fr)"
					: "minmax(0, 1fr)",
				width: "100vw",
				height: "100vh",
				minWidth: 0,
				minHeight: 0,
				overflow: "hidden",
			}}
		>
			{stageEnabled ? (
				<div
					style={{
						display: "grid",
						gridTemplateRows: "minmax(0, 1fr) minmax(0, 1fr)",
						minWidth: 0,
						minHeight: 0,
						paddingRight: surfaceInset ? 48 : 0,
					}}
				>
					{liveVisible ? (
						<StageWindow
							compact
							stageView="3d-viz"
							followPreload={false}
							showGroupShortcuts={false}
							showSelection
							showFloorGrid
						/>
					) : (
						<div data-testid="native-stage-released" />
					)}
					<StageWindow
						compact
						stageView="2d"
						followPreload
						showGroupShortcuts={false}
						showSelection={false}
						showFloorGrid
					/>
				</div>
			) : (
				<div data-testid="packaged-stage-output-baseline" />
			)}
			{config.fixtureSheet && (
				<div style={{ minWidth: 0, minHeight: 0, overflow: "hidden" }}>
					<FixtureSheetWindow compact active={stageEnabled} />
				</div>
			)}
		</div>
	);
}
