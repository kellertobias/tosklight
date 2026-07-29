import {
	type MutableRefObject,
	useEffect,
	useRef,
	useState,
} from "react";
import { ServerRuntime } from "./api/ServerRuntime";
import { ConnectionState } from "./components/shell/ConnectionState";
import { DeskLoadingOverlay } from "./components/shell/DeskLoadingOverlay";
import { frontendPerformanceDiagnostics } from "./features/frontendWarmup/diagnostics";
import { PatchFeatureBoundary } from "./features/patch/PatchFeatureBoundary";
import { useProgrammerPreloadLifecycleView } from "./features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView";
import {
	useProgrammingCommandLineActions,
	useProgrammingCommandLineReady,
} from "./features/programmingInteraction/ProgrammingInteractionView";
import { useDesktopBridge } from "./platform/desktop";
import { AppProvider } from "./state/AppContext";
import type { StageRenderQuality } from "./types";
import { FixtureSheetWindow } from "./windows/FixtureSheetWindow";
import { StageWindow } from "./windows/StageWindow";

const qualities: readonly StageRenderQuality[] = [
	"lines_only",
	"lines_and_beams",
	"beams",
	"improved_beams",
];

interface PackagedStageBenchmarkAppProps {
	durationSeconds: number;
	profile: string;
}

type AdditionalStageWindowState = "pending" | "opened" | "error";

interface PackagedBenchmarkState {
	quality: StageRenderQuality;
	liveView: "2d" | "3d";
	liveVisible: boolean;
	additionalStageWindow: AdditionalStageWindowState;
	contextRecoveryMethod: ContextRecoveryMethod;
	activeUiSurfaces: readonly ["stage-3d", "fixture-sheet"];
}

function useAdditionalStageWindow(
	desktop: ReturnType<typeof useDesktopBridge>,
	stageEnabled: boolean,
	setAdditionalStageWindow: (state: AdditionalStageWindowState) => void,
) {
	useEffect(() => {
		if (!stageEnabled) return;
		const timer = window.setTimeout(() => {
			void desktop.openStageViewWindow().then(
				() => setAdditionalStageWindow("opened"),
				() => setAdditionalStageWindow("error"),
			);
		}, 2_000);
		return () => window.clearTimeout(timer);
	}, [desktop, setAdditionalStageWindow, stageEnabled]);
}

function usePackagedStagePhases(
	stageEnabled: boolean,
	setQualityIndex: (update: (current: number) => number) => void,
	setLiveView: (view: "2d" | "3d") => void,
	setLiveVisible: (visible: boolean) => void,
	benchmarkState: MutableRefObject<PackagedBenchmarkState>,
) {
	useEffect(() => {
		if (!stageEnabled) return;
		const startedAt = Date.now();
		let lastContextRecoveryCycle = -1;
		const phase = window.setInterval(() => {
			const elapsed = Date.now() - startedAt;
			setQualityIndex((current) => (current + 1) % qualities.length);
			setLiveView(elapsed % 12_000 >= 8_000 ? "2d" : "3d");
			setLiveVisible(
				!(elapsed % 15_000 >= 12_000 && elapsed % 15_000 < 13_000),
			);
			const contextRecoveryCycle = Math.floor(elapsed / 18_000);
			if (
				elapsed % 18_000 >= 16_000 &&
				contextRecoveryCycle !== lastContextRecoveryCycle
			) {
				lastContextRecoveryCycle = contextRecoveryCycle;
				const canvas = document.querySelector<HTMLCanvasElement>(
					".stage-3d-canvas canvas",
				);
				benchmarkState.current.contextRecoveryMethod =
					exerciseContextRecovery(canvas);
			}
		}, 1_000);
		return () => window.clearInterval(phase);
	}, [
		benchmarkState,
		setLiveView,
		setLiveVisible,
		setQualityIndex,
		stageEnabled,
	]);
}

function PackagedStageExercise() {
	const commandReady = useProgrammingCommandLineReady();
	const command = useProgrammingCommandLineActions();
	const preload = useProgrammerPreloadLifecycleView();
	const commandRef = useRef(command);
	const preloadActionsRef = useRef(preload.actions);
	commandRef.current = command;
	preloadActionsRef.current = preload.actions;

	useEffect(() => {
		if (!commandReady || !preload.ready) return;
		let cancelled = false;
		const wait = (milliseconds: number) =>
			new Promise<void>((resolve) => window.setTimeout(resolve, milliseconds));
		const run = async () => {
			let high = false;
			while (!cancelled) {
				const commandActions = commandRef.current;
				const preloadActions = preloadActionsRef.current;
				if (!commandActions || !preloadActions) {
					await wait(100);
					continue;
				}
				await commandActions.execute(`FIXTURE 101 AT ${high ? 20 : 80}`);
				await wait(750);
				if (cancelled) break;
				await preloadActions.enter();
				await wait(250);
				await commandActions.execute(`FIXTURE 101 AT ${high ? 80 : 20}`);
				await wait(750);
				await preloadActions.release();
				high = !high;
				await wait(250);
			}
		};
		void run().catch(() => undefined);
		return () => {
			cancelled = true;
		};
	}, [commandReady, preload.ready]);
	return null;
}

export function PackagedStageBenchmarkApp({
	durationSeconds,
	profile,
}: PackagedStageBenchmarkAppProps) {
	const desktop = useDesktopBridge();
	const [prepared, setPrepared] = useState(false);

	useEffect(() => {
		let cancelled = false;
		const poll = async () => {
			while (!cancelled) {
				const isPrepared = await desktop
					.packagedStageBenchmarkPrepared()
					.catch(() => false);
				if (isPrepared) {
					setPrepared(true);
					return;
				}
				await new Promise<void>((resolve) => window.setTimeout(resolve, 100));
			}
		};
		void poll();
		return () => {
			cancelled = true;
		};
	}, [desktop]);

	if (!prepared) {
		return (
			<div data-testid="packaged-stage-preparing">
				Preparing packaged Stage profile {profile}
			</div>
		);
	}

	return (
		<PreparedPackagedStageBenchmark
			durationSeconds={durationSeconds}
			profile={profile}
		/>
	);
}

type PackagedBenchmarkTimelineSample = ReturnType<
	typeof frontendPerformanceDiagnostics.stageBenchmarkSample
> & {
	quality: StageRenderQuality;
	liveView: "2d" | "3d";
	liveVisible: boolean;
	additionalStageWindow: AdditionalStageWindowState;
};

async function recordPackagedStageComplete(
	desktop: ReturnType<typeof useDesktopBridge>,
	profile: string,
	startedAt: string,
	timeline: PackagedBenchmarkTimelineSample[],
	benchmarkState: MutableRefObject<PackagedBenchmarkState>,
) {
	const frontend = frontendPerformanceDiagnostics.stageBenchmarkSample();
	const frontendSnapshot = frontendPerformanceDiagnostics.snapshot();
	try {
		await desktop.appendPackagedStageBenchmarkSample({
			schemaVersion: 1,
			kind: "complete",
			measurementSurface: "packaged-tauri-webview",
			profile,
			startedAt,
			recordedAt: new Date().toISOString(),
			...benchmarkState.current,
			timeline,
			frontend: {
				stage: {
					frames: frontendSnapshot.stage.frames,
					sceneBuilds: frontend.sceneBuilds,
					renders: frontend.renders,
					rafCallbacks: frontend.rafCallbacks,
					rendererContextsCreated: frontend.rendererContextsCreated,
					rendererContextsDisposed: frontend.rendererContextsDisposed,
					rendererContextLosses: frontend.rendererContextLosses,
					rendererContextRestores: frontend.rendererContextRestores,
					desktopMirrorRenders: frontend.desktopMirrorRenders,
					modelCacheHits: frontend.modelCacheHits,
					modelCacheMisses: frontend.modelCacheMisses,
					modelCacheDisposals: frontend.modelCacheDisposals,
				},
			},
			initialBrowserMemoryBytes: null,
			browserMemoryBytes: null,
			capabilities: {
				userAgent: navigator.userAgent,
				platform: navigator.platform,
				...frontend.rendererCapabilities,
			},
		});
	} catch (reason) {
		await desktop
			.appendPackagedStageBenchmarkSample({
				schemaVersion: 1,
				kind: "error",
				message: reason instanceof Error ? reason.message : String(reason),
			})
			.catch(() => undefined);
	}
}

function usePackagedStageSampling(
	desktop: ReturnType<typeof useDesktopBridge>,
	durationSeconds: number,
	profile: string,
	setStageEnabled: (enabled: boolean) => void,
	benchmarkState: MutableRefObject<PackagedBenchmarkState>,
) {
	useEffect(() => {
		const startedAt = new Date().toISOString();
		void desktop.appendPackagedStageBenchmarkSample({
			schemaVersion: 1,
			kind: "started",
			measurementSurface: "packaged-tauri-webview",
			profile,
			startedAt,
		});
		const timerProbe = window.setTimeout(() => {
			void desktop.appendPackagedStageBenchmarkSample({
				schemaVersion: 1,
				kind: "timer",
				recordedAt: new Date().toISOString(),
			});
		}, 1_000);
		const stageStart = window.setTimeout(() => {
			void desktop.appendPackagedStageBenchmarkSample({
				schemaVersion: 1,
				kind: "stage-started",
				measurementSurface: "packaged-tauri-webview",
				profile,
				recordedAt: new Date().toISOString(),
			});
			setStageEnabled(true);
		}, durationSeconds * 1_000);
		const timeline: PackagedBenchmarkTimelineSample[] = [];
		let renderCursor = 0;
		let completed = false;
		const sample = () => {
			const frontend =
				frontendPerformanceDiagnostics.stageBenchmarkSample(renderCursor);
			renderCursor = frontend.latestRenderSequence;
			timeline.push({ ...benchmarkState.current, ...frontend });
		};
		const recordComplete = () =>
			recordPackagedStageComplete(
				desktop,
				profile,
				startedAt,
				timeline,
				benchmarkState,
			);
		sample();
		const sampler = window.setInterval(sample, 100);
		const finish = window.setTimeout(() => {
			completed = true;
			window.clearInterval(sampler);
			sample();
			void desktop.appendPackagedStageBenchmarkSample({
				schemaVersion: 1,
				kind: "finishing",
				recordedAt: new Date().toISOString(),
			});
			void recordComplete();
		}, durationSeconds * 2 * 1_000);
		return () => {
			window.clearTimeout(timerProbe);
			window.clearTimeout(stageStart);
			window.clearInterval(sampler);
			window.clearTimeout(finish);
			if (!completed) {
				sample();
				void recordComplete();
			}
		};
	}, [benchmarkState, desktop, durationSeconds, profile, setStageEnabled]);
}

function PreparedPackagedStageBenchmark({
	durationSeconds,
	profile,
}: PackagedStageBenchmarkAppProps) {
	const desktop = useDesktopBridge();
	const [stageEnabled, setStageEnabled] = useState(false);
	const [qualityIndex, setQualityIndex] = useState(0);
	const [liveVisible, setLiveVisible] = useState(true);
	const [liveView, setLiveView] = useState<"2d" | "3d">("3d");
	const [additionalStageWindow, setAdditionalStageWindow] =
		useState<AdditionalStageWindowState>("pending");
	const benchmarkState = useRef<PackagedBenchmarkState>({
		quality: qualities[0],
		liveView,
		liveVisible,
		additionalStageWindow,
		contextRecoveryMethod: "not_attempted" as ContextRecoveryMethod,
		activeUiSurfaces: ["stage-3d", "fixture-sheet"],
	});
	benchmarkState.current = {
		quality: qualities[qualityIndex] ?? "lines_and_beams",
		liveView,
		liveVisible,
		additionalStageWindow,
		contextRecoveryMethod: benchmarkState.current.contextRecoveryMethod,
		activeUiSurfaces: ["stage-3d", "fixture-sheet"],
	};
	useAdditionalStageWindow(desktop, stageEnabled, setAdditionalStageWindow);
	usePackagedStagePhases(
		stageEnabled,
		setQualityIndex,
		setLiveView,
		setLiveVisible,
		benchmarkState,
	);
	usePackagedStageSampling(
		desktop,
		durationSeconds,
		profile,
		setStageEnabled,
		benchmarkState,
	);

	const quality = qualities[qualityIndex] ?? "lines_and_beams";
	return (
		<ServerRuntime sessionRole="primary">
			<PackagedStageExercise />
			<AppProvider>
				<PatchFeatureBoundary>
					<div
						style={{
							display: "grid",
							gridTemplateColumns: "1fr 1fr",
							height: "100vh",
							minHeight: 0,
						}}
					>
						{stageEnabled && liveVisible ? (
							<StageWindow
								compact
								stageView={liveView}
								showGroupShortcuts={false}
								followPreload={false}
								stageRenderQuality={quality}
								showSelection
								showFloorGrid
								showBeamGuides
							/>
						) : (
							<div
								data-testid={
									stageEnabled
										? "packaged-stage-live-released"
										: "packaged-stage-output-baseline"
								}
								style={
									stageEnabled
										? undefined
										: {
												alignItems: "center",
												background: "#090d12",
												color: "#dbe8f5",
												display: "flex",
												flexDirection: "column",
												fontFamily: "system-ui, sans-serif",
												gap: 8,
												justifyContent: "center",
												textAlign: "center",
											}
								}
							>
								{!stageEnabled && (
									<>
										<strong>Packaged Stage benchmark</strong>
										<span>No Stage output baseline</span>
										<small>
											Stage views appear after {durationSeconds} seconds.
										</small>
									</>
								)}
							</div>
						)}
						{stageEnabled ? (
							<div data-testid="packaged-stage-fixture-sheet">
								<FixtureSheetWindow compact />
							</div>
						) : (
							<div
								data-testid="packaged-stage-output-baseline-secondary"
								style={{ background: "#090d12" }}
							/>
						)}
					</div>
					<ConnectionState />
					<DeskLoadingOverlay />
				</PatchFeatureBoundary>
			</AppProvider>
		</ServerRuntime>
	);
}

type ContextRecoveryMethod =
	| "not_attempted"
	| "canvas_unavailable"
	| "webgl_lose_context"
	| "synthetic_events";

function exerciseContextRecovery(
	canvas: HTMLCanvasElement | null,
): ContextRecoveryMethod {
	if (!canvas) return "canvas_unavailable";
	const context =
		canvas.getContext("webgl2") ?? canvas.getContext("webgl") ?? null;
	const extension = context?.getExtension("WEBGL_lose_context");
	if (extension) {
		extension.loseContext();
		window.setTimeout(() => extension.restoreContext(), 50);
		return "webgl_lose_context";
	}
	canvas.dispatchEvent(new Event("webglcontextlost", { cancelable: true }));
	canvas.dispatchEvent(new Event("webglcontextrestored"));
	return "synthetic_events";
}
