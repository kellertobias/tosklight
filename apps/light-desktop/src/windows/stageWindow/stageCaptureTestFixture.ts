import type { StagePanePicture } from "../../platform/desktop/types";

export function capturePicture(overrides: Partial<StagePanePicture> = {}): StagePanePicture {
	return { atmosphere: 0, ambient: 0.35, quality: "draft", exposure: 1,
		laserBrightness: 1, lampFogCloudiness: 0.7, lampFogTurbulence: 1,
		laserFogCloudiness: 0, laserFogTurbulence: 0, detailedPeople: false,
		showLabels: false, floorGrid: true, showBeamGuides: true,
		background: [0.008, 0.01, 0.016], mode: "lines_3d", followPreload: false, ...overrides };
}
