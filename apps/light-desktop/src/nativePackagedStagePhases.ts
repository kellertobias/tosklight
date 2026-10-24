import type { DesktopBridge, PackagedStageBenchmarkConfig, StagePaneBenchmarkObservations } from "./platform/desktop/types";

export const BASELINE_PHASES = "static-camera-hidden";
export const BASELINE_PHASE_MILLIS = 10_000;
export interface BaselinePaneWitness { paneId: string; attachmentId: string }
export interface BaselineCompositorSnapshot {
	totals: Record<string, number>;
	observedEpochMillis: number | null;
}
export interface BaselineCompositorDelta {
	values: Record<string, number | null>;
	beforeObservedEpochMillis: number | null;
	afterObservedEpochMillis: number | null;
	beforeArrivalLagMillis: number | null;
	afterArrivalLagMillis: number | null;
	nominalPollMillis: 100;
	boundary: string;
}
const compositorDelta = (before: BaselineCompositorSnapshot, after: BaselineCompositorSnapshot,
	started: number, finished: number): BaselineCompositorDelta => ({
	values: Object.fromEntries(Object.keys(after.totals).map((key) => {
		const delta = after.totals[key] - before.totals[key];
		return [key, Number.isFinite(delta) && delta >= 0 ? delta : null];
	})),
	beforeObservedEpochMillis: before.observedEpochMillis,
	afterObservedEpochMillis: after.observedEpochMillis,
	beforeArrivalLagMillis: before.observedEpochMillis === null ? null : Math.max(0, started - before.observedEpochMillis),
	afterArrivalLagMillis: after.observedEpochMillis === null ? null : Math.max(0, finished - after.observedEpochMillis),
	nominalPollMillis: 100,
	boundary: "Arrival-boundary counter deltas at nominal 100ms polling; actual observation ages are retained. GPU timings are grouped separately by submission epoch, not arrival.",
});
export interface BaselinePhaseRow {
	phase: "animated" | "static" | "camera" | "hidden" | "revealed";
	startedEpochMillis: number;
	finishedEpochMillis: number | null;
	status: "pending" | "completed" | "failed";
	cameraRequests?: number;
	cameraBefore?: number[] | null;
	cameraAfter?: number[] | null;
	compositorCounters?: BaselineCompositorDelta;
}
export interface HiddenCompositorInterval {
	paneId: string;
	attachmentId: string;
	startObservedEpochMicros: number | null;
	endObservedEpochMicros: number;
	counters: Record<string, number | null>;
	potentialAcquireAttemptsUpperBound: number | null;
}
export interface HiddenCompositorIntervals {
	rows: HiddenCompositorInterval[];
	overflow: number;
	invalid: number;
	limit: 240;
	boundary: string;
}
type HiddenBatch = { observedEpochMicros: number; windowVisible: false; samples: StagePaneBenchmarkObservations };
const INTERVAL_COUNTERS = ["drawAttempts", "submissions", "occluded", "timeouts", "reconfigures", "errors", "cpuWallMicros", "acquireWallMicros"] as const;
const SKIP_COUNTERS = ["cleanSkips", "hiddenSkips", "busySkips", "pacedSkips"] as const;
const counter = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0;

/** Native counter deltas cover consecutive drain observations, not JavaScript arrival phases. */
export function hiddenCompositorIntervals(batches: HiddenBatch[], panes: BaselinePaneWitness[]): HiddenCompositorIntervals {
	const result: HiddenCompositorIntervals = { rows: [], overflow: 0, invalid: 0, limit: 240,
		boundary: "Per-pane native counter drain intervals; the first prior boundary is unknown. No GPU utilization or total GPU time is inferred." };
	const expected = new Set(panes.map((pane) => `${pane.paneId}\0${pane.attachmentId}`));
	const previous = new Map<string, number>();
	for (const batch of batches) for (const observation of batch.samples.compositors ?? []) {
		const key = `${observation.paneId}\0${observation.attachmentId}`;
		const end = observation.observedEpochMicros;
		const start = previous.get(key) ?? null;
		if (batch.windowVisible !== false || !expected.has(key) || !counter(end) || !counter(batch.observedEpochMicros) || end > batch.observedEpochMicros || (start !== null && end <= start)) {
			result.invalid++; continue;
		}
		previous.set(key, end);
		if (result.rows.length >= result.limit) { result.overflow++; continue; }
		const raw = observation.measurement;
		if (INTERVAL_COUNTERS.some((name) => !counter(raw[name]))) { result.invalid++; continue; }
		const counters: Record<string, number | null> = {};
		for (const name of [...INTERVAL_COUNTERS, ...SKIP_COUNTERS]) counters[name] = counter(raw[name]) ? raw[name]! : null;
		const skipped = SKIP_COUNTERS.every((name) => counter(raw[name]))
			? SKIP_COUNTERS.reduce((sum, name) => sum + raw[name]!, 0) : null;
		if (skipped !== null && skipped > raw.drawAttempts) { result.invalid++; continue; }
		result.rows.push({ paneId: observation.paneId, attachmentId: observation.attachmentId,
			startObservedEpochMicros: start, endObservedEpochMicros: end, counters,
			potentialAcquireAttemptsUpperBound: skipped === null ? null : raw.drawAttempts - skipped });
	}
	return result;
}
export interface BaselinePhaseReport {
	status: "pending" | "completed" | "failed";
	measurementStatus: "pending";
	rows: BaselinePhaseRow[];
	nativeVisibility: {
		hiddenEpochMicros: number; revealedEpochMicros: number;
		hiddenVerified: boolean; revealedVerified: boolean;
	} | null;
	hiddenCompositorIntervals?: HiddenCompositorIntervals;
	attachmentsBeforeHidden: BaselinePaneWitness[];
	attachmentsAfterReveal: BaselinePaneWitness[];
	error: string | null;
}
export const newBaselinePhaseReport = (): BaselinePhaseReport => ({
	status: "pending", measurementStatus: "pending", rows: [], nativeVisibility: null,
	attachmentsBeforeHidden: [], attachmentsAfterReveal: [], error: null,
});

export function validateBaselinePhases(config: PackagedStageBenchmarkConfig) {
	if (!config.baselinePhases) return;
	if (![BASELINE_PHASES, "animated-camera-hidden"].includes(config.baselinePhases) || !config.baselineLayout ||
		!config.stageEnabled || config.profile !== "stage-500" || config.durationSeconds < 40)
		throw new Error("Baseline phases require fixed Stage-500 and at least 40 seconds");
}

const wait = (millis: number, signal: AbortSignal) => new Promise<void>((resolve, reject) => {
	if (signal.aborted) { reject(new Error("Baseline phases cancelled")); return; }
	const aborted = () => { clearTimeout(timer); reject(new Error("Baseline phases cancelled")); };
	const timer = setTimeout(() => { signal.removeEventListener("abort", aborted); resolve(); }, millis);
	signal.addEventListener("abort", aborted, { once: true });
});

/** No pane is unmounted. The native owner hides and restores its own window on a Rust timer. */
export async function runBaselinePhases({ config, desktop, report, panes, signal,
	beforeHidden = async () => {}, collectHidden = async () => {}, afterHidden = () => {}, compositorSnapshot,
	now = Date.now, sleep = wait,
}: {
	config: PackagedStageBenchmarkConfig;
	desktop: DesktopBridge;
	report: BaselinePhaseReport;
	panes: () => BaselinePaneWitness[];
	signal: AbortSignal;
	beforeHidden?: () => Promise<void>;
	collectHidden?: (batches: Array<{ observedEpochMicros: number; windowVisible: false; samples: StagePaneBenchmarkObservations }>) => Promise<void>;
	afterHidden?: () => void;
	compositorSnapshot?: () => BaselineCompositorSnapshot;
	now?: () => number;
	sleep?: (millis: number, signal: AbortSignal) => Promise<void>;
}) {
	validateBaselinePhases(config);
	if (!config.baselinePhases) return;
	const span = async (phase: BaselinePhaseRow["phase"], work: (row: BaselinePhaseRow) => Promise<void>) => {
		const row: BaselinePhaseRow = { phase, startedEpochMillis: now(), finishedEpochMillis: null, status: "pending" };
		report.rows.push(row);
		const before = compositorSnapshot?.();
		try { await work(row); row.status = "completed"; }
		catch (error) { row.status = "failed"; throw error; }
		finally {
			row.finishedEpochMillis = now();
			const after = compositorSnapshot?.();
			if (before && after) row.compositorCounters = compositorDelta(before, after, row.startedEpochMillis, row.finishedEpochMillis);
		}
	};
	try {
		await span(config.baselinePhases === "animated-camera-hidden" ? "animated" : "static", () => sleep(BASELINE_PHASE_MILLIS, signal));
		const cameraPanes = panes();
		const required = config.baselineLayout === "dual" ? 2 : 1;
		if (cameraPanes.length !== required || cameraPanes.some((pane) => !pane.paneId.startsWith("main:") || !pane.attachmentId))
			throw new Error("Baseline camera phase has no complete owned pane identities");
		await span("camera", async (row) => {
			row.cameraBefore = await desktop.stagePaneCamera();
			row.cameraRequests = 0;
			const deadline = now() + BASELINE_PHASE_MILLIS;
			while (now() < deadline - 50) {
				if (signal.aborted) throw new Error("Baseline phases cancelled");
				await Promise.all(cameraPanes.map((pane) => desktop.sendStagePaneInput("orbit", 0.3, 0, pane.paneId.slice(5))));
				row.cameraRequests += cameraPanes.length;
				await sleep(Math.min(17, Math.max(0, deadline - now())), signal);
			}
			await sleep(Math.max(0, deadline - now()), signal);
			row.cameraAfter = await desktop.stagePaneCamera();
		});
		report.attachmentsBeforeHidden = panes();
		await span("hidden", async () => {
			if (!desktop.exercisePackagedStageBenchmarkVisibility)
				throw new Error("Native benchmark visibility command is unavailable");
			await beforeHidden();
			try {
				const { observations, ...evidence } = await desktop.exercisePackagedStageBenchmarkVisibility();
				report.nativeVisibility = evidence;
				report.hiddenCompositorIntervals = hiddenCompositorIntervals(observations, report.attachmentsBeforeHidden);
				await collectHidden(observations);
			} finally { afterHidden(); }
		});
		await span("revealed", () => sleep(BASELINE_PHASE_MILLIS, signal));
		report.attachmentsAfterReveal = panes();
		if (signal.aborted) throw new Error("Baseline phases cancelled");
		report.status = "completed";
	} catch (error) {
		report.status = "failed";
		report.error = error instanceof Error ? error.message : String(error);
	}
}
