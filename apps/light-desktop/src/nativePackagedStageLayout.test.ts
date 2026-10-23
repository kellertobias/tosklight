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
