import { expect, type Page, type TestInfo } from "@playwright/test";
import type {
	FrontendPerformanceSnapshot,
	FrontendStageFrameDiagnostic,
} from "../../../apps/light-desktop/src/features/frontendWarmup/diagnostics";
import type { PaneHandle } from "../window-system/desktopScenario";
import type { PaneType, StageRenderQuality } from "../window-system/paneTypes";

export class BrowserStageVisualizer {
	private faultInstalled = false;

	constructor(
		private readonly page: Page,
		private readonly testInfo: TestInfo,
	) {}

	async prepareSocketRecoveryProof(): Promise<void> {
		if (this.faultInstalled) return;
		this.faultInstalled = true;
		await this.page.addInitScript(() => {
			type StageFaultWindow = Window & {
				__TOSKLIGHT_STAGE_SOCKET_FAULT__?: {
					blocked: boolean;
					sockets: Set<WebSocket>;
				};
			};
			const faultWindow = window as StageFaultWindow;
			const NativeWebSocket = window.WebSocket;
			const control = { blocked: false, sockets: new Set<WebSocket>() };
			faultWindow.__TOSKLIGHT_STAGE_SOCKET_FAULT__ = control;
			const StageFaultWebSocket = new Proxy(NativeWebSocket, {
				construct(Target, argumentsList) {
					const socket = Reflect.construct(Target, argumentsList) as WebSocket;
					if (
						!String(argumentsList[0]).includes("/api/v2/visualization/stream")
					)
						return socket;
					control.sockets.add(socket);
					socket.addEventListener("close", () =>
						control.sockets.delete(socket),
					);
					socket.addEventListener("open", () => {
						if (control.blocked)
							socket.close(4001, "STAGE-001 visualization-only interruption");
					});
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

	async resumeVisualization(): Promise<void> {
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
	}

	async expectLane(
		pane: PaneHandle<PaneType.Stage>,
		lane: "live" | "preload",
	): Promise<void> {
		const root = this.root(pane);
		await expect(root).toHaveAttribute("data-visualization-lane", lane);
		await expect(root).toHaveAttribute("data-visualization-state", "ready");
		await expect(root).toHaveAttribute(
			"data-visualization-revision",
			/^[1-9][0-9]*$/u,
		);
	}

	async expectQuality(
		pane: PaneHandle<PaneType.Stage>,
		quality: StageRenderQuality,
	): Promise<void> {
		await expect(this.root(pane).locator(".stage-canvas-3d")).toHaveAttribute(
			"data-stage-render-quality",
			renderQualityValue(quality),
		);
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

	async beginChangingFrameMeasurement(): Promise<number> {
		return (await this.diagnostics()).stage.frames.length;
	}

	async waitForChangingFrame(): Promise<void> {
		await this.page.waitForTimeout(220);
	}

	async assertChangingFrameBudget(afterFrame: number): Promise<{
		samples: number;
		p95Millis: number;
		maxMillis: number;
	}> {
		await expect
			.poll(
				async () =>
					(await this.diagnostics()).stage.frames
						.slice(afterFrame)
						.filter((frame) => frame.sourceToSettledCanvasMs !== null).length,
				{ timeout: 5_000 },
			)
			.toBeGreaterThanOrEqual(2);
		const diagnostics = await this.diagnostics();
		const settled = diagnostics.stage.frames.slice(afterFrame).filter(
			(
				frame,
			): frame is FrontendStageFrameDiagnostic & {
				sourceToSettledCanvasMs: number;
			} => frame.sourceToSettledCanvasMs !== null,
		);
		expect(settled.length).toBeGreaterThanOrEqual(2);
		const durations = settled
			.map((frame) => frame.sourceToSettledCanvasMs)
			.sort((left, right) => left - right);
		const report = {
			samples: durations.length,
			p95Millis: percentile(durations, 95),
			maxMillis: durations.at(-1) ?? Number.POSITIVE_INFINITY,
		};
		await this.testInfo.attach("stage-visualization-timing.json", {
			body: Buffer.from(
				JSON.stringify({ report, diagnostics: diagnostics.stage }, null, 2),
			),
			contentType: "application/json",
		});
		expect(report.p95Millis).toBeLessThanOrEqual(120);
		expect(report.maxMillis).toBeLessThanOrEqual(200);
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
