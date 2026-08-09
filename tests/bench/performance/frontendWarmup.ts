import fs from "node:fs/promises";
import { gzipSync } from "node:zlib";
import type { CDPSession, Page, TestInfo } from "@playwright/test";
import type { BrowserScenarioWorld } from "../core/browserScenario";
import type { DeskDriver } from "../core/desk";
import { expect } from "../core/fixtures";

export interface FrontendWarmupEvidence {
	profile: string;
	cpuThrottle: number;
	warm: boolean;
	networkRequestCount: number;
	networkTransferredBytes: number;
	diagnostics: {
		firstUsablePaintAt: number | null;
		snapshotRequestCount: number;
		snapshotPayloadBytes: number;
		maxSnapshotConcurrency: number;
		warmup: {
			status: string;
			concurrency: number;
			peakActive: number;
			retainedBytes: number;
			retainedByteBudget: number;
			tasks: Array<{
				startedAt: number | null;
				finishedAt: number | null;
			}>;
		} | null;
		longTasks: Array<{ startedAt: number; durationMs: number }>;
		eventLags: Array<{ lagMs: number }>;
		surfaceSwitches: Array<{ durationMs: number }>;
	};
	browserMemoryBytes: number | null;
	switchSampleCount: number;
	switchP50Ms: number;
	switchP95Ms: number;
	snapshotRequestsDuringSwitches: number;
	loadingPlaceholders: string[];
}

export async function measureFrontendWarmup(
	page: Page,
	world: BrowserScenarioWorld,
	desk: DeskDriver,
	baseUrl: string,
	testInfo: TestInfo,
	options: {
		profile: string;
		cpuThrottle: number;
		warm: boolean;
		emitReconciliationEvent?: () => Promise<void>;
	},
): Promise<FrontendWarmupEvidence> {
	const cdp = await page.context().newCDPSession(page);
	await cdp.send("Network.enable");
	await cdp.send("Emulation.setCPUThrottlingRate", {
		rate: options.cpuThrottle,
	});
	const trace = await startTrace(cdp);
	let networkRequestCount = 0;
	let networkTransferredBytes = 0;
	cdp.on("Network.requestWillBeSent", () => networkRequestCount++);
	cdp.on("Network.loadingFinished", ({ encodedDataLength }) => {
		networkTransferredBytes += encodedDataLength;
	});
	const warmupReady = options.warm
		? page
				.waitForEvent("console", {
					predicate: (message) =>
						message.type() === "debug" &&
						message.text() === "[ToskLight] frontend warm-up ready",
					timeout: 30_000,
				})
				.then(() => undefined)
		: undefined;
	await desk.enableControllableDesktop();
	await desk.open(
		options.warm ? baseUrl : `${baseUrl}?frontend-warmup-disabled`,
		{ beforeReadinessChecks: warmupReady },
	);
	await world.app.expect.ready();
	const opened = await frontendProgress(page);
	expect(opened.firstUsablePaintAt).not.toBeNull();
	if (options.warm) expect(opened.warmupStatus).toBe("ready");
	if (options.warm && options.emitReconciliationEvent) {
		await options.emitReconciliationEvent();
		await expect
			.poll(
				() =>
					page.evaluate(
						() =>
							window.__TOSKLIGHT_FRONTEND_PERFORMANCE__?.progress()
								.eventLagCount ?? 0,
					),
				{ timeout: 10_000 },
			)
			.toBeGreaterThan(0);
		await expect
			.poll(
				() =>
					page.evaluate(
						() =>
							window.__TOSKLIGHT_FRONTEND_PERFORMANCE__?.progress()
								.snapshotRequestRunning ?? false,
					),
				{ timeout: 10_000 },
			)
			.toBe(false);
		await page.evaluate(
			() =>
				new Promise<void>((resolve) =>
					requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
				),
		);
	}
	const loadingPlaceholders = new Set<string>();
	const builtIns = [
		"stage",
		"fixtures",
		"presets",
		"cuelists",
		"dynamics",
		"channels",
	] as const;
	for (let index = 0; index < 42; index++) {
		await world.builtIn.open(builtIns[index % builtIns.length]);
		for (const text of await visibleLoadingText(page))
			loadingPlaceholders.add(text);
	}
	const before = await frontendDiagnostics(page);
	for (let index = 0; index < 40; index++) {
		await world.builtIn.open(builtIns[index % builtIns.length]);
		await page
			.getByRole("main")
			.evaluate((element) => element.getBoundingClientRect().width);
		for (const text of await visibleLoadingText(page))
			loadingPlaceholders.add(text);
	}
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
			),
	);
	const diagnostics = await frontendDiagnostics(page);
	const samples = diagnostics.surfaceSwitches
		.slice(before.surfaceSwitches.length)
		.map(({ durationMs }) => durationMs)
		.sort((left, right) => left - right);
	const browserMemoryBytes =
		(await page.evaluate(() =>
			window.__TOSKLIGHT_FRONTEND_PERFORMANCE__?.browserMemoryBytes(),
		)) ?? null;
	const traceEvents = await trace.finish();
	const evidence: FrontendWarmupEvidence = {
		profile: options.profile,
		cpuThrottle: options.cpuThrottle,
		warm: options.warm,
		networkRequestCount,
		networkTransferredBytes,
		diagnostics,
		browserMemoryBytes,
		switchSampleCount: samples.length,
		switchP50Ms: percentile(samples, 0.5),
		switchP95Ms: percentile(samples, 0.95),
		snapshotRequestsDuringSwitches:
			diagnostics.snapshotRequestCount - before.snapshotRequestCount,
		loadingPlaceholders: [...loadingPlaceholders],
	};
	const evidenceName = `${options.profile}-${options.cpuThrottle}x-${options.warm ? "warm" : "baseline"}.json`;
	const evidencePath = testInfo.outputPath(evidenceName);
	await fs.writeFile(evidencePath, JSON.stringify(evidence, null, 2));
	await testInfo.attach(evidenceName, {
		path: evidencePath,
		contentType: "application/json",
	});
	const traceName = `${options.profile}-${options.cpuThrottle}x-${options.warm ? "warm" : "baseline"}.trace.json.gz`;
	const tracePath = testInfo.outputPath(traceName);
	await fs.writeFile(
		tracePath,
		gzipSync(
			JSON.stringify({
				traceEvents,
				metadata: {
					profile: options.profile,
					cpuThrottle: options.cpuThrottle,
					warm: options.warm,
				},
			}),
		),
	);
	await testInfo.attach(traceName, {
		path: tracePath,
		contentType: "application/gzip",
	});
	await cdp.send("Emulation.setCPUThrottlingRate", { rate: 1 });
	await cdp.detach();
	return evidence;
}

async function frontendProgress(page: Page) {
	return page.evaluate(() => {
		const progress = window.__TOSKLIGHT_FRONTEND_PERFORMANCE__?.progress();
		if (!progress) throw new Error("Frontend progress is unavailable");
		return progress;
	});
}

async function startTrace(cdp: CDPSession) {
	const traceEvents: unknown[] = [];
	cdp.on("Tracing.dataCollected", ({ value }) => traceEvents.push(...value));
	await cdp.send("Tracing.start", {
		categories: [
			"blink.user_timing",
			"devtools.timeline",
			"disabled-by-default-devtools.timeline",
			"loading",
			"v8.execute",
		].join(","),
		options: "sampling-frequency=10000",
		transferMode: "ReportEvents",
	});
	return {
		async finish() {
			const complete = new Promise<void>((resolve) =>
				cdp.once("Tracing.tracingComplete", () => resolve()),
			);
			await cdp.send("Tracing.end");
			await complete;
			return traceEvents;
		},
	};
}

async function frontendDiagnostics(page: Page) {
	return page.evaluate(() => {
		const diagnostics = window.__TOSKLIGHT_FRONTEND_PERFORMANCE__?.snapshot();
		if (!diagnostics) throw new Error("Frontend diagnostics are unavailable");
		return diagnostics;
	});
}

async function visibleLoadingText(page: Page) {
	return page
		.locator('[role="status"]')
		.filter({ hasText: /loading/i })
		.evaluateAll((elements) =>
			elements
				.filter((element) => {
					const style = getComputedStyle(element);
					const rect = element.getBoundingClientRect();
					return (
						style.display !== "none" &&
						style.visibility !== "hidden" &&
						rect.width > 0 &&
						rect.height > 0
					);
				})
				.map((element) => element.textContent?.trim() ?? ""),
		);
}

function percentile(values: readonly number[], ratio: number) {
	if (!values.length) return Number.POSITIVE_INFINITY;
	return values[
		Math.min(values.length - 1, Math.ceil(values.length * ratio) - 1)
	];
}
