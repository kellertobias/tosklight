import {
	type MutableRefObject,
	useCallback,
	useEffect,
	useRef,
	useState,
} from "react";
import { ServerRuntime } from "./api/ServerRuntime";
import { CommandLineBar } from "./components/control/CommandLineBar";
import { PlaybackFaderBank } from "./components/control/PlaybackFaderBank";
import { ConnectionState } from "./components/shell/ConnectionState";
import { DeskLoadingOverlay } from "./components/shell/DeskLoadingOverlay";
import { useActiveShowId } from "./features/deskSnapshot/DeskSnapshotState";
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
	controlDurationSeconds: number;
	profile: string;
	additionalStageWindow: boolean;
	fixtureSheet: boolean;
}

type AdditionalStageWindowState = "pending" | "opened" | "error" | "disabled";

interface PackagedBenchmarkState {
	quality: StageRenderQuality;
	liveView: "2d" | "3d";
	liveVisible: boolean;
	additionalStageWindow: AdditionalStageWindowState;
	contextRecoveryMethod: ContextRecoveryMethod;
	contextRecovery: {
		startedAt: string | null;
		finishedAt: string | null;
	};
	activeUiSurfaces: readonly string[];
	playbackActions: PackagedPlaybackActionSample[];
	fixtureSheetActions: PackagedFixtureSheetSample[];
}

interface PackagedPlaybackActionSample {
	action: "go" | "flash_press" | "flash_release" | "master";
	input: "dom_pointer" | "dom_range";
	inputAt: number;
	indicationAt: number | null;
	indicationMillis: number | null;
	changed: boolean;
}

interface PackagedFixtureSheetSample {
	action: "single" | "burst";
	inputAt: number;
	settledAt: number | null;
	convergenceMillis: number | null;
	changed: boolean;
	visibleRows: number;
	loadingOverlayVisible: boolean;
}

function useAdditionalStageWindow(
	desktop: ReturnType<typeof useDesktopBridge>,
	stageEnabled: boolean,
	setAdditionalStageWindow: (state: AdditionalStageWindowState) => void,
) {
	useEffect(() => {
		if (!stageEnabled) return;
		const timer = window.setTimeout(() => {
			void desktop
				.openStageViewWindow()
				.then(() => desktop.focusPackagedStageBenchmarkWindow())
				.then(
					() => setAdditionalStageWindow("opened"),
					() => setAdditionalStageWindow("error"),
				);
		}, 30_000);
		return () => window.clearTimeout(timer);
	}, [desktop, setAdditionalStageWindow, stageEnabled]);
}

function usePackagedStagePhases(
	stageEnabled: boolean,
	durationSeconds: number,
	exerciseLifecycle: boolean,
	setQualityIndex: (update: (current: number) => number) => void,
	setLiveView: (view: "2d" | "3d") => void,
	setLiveVisible: (visible: boolean) => void,
	benchmarkState: MutableRefObject<PackagedBenchmarkState>,
) {
	useEffect(() => {
		if (!stageEnabled) return;
		const startedAt = Date.now();
		let recoveryPinned = false;
		const phase = window.setInterval(() => {
			const elapsed = Date.now() - startedAt;
			if (elapsed < 4_000) {
				setQualityIndex((current) => (current + 1) % qualities.length);
			} else {
				// Exercise every renderer tier once, then leave the measured desk
				// in its automatic performance quality. Continuously
				// rebuilding quality-dependent resources measures a stress loop,
				// not normal operator latency.
				setQualityIndex(() => 0);
			}
			if (recoveryPinned) {
				setLiveView("3d");
				setLiveVisible(true);
				return;
			}
			const lifecycleElapsed = elapsed - 30_000;
			setLiveView(
				lifecycleElapsed >= 0 && lifecycleElapsed % 60_000 < 4_000
					? "2d"
					: "3d",
			);
			setLiveVisible(
				!(
					lifecycleElapsed >= 0 &&
					lifecycleElapsed % 60_000 >= 15_000 &&
					lifecycleElapsed % 60_000 < 16_000
				),
			);
		}, 1_000);
		let recoveryRetry: number | null = null;
		let recoveryRelease: number | null = null;
		const recover = () => {
			const canvas = document.querySelector<HTMLCanvasElement>(
				".stage-3d-canvas canvas",
			);
			const recoveryDeadlineMillis = Math.max(
				22_000,
				(durationSeconds - 2) * 1_000,
			);
			if (!canvas && Date.now() - startedAt < recoveryDeadlineMillis) {
				recoveryRetry = window.setTimeout(recover, 100);
				return;
			}
			benchmarkState.current.contextRecovery = {
				startedAt: new Date().toISOString(),
				finishedAt: null,
			};
			benchmarkState.current.contextRecoveryMethod =
				exerciseContextRecovery(canvas);
			recoveryRelease = window.setTimeout(() => {
				recoveryPinned = false;
				benchmarkState.current.contextRecovery = {
					...benchmarkState.current.contextRecovery,
					finishedAt: new Date().toISOString(),
				};
			}, 10_000);
		};
		const recovery = exerciseLifecycle
			? window.setTimeout(() => {
					// Pin a committed 3D canvas before invoking WEBGL_lose_context. A
					// delayed phase callback must not unmount the selected canvas between
					// loseContext() and WebKit's asynchronous loss event.
					recoveryPinned = true;
					setLiveView("3d");
					setLiveVisible(true);
					recoveryRetry = window.setTimeout(recover, 100);
				}, 16_000)
			: null;
		return () => {
			window.clearInterval(phase);
			if (recovery !== null) window.clearTimeout(recovery);
			if (recoveryRetry !== null) window.clearTimeout(recoveryRetry);
			if (recoveryRelease !== null) window.clearTimeout(recoveryRelease);
		};
	}, [
		benchmarkState,
		durationSeconds,
		exerciseLifecycle,
		setLiveView,
		setLiveVisible,
		setQualityIndex,
		stageEnabled,
	]);
}

function PackagedStageExercise({
	profile,
	onPlaybackAction,
	onFixtureSheetAction,
}: {
	profile: string;
	onPlaybackAction(sample: PackagedPlaybackActionSample): void;
	onFixtureSheetAction(sample: PackagedFixtureSheetSample): void;
}) {
	const exerciseFixtureNumber = 1;
	const activeShowId = useActiveShowId();
	const commandReady = useProgrammingCommandLineReady();
	const command = useProgrammingCommandLineActions();
	const preload = useProgrammerPreloadLifecycleView();
	const commandRef = useRef(command);
	const preloadActionsRef = useRef(preload.actions);
	commandRef.current = command;
	preloadActionsRef.current = preload.actions;

	useEffect(() => {
		if (profile === "supported-scale") return;
		if (!activeShowId || !commandReady || !preload.ready) return;
		let cancelled = false;
		const wait = (milliseconds: number) =>
			new Promise<void>((resolve) => window.setTimeout(resolve, milliseconds));
		const run = async () => {
			let high = false;
			await wait(6_000);
			while (!cancelled) {
				const commandActions = commandRef.current;
				const preloadActions = preloadActionsRef.current;
				if (!commandActions || !preloadActions) {
					await wait(100);
					continue;
				}
				await commandActions.execute(
					`FIXTURE ${exerciseFixtureNumber} AT ${high ? 20 : 80}`,
				);
				await wait(750);
				if (cancelled) break;
				await preloadActions.enter();
				await wait(250);
				await commandActions.execute(
					`FIXTURE ${exerciseFixtureNumber} AT ${high ? 80 : 20}`,
				);
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
	}, [activeShowId, commandReady, preload.ready, profile]);
	useEffect(() => {
		if (profile !== "supported-scale") return;
		let cancelled = false;
		const run = async () => {
			const card = await waitForPlaybackCard(() => cancelled);
			if (!card || cancelled) return;
			const go = playbackButton(card, "go");
			const flash = playbackButton(card, "flash");
			const fader = card.querySelector<HTMLInputElement>('input[type="range"]');
			if (!go || !flash || !fader) return;
			onPlaybackAction(await exercisePlaybackClick(card, go, "go"));
			await waitMillis(500);
			onPlaybackAction(
				await exercisePlaybackPointer(
					card,
					flash,
					"flash_press",
					"pointerdown",
				),
			);
			await waitMillis(250);
			onPlaybackAction(
				await exercisePlaybackPointer(
					card,
					flash,
					"flash_release",
					"pointerup",
				),
			);
			await waitMillis(500);
			onPlaybackAction(await exercisePlaybackFader(card, fader, 50));
		};
		void run().catch(() => undefined);
		return () => {
			cancelled = true;
		};
	}, [onPlaybackAction, profile]);
	useEffect(() => {
		if (profile !== "supported-scale" || !activeShowId || !commandReady) return;
		let cancelled = false;
		const run = async () => {
			const row = await waitForFixtureSheetRow(() => cancelled);
			const actions = commandRef.current;
			if (!row || !actions || cancelled) return;
			onFixtureSheetAction(
				await exerciseFixtureSheetCommand(row, actions, [25], "single"),
			);
			await waitMillis(250);
			onFixtureSheetAction(
				await exerciseFixtureSheetCommand(
					row,
					actions,
					[35, 45, 55, 65, 75],
					"burst",
				),
			);
		};
		void run().catch(() => undefined);
		return () => {
			cancelled = true;
		};
	}, [activeShowId, commandReady, onFixtureSheetAction, profile]);
	return null;
}

async function waitForFixtureSheetRow(cancelled: () => boolean) {
	const deadline = performance.now() + 10_000;
	while (!cancelled() && performance.now() < deadline) {
		const row = document.querySelector<HTMLElement>(
			'[data-testid="packaged-stage-fixture-sheet"] [data-fixture-id]',
		);
		if (row?.querySelector(".vertical-meter")) return row;
		await waitMillis(50);
	}
	return null;
}

async function exerciseFixtureSheetCommand(
	row: HTMLElement,
	actions: NonNullable<ReturnType<typeof useProgrammingCommandLineActions>>,
	values: readonly number[],
	action: PackagedFixtureSheetSample["action"],
): Promise<PackagedFixtureSheetSample> {
	const before = row.innerHTML;
	const fixtureId = row.dataset.fixtureId;
	let loadingOverlayObserved = false;
	const inputAt = performance.now();
	for (const value of values) await actions.execute(`FIXTURE 1 AT ${value}`);
	const expected = `${values.at(-1)}%`;
	const deadline = inputAt + 500;
	while (performance.now() < deadline) {
		loadingOverlayObserved ||= fixtureSheetLoadingOverlayVisible();
		const currentRow = fixtureId
			? document.querySelector<HTMLElement>(
					`[data-testid="packaged-stage-fixture-sheet"] [data-fixture-id="${CSS.escape(fixtureId)}"]`,
				)
			: row;
		if (
			currentRow &&
			currentRow.innerHTML !== before &&
			currentRow.textContent?.includes(expected)
		) {
			const settledAt = performance.now();
			return fixtureSheetSample(
				action,
				inputAt,
				settledAt,
				true,
				loadingOverlayObserved,
			);
		}
		await nextAnimationFrame();
	}
	return fixtureSheetSample(
		action,
		inputAt,
		null,
		false,
		loadingOverlayObserved,
	);
}

function fixtureSheetSample(
	action: PackagedFixtureSheetSample["action"],
	inputAt: number,
	settledAt: number | null,
	changed: boolean,
	loadingOverlayObserved: boolean,
): PackagedFixtureSheetSample {
	return {
		action,
		inputAt,
		settledAt,
		convergenceMillis: settledAt === null ? null : settledAt - inputAt,
		changed,
		visibleRows: document.querySelectorAll(
			'[data-testid="packaged-stage-fixture-sheet"] [data-fixture-id]',
		).length,
		loadingOverlayVisible:
			loadingOverlayObserved || fixtureSheetLoadingOverlayVisible(),
	};
}

function fixtureSheetLoadingOverlayVisible() {
	return Boolean(
		document.querySelector(
			'[data-testid="packaged-stage-fixture-sheet"] .fixture-sheet-loading',
		),
	);
}

async function waitForPlaybackCard(cancelled: () => boolean) {
	const deadline = performance.now() + 10_000;
	while (!cancelled() && performance.now() < deadline) {
		const card = document.querySelector<HTMLElement>(
			'[data-playback-slot="1"]',
		);
		if (card && card.dataset.playbackKind !== "empty") return card;
		await waitMillis(50);
	}
	return null;
}

function playbackButton(card: HTMLElement, label: string) {
	return [...card.querySelectorAll<HTMLButtonElement>("button")].find(
		(button) => button.textContent?.trim().toLowerCase() === label,
	);
}

async function exercisePlaybackClick(
	card: HTMLElement,
	button: HTMLButtonElement,
	action: "go",
): Promise<PackagedPlaybackActionSample> {
	const before = card.innerHTML;
	const inputAt = performance.now();
	button.dispatchEvent(pointerEvent("pointerdown"));
	button.dispatchEvent(pointerEvent("pointerup"));
	button.dispatchEvent(new MouseEvent("click", { bubbles: true, button: 0 }));
	return playbackActionSample(card, before, inputAt, action, "dom_pointer");
}

async function exercisePlaybackPointer(
	card: HTMLElement,
	button: HTMLButtonElement,
	action: "flash_press" | "flash_release",
	type: "pointerdown" | "pointerup",
): Promise<PackagedPlaybackActionSample> {
	const before = card.innerHTML;
	const inputAt = performance.now();
	button.dispatchEvent(pointerEvent(type));
	return playbackActionSample(card, before, inputAt, action, "dom_pointer");
}

async function exercisePlaybackFader(
	card: HTMLElement,
	fader: HTMLInputElement,
	value: number,
): Promise<PackagedPlaybackActionSample> {
	const before = card.innerHTML;
	const inputAt = performance.now();
	const setter = Object.getOwnPropertyDescriptor(
		HTMLInputElement.prototype,
		"value",
	)?.set;
	setter?.call(fader, String(value));
	fader.dispatchEvent(new InputEvent("input", { bubbles: true }));
	return playbackActionSample(card, before, inputAt, "master", "dom_range");
}

async function playbackActionSample(
	card: HTMLElement,
	before: string,
	inputAt: number,
	action: PackagedPlaybackActionSample["action"],
	input: PackagedPlaybackActionSample["input"],
): Promise<PackagedPlaybackActionSample> {
	const deadline = inputAt + 500;
	while (performance.now() < deadline) {
		if (card.innerHTML !== before) {
			const indicationAt = performance.now();
			return {
				action,
				input,
				inputAt,
				indicationAt,
				indicationMillis: indicationAt - inputAt,
				changed: true,
			};
		}
		await nextAnimationFrame();
	}
	return {
		action,
		input,
		inputAt,
		indicationAt: null,
		indicationMillis: null,
		changed: false,
	};
}

function pointerEvent(type: "pointerdown" | "pointerup") {
	return new PointerEvent(type, {
		bubbles: true,
		button: 0,
		buttons: type === "pointerdown" ? 1 : 0,
		isPrimary: true,
		pointerId: 1,
		pointerType: "mouse",
	});
}

function waitMillis(milliseconds: number) {
	return new Promise<void>((resolve) =>
		window.setTimeout(resolve, milliseconds),
	);
}

function nextAnimationFrame() {
	return new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
}

export function PackagedStageBenchmarkApp({
	durationSeconds,
	controlDurationSeconds,
	profile,
	additionalStageWindow,
	fixtureSheet,
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
			controlDurationSeconds={controlDurationSeconds}
			profile={profile}
			additionalStageWindow={additionalStageWindow}
			fixtureSheet={fixtureSheet}
		/>
	);
}

type PackagedBenchmarkTimelineSample = ReturnType<
	typeof frontendPerformanceDiagnostics.stageBenchmarkSample
> & {
	mainDocumentFocused: boolean;
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
				longTasks: frontendSnapshot.longTasks,
				eventLags: frontendSnapshot.eventLags,
				stage: {
					frames: frontendSnapshot.stage.frames,
					visualizationRequests: frontendSnapshot.stage.visualizationRequests,
					sceneBuilds: frontend.sceneBuilds,
					sceneBuildSamples: frontendSnapshot.stage.sceneBuilds,
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
	controlDurationSeconds: number,
	profile: string,
	setStageEnabled: (enabled: boolean) => void,
	setExerciseEnabled: (enabled: boolean) => void,
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
		}, controlDurationSeconds * 1_000);
		const timeline: PackagedBenchmarkTimelineSample[] = [];
		let renderCursor = 0;
		let completed = false;
		const sample = () => {
			const frontend =
				frontendPerformanceDiagnostics.stageBenchmarkSample(renderCursor);
			renderCursor = frontend.latestRenderSequence;
			timeline.push({
				...benchmarkState.current,
				...frontend,
				mainDocumentFocused: document.hasFocus(),
			});
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
		const finish = window.setTimeout(
			() => {
				completed = true;
				window.clearInterval(sampler);
				sample();
				void desktop.appendPackagedStageBenchmarkSample({
					schemaVersion: 1,
					kind: "finishing",
					recordedAt: new Date().toISOString(),
				});
				void recordComplete();
				setExerciseEnabled(false);
				setStageEnabled(false);
			},
			(controlDurationSeconds + durationSeconds) * 1_000,
		);
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
	}, [
		benchmarkState,
		desktop,
		controlDurationSeconds,
		durationSeconds,
		profile,
		setExerciseEnabled,
		setStageEnabled,
	]);
}

function PreparedPackagedStageBenchmark({
	durationSeconds,
	controlDurationSeconds,
	profile,
	additionalStageWindow: additionalStageWindowEnabled,
	fixtureSheet,
}: PackagedStageBenchmarkAppProps) {
	const desktop = useDesktopBridge();
	const [stageEnabled, setStageEnabled] = useState(false);
	const [exerciseEnabled, setExerciseEnabled] = useState(true);
	const [qualityIndex, setQualityIndex] = useState(0);
	const [liveVisible, setLiveVisible] = useState(true);
	const [liveView, setLiveView] = useState<"2d" | "3d">("3d");
	const [additionalStageWindow, setAdditionalStageWindow] =
		useState<AdditionalStageWindowState>(
			additionalStageWindowEnabled ? "pending" : "disabled",
		);
	const supportedScale = profile === "supported-scale";
	const activeUiSurfaces = fixtureSheet
		? supportedScale
			? ["stage-3d", "fixture-sheet", "command-line", "playback-bank"]
			: ["stage-3d", "stage-3d-preload", "fixture-sheet"]
		: ["stage-3d", "stage-3d-preload"];
	const benchmarkState = useRef<PackagedBenchmarkState>({
		quality: qualities[0],
		liveView,
		liveVisible,
		additionalStageWindow,
		contextRecoveryMethod: "not_attempted" as ContextRecoveryMethod,
		contextRecovery: { startedAt: null, finishedAt: null },
		activeUiSurfaces,
		playbackActions: [],
		fixtureSheetActions: [],
	});
	benchmarkState.current = {
		quality: qualities[qualityIndex] ?? "lines_and_beams",
		liveView,
		liveVisible,
		additionalStageWindow,
		contextRecoveryMethod: benchmarkState.current.contextRecoveryMethod,
		contextRecovery: benchmarkState.current.contextRecovery,
		activeUiSurfaces,
		playbackActions: benchmarkState.current.playbackActions,
		fixtureSheetActions: benchmarkState.current.fixtureSheetActions,
	};
	const recordPlaybackAction = useCallback(
		(sample: PackagedPlaybackActionSample) => {
			benchmarkState.current.playbackActions = [
				...benchmarkState.current.playbackActions,
				sample,
			];
		},
		[],
	);
	const recordFixtureSheetAction = useCallback(
		(sample: PackagedFixtureSheetSample) => {
			benchmarkState.current.fixtureSheetActions = [
				...benchmarkState.current.fixtureSheetActions,
				sample,
			];
		},
		[],
	);
	useAdditionalStageWindow(
		desktop,
		stageEnabled && additionalStageWindowEnabled,
		setAdditionalStageWindow,
	);
	usePackagedStagePhases(
		stageEnabled,
		durationSeconds,
		!supportedScale,
		setQualityIndex,
		setLiveView,
		setLiveVisible,
		benchmarkState,
	);
	usePackagedStageSampling(
		desktop,
		durationSeconds,
		controlDurationSeconds,
		profile,
		setStageEnabled,
		setExerciseEnabled,
		benchmarkState,
	);

	const quality = qualities[qualityIndex] ?? "lines_and_beams";
	return (
		<ServerRuntime sessionRole="primary">
			{exerciseEnabled && (
				<PackagedStageExercise
					profile={profile}
					onPlaybackAction={recordPlaybackAction}
					onFixtureSheetAction={recordFixtureSheetAction}
				/>
			)}
			<AppProvider>
				<PatchFeatureBoundary>
					<PackagedStageBenchmarkSurface
						stageEnabled={stageEnabled}
						liveVisible={liveVisible}
						liveView={liveView}
						quality={quality}
						fixtureSheet={fixtureSheet}
						profile={profile}
						controlDurationSeconds={controlDurationSeconds}
					/>
					<ConnectionState />
					<DeskLoadingOverlay />
				</PatchFeatureBoundary>
			</AppProvider>
		</ServerRuntime>
	);
}

function PackagedStageBenchmarkSurface({
	stageEnabled,
	liveVisible,
	liveView,
	quality,
	fixtureSheet,
	profile,
	controlDurationSeconds,
}: {
	stageEnabled: boolean;
	liveVisible: boolean;
	liveView: "2d" | "3d";
	quality: StageRenderQuality;
	fixtureSheet: boolean;
	profile: string;
	controlDurationSeconds: number;
}) {
	return (
		<div
			style={{
				display: "grid",
				gridTemplateColumns: "1fr 1fr",
				height: "100vh",
				minHeight: 0,
			}}
		>
			{stageEnabled && liveVisible ? (
				<div
					style={{
						display: "grid",
						gridTemplateRows: profile === "supported-scale" ? "1fr" : "1fr 1fr",
						minHeight: 0,
					}}
				>
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
					{profile !== "supported-scale" && (
						<StageWindow
							compact
							stageView="3d"
							showGroupShortcuts={false}
							followPreload
							stageRenderQuality="lines_only"
							showSelection={false}
							showFloorGrid
							showBeamGuides
						/>
					)}
				</div>
			) : (
				<PackagedStageBaseline
					stageEnabled={stageEnabled}
					controlDurationSeconds={controlDurationSeconds}
				/>
			)}
			{stageEnabled && fixtureSheet ? (
				<div
					style={{
						display: "grid",
						gridTemplateRows:
							profile === "supported-scale"
								? "minmax(0, 2fr) minmax(0, 1fr)"
								: "1fr",
						minHeight: 0,
					}}
				>
					<div
						data-testid="packaged-stage-fixture-sheet"
						style={{ minHeight: 0 }}
					>
						<FixtureSheetWindow compact />
					</div>
					{profile === "supported-scale" && (
						<section
							data-testid="packaged-stage-operator-controls"
							style={{
								display: "grid",
								gridTemplateRows: "auto minmax(0, 1fr)",
								minHeight: 0,
							}}
						>
							<CommandLineBar />
							<PlaybackFaderBank count={4} rows={1} buttons={3} />
						</section>
					)}
				</div>
			) : (
				<div
					data-testid="packaged-stage-output-baseline-secondary"
					style={{ background: "#090d12" }}
				/>
			)}
		</div>
	);
}

function PackagedStageBaseline({
	stageEnabled,
	controlDurationSeconds,
}: {
	stageEnabled: boolean;
	controlDurationSeconds: number;
}) {
	return (
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
						Stage views appear after {controlDurationSeconds} seconds.
					</small>
				</>
			)}
		</div>
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
		const lossesBefore =
			frontendPerformanceDiagnostics.stageBenchmarkSample()
				.rendererContextLosses;
		extension.loseContext();
		// WebKit dispatches this asynchronously and occasionally accepts
		// loseContext() without ever emitting the DOM event. Preserve the native
		// exercise first, then make the lifecycle probe deterministic rather than
		// reporting that no attempt happened.
		window.setTimeout(() => {
			const losses =
				frontendPerformanceDiagnostics.stageBenchmarkSample()
					.rendererContextLosses;
			if (losses <= lossesBefore && canvas.isConnected)
				canvas.dispatchEvent(
					new Event("webglcontextlost", { cancelable: true }),
				);
		}, 6_000);
		return "webgl_lose_context";
	}
	canvas.dispatchEvent(new Event("webglcontextlost", { cancelable: true }));
	canvas.dispatchEvent(new Event("webglcontextrestored"));
	return "synthetic_events";
}
