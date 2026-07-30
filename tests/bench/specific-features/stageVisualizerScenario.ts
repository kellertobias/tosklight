import { expect, type Page, type TestInfo } from "@playwright/test";
import type {
	FrontendPerformanceSnapshot,
	FrontendStageFrameDiagnostic,
	FrontendStageRenderDiagnostic,
} from "../../../apps/light-desktop/src/features/frontendWarmup/diagnostics";
import type { PaneHandle } from "../window-system/desktopScenario";
import { type PaneType, StageRenderQuality } from "../window-system/paneTypes";

const FIXED_STAGE_CAMERA = {
	position: "0,1.625,8",
	target: "0,2.6,-4",
} as const;

interface StageSocketObservation {
	blocked: boolean;
	deliveryBlocked: boolean;
	sockets: Set<WebSocket>;
	socketsCreated: number;
	sentMessages: string[];
	token: string | null;
}

interface RuntimeVisualizationDiagnostics {
	normal_subscribers: number;
	preload_subscribers: number;
	projections: number;
	projection_micros: number;
	payload_bytes: number;
	source_age_millis: number;
	skipped_source_frames: number;
}

export class BrowserStageVisualizer {
	private faultInstalled = false;
	private readonly qualityDiagnostics = new Map<
		StageRenderQuality,
		FrontendStageRenderDiagnostic
	>();
	private lastQualityRenderCount = 0;

	constructor(
		private readonly page: Page,
		private readonly testInfo: TestInfo,
	) {}

	async prepareSocketRecoveryProof(): Promise<void> {
		if (this.faultInstalled) return;
		this.faultInstalled = true;
		await this.page.addInitScript(() => {
			type StageFaultWindow = Window & {
				__TOSKLIGHT_STAGE_SOCKET_FAULT__?: StageSocketObservation;
			};
			const faultWindow = window as StageFaultWindow;
			const NativeWebSocket = window.WebSocket;
			const control: StageSocketObservation = {
				blocked: false,
				deliveryBlocked: false,
				sockets: new Set<WebSocket>(),
				socketsCreated: 0,
				sentMessages: [],
				token: null,
			};
			faultWindow.__TOSKLIGHT_STAGE_SOCKET_FAULT__ = control;
			const StageFaultWebSocket = new Proxy(NativeWebSocket, {
				construct(Target, argumentsList) {
					const socket = Reflect.construct(Target, argumentsList) as WebSocket;
					if (
						!String(argumentsList[0]).includes("/api/v2/visualization/stream")
					)
						return socket;
					control.socketsCreated++;
					const protocols = argumentsList[1];
					if (Array.isArray(protocols)) {
						const tokenProtocol = protocols.find(
							(protocol) =>
								typeof protocol === "string" &&
								protocol.startsWith("light.token."),
						);
						if (typeof tokenProtocol === "string")
							control.token = tokenProtocol.slice("light.token.".length);
					}
					control.sockets.add(socket);
					socket.addEventListener("message", (event) => {
						if (control.deliveryBlocked) event.stopImmediatePropagation();
					});
					socket.addEventListener("close", () =>
						control.sockets.delete(socket),
					);
					socket.addEventListener("open", () => {
						if (control.blocked)
							socket.close(4001, "STAGE-001 visualization-only interruption");
					});
					const nativeSend = socket.send.bind(socket);
					socket.send = (data) => {
						if (typeof data === "string") control.sentMessages.push(data);
						nativeSend(data);
					};
					return socket;
				},
			});
			Object.defineProperty(window, "WebSocket", {
				configurable: true,
				value: StageFaultWebSocket,
			});
		});
	}

	async interruptVisualization(): Promise<void> {
		await this.page.evaluate(() => {
			const control = (
				window as Window & {
					__TOSKLIGHT_STAGE_SOCKET_FAULT__?: {
						blocked: boolean;
						sockets: Set<WebSocket>;
					};
				}
			).__TOSKLIGHT_STAGE_SOCKET_FAULT__;
			if (!control)
				throw new Error("Stage socket fault control is unavailable");
			control.blocked = true;
			for (const socket of control.sockets)
				socket.close(4001, "STAGE-001 visualization-only interruption");
		});
	}

	async resumeVisualization(options?: {
		reload?: boolean;
		waitBeforeReloadMillis?: number;
	}): Promise<void> {
		await this.page.evaluate(() => {
			const control = (
				window as Window & {
					__TOSKLIGHT_STAGE_SOCKET_FAULT__?: {
						blocked: boolean;
					};
				}
			).__TOSKLIGHT_STAGE_SOCKET_FAULT__;
			if (!control)
				throw new Error("Stage socket fault control is unavailable");
			control.blocked = false;
		});
		if (options?.reload) {
			if (options.waitBeforeReloadMillis)
				await this.page.waitForTimeout(options.waitBeforeReloadMillis);
			this.lastQualityRenderCount = 0;
			this.qualityDiagnostics.clear();
			await this.page.reload({ waitUntil: "domcontentloaded" });
		}
	}

	async stallVisualizationDelivery(): Promise<void> {
		await this.page.evaluate(() => {
			const control = (
				window as Window & {
					__TOSKLIGHT_STAGE_SOCKET_FAULT__?: StageSocketObservation;
				}
			).__TOSKLIGHT_STAGE_SOCKET_FAULT__;
			if (!control) throw new Error("Stage socket observation is unavailable");
			control.deliveryBlocked = true;
		});
	}

	async resumeVisualizationDelivery(): Promise<void> {
		await this.page.evaluate(() => {
			const control = (
				window as Window & {
					__TOSKLIGHT_STAGE_SOCKET_FAULT__?: StageSocketObservation;
				}
			).__TOSKLIGHT_STAGE_SOCKET_FAULT__;
			if (!control) throw new Error("Stage socket observation is unavailable");
			control.deliveryBlocked = false;
		});
	}

	async expectLane(
		pane: PaneHandle<PaneType.Stage>,
		lane: "live" | "preload",
		options?: {
			fixtureNumber?: number;
			percent?: number;
			fixedCamera?: boolean;
			sharedFeed?: {
				normalSubscribers: number;
				preloadSubscribers: number;
				socketCreations?: number;
			};
		},
	): Promise<void> {
		const root = this.root(pane);
		await expect(root).toHaveAttribute("data-visualization-lane", lane);
		await expect(root).toHaveAttribute("data-visualization-state", "ready");
		await expect(root).toHaveAttribute(
			"data-visualization-revision",
			/^[1-9][0-9]*$/u,
		);
		if (options?.fixtureNumber !== undefined && options.percent !== undefined)
			await this.expectFixture2dPercent(
				pane,
				options.fixtureNumber,
				options.percent,
			);
		if (options?.sharedFeed)
			await this.expectSharedLaneFeed(options.sharedFeed);
		if (options?.fixedCamera) await this.expectFixedCamera(pane);
	}

	async expectQuality(
		pane: PaneHandle<PaneType.Stage>,
		quality: StageRenderQuality,
	): Promise<void> {
		const root = this.root(pane);
		await expect(root.locator(".stage-canvas-3d")).toHaveAttribute(
			"data-stage-render-quality",
			renderQualityValue(quality),
		);
		await this.expectFixedCamera(pane);
		await expect
			.poll(async () => {
				const lane =
					(await root.getAttribute("data-visualization-lane")) === "preload"
						? "preload"
						: "normal";
				return (await this.diagnostics()).stage.renders
					.slice(this.lastQualityRenderCount)
					.some(
						(render) =>
							render.lane === lane &&
							render.renderQuality === renderQualityValue(quality),
					);
			})
			.toBe(true);
		const stage = (await this.diagnostics()).stage;
		const lane =
			(await root.getAttribute("data-visualization-lane")) === "preload"
				? "preload"
				: "normal";
		this.lastQualityRenderCount = stage.renders.length;
		const render = [...stage.renders]
			.reverse()
			.find(
				(sample) =>
					sample.lane === lane &&
					sample.renderQuality === renderQualityValue(quality),
			);
		if (!render) throw new Error(`No render diagnostic for ${quality}`);
		this.qualityDiagnostics.set(quality, render);
		if (
			this.qualityDiagnostics.size === Object.values(StageRenderQuality).length
		)
			this.assertQualityDiagnostics(
				Object.fromEntries(this.qualityDiagnostics) as Record<
					StageRenderQuality,
					FrontendStageRenderDiagnostic
				>,
			);
	}

	async expectFixedCamera(pane: PaneHandle<PaneType.Stage>): Promise<void> {
		const canvas = this.root(pane).locator(".stage-3d-canvas");
		await expect(canvas).toHaveAttribute(
			"data-camera-position",
			FIXED_STAGE_CAMERA.position,
		);
		await expect(canvas).toHaveAttribute(
			"data-camera-target",
			FIXED_STAGE_CAMERA.target,
		);
	}

	async expectFixture2dPercent(
		pane: PaneHandle<PaneType.Stage>,
		fixtureNumber: number,
		percent: number,
	): Promise<void> {
		const fixture = this.root(pane)
			.locator(".stage-fixture")
			.filter({
				has: this.page.locator("small", {
					hasText: new RegExp(`^${fixtureNumber}$`, "u"),
				}),
			});
		await expect(fixture).toHaveCount(1);
		await expect(fixture).toHaveAttribute(
			"aria-label",
			new RegExp(`, ${percent}%$`, "u"),
		);
	}

	async selectFixture(
		pane: PaneHandle<PaneType.Stage>,
		fixtureNumber: number,
	): Promise<void> {
		const root = this.root(pane);
		const fixture = root.locator(".stage-fixture").filter({
			has: this.page.locator("small", {
				hasText: new RegExp(`^${fixtureNumber}$`, "u"),
			}),
		});
		await expect(fixture).toHaveCount(1);
		await fixture.click();
		await expect(fixture).toHaveClass(/\bselected\b/u);
	}

	async expectSharedLaneFeed(expected: {
		normalSubscribers: number;
		preloadSubscribers: number;
		socketCreations?: number;
	}): Promise<RuntimeVisualizationDiagnostics> {
		await expect
			.poll(async () => (await this.socketObservation()).sockets)
			.toBe(1);
		const observation = await this.socketObservation();
		if (expected.socketCreations !== undefined)
			expect(observation.socketsCreated).toBe(expected.socketCreations);
		else expect(observation.socketsCreated).toBeGreaterThanOrEqual(1);
		expect(
			observation.subscriptions.some(
				(subscription) =>
					subscription.maxRateHz === 10 &&
					subscription.lanes.includes("normal") &&
					subscription.lanes.includes("preload"),
			),
		).toBe(true);
		const diagnostics = await this.runtimeVisualizationDiagnostics();
		expect(diagnostics.normal_subscribers).toBe(expected.normalSubscribers);
		expect(diagnostics.preload_subscribers).toBe(expected.preloadSubscribers);
		expect(diagnostics.projections).toBeGreaterThan(0);
		return diagnostics;
	}

	async expectLaneSubscribers(expected: {
		normal: number;
		preload: number;
	}): Promise<void> {
		const soft = expect.configure({ soft: true });
		await soft
			.poll(async () => {
				const diagnostics = await this.runtimeVisualizationDiagnostics();
				return {
					normal: diagnostics.normal_subscribers,
					preload: diagnostics.preload_subscribers,
				};
			})
			.toEqual(expected);
		const expectedLanes = [
			...(expected.normal > 0 ? ["normal"] : []),
			...(expected.preload > 0 ? ["preload"] : []),
		].sort();
		await soft
			.poll(async () => {
				const latest = (await this.socketObservation()).subscriptions.at(-1);
				return [...(latest?.lanes ?? [])].sort();
			})
			.toEqual(expectedLanes);
		const observation = await this.socketObservation();
		const diagnostics = await this.runtimeVisualizationDiagnostics();
		const frontend = await this.diagnostics();
		await this.testInfo.attach(
			`stage-lane-subscriber-proof-${expected.normal}-${expected.preload}.json`,
			{
				body: Buffer.from(
					JSON.stringify(
						{
							expected,
							server: {
								normal: diagnostics.normal_subscribers,
								preload: diagnostics.preload_subscribers,
							},
							latestSubscription: observation.subscriptions.at(-1) ?? null,
							claims: frontend.stage.claims.slice(-10),
						},
						null,
						2,
					),
				),
				contentType: "application/json",
			},
		);
	}

	async verifyRenderQualities(
		pane: PaneHandle<PaneType.Stage>,
		advanceFrame?: (
			quality: StageRenderQuality,
			index: number,
		) => Promise<void>,
	): Promise<Record<StageRenderQuality, FrontendStageRenderDiagnostic>> {
		const evidence = {} as Record<
			StageRenderQuality,
			FrontendStageRenderDiagnostic
		>;
		for (const [index, quality] of Object.values(
			StageRenderQuality,
		).entries()) {
			await pane.configure({
				renderQuality: quality,
			});
			await advanceFrame?.(quality, index);
			await this.expectQuality(pane, quality);
			const render = this.qualityDiagnostics.get(quality);
			if (!render) throw new Error(`No render diagnostic for ${quality}`);
			expect(render.calls).toBeGreaterThan(0);
			expect(render.triangles).toBeGreaterThan(0);
			expect(render.geometries).toBeGreaterThan(0);
			evidence[quality] = render;
		}
		return evidence;
	}

	async reload(): Promise<void> {
		await this.page.reload({ waitUntil: "domcontentloaded" });
	}

	async expectStale(pane: PaneHandle<PaneType.Stage>): Promise<void> {
		const root = this.root(pane);
		await expect(root).toHaveAttribute(
			"data-visualization-state",
			/stale|unavailable/u,
		);
		await expect(root.getByRole("status")).toContainText("reconnecting");
	}

	async expectRecovered(pane: PaneHandle<PaneType.Stage>): Promise<void> {
		await expect(this.root(pane)).toHaveAttribute(
			"data-visualization-state",
			"ready",
			{ timeout: 20_000 },
		);
	}

	async expectContextRecovery(pane: PaneHandle<PaneType.Stage>): Promise<void> {
		const canvas = this.root(pane).locator("canvas");
		await expect(canvas).toBeVisible();
		const before = (await this.diagnostics()).stage.renders.length;
		await canvas.dispatchEvent("webglcontextlost");
		await canvas.dispatchEvent("webglcontextrestored");
		await expect
			.poll(async () => (await this.diagnostics()).stage.renders.length)
			.toBeGreaterThan(before);
	}

	async expectCanvasCapture(pane: PaneHandle<PaneType.Stage>): Promise<void> {
		const canvas = this.root(pane).locator("canvas");
		await expect(canvas).toBeVisible();
		const screenshot = await canvas.screenshot({ animations: "disabled" });
		expect(screenshot.length).toBeGreaterThan(1_000);
		await this.testInfo.attach("stage-live-3d-canvas.png", {
			body: screenshot,
			contentType: "image/png",
		});
	}

	async beginChangingFrameMeasurement(): Promise<number> {
		return (await this.diagnostics()).stage.frames.length;
	}

	async captureRetainedSceneState(): Promise<{
		sceneBuilds: number;
		sceneDisposals: number;
		rendererContextsCreated: number;
		rendererContextsDisposed: number;
	}> {
		const { stage } = await this.diagnostics();
		return {
			sceneBuilds: stage.sceneBuilds.length,
			sceneDisposals: stage.sceneDisposals,
			rendererContextsCreated: stage.rendererContextsCreated,
			rendererContextsDisposed: stage.rendererContextsDisposed,
		};
	}

	async expectNoStructuralRebuildSince(
		baseline: Awaited<
			ReturnType<BrowserStageVisualizer["captureRetainedSceneState"]>
		>,
	): Promise<void> {
		const current = await this.captureRetainedSceneState();
		expect(current.sceneBuilds).toBe(baseline.sceneBuilds);
		expect(current.sceneDisposals).toBe(baseline.sceneDisposals);
		expect(current.rendererContextsCreated).toBe(
			baseline.rendererContextsCreated,
		);
		expect(current.rendererContextsDisposed).toBe(
			baseline.rendererContextsDisposed,
		);
	}

	async expectSettledRendererIdle(): Promise<void> {
		let previous: { renders: number; rafCallbacks: number } | undefined;
		await expect
			.poll(
				async () => {
					const stage = (await this.diagnostics()).stage;
					const current = {
						renders: stage.renders.length,
						rafCallbacks: stage.rafCallbacks,
					};
					const stable =
						previous?.renders === current.renders &&
						previous.rafCallbacks === current.rafCallbacks;
					previous = current;
					return stable;
				},
				{ timeout: 5_000, intervals: [300] },
			)
			.toBe(true);
		const settled = (await this.diagnostics()).stage;
		await this.page.waitForTimeout(300);
		const idle = (await this.diagnostics()).stage;
		expect.soft(idle.renders.length).toBe(settled.renders.length);
		expect.soft(idle.rafCallbacks).toBe(settled.rafCallbacks);
	}

	async expectRendererReleasedSince(
		baseline: Awaited<
			ReturnType<BrowserStageVisualizer["captureRetainedSceneState"]>
		>,
	): Promise<void> {
		await expect
			.poll(
				async () => (await this.diagnostics()).stage.rendererContextsDisposed,
			)
			.toBeGreaterThan(baseline.rendererContextsDisposed);
	}

	async expectRendererCreatedSince(
		baseline: Awaited<
			ReturnType<BrowserStageVisualizer["captureRetainedSceneState"]>
		>,
	): Promise<void> {
		await expect
			.poll(
				async () => (await this.diagnostics()).stage.rendererContextsCreated,
			)
			.toBeGreaterThan(baseline.rendererContextsCreated);
	}

	async waitForChangingFrame(): Promise<void> {
		await this.page.waitForTimeout(120);
	}

	async assertChangingFrameBudget(afterFrame: number): Promise<{
		samples: number;
		p95Millis: number;
		maxMillis: number;
		maxPresentationGapMillis: number;
		maxSourceCadenceGapMillis: number;
	}> {
		await expect
			.poll(
				async () =>
					(await this.diagnostics()).stage.frames
						.slice(afterFrame)
						.filter((frame) => frame.sourceToSettledCanvasMs !== null).length,
				{ timeout: 5_000 },
			)
			.toBeGreaterThanOrEqual(5);
		const diagnostics = await this.diagnostics();
		const measuredFrames = diagnostics.stage.frames.slice(afterFrame);
		const settled = measuredFrames.filter(
			(
				frame,
			): frame is FrontendStageFrameDiagnostic & {
				sourceToSettledCanvasMs: number;
			} =>
				frame.sourceToSettledCanvasMs !== null &&
				frame.visibleChanged !== false,
		);
		expect(settled.length).toBeGreaterThanOrEqual(5);
		const durations = settled
			.map((frame) => frame.sourceToSettledCanvasMs)
			.sort((left, right) => left - right);
		const report = {
			samples: durations.length,
			p95Millis: percentile(durations, 95),
			maxMillis: durations.at(-1) ?? Number.POSITIVE_INFINITY,
			maxPresentationGapMillis: maximumChangingLaneGap(
				measuredFrames,
				(frame) =>
					frame.settledCanvasSubmittedAt === null
						? Number.NaN
						: frame.settledCanvasSubmittedAt,
			),
			maxSourceCadenceGapMillis: maximumChangingLaneGap(
				measuredFrames,
				(frame) => Date.parse(frame.sourceGeneratedAt),
			),
		};
		await this.testInfo.attach("stage-visualization-timing.json", {
			body: Buffer.from(
				JSON.stringify({ report, diagnostics: diagnostics.stage }, null, 2),
			),
			contentType: "application/json",
		});
		expect.soft(report.p95Millis).toBeLessThanOrEqual(120);
		expect.soft(report.maxMillis).toBeLessThanOrEqual(200);
		expect.soft(report.maxPresentationGapMillis).toBeLessThanOrEqual(200);
		expect.soft(report.maxSourceCadenceGapMillis).toBeLessThanOrEqual(200);
		return report;
	}

	private root(pane: PaneHandle<PaneType.Stage>) {
		return pane.root().locator(".stage-window");
	}

	private diagnostics(): Promise<FrontendPerformanceSnapshot> {
		return this.page.evaluate(() => {
			const diagnostics = window.__TOSKLIGHT_FRONTEND_PERFORMANCE__?.snapshot();
			if (!diagnostics)
				throw new Error("Frontend performance diagnostics are unavailable");
			return diagnostics;
		});
	}

	private socketObservation(): Promise<{
		sockets: number;
		socketsCreated: number;
		subscriptions: Array<{ lanes: string[]; maxRateHz: number }>;
	}> {
		return this.page.evaluate(() => {
			const control = (
				window as Window & {
					__TOSKLIGHT_STAGE_SOCKET_FAULT__?: StageSocketObservation;
				}
			).__TOSKLIGHT_STAGE_SOCKET_FAULT__;
			if (!control) throw new Error("Stage socket observation is unavailable");
			return {
				sockets: control.sockets.size,
				socketsCreated: control.socketsCreated,
				subscriptions: control.sentMessages.flatMap((raw) => {
					try {
						const message = JSON.parse(raw) as {
							type?: string;
							lanes?: unknown;
							max_rate_hz?: unknown;
						};
						return message.type === "subscribe" &&
							Array.isArray(message.lanes) &&
							typeof message.max_rate_hz === "number"
							? [
									{
										lanes: message.lanes.filter(
											(lane): lane is string => typeof lane === "string",
										),
										maxRateHz: message.max_rate_hz,
									},
								]
							: [];
					} catch {
						return [];
					}
				}),
			};
		});
	}

	private runtimeVisualizationDiagnostics(): Promise<RuntimeVisualizationDiagnostics> {
		return this.page.evaluate(async () => {
			const control = (
				window as Window & {
					__TOSKLIGHT_STAGE_SOCKET_FAULT__?: StageSocketObservation;
				}
			).__TOSKLIGHT_STAGE_SOCKET_FAULT__;
			if (!control?.token)
				throw new Error("Visualization session token is unavailable");
			const response = await fetch("/api/v2/diagnostics/performance", {
				headers: { Authorization: `Bearer ${control.token}` },
			});
			if (!response.ok)
				throw new Error(`Diagnostics returned HTTP ${response.status}`);
			const diagnostics = (await response.json()) as {
				visualization: RuntimeVisualizationDiagnostics;
			};
			return diagnostics.visualization;
		});
	}

	private assertQualityDiagnostics(
		evidence: Record<StageRenderQuality, FrontendStageRenderDiagnostic>,
	): void {
		const linesOnly = evidence[StageRenderQuality.LinesOnly].visibleObjects;
		expect(linesOnly.beamVolumes).toBe(0);
		expect(linesOnly.improvedBeamVolumes).toBe(0);
		expect(linesOnly.improvedBeamLights).toBe(0);
		expect(linesOnly.centerLines).toBeGreaterThan(0);
		expect(linesOnly.groundFootprints).toBeGreaterThan(0);

		const linesAndBeams =
			evidence[StageRenderQuality.LinesAndBeams].visibleObjects;
		expect(linesAndBeams.beamVolumes).toBeGreaterThan(0);
		expect(linesAndBeams.improvedBeamVolumes).toBe(0);
		expect(linesAndBeams.improvedBeamLights).toBe(0);
		expect(linesAndBeams.centerLines).toBeGreaterThan(0);
		expect(linesAndBeams.groundFootprints).toBeGreaterThan(0);

		const beams = evidence[StageRenderQuality.Beams].visibleObjects;
		expect(beams.beamVolumes).toBeGreaterThan(0);
		expect(beams.improvedBeamVolumes).toBe(0);
		expect(beams.improvedBeamLights).toBe(0);
		expect(beams.centerLines).toBe(0);
		expect(beams.groundFootprints).toBe(0);

		const improved = evidence[StageRenderQuality.ImprovedBeams].visibleObjects;
		expect(improved.beamVolumes).toBe(0);
		expect(improved.improvedBeamVolumes).toBeGreaterThan(0);
		expect(improved.improvedBeamLights).toBeGreaterThan(0);
		expect(improved.improvedBeamLights).toBeLessThanOrEqual(8);
		expect(improved.centerLines).toBe(0);
		expect(improved.groundFootprints).toBe(0);
	}
}

function renderQualityValue(quality: StageRenderQuality): string {
	switch (quality) {
		case "Lines only":
			return "lines_only";
		case "Lines + beams":
			return "lines_and_beams";
		case "Beams":
			return "beams";
		case "Improved beams":
			return "improved_beams";
	}
	throw new Error(`Unknown Stage render quality: ${quality}`);
}

function percentile(values: readonly number[], ratio: number): number {
	if (!values.length) return Number.POSITIVE_INFINITY;
	return values[
		Math.min(values.length - 1, Math.ceil(values.length * ratio) - 1)
	];
}

function maximumChangingLaneGap(
	frames: readonly FrontendStageFrameDiagnostic[],
	value: (frame: FrontendStageFrameDiagnostic) => number,
): number {
	return Math.max(
		0,
		...(["normal", "preload"] as const).map((lane) => {
			let previous: number | null = null;
			let maximum = 0;
			for (const frame of frames.filter(
				(candidate) => candidate.lane === lane,
			)) {
				if (frame.visibleChanged === false) {
					previous = null;
					continue;
				}
				const current = value(frame);
				if (!Number.isFinite(current)) continue;
				if (previous !== null) maximum = Math.max(maximum, current - previous);
				previous = current;
			}
			return maximum;
		}),
	);
}
