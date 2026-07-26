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

	snapshot(): FrontendPerformanceSnapshot {
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
