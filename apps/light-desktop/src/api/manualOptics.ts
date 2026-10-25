import type { InstalledFixtureAppearance } from "@tosklight/patch/stage-geometry";
import type { PatchInstalledFixtureAppearance } from "../features/patch/contracts";

export function manualOpticsFromWire(appearance: {
	manual_optics?: InstalledFixtureAppearance["manual_optics"] | null;
}): Pick<PatchInstalledFixtureAppearance, "manualOptics"> {
	const optics = appearance.manual_optics;
	return optics
		? {
				manualOptics: {
					zoomDegrees: optics.zoom_degrees,
					shaperBlades: [...optics.shaper_blades],
				},
			}
		: {};
}

export function manualOpticsToWire(
	appearance: Pick<PatchInstalledFixtureAppearance, "manualOptics">,
): Pick<InstalledFixtureAppearance, "manual_optics"> {
	const optics = appearance.manualOptics;
	return optics
		? {
				manual_optics: {
					zoom_degrees: optics.zoomDegrees,
					shaper_blades: [...optics.shaperBlades],
				},
			}
		: {};
}
