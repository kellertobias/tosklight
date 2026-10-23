import type {
	DesktopBridge,
	PackagedStageBenchmarkConfig,
} from "./platform/desktop/types";

export const BASELINE_PIXELS = { width: 1920, height: 1080 } as const;

/** Compact Stage has no header or group strip, so its entire box is drawable. */
export function baselineStageLayout(
	layout: "single" | "dual",
	devicePixelRatio: number,
) {
	if (!Number.isFinite(devicePixelRatio) || devicePixelRatio <= 0)
		throw new Error("Baseline display scale must be positive and finite");
	const panes = layout === "dual" ? 2 : 1;
	return {
		panes,
		width: BASELINE_PIXELS.width / devicePixelRatio,
		height: BASELINE_PIXELS.height / devicePixelRatio,
		panePixelWidth: BASELINE_PIXELS.width,
		panePixelHeight: BASELINE_PIXELS.height / panes,
	};
}

/** Override only this benchmark subtree; normal Control picture policy is unchanged. */
export function baselineStageBridge(
	bridge: DesktopBridge,
	config: PackagedStageBenchmarkConfig,
): DesktopBridge {
	if (!config.baselineLayout) return bridge;
	return {
		...bridge,
		...(config.baselineLayout === "dual" ? {
			// Benchmark-only allocation exception: both panes still request the same actual 3D view.
			openStagePane: (id: string, _live3d: boolean, geometry: Parameters<DesktopBridge["openStagePane"]>[2]) =>
				bridge.openStagePane(id, false, geometry),
		} : {}),
		setStagePanePicture: (id, picture) =>
			bridge.setStagePanePicture(id, {
				...picture,
				quality: config.baselineQuality ?? "draft",
				mode: config.baselineView ?? "lines_3d",
			}),
	};
}
