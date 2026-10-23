import { describe, expect, it, vi } from "vitest";
import { baselineStageBridge, baselineStageLayout } from "./nativePackagedStageLayout";
import type { DesktopBridge, PackagedStageBenchmarkConfig, StagePanePicture } from "./platform/desktop/types";

const baseConfig = { baselineLayout: "single" } as PackagedStageBenchmarkConfig;

describe("fixed physical Stage budget", () => {
	for (const dpr of [1, 1.25, 1.5, 2, 3]) {
		for (const layout of ["single", "dual"] as const) {
			it(`${layout} retains exactly 1920x1080 drawable pixels at scale ${dpr}`, () => {
				const box = baselineStageLayout(layout, dpr);
				expect(Math.round(box.width * dpr)).toBe(1920);
				expect(Math.round((box.height / box.panes) * dpr)).toBe(box.panePixelHeight);
				expect(box.panePixelWidth * box.panePixelHeight * box.panes).toBe(1920 * 1080);
			});
		}
	}
	it("rejects invalid scale rather than measuring a different workload", () => {
		for (const dpr of [0, -1, Number.NaN, Number.POSITIVE_INFINITY])
			expect(() => baselineStageLayout("dual", dpr)).toThrow("display scale");
	});
});

describe("benchmark picture policy", () => {
	it("leaves the original bridge unchanged outside fixed baseline runs", () => {
		const bridge = {} as DesktopBridge;
		expect(baselineStageBridge(bridge, {} as PackagedStageBenchmarkConfig)).toBe(bridge);
	});
	it("overrides full3d/quality for each lane while preserving other scene settings", async () => {
		const send = vi.fn(async () => undefined);
		const bridge = { setStagePanePicture: send } as unknown as DesktopBridge;
		const scoped = baselineStageBridge(bridge, { ...baseConfig, baselineLayout: "dual", baselineQuality: "ultra", baselineView: "full_3d" });
		for (const followPreload of [false, true]) {
			const original = { mode: "lines_3d", quality: "draft", followPreload, ambient: 0.2 } as StagePanePicture;
			await scoped.setStagePanePicture(String(followPreload), original);
			expect(send).toHaveBeenLastCalledWith(String(followPreload), { ...original, quality: "ultra", mode: "full_3d" });
			expect(original).toMatchObject({ mode: "lines_3d", quality: "draft" });
		}
		expect(bridge.setStagePanePicture).toBe(send);
	});
});


it("bypasses exclusive operator 3D allocation only inside the fixed dual benchmark", async () => {
	const open = vi.fn(async (_id: string, _live3d: boolean, _geometry: unknown) => undefined);
	const bridge = { openStagePane: open } as unknown as DesktopBridge;
	const geometry = { x: 0, y: 0, width: 960, height: 270, scale: 2, surfaceWidth: 1920, surfaceHeight: 1080 };
	const dual = baselineStageBridge(bridge, { ...baseConfig, baselineLayout: "dual" });
	await dual.openStagePane("live", true, geometry);
	await dual.openStagePane("preload", true, geometry);
	expect(open.mock.calls).toEqual([["live", false, geometry], ["preload", false, geometry]]);
	expect(baselineStageBridge(bridge, { ...baseConfig, baselineLayout: "single" }).openStagePane).toBe(open);
	expect(baselineStageBridge(bridge, {} as PackagedStageBenchmarkConfig).openStagePane).toBe(open);
});
